//! Minimal MCP client with stdio and streamable-HTTP transports.
//!
//! Servers are declared in a JSON file and either spawned as child processes
//! or reached over HTTP. The client performs the initialize handshake,
//! discovers tools and maps each one onto the existing `AgentTool` contract.
//! Annotations are treated conservatively: a tool is write-gated unless it
//! explicitly declares `readOnlyHint: true`, and `destructiveHint: true`
//! raises its risk. HTTP requires HTTPS except for loopback, and bearer tokens
//! come from an environment variable, never from the config file.

use std::collections::BTreeMap;
use std::io::Read;
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
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default)]
    pub command: Option<Vec<String>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Read the HTTP bearer token from this environment variable.
    #[serde(default)]
    pub bearer_token_env: Option<String>,
    /// Read the HTTP bearer token from this 0600 regular file instead.
    #[serde(default)]
    pub bearer_token_file: Option<std::path::PathBuf>,
    /// Exchange a subject token for an HTTP bearer token at spawn time.
    #[serde(default)]
    pub token_exchange: Option<crate::mcp_exchange::TokenExchangeConfig>,
    /// Per-request timeout in seconds (1..=300, default 20).
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[allow(clippy::large_enum_variant)]
enum Transport {
    Stdio {
        _child: Child,
        stdin: Mutex<ChildStdin>,
        stdout: Mutex<BufReader<ChildStdout>>,
    },
    Http {
        client: reqwest::Client,
        url: String,
        session_id: Mutex<Option<String>>,
        bearer_token: Mutex<Option<String>>,
        token_expires_at_ms: Mutex<Option<u128>>,
        exchange: Option<crate::mcp_exchange::TokenExchangeConfig>,
    },
}

struct McpClient {
    transport: Transport,
    next_id: AtomicU64,
    request_lock: Mutex<()>,
    server_name: String,
    timeout: Duration,
}

fn read_bearer_token_file(path: &std::path::Path, server: &str) -> Result<String, ToolError> {
    let path_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ToolError::InvalidArguments(format!(
            "MCP server '{server}' cannot stat bearer token file {}: {error}",
            path.display()
        ))
    })?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(ToolError::InvalidArguments(format!(
            "MCP server '{server}' bearer token file must be a regular non-symlink file"
        )));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path).map_err(|error| {
        ToolError::InvalidArguments(format!(
            "MCP server '{server}' cannot open bearer token file: {error}"
        ))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ToolError::InvalidArguments(format!(
            "MCP server '{server}' cannot stat bearer token file: {error}"
        ))
    })?;
    if metadata.len() == 0 || metadata.len() > 4096 {
        return Err(ToolError::InvalidArguments(format!(
            "MCP server '{server}' bearer token file must be 1..=4096 bytes"
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{server}' bearer token file must not be group/world accessible (chmod 600)"
            )));
        }
    }
    let mut raw = String::new();
    file.read_to_string(&mut raw).map_err(|error| {
        ToolError::InvalidArguments(format!(
            "MCP server '{server}' cannot read bearer token file: {error}"
        ))
    })?;
    let token = raw.trim();
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return Err(ToolError::InvalidArguments(format!(
            "MCP server '{server}' bearer token file must contain one non-empty token"
        )));
    }
    Ok(token.to_string())
}

