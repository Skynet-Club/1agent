//! Minimal MCP stdio client.
//!
//! Servers are declared in a JSON file and spawned as child processes. The
//! client performs the initialize handshake, discovers tools and maps each one
//! onto the existing `AgentTool` contract. Annotations are treated
//! conservatively: a tool is write-gated unless it explicitly declares
//! `readOnlyHint: true`, and `destructiveHint: true` raises its risk.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const PROTOCOL_VERSION: &str = "2024-11-05";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_SERVERS: usize = 16;
const MAX_TOOLS_PER_SERVER: usize = 64;
const MAX_TOOL_DESCRIPTION_CHARS: usize = 2000;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServerConfig {
    pub name: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

struct McpClient {
    _child: Child,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicU64,
    request_lock: Mutex<()>,
    server_name: String,
}

impl McpClient {
    async fn spawn(server: &McpServerConfig) -> Result<Arc<Self>, ToolError> {
        if server.name.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "MCP server name must not be empty".into(),
            ));
        }
        let (program, arguments) = server.command.split_first().ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "MCP server '{}' has an empty command",
                server.name
            ))
        })?;
        let mut command = Command::new(program);
        command
            .args(arguments)
            .envs(&server.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = super::spawn_with_retry(&mut command)
            .await
            .map_err(|error| {
                ToolError::Failed(format!(
                    "cannot start MCP server '{}' ({}): {error}",
                    server.name, program
                ))
            })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            ToolError::Failed(format!("MCP server '{}' has no stdin", server.name))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ToolError::Failed(format!("MCP server '{}' has no stdout", server.name))
        })?;
        let client = Arc::new(Self {
            _child: child,
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            next_id: AtomicU64::new(1),
            request_lock: Mutex::new(()),
            server_name: server.name.clone(),
        });
        client.initialize().await?;
        Ok(client)
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), ToolError> {
        let message = json!({"jsonrpc": "2.0", "method": method, "params": params});
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(format!("{message}\n").as_bytes())
            .await
            .map_err(|error| {
                ToolError::Failed(format!(
                    "cannot write to MCP server '{}': {error}",
                    self.server_name
                ))
            })?;
        stdin.flush().await.map_err(|error| {
            ToolError::Failed(format!(
                "cannot flush MCP server '{}': {error}",
                self.server_name
            ))
        })
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, ToolError> {
        let _guard = self.request_lock.lock().await;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(format!("{message}\n").as_bytes())
                .await
                .map_err(|error| {
                    ToolError::Failed(format!(
                        "cannot write to MCP server '{}': {error}",
                        self.server_name
                    ))
                })?;
            stdin.flush().await.map_err(|error| {
                ToolError::Failed(format!(
                    "cannot flush MCP server '{}': {error}",
                    self.server_name
                ))
            })?;
        }
        loop {
            let line = {
                let mut stdout = self.stdout.lock().await;
                let mut line = String::new();
                let read = tokio::time::timeout(REQUEST_TIMEOUT, stdout.read_line(&mut line))
                    .await
                    .map_err(|_| ToolError::TimedOut)?
                    .map_err(|error| {
                        ToolError::Failed(format!(
                            "cannot read from MCP server '{}': {error}",
                            self.server_name
                        ))
                    })?;
                if read == 0 {
                    return Err(ToolError::Failed(format!(
                        "MCP server '{}' closed stdout",
                        self.server_name
                    )));
                }
                line
            };
            let value: Value = serde_json::from_str(&line).map_err(|error| {
                ToolError::Failed(format!(
                    "invalid JSON from MCP server '{}': {error}",
                    self.server_name
                ))
            })?;
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(ToolError::Failed(format!(
                    "MCP server '{}' error: {error}",
                    self.server_name
                )));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn initialize(&self) -> Result<(), ToolError> {
        self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "1agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
        .await?;
        self.notify("notifications/initialized", json!({})).await
    }

    async fn list_tools(&self) -> Result<Vec<Value>, ToolError> {
        let result = self.request("tools/list", json!({})).await?;
        Ok(result
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, ToolError> {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
            .await
    }
}

fn sanitize_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn annotation_bool(tool: &Value, key: &str) -> bool {
    tool.get("annotations")
        .and_then(|annotations| annotations.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn tool_risk(tool: &Value) -> RiskLevel {
    if annotation_bool(tool, "destructiveHint") {
        RiskLevel::High
    } else {
        RiskLevel::Medium
    }
}

fn tool_requires_write(tool: &Value) -> bool {
    !annotation_bool(tool, "readOnlyHint")
}

struct McpTool {
    client: Arc<McpClient>,
    remote_name: String,
    spec: ToolSpec,
    risk: RiskLevel,
    requires_write: bool,
}

#[async_trait]
impl AgentTool for McpTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn risk(&self) -> RiskLevel {
        self.risk
    }

    fn requires_write(&self) -> bool {
        self.requires_write
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(60)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let result = self.client.call_tool(&self.remote_name, arguments).await?;
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut text = String::new();
        if let Some(content) = result.get("content").and_then(Value::as_array) {
            for item in content {
                if let Some(value) = item.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(value);
                } else {
                    text.push_str(&item.to_string());
                }
            }
        } else {
            text = result.to_string();
        }
        if is_error {
            return Err(ToolError::Failed(format!(
                "MCP tool '{}' reported an error: {text}",
                self.spec.name
            )));
        }
        let mut metadata = BTreeMap::new();
        metadata.insert("mcp_tool".into(), self.remote_name.clone());
        Ok(ToolOutcome {
            summary: format!("MCP 工具 {} 已执行", self.remote_name),
            content: text,
            metadata,
        })
    }
}

fn build_tool(
    client: Arc<McpClient>,
    server_name: &str,
    tool: &Value,
) -> Result<McpTool, ToolError> {
    let remote_name = tool
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ToolError::Failed("MCP tool has no name".into()))?;
    let description = tool
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("MCP tool");
    let schema = tool
        .get("inputSchema")
        .cloned()
        .unwrap_or_else(|| json!({"type": "object"}));
    let description = format!(
        "{description}\nMCP server: {server_name}. Input schema: {}",
        bounded(&schema.to_string(), MAX_TOOL_DESCRIPTION_CHARS)
    );
    Ok(McpTool {
        client,
        remote_name: remote_name.to_string(),
        spec: ToolSpec {
            name: format!(
                "mcp_{}_{}",
                sanitize_name(server_name),
                sanitize_name(remote_name)
            ),
            description,
            parameters: json!({"type": "object", "additionalProperties": true}),
        },
        risk: tool_risk(tool),
        requires_write: tool_requires_write(tool),
    })
}

fn bounded(value: &str, max_chars: usize) -> String {
    let mut bounded: String = value.chars().take(max_chars).collect();
    if value.chars().nth(max_chars).is_some() {
        bounded.push('…');
    }
    bounded
}

/// Spawn configured MCP servers and return their tools.
pub async fn load_mcp_tools(path: &Path) -> Result<Vec<Arc<dyn AgentTool>>, ToolError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ToolError::InvalidArguments(format!("cannot stat MCP config: {error}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ToolError::InvalidArguments(
            "MCP config must be a regular file".into(),
        ));
    }
    if metadata.len() > MAX_CONFIG_BYTES as u64 {
        return Err(ToolError::InvalidArguments(
            "MCP config is too large".into(),
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| ToolError::InvalidArguments(format!("cannot read MCP config: {error}")))?;
    let config: McpConfig = serde_json::from_slice(&bytes)
        .map_err(|error| ToolError::InvalidArguments(format!("invalid MCP config: {error}")))?;
    if config.servers.len() > MAX_SERVERS {
        return Err(ToolError::InvalidArguments(format!(
            "at most {MAX_SERVERS} MCP servers are supported"
        )));
    }

    let mut tools: Vec<Arc<dyn AgentTool>> = Vec::new();
    for server in &config.servers {
        let client = McpClient::spawn(server).await?;
        let remote_tools = client.list_tools().await?;
        if remote_tools.len() > MAX_TOOLS_PER_SERVER {
            return Err(ToolError::Failed(format!(
                "MCP server '{}' exposes more than {MAX_TOOLS_PER_SERVER} tools",
                server.name
            )));
        }
        for tool in &remote_tools {
            tools.push(Arc::new(build_tool(client.clone(), &server.name, tool)?));
        }
    }
    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ToolCall, ToolStatus};
    use crate::tools::ToolRegistry;

    #[cfg(unix)]
    fn fake_server(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("fake_mcp.py");
        let script = r#"
import json
import sys

def send(value):
    print(json.dumps(value), flush=True)

for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    request_id = message.get("id")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": request_id, "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fake", "version": "1"}
        }})
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": request_id, "result": {"tools": [
            {
                "name": "echo",
                "description": "Echo text",
                "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
                "annotations": {"readOnlyHint": True}
            },
            {
                "name": "write",
                "description": "Write text",
                "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}},
                "annotations": {}
            }
        ]}})
    elif method == "tools/call":
        params = message.get("params", {})
        name = params.get("name")
        arguments = params.get("arguments", {})
        send({"jsonrpc": "2.0", "id": request_id, "result": {
            "content": [{"type": "text", "text": f"{name}:{arguments.get('text', '')}"}],
            "isError": False
        }})
    else:
        send({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32601, "message": "method not found"}})