impl McpClient {
    async fn spawn(server: &McpServerConfig) -> Result<Arc<Self>, ToolError> {
        if server.name.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "MCP server name must not be empty".into(),
            ));
        }
        let timeout_seconds = server.timeout_seconds.unwrap_or(REQUEST_TIMEOUT.as_secs());
        if !(1..=300).contains(&timeout_seconds) {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' timeout_seconds must be 1..=300",
                server.name
            )));
        }
        let timeout = Duration::from_secs(timeout_seconds);
        if server.command.is_some() && server.token_exchange.is_some() {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' token_exchange is only valid for HTTP servers",
                server.name
            )));
        }
        let transport = match (&server.command, &server.url) {
            (Some(command), None) => Self::stdio_transport(server, command).await?,
            (None, Some(url)) => Self::http_transport(server, url, timeout).await?,
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "MCP server '{}' must set exactly one of command or url",
                    server.name
                )));
            }
        };
        let client = Arc::new(Self {
            transport,
            next_id: AtomicU64::new(1),
            request_lock: Mutex::new(()),
            server_name: server.name.clone(),
            timeout,
        });
        client.initialize().await?;
        Ok(client)
    }

    async fn stdio_transport(
        server: &McpServerConfig,
        command: &[String],
    ) -> Result<Transport, ToolError> {
        let (program, arguments) = command.split_first().ok_or_else(|| {
            ToolError::InvalidArguments(format!(
                "MCP server '{}' has an empty command",
                server.name
            ))
        })?;
        let mut child_command = Command::new(program);
        child_command
            .args(arguments)
            .envs(&server.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = super::spawn_with_retry(&mut child_command)
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
        Ok(Transport::Stdio {
            _child: child,
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
        })
    }

    async fn http_transport(
        server: &McpServerConfig,
        url: &str,
        timeout: Duration,
    ) -> Result<Transport, ToolError> {
        let parsed = reqwest::Url::parse(url).map_err(|error| {
            ToolError::InvalidArguments(format!(
                "MCP server '{}' has an invalid URL: {error}",
                server.name
            ))
        })?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' URL must not embed credentials",
                server.name
            )));
        }
        let loopback = parsed
            .host_str()
            .is_some_and(crate::net_policy::is_loopback_host);
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' URL must use https (http is allowed only for loopback)",
                server.name
            )));
        }
        if server.bearer_token_env.is_some() && server.bearer_token_file.is_some() {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' must set only one of bearer_token_env or bearer_token_file",
                server.name
            )));
        }
        if server.token_exchange.is_some()
            && (server.bearer_token_env.is_some() || server.bearer_token_file.is_some())
        {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' must set only one authentication source (token_exchange, bearer_token_env or bearer_token_file)",
                server.name
            )));
        }
        let (bearer_token, token_expires_at_ms) = if let Some(exchange) = &server.token_exchange {
            let token = crate::mcp_exchange::exchange(exchange)
                .await
                .map_err(|error| {
                    ToolError::InvalidArguments(format!(
                        "MCP server '{}' token exchange failed: {error}",
                        server.name
                    ))
                })?;
            (
                Some(token.access_token),
                Some(crate::current_time_ms() + u128::from(token.expires_in_seconds) * 1_000),
            )
        } else {
            (
                match (&server.bearer_token_env, &server.bearer_token_file) {
                    (Some(variable), None) => {
                        if variable.trim().is_empty() {
                            return Err(ToolError::InvalidArguments(format!(
                                "MCP server '{}' bearer token environment variable is empty",
                                server.name
                            )));
                        }
                        let token = std::env::var(variable).map_err(|_| {
                    ToolError::InvalidArguments(format!(
                        "MCP server '{}' bearer token environment variable {variable} is not set",
                        server.name
                    ))
                })?;
                        if token.trim().is_empty() {
                            return Err(ToolError::InvalidArguments(format!(
                                "MCP server '{}' bearer token environment variable {variable} is empty",
                                server.name
                            )));
                        }
                        Some(token)
                    }
                    (None, Some(path)) => Some(read_bearer_token_file(path, &server.name)?),
                    (None, None) => None,
                    (Some(_), Some(_)) => unreachable!("conflict checked above"),
                },
                None,
            )
        };
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| {
                ToolError::Failed(format!(
                    "cannot build MCP HTTP client for '{}': {error}",
                    server.name
                ))
            })?;
        Ok(Transport::Http {
            client,
            url: parsed.to_string(),
            session_id: Mutex::new(None),
            bearer_token: Mutex::new(bearer_token),
            token_expires_at_ms: Mutex::new(token_expires_at_ms),
            exchange: server.token_exchange.clone(),
        })
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), ToolError> {
        let message = json!({"jsonrpc": "2.0", "method": method, "params": params});
        match &self.transport {
            Transport::Stdio { stdin, .. } => {
                let mut stdin = stdin.lock().await;
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
            Transport::Http {
                client,
                url,
                session_id,
                bearer_token,
                token_expires_at_ms,
                exchange,
            } => {
                let response = self
                    .http_post_with_refresh(
                        client,
                        url,
                        session_id,
                        bearer_token,
                        token_expires_at_ms,
                        exchange.as_ref(),
                        &message,
                    )
                    .await?;
                let status = response.status();
                if let Some(value) = response.headers().get("Mcp-Session-Id")
                    && let Ok(value) = value.to_str()
                {
                    *session_id.lock().await = Some(value.to_string());
                }
                if !status.is_success() {
                    let body = read_http_body(response, &self.server_name)
                        .await
                        .unwrap_or_default();
                    return Err(ToolError::Failed(format!(
                        "MCP server '{}' HTTP {status}: {}",
                        self.server_name,
                        bounded(&body, 300)
                    )));
                }
                Ok(())
            }
        }
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, ToolError> {
        let _guard = self.request_lock.lock().await;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        match &self.transport {
            Transport::Stdio { stdin, stdout, .. } => {
                {
                    let mut stdin = stdin.lock().await;
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
                        let mut stdout = stdout.lock().await;
                        read_line_capped(
                            &mut *stdout,
                            MAX_RESPONSE_BYTES,
                            self.timeout,
                            &self.server_name,
                        )
                        .await?
                        .ok_or_else(|| {
                            ToolError::Failed(format!(
                                "MCP server '{}' closed stdout",
                                self.server_name
                            ))
                        })?
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
                    return response_value(&value, id, &self.server_name);
                }
            }
            Transport::Http {
                client,
                url,
                session_id,
                bearer_token,
                token_expires_at_ms,
                exchange,
            } => {
                let response = self
                    .http_post_with_refresh(
                        client,
                        url,
                        session_id,
                        bearer_token,
                        token_expires_at_ms,
                        exchange.as_ref(),
                        &message,
                    )
                    .await?;
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                if let Some(value) = response.headers().get("Mcp-Session-Id")
                    && let Ok(value) = value.to_str()
                {
                    *session_id.lock().await = Some(value.to_string());
                }
                let body = read_http_body(response, &self.server_name).await?;
                if !status.is_success() {
                    return Err(ToolError::Failed(format!(
                        "MCP server '{}' HTTP {status}: {}",
                        self.server_name,
                        bounded(&body, 300)
                    )));
                }
                if content_type.contains("text/event-stream") {
                    sse_response(&body, id, &self.server_name)
                } else {
                    json_response(&body, id, &self.server_name)
                }
            }
        }
    }

    async fn http_post(
        &self,
        client: &reqwest::Client,
        url: &str,
        session_id: &Mutex<Option<String>>,
        bearer_token: &Mutex<Option<String>>,
        message: &Value,
    ) -> Result<reqwest::Response, ToolError> {
        let mut request = client
            .post(url)
            .header(
                reqwest::header::ACCEPT,
                "application/json, text/event-stream",
            )
            .json(message);
        if let Some(token) = bearer_token.lock().await.clone() {
            request = request.bearer_auth(token);
        }
        if let Some(session) = session_id.lock().await.clone() {
            request = request.header("Mcp-Session-Id", session);
        }
        request.send().await.map_err(|error| {
            ToolError::Failed(format!(
                "MCP server '{}' HTTP request failed: {error}",
                self.server_name
            ))
        })
    }

    async fn refresh_token(
        &self,
        config: &crate::mcp_exchange::TokenExchangeConfig,
        bearer_token: &Mutex<Option<String>>,
        token_expires_at_ms: &Mutex<Option<u128>>,
    ) -> Result<(), ToolError> {
        let refreshed = crate::mcp_exchange::exchange(config)
            .await
            .map_err(|error| {
                ToolError::Failed(format!(
                    "MCP server '{}' token refresh failed: {error}",
                    self.server_name
                ))
            })?;
        *token_expires_at_ms.lock().await =
            Some(crate::current_time_ms() + u128::from(refreshed.expires_in_seconds) * 1_000);
        *bearer_token.lock().await = Some(refreshed.access_token);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn http_post_with_refresh(
        &self,
        client: &reqwest::Client,
        url: &str,
        session_id: &Mutex<Option<String>>,
        bearer_token: &Mutex<Option<String>>,
        token_expires_at_ms: &Mutex<Option<u128>>,
        exchange: Option<&crate::mcp_exchange::TokenExchangeConfig>,
        message: &Value,
    ) -> Result<reqwest::Response, ToolError> {
        if let Some(config) = exchange
            && token_expires_at_ms
                .lock()
                .await
                .is_some_and(|expires| expires <= crate::current_time_ms() + 30_000)
        {
            self.refresh_token(config, bearer_token, token_expires_at_ms)
                .await?;
        }
        let mut response = self
            .http_post(client, url, session_id, bearer_token, message)
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            && let Some(config) = exchange
        {
            self.refresh_token(config, bearer_token, token_expires_at_ms)
                .await?;
            response = self
                .http_post(client, url, session_id, bearer_token, message)
                .await?;
        }
        Ok(response)
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

async fn read_line_capped<R>(
    reader: &mut R,
    max_bytes: usize,
    timeout: Duration,
    server: &str,
) -> Result<Option<String>, ToolError>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut bytes = Vec::new();
    loop {
        let available = tokio::time::timeout(timeout, reader.fill_buf())
            .await
            .map_err(|_| ToolError::TimedOut)?
            .map_err(|error| {
                ToolError::Failed(format!("cannot read from MCP server '{server}': {error}"))
            })?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        if bytes.len().saturating_add(take) > max_bytes {
            return Err(ToolError::Failed(format!(
                "MCP server '{server}' response exceeds {max_bytes} bytes"
            )));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            break;
        }
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| ToolError::Failed(format!("MCP server '{server}' returned non-UTF-8 output")))
}

async fn read_http_body(
    mut response: reqwest::Response,
    server: &str,
) -> Result<String, ToolError> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        ToolError::Failed(format!(
            "cannot read HTTP response from MCP server '{server}': {error}"
        ))
    })? {
        push_capped(&mut body, &chunk, server)?;
    }
    String::from_utf8(body)
        .map_err(|_| ToolError::Failed(format!("MCP server '{server}' returned non-UTF-8 body")))
}

fn push_capped(body: &mut Vec<u8>, chunk: &[u8], server: &str) -> Result<(), ToolError> {
    if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
        return Err(ToolError::Failed(format!(
            "MCP server '{server}' response exceeds {MAX_RESPONSE_BYTES} bytes"
        )));
    }
    body.extend_from_slice(chunk);
    Ok(())
}

fn json_response(body: &str, id: u64, server: &str) -> Result<Value, ToolError> {
    let value: Value = serde_json::from_str(body).map_err(|error| {
        ToolError::Failed(format!("invalid JSON from MCP server '{server}': {error}"))
    })?;
    response_value(&value, id, server)
}

fn sse_response(body: &str, id: u64, server: &str) -> Result<Value, ToolError> {
    for value in sse_values(body) {
        if value.get("id").and_then(Value::as_u64) == Some(id) {
            return response_value(&value, id, server);
        }
    }
    Err(ToolError::Failed(format!(
        "MCP server '{server}' returned no SSE response for request {id}"
    )))
}

fn sse_values(body: &str) -> Vec<Value> {
    let mut values = Vec::new();
    let mut data = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        } else if line.trim().is_empty() {
            flush_sse_data(&mut data, &mut values);
        }
    }
    flush_sse_data(&mut data, &mut values);
    values
}