"#;
        std::fs::write(&path, script).unwrap();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn discovers_and_calls_mcp_tools() {
        let dir = tempfile::tempdir().unwrap();
        let script = fake_server(dir.path());
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "command": ["python3", script.display().to_string()]
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let tools = load_mcp_tools(&config_path).await.unwrap();
        assert_eq!(tools.len(), 2);
        let echo = tools
            .iter()
            .find(|tool| tool.spec().name == "mcp_fake_echo")
            .unwrap();
        assert!(!echo.requires_write());
        let write = tools
            .iter()
            .find(|tool| tool.spec().name == "mcp_fake_write")
            .unwrap();
        assert!(write.requires_write());

        let outcome = echo
            .execute(
                json!({"text": "hello"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.content, "echo:hello");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_mcp_tools_are_denied_without_write_permission() {
        let dir = tempfile::tempdir().unwrap();
        let script = fake_server(dir.path());
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "command": ["python3", script.display().to_string()]
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let tools = load_mcp_tools(&config_path).await.unwrap();
        let write = tools
            .iter()
            .find(|tool| tool.spec().name == "mcp_fake_write")
            .unwrap()
            .clone();
        let mut registry = ToolRegistry::default();
        registry.register(write);
        let observation = registry
            .execute(
                &ToolCall {
                    id: "mcp-1".into(),
                    name: "mcp_fake_write".into(),
                    arguments: json!({"text": "x"}),
                },
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await;
        assert_eq!(observation.status, ToolStatus::PermissionDenied);
    }
}