fn flush_sse_data(data: &mut String, values: &mut Vec<Value>) {
    if !data.trim().is_empty()
        && let Ok(value) = serde_json::from_str(data.trim())
    {
        values.push(value);
    }
    data.clear();
}

fn response_value(value: &Value, id: u64, server: &str) -> Result<Value, ToolError> {
    let target = match value {
        Value::Array(items) => items
            .iter()
            .find(|item| item.get("id").and_then(Value::as_u64) == Some(id)),
        _ => (value.get("id").and_then(Value::as_u64) == Some(id)).then_some(value),
    };
    let Some(target) = target else {
        return Err(ToolError::Failed(format!(
            "MCP server '{server}' returned no response for request {id}"
        )));
    };
    if let Some(error) = target.get("error") {
        return Err(ToolError::Failed(format!(
            "MCP server '{server}' error: {error}"
        )));
    }
    Ok(target.get("result").cloned().unwrap_or(Value::Null))
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
        self.client.timeout + Duration::from_secs(5)
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
        metadata.insert("mcp_server".into(), self.client.server_name.clone());
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
    #[cfg(unix)]
    use crate::domain::{ToolCall, ToolStatus};
    #[cfg(unix)]
    use crate::tools::ToolRegistry;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn fake_http_server() -> (std::net::SocketAddr, Arc<std::sync::Mutex<Vec<String>>>) {
        fake_http_server_with_token(None).await
    }

    async fn fake_http_server_with_token(
        required: Option<&'static str>,
    ) -> (std::net::SocketAddr, Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_for_server = captured.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let captured = captured_for_server.clone();
                tokio::spawn(async move {
                    let mut buffer = Vec::new();
                    let mut chunk = [0u8; 4096];
                    loop {
                        let read = socket.read(&mut chunk).await.unwrap_or(0);
                        if read == 0 {
                            return;
                        }
                        buffer.extend_from_slice(&chunk[..read]);
                        let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
                        else {
                            continue;
                        };
                        let headers = String::from_utf8_lossy(&buffer[..end]).to_string();
                        let authorization = headers.lines().find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("authorization")
                                .then(|| value.trim().to_string())
                        });
                        if let Some(authorization) = &authorization {
                            captured.lock().unwrap().push(authorization.clone());
                        }
                        if let Some(required) = required {
                            let expected = format!("Bearer {required}");
                            if authorization.as_deref() != Some(expected.as_str()) {
                                socket
                                    .write_all(
                                        b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                    )
                                    .await
                                    .ok();
                                return;
                            }
                        }
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())
                                    .flatten()
                            })
                            .unwrap_or(0);
                        if buffer.len() < end + 4 + length {
                            continue;
                        }
                        let message: Value =
                            serde_json::from_slice(&buffer[end + 4..end + 4 + length])
                                .unwrap_or(Value::Null);
                        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
                        let id = message.get("id").cloned().unwrap_or(Value::Null);
                        let request_session = headers.lines().find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("mcp-session-id")
                                .then(|| value.trim().to_string())
                        });
                        // initialize returns the session id; every later request
                        // must reuse it instead of creating a new session.
                        if method != "initialize" && request_session.as_deref() != Some("sess-1") {
                            socket
                                .write_all(
                                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                )
                                .await
                                .ok();
                            return;
                        }
                        let (status, session, payload) = match method {
                            "initialize" => (
                                "200 OK",
                                Some("sess-1"),
                                json!({"jsonrpc": "2.0", "id": id, "result": {
                                    "protocolVersion": PROTOCOL_VERSION,
                                    "capabilities": {"tools": {}},
                                    "serverInfo": {"name": "fake", "version": "1"}
                                }}),
                            ),
                            "notifications/initialized" => ("202 Accepted", None, Value::Null),
                            "tools/list" => (
                                "200 OK",
                                None,
                                json!({"jsonrpc": "2.0", "id": id, "result": {"tools": [{
                                    "name": "echo",
                                    "description": "Echo text",
                                    "inputSchema": {"type": "object"},
                                    "annotations": {"readOnlyHint": true}
                                }]}}),
                            ),
                            "tools/call" => {
                                let name = message
                                    .get("params")
                                    .and_then(|params| params.get("name"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown");
                                let text = message
                                    .get("params")
                                    .and_then(|params| params.get("arguments"))
                                    .and_then(|arguments| arguments.get("text"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                (
                                    "200 OK",
                                    None,
                                    json!({"jsonrpc": "2.0", "id": id, "result": {
                                        "content": [{"type": "text", "text": format!("{name}:{text}")}],
                                        "isError": false
                                    }}),
                                )
                            }
                            _ => (
                                "200 OK",
                                None,
                                json!({"jsonrpc": "2.0", "id": id, "error": {
                                    "code": -32601,
                                    "message": "method not found"
                                }}),
                            ),
                        };
                        let body = if payload.is_null() {
                            Vec::new()
                        } else {
                            serde_json::to_vec(&payload).unwrap()
                        };
                        let mut response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                            body.len()
                        );
                        if let Some(session) = session {
                            response.push_str(&format!("Mcp-Session-Id: {session}\r\n"));
                        }
                        response.push_str("\r\n");
                        socket.write_all(response.as_bytes()).await.ok();
                        socket.write_all(&body).await.ok();
                        socket.flush().await.ok();
                        return;
                    }
                });
            }
        });
        (address, captured)
    }

    #[tokio::test]
    async fn http_mcp_discovers_and_calls_tools() {
        let (address, _authorization) = fake_http_server().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "url": format!("http://{address}/mcp")
                }]
            }))
            .unwrap(),
        )
        .unwrap();

        let tools = load_mcp_tools(&config_path).await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].spec().name, "mcp_fake_echo");
        assert!(!tools[0].requires_write());
        let outcome = tools[0]
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
    fn write_token_file(path: &std::path::Path, token: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, token).unwrap();
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(mode);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    async fn fake_token_endpoint() -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let _ = socket.read(&mut buffer).await;
            let body =
                br#"{"access_token":"exchanged-token","token_type":"Bearer","expires_in":60}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.ok();
            socket.write_all(body).await.ok();
        });
        address
    }

    async fn fake_rotating_token_endpoint(expires_in: u64) -> std::net::SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for token in ["stale-token", "refreshed-token"] {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                let body = format!(
                    r#"{{"access_token":"{token}","token_type":"Bearer","expires_in":{expires_in}}}"#
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.ok();
                socket.write_all(body.as_bytes()).await.ok();
            }
        });
        address
    }

    #[tokio::test]
    async fn http_mcp_refreshes_an_exchanged_token_on_401() {
        let (address, authorization) = fake_http_server_with_token(Some("refreshed-token")).await;
        let token_address = fake_rotating_token_endpoint(60).await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "url": format!("http://{address}/mcp"),
                    "token_exchange": {
                        "token_endpoint": format!("http://{token_address}/token"),
                        "audience": "mcp://fake",
                        "scope": "tools.read",
                        "subject_token_env": "MCP_REFRESH_SUBJECT_TEST",
                        "max_ttl_seconds": 60
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        unsafe { std::env::set_var("MCP_REFRESH_SUBJECT_TEST", "subject-secret") };
        let tools = load_mcp_tools(&config_path).await.unwrap();
        unsafe { std::env::remove_var("MCP_REFRESH_SUBJECT_TEST") };
        assert_eq!(tools.len(), 1);
        let auth = authorization.lock().unwrap().clone();
        assert!(auth.iter().any(|value| value == "Bearer stale-token"));
        assert!(auth.iter().any(|value| value == "Bearer refreshed-token"));
    }

    #[tokio::test]
    async fn http_mcp_refreshes_before_expiry() {
        let (address, authorization) = fake_http_server().await;
        let token_address = fake_rotating_token_endpoint(31).await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "url": format!("http://{address}/mcp"),
                    "token_exchange": {
                        "token_endpoint": format!("http://{token_address}/token"),
                        "audience": "mcp://fake",
                        "scope": "tools.read",
                        "subject_token_env": "MCP_PROACTIVE_SUBJECT_TEST",
                        "max_ttl_seconds": 60
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        unsafe { std::env::set_var("MCP_PROACTIVE_SUBJECT_TEST", "subject-secret") };
        let tools = load_mcp_tools(&config_path).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1_500)).await;
        tools[0]
            .execute(
                json!({"text": "after expiry"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        unsafe { std::env::remove_var("MCP_PROACTIVE_SUBJECT_TEST") };
        let auth = authorization.lock().unwrap();
        assert_eq!(
            auth.last().map(String::as_str),
            Some("Bearer refreshed-token")
        );
    }

    #[tokio::test]
    async fn http_mcp_exchanges_a_subject_token_at_spawn() {
        let (address, authorization) = fake_http_server().await;
        let token_address = fake_token_endpoint().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "url": format!("http://{address}/mcp"),
                    "token_exchange": {
                        "token_endpoint": format!("http://{token_address}/token"),
                        "audience": "mcp://fake",
                        "scope": "tools.read",
                        "subject_token_env": "MCP_EXCHANGE_SUBJECT_TEST",
                        "max_ttl_seconds": 60
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        // The test process owns this unique variable for the duration of the test.
        unsafe { std::env::set_var("MCP_EXCHANGE_SUBJECT_TEST", "subject-secret") };
        let tools = load_mcp_tools(&config_path).await.unwrap();
        unsafe { std::env::remove_var("MCP_EXCHANGE_SUBJECT_TEST") };
        assert_eq!(tools.len(), 1);
        assert!(
            authorization
                .lock()
                .unwrap()
                .iter()
                .any(|value| value == "Bearer exchanged-token")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn http_mcp_reads_a_bearer_token_from_a_private_file() {
        let (address, authorization) = fake_http_server().await;
        let dir = tempfile::tempdir().unwrap();
        let token_file = dir.path().join("mcp.token");
        write_token_file(&token_file, "file-token\n", 0o600);
        let config_path = dir.path().join("mcp.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "fake",
                    "url": format!("http://{address}/mcp"),
                    "bearer_token_file": token_file
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let tools = load_mcp_tools(&config_path).await.unwrap();
        assert_eq!(tools.len(), 1);
        assert!(
            authorization
                .lock()
                .unwrap()
                .iter()
                .any(|value| value == "Bearer file-token")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn http_mcp_rejects_unsafe_bearer_token_files() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("mcp.json");
        let token_file = dir.path().join("mcp.token");
        for (name, token, mode) in [
            ("loose", "token", 0o644),
            ("empty", "", 0o600),
            ("whitespace", "two tokens", 0o600),
        ] {
            write_token_file(&token_file, token, mode);
            std::fs::write(
                &config_path,
                serde_json::to_vec(&json!({
                    "servers": [{
                        "name": name,
                        "url": "http://127.0.0.1:1/mcp",
                        "bearer_token_file": &token_file
                    }]
                }))
                .unwrap(),
            )
            .unwrap();
            assert!(load_mcp_tools(&config_path).await.is_err(), "{name}");
        }

        let target = dir.path().join("target.token");
        write_token_file(&target, "token", 0o600);
        let link = dir.path().join("link.token");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "symlink",
                    "url": "http://127.0.0.1:1/mcp",
                    "bearer_token_file": &link
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(load_mcp_tools(&config_path).await.is_err());

        std::fs::write(
            &config_path,
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "both",
                    "url": "http://127.0.0.1:1/mcp",
                    "bearer_token_env": "TOKEN",
                    "bearer_token_file": &target
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(load_mcp_tools(&config_path).await.is_err());
    }

    #[tokio::test]
    async fn capped_line_reader_bounds_memory() {
        let data = b"{\"a\":1}\nrest";
        let mut reader = tokio::io::BufReader::new(&data[..]);
        let line = read_line_capped(&mut reader, 64, Duration::from_secs(1), "fake")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(line, "{\"a\":1}\n");

        let data = b"123456\n";
        let mut small = tokio::io::BufReader::new(&data[..]);
        assert!(
            read_line_capped(&mut small, 3, Duration::from_secs(1), "fake")
                .await
                .is_err()
        );
    }

    #[test]
    fn response_body_cap_fails_closed() {
        let mut body = vec![0u8; MAX_RESPONSE_BYTES - 1];
        assert!(push_capped(&mut body, &[0, 0], "fake").is_err());
        assert!(push_capped(&mut body, &[0], "fake").is_ok());
    }

    #[test]
    fn sse_messages_are_parsed() {
        let body =
            "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":{\"ok\":true}}\n\n";
        let value = sse_response(body, 7, "fake").unwrap();
        assert_eq!(value["ok"], true);
        assert!(sse_response(body, 8, "fake").is_err());
    }

    #[tokio::test]
    async fn invalid_http_configs_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            json!({"name": "both", "command": ["true"], "url": "http://127.0.0.1:1/mcp"}),
            json!({"name": "neither"}),
            json!({"name": "remote", "url": "http://example.com/mcp"}),
            json!({"name": "credentials", "url": "https://user:pass@example.com/mcp"}),
            json!({"name": "timeout-zero", "url": "https://example.com/mcp", "timeout_seconds": 0}),
            json!({"name": "timeout-large", "url": "https://example.com/mcp", "timeout_seconds": 301}),
        ];
        for server in cases {
            let path = dir
                .path()
                .join(format!("mcp-{}.json", server["name"].as_str().unwrap()));
            std::fs::write(
                &path,
                serde_json::to_vec(&json!({"servers": [server]})).unwrap(),
            )
            .unwrap();
            assert!(load_mcp_tools(&path).await.is_err());
        }
    }

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
        assert_eq!(
            outcome.metadata.get("mcp_server").map(String::as_str),
            Some("fake")
        );
        assert_eq!(
            outcome.metadata.get("mcp_tool").map(String::as_str),
            Some("echo")
        );
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
