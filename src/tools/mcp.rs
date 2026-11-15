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
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const PROTOCOL_VERSION: &str = "2025-11-25";
const MODERN_PROTOCOL_VERSION: &str = "2026-07-28";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_SERVERS: usize = 16;
const MAX_TOOLS_PER_SERVER: usize = 64;
const MAX_TOOL_DESCRIPTION_CHARS: usize = 2000;
const MAX_TOOL_SCHEMA_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_TASK_WAIT: Duration = Duration::from_secs(120);
const MAX_TASK_POLLS: usize = 120;
const TASK_POLL_MIN_MS: u64 = 250;
const TASK_POLL_MAX_MS: u64 = 5_000;

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
    /// OAuth 2.1 authorization-code + PKCE for a remote HTTP server.
    #[serde(default)]
    pub oauth: Option<crate::mcp_oauth::OAuthConfig>,
    /// Per-request timeout in seconds (1..=300, default 20).
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Opt in to the `io.modelcontextprotocol/tasks` extension for modern
    /// servers. Task polling is bounded; in-task input still fails closed.
    #[serde(default)]
    pub tasks: bool,
    /// Optional tool-definition pin file. First use writes it 0600; later
    /// connections fail closed if any tool definition changed.
    #[serde(default)]
    pub pin_file: Option<PathBuf>,
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
        oauth: Option<crate::mcp_oauth::OAuthConfig>,
    },
}

struct McpClient {
    transport: Transport,
    next_id: AtomicU64,
    request_lock: Mutex<()>,
    server_name: String,
    timeout: Duration,
    modern: AtomicBool,
    tasks: bool,
    protocol_version: Mutex<String>,
    pin_file: Option<PathBuf>,
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
        if server.command.is_some() && (server.token_exchange.is_some() || server.oauth.is_some()) {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' token_exchange/oauth is only valid for HTTP servers",
                server.name
            )));
        }
        if server.oauth.is_some()
            && (server.token_exchange.is_some()
                || server.bearer_token_env.is_some()
                || server.bearer_token_file.is_some())
        {
            return Err(ToolError::InvalidArguments(format!(
                "MCP server '{}' must set only one authentication source",
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
            modern: AtomicBool::new(false),
            tasks: server.tasks,
            protocol_version: Mutex::new(PROTOCOL_VERSION.into()),
            pin_file: server.pin_file.clone(),
        });
        client.negotiate().await?;
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
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        // A third-party MCP server must not inherit provider keys, tokens or
        // webhook secrets. Only the minimum process environment is passed
        // through; `server.env` is the operator's explicit allowlist.
        for key in ["PATH", "HOME", "TMPDIR", "LANG", "LC_ALL", "SystemRoot"] {
            if let Some(value) = std::env::var_os(key) {
                child_command.env(key, value);
            }
        }
        child_command.envs(&server.env);
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
        if !loopback {
            // Close the MCP HTTP SSRF gap: reject private, link-local and
            // metadata addresses before reqwest resolves the host.
            super::web::resolve_public(&parsed).await?;
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
        let (bearer_token, token_expires_at_ms) = if let Some(oauth) = &server.oauth {
            let token = crate::mcp_oauth::access_token(oauth)
                .await
                .map_err(|error| {
                    ToolError::InvalidArguments(format!(
                        "MCP server '{}' OAuth failed: {error}",
                        server.name
                    ))
                })?;
            (Some(token.access_token), Some(token.expires_at_ms))
        } else if let Some(exchange) = &server.token_exchange {
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
            oauth: server.oauth.clone(),
        })
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), ToolError> {
        let modern = self.modern.load(Ordering::SeqCst);
        let version = self.protocol_version.lock().await.clone();
        let params = if modern {
            with_modern_meta(params, &version, self.tasks)
        } else {
            params
        };
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
                oauth,
            } => {
                let response = self
                    .http_post_with_refresh(
                        client,
                        url,
                        session_id,
                        bearer_token,
                        token_expires_at_ms,
                        exchange.as_ref(),
                        oauth.as_ref(),
                        &message,
                        method,
                        None,
                        modern,
                        &version,
                        &[],
                    )
                    .await?;
                let status = response.status();
                if !modern
                    && let Some(value) = response.headers().get("Mcp-Session-Id")
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
        self.request_with_headers(method, params, &[]).await
    }

    async fn request_with_headers(
        &self,
        method: &str,
        params: Value,
        extra_headers: &[(String, String)],
    ) -> Result<Value, ToolError> {
        let _guard = self.request_lock.lock().await;
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let modern = self.modern.load(Ordering::SeqCst) || method == "server/discover";
        let version = if method == "server/discover" {
            // The probe itself must announce the modern version; the cached
            // negotiated version is still legacy until discover succeeds.
            MODERN_PROTOCOL_VERSION.to_string()
        } else {
            self.protocol_version.lock().await.clone()
        };
        let tool_name = params
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let params = if modern {
            with_modern_meta(params, &version, self.tasks)
        } else {
            params
        };
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
                oauth,
            } => {
                let response = self
                    .http_post_with_refresh(
                        client,
                        url,
                        session_id,
                        bearer_token,
                        token_expires_at_ms,
                        exchange.as_ref(),
                        oauth.as_ref(),
                        &message,
                        method,
                        tool_name.as_deref(),
                        modern,
                        &version,
                        extra_headers,
                    )
                    .await?;
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                if !modern
                    && let Some(value) = response.headers().get("Mcp-Session-Id")
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

    #[allow(clippy::too_many_arguments)]
    async fn http_post(
        &self,
        client: &reqwest::Client,
        url: &str,
        session_id: &Mutex<Option<String>>,
        bearer_token: &Mutex<Option<String>>,
        message: &Value,
        method: &str,
        tool_name: Option<&str>,
        modern: bool,
        protocol_version: &str,
        extra_headers: &[(String, String)],
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
        if modern {
            request = request
                .header("MCP-Protocol-Version", protocol_version)
                .header("Mcp-Method", method);
            if let Some(tool_name) = tool_name {
                request = request.header("Mcp-Name", tool_name);
            }
        } else if let Some(session) = session_id.lock().await.clone() {
            request = request.header("Mcp-Session-Id", session);
        }
        for (name, value) in extra_headers {
            request = request.header(name, value);
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

    async fn refresh_oauth(
        &self,
        config: &crate::mcp_oauth::OAuthConfig,
        bearer_token: &Mutex<Option<String>>,
        token_expires_at_ms: &Mutex<Option<u128>>,
    ) -> Result<(), ToolError> {
        let token = crate::mcp_oauth::access_token(config)
            .await
            .map_err(|error| {
                ToolError::Failed(format!(
                    "MCP server '{}' OAuth refresh failed: {error}",
                    self.server_name
                ))
            })?;
        *token_expires_at_ms.lock().await = Some(token.expires_at_ms);
        *bearer_token.lock().await = Some(token.access_token);
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
        oauth: Option<&crate::mcp_oauth::OAuthConfig>,
        message: &Value,
        method: &str,
        tool_name: Option<&str>,
        modern: bool,
        protocol_version: &str,
        extra_headers: &[(String, String)],
    ) -> Result<reqwest::Response, ToolError> {
        let expiring = token_expires_at_ms
            .lock()
            .await
            .is_some_and(|expires| expires <= crate::current_time_ms() + 30_000);
        if let Some(config) = exchange
            && expiring
        {
            self.refresh_token(config, bearer_token, token_expires_at_ms)
                .await?;
        } else if let Some(config) = oauth
            && expiring
        {
            self.refresh_oauth(config, bearer_token, token_expires_at_ms)
                .await?;
        }
        let mut response = self
            .http_post(
                client,
                url,
                session_id,
                bearer_token,
                message,
                method,
                tool_name,
                modern,
                protocol_version,
                extra_headers,
            )
            .await?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(config) = exchange {
                self.refresh_token(config, bearer_token, token_expires_at_ms)
                    .await?;
            } else if let Some(config) = oauth {
                self.refresh_oauth(config, bearer_token, token_expires_at_ms)
                    .await?;
            } else {
                return Ok(response);
            }
            response = self
                .http_post(
                    client,
                    url,
                    session_id,
                    bearer_token,
                    message,
                    method,
                    tool_name,
                    modern,
                    protocol_version,
                    extra_headers,
                )
                .await?;
        }
        Ok(response)
    }

    async fn negotiate(&self) -> Result<(), ToolError> {
        // Modern servers answer server/discover without a session. Legacy
        // servers reject it, and we fall back to the initialize handshake.
        if let Ok(result) = self.request("server/discover", json!({})).await {
            self.modern.store(true, Ordering::SeqCst);
            let version = result
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(MODERN_PROTOCOL_VERSION);
            *self.protocol_version.lock().await = version.to_string();
            return Ok(());
        }
        let mut last_error = None;
        for version in [PROTOCOL_VERSION, "2025-06-18", "2024-11-05"] {
            match self
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": version,
                        "capabilities": {},
                        "clientInfo": {
                            "name": "1agent",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                )
                .await
            {
                Ok(result) => {
                    let negotiated = result
                        .get("protocolVersion")
                        .and_then(Value::as_str)
                        .unwrap_or(version);
                    *self.protocol_version.lock().await = negotiated.to_string();
                    self.notify("notifications/initialized", json!({})).await?;
                    return Ok(());
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ToolError::Failed(format!(
                "MCP server '{}' did not negotiate a protocol version",
                self.server_name
            ))
        }))
    }

    async fn list_tools(&self) -> Result<Vec<Value>, ToolError> {
        let result = self.request("tools/list", json!({})).await?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(path) = &self.pin_file {
            self.verify_tool_pins(path, &tools)?;
        }
        Ok(tools)
    }

    fn verify_tool_pins(&self, path: &Path, tools: &[Value]) -> Result<(), ToolError> {
        let current = tool_hashes(tools)?;
        let pinned = match read_tool_pins(path)? {
            Some(pinned) => pinned,
            None => {
                let pinned = ToolPinFile {
                    schema_version: 1,
                    server: self.server_name.clone(),
                    tools: current,
                };
                write_tool_pins(path, &pinned)?;
                return Ok(());
            }
        };
        if pinned.server != self.server_name {
            return Err(ToolError::Failed(format!(
                "MCP pin file {} belongs to server '{}', not '{}'",
                path.display(),
                pinned.server,
                self.server_name
            )));
        }
        if pinned.tools == current {
            return Ok(());
        }
        let added = current
            .keys()
            .filter(|name| !pinned.tools.contains_key(*name))
            .cloned()
            .collect::<Vec<_>>();
        let removed = pinned
            .tools
            .keys()
            .filter(|name| !current.contains_key(*name))
            .cloned()
            .collect::<Vec<_>>();
        let changed = current
            .iter()
            .filter(|(name, hash)| {
                pinned
                    .tools
                    .get(*name)
                    .is_some_and(|old_hash| old_hash != *hash)
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        Err(ToolError::Failed(format!(
            "MCP tool definitions changed; refusing to continue (added={added:?}, removed={removed:?}, changed={changed:?}). Review and delete {} to re-pin.",
            path.display()
        )))
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, ToolError> {
        self.call_tool_with_headers(name, arguments, &[]).await
    }

    async fn call_tool_with_headers(
        &self,
        name: &str,
        arguments: Value,
        extra_headers: &[(String, String)],
    ) -> Result<Value, ToolError> {
        let result = self
            .request_with_headers(
                "tools/call",
                json!({"name": name, "arguments": arguments}),
                extra_headers,
            )
            .await?;
        self.resolve_tool_result(result, name).await
    }

    async fn resolve_tool_result(&self, result: Value, tool: &str) -> Result<Value, ToolError> {
        match result
            .get("resultType")
            .and_then(Value::as_str)
            .unwrap_or("complete")
        {
            "complete" => Ok(result),
            "input_required" => Err(ToolError::InputRequired(input_required_description(
                &result,
                tool,
                "tools/call",
            ))),
            "task" => {
                if !self.tasks {
                    return Err(ToolError::Failed(format!(
                        "MCP server '{}' returned a task for '{tool}' but tasks is not enabled for this server; set \"tasks\": true or use a server that completes synchronously. The task was not polled.",
                        self.server_name
                    )));
                }
                self.poll_task(result, tool).await
            }
            other => Err(ToolError::Failed(format!(
                "MCP server '{}' returned unsupported resultType '{other}' for '{tool}'; refusing to guess",
                self.server_name
            ))),
        }
    }

    /// Bounded `tasks/get` polling. The task id is treated as an untrusted
    /// handle; in-task input fails closed and never auto-answers elicitation.
    async fn poll_task(&self, created: Value, tool: &str) -> Result<Value, ToolError> {
        let task_id = created
            .get("taskId")
            .and_then(Value::as_str)
            .filter(|id| {
                !id.is_empty()
                    && id.chars().count() <= 128
                    && id.chars().all(|ch| ch.is_ascii_graphic())
            })
            .ok_or_else(|| {
                ToolError::Failed(format!(
                    "MCP server '{}' returned a task without a safe taskId",
                    self.server_name
                ))
            })?
            .to_string();
        let ttl_ms = created
            .get("ttlMs")
            .and_then(Value::as_u64)
            .unwrap_or(60_000)
            .min(MAX_TASK_WAIT.as_millis() as u64);
        let interval_ms = created
            .get("pollIntervalMs")
            .and_then(Value::as_u64)
            .unwrap_or(1_000)
            .clamp(TASK_POLL_MIN_MS, TASK_POLL_MAX_MS);
        let deadline = std::time::Instant::now() + Duration::from_millis(ttl_ms.max(1));
        for _ in 0..MAX_TASK_POLLS {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(interval_ms).min(remaining)).await;
            let state = self
                .request("tasks/get", json!({"taskId": task_id}))
                .await?;
            let task = state.get("task").unwrap_or(&state);
            match task.get("status").and_then(Value::as_str) {
                Some("working") => continue,
                Some("input_required") => {
                    return Err(ToolError::InputRequired(format!(
                        "MCP task {task_id} for '{tool}' needs input during execution: {}",
                        input_request_details(task)
                    )));
                }
                Some("completed") => {
                    return task.get("result").cloned().ok_or_else(|| {
                        ToolError::Failed(format!(
                            "MCP task {task_id} for '{tool}' completed without a result"
                        ))
                    });
                }
                Some("failed") => {
                    return Err(ToolError::Failed(format!(
                        "MCP task {task_id} for '{tool}' failed: {}",
                        bounded(
                            &task
                                .get("error")
                                .cloned()
                                .unwrap_or(Value::Null)
                                .to_string(),
                            300
                        )
                    )));
                }
                Some("cancelled") => {
                    return Err(ToolError::Failed(format!(
                        "MCP task {task_id} for '{tool}' was cancelled"
                    )));
                }
                other => {
                    return Err(ToolError::Failed(format!(
                        "MCP task {task_id} for '{tool}' returned an invalid status {other:?}; refusing to continue"
                    )));
                }
            }
        }
        let _ = self
            .request("tasks/cancel", json!({"taskId": task_id}))
            .await;
        Err(ToolError::Failed(format!(
            "MCP task {task_id} for '{tool}' did not finish within {} ms; cancellation requested",
            ttl_ms
        )))
    }
}

fn input_required_description(result: &Value, tool: &str, method: &str) -> String {
    format!(
        "MCP tool '{tool}' returned {method} input_required: {}. Interactive elicitation/sampling/roots input is refused; no input was sent. Ask the human to handle it out-of-band.",
        input_request_details(result)
    )
}

fn input_request_details(result: &Value) -> String {
    let mut details = Vec::new();
    if let Some(requests) = result.get("inputRequests").and_then(Value::as_object) {
        for (key, request) in requests.iter().take(8) {
            let key: String = key.chars().take(64).collect();
            let method = request
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .chars()
                .take(80)
                .collect::<String>();
            let mut detail = format!("{key} ({method})");
            if let Some(url) = request
                .get("params")
                .and_then(|params| params.get("url"))
                .and_then(Value::as_str)
                && let Ok(parsed) = reqwest::Url::parse(url)
            {
                detail.push_str(&format!(
                    " url-origin={}",
                    parsed.origin().ascii_serialization()
                ));
            }
            details.push(detail);
        }
    }
    if result.get("requestState").is_some() {
        details.push("requestState present (not echoed or inspected)".into());
    }
    if details.is_empty() {
        "no inputRequests; the server returned an empty input_required result".into()
    } else {
        details.join("; ")
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolPinFile {
    schema_version: u32,
    server: String,
    tools: BTreeMap<String, String>,
}

fn tool_hashes(tools: &[Value]) -> Result<BTreeMap<String, String>, ToolError> {
    let mut hashes = BTreeMap::new();
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| ToolError::Failed("MCP tool has no name".into()))?;
        let canonical = canonical_json(tool);
        let bytes = serde_json::to_vec(&canonical)
            .map_err(|error| ToolError::Failed(format!("cannot canonicalize MCP tool: {error}")))?;
        hashes.insert(name.to_string(), hex::encode(Sha256::digest(&bytes)));
    }
    Ok(hashes)
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            let mut sorted = serde_json::Map::new();
            for key in keys {
                let value = map.get(key).cloned().unwrap_or(Value::Null);
                sorted.insert(key.clone(), canonical_json(&value));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn read_tool_pins(path: &Path) -> Result<Option<ToolPinFile>, ToolError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ToolError::Failed(format!(
                "cannot read MCP pin file {}: {error}",
                path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ToolError::Failed(format!(
            "MCP pin file {} must be a regular non-symlink file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ToolError::Failed(format!(
                "MCP pin file {} must be private (chmod 600)",
                path.display()
            )));
        }
    }
    let bytes = std::fs::read(path).map_err(|error| {
        ToolError::Failed(format!(
            "cannot read MCP pin file {}: {error}",
            path.display()
        ))
    })?;
    let pinned: ToolPinFile = serde_json::from_slice(&bytes).map_err(|error| {
        ToolError::Failed(format!("invalid MCP pin file {}: {error}", path.display()))
    })?;
    if pinned.schema_version != 1 {
        return Err(ToolError::Failed(format!(
            "unsupported MCP pin schema {} in {}",
            pinned.schema_version,
            path.display()
        )));
    }
    Ok(Some(pinned))
}

fn write_tool_pins(path: &Path, pins: &ToolPinFile) -> Result<(), ToolError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            ToolError::Failed(format!(
                "cannot create MCP pin directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let bytes = serde_json::to_vec_pretty(pins)
        .map_err(|error| ToolError::Failed(format!("cannot serialize MCP pins: {error}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| {
                ToolError::Failed(format!(
                    "cannot create MCP pin file {}: {error}",
                    path.display()
                ))
            })?;
        use std::io::Write;
        file.write_all(&bytes).map_err(|error| {
            ToolError::Failed(format!(
                "cannot write MCP pin file {}: {error}",
                path.display()
            ))
        })?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes).map_err(|error| {
            ToolError::Failed(format!(
                "cannot write MCP pin file {}: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn with_modern_meta(params: Value, protocol_version: &str, tasks: bool) -> Value {
    let mut object = match params {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("value".into(), other);
            map
        }
    };
    let capabilities = if tasks {
        json!({"extensions": {"io.modelcontextprotocol/tasks": {}}})
    } else {
        json!({})
    };
    object.insert(
        "_meta".into(),
        json!({
            "io.modelcontextprotocol/protocolVersion": protocol_version,
            "io.modelcontextprotocol/clientInfo": {
                "name": "1agent",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "io.modelcontextprotocol/clientCapabilities": capabilities,
        }),
    );
    Value::Object(object)
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

fn tool_requires_write(_tool: &Value) -> bool {
    // Server-provided annotations are untrusted: a malicious server can mark a
    // destructive tool `readOnlyHint: true` and have it bypass read-only mode,
    // the kill switch and write-level Authority checks. Until an operator-side
    // allowlist exists, every MCP tool is write-capable.
    true
}

fn validate_tool_arguments(schema: &Value, arguments: &Value) -> Result<(), ToolError> {
    let Some(object) = arguments.as_object() else {
        if schema.get("type").and_then(Value::as_str) == Some("object") {
            return Err(ToolError::InvalidArguments(
                "MCP arguments must be a JSON object".into(),
            ));
        }
        return Ok(());
    };
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !object.contains_key(name) {
                return Err(ToolError::InvalidArguments(format!(
                    "missing required argument '{name}'"
                )));
            }
        }
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(properties) = properties {
        for (name, value) in object {
            match properties.get(name) {
                Some(property) => validate_value_type(property, value, name)?,
                None if schema.get("additionalProperties").and_then(Value::as_bool)
                    == Some(false) =>
                {
                    return Err(ToolError::InvalidArguments(format!(
                        "unknown argument '{name}'"
                    )));
                }
                None => {}
            }
        }
    }
    Ok(())
}

fn validate_value_type(schema: &Value, value: &Value, path: &str) -> Result<(), ToolError> {
    let Some(expected) = schema.get("type").and_then(Value::as_str) else {
        return Ok(());
    };
    let valid = match expected {
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "null" => value.is_null(),
        _ => true,
    };
    if !valid {
        return Err(ToolError::InvalidArguments(format!(
            "argument '{path}' must be a {expected}"
        )));
    }
    if expected == "array"
        && let Some(items) = schema.get("items")
        && let Some(array) = value.as_array()
    {
        for (index, item) in array.iter().enumerate() {
            validate_value_type(items, item, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

struct McpTool {
    client: Arc<McpClient>,
    remote_name: String,
    spec: ToolSpec,
    input_schema: Value,
    /// `argument name -> HTTP header name` from the modern `x-mcp-header`
    /// schema annotation.
    header_map: BTreeMap<String, String>,
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
        if self.client.tasks {
            self.client.timeout + MAX_TASK_WAIT + Duration::from_secs(5)
        } else {
            self.client.timeout + Duration::from_secs(5)
        }
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_network {
            return Err(ToolError::PermissionDenied(
                "MCP tools require --allow-network".into(),
            ));
        }
        validate_tool_arguments(&self.input_schema, &arguments)?;
        let headers = tool_header_values(&self.header_map, &arguments)?;
        let result = if headers.is_empty() {
            self.client.call_tool(&self.remote_name, arguments).await?
        } else {
            self.client
                .call_tool_with_headers(&self.remote_name, arguments, &headers)
                .await?
        };
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

fn tool_header_map(schema: &Value) -> Result<BTreeMap<String, String>, ToolError> {
    let mut headers = BTreeMap::new();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(headers);
    };
    for (argument, property) in properties {
        let Some(header) = property.get("x-mcp-header").and_then(Value::as_str) else {
            continue;
        };
        if !valid_header_name(header) {
            return Err(ToolError::Failed(format!(
                "MCP tool declares an unsafe x-mcp-header name '{header}' for argument '{argument}'"
            )));
        }
        let header = format!("Mcp-Param-{header}");
        if reserved_header(&header) {
            return Err(ToolError::Failed(format!(
                "MCP tool declares a reserved x-mcp-header name '{header}' for argument '{argument}'"
            )));
        }
        if headers.len() >= 16 {
            return Err(ToolError::Failed(
                "MCP tool declares more than 16 x-mcp-header parameters".into(),
            ));
        }
        headers.insert(argument.clone(), header);
    }
    Ok(headers)
}

fn tool_header_values(
    header_map: &BTreeMap<String, String>,
    arguments: &Value,
) -> Result<Vec<(String, String)>, ToolError> {
    let mut headers = Vec::with_capacity(header_map.len());
    for (argument, header) in header_map {
        let Some(value) = arguments.get(argument) else {
            continue;
        };
        let text = match value {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            _ => {
                return Err(ToolError::InvalidArguments(format!(
                    "x-mcp-header argument '{argument}' must be a string, number or boolean"
                )));
            }
        };
        if text.len() > 4096
            || text
                .chars()
                .any(|character| character.is_control() && character != '\t')
        {
            return Err(ToolError::InvalidArguments(format!(
                "x-mcp-header argument '{argument}' contains an unsafe value"
            )));
        }
        headers.push((header.clone(), mirrored_header_value(&text)));
    }
    Ok(headers)
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn mirrored_header_value(value: &str) -> String {
    if value == value.trim()
        && value
            .chars()
            .all(|character| (' '..='~').contains(&character))
        && !value.starts_with("=?base64?")
        && !value.ends_with("?=")
    {
        return value.to_string();
    }
    use base64::Engine;
    format!(
        "=?base64?{}?=",
        base64::engine::general_purpose::STANDARD.encode(value.as_bytes())
    )
}

fn reserved_header(name: &str) -> bool {
    [
        "mcp-protocol-version",
        "mcp-method",
        "mcp-name",
        "authorization",
        "host",
        "content-length",
        "content-type",
        "connection",
        "transfer-encoding",
        "cookie",
    ]
    .iter()
    .any(|reserved| name.eq_ignore_ascii_case(reserved))
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
    let schema_is_bounded = serde_json::to_vec(&schema)
        .map(|bytes| bytes.len() <= MAX_TOOL_SCHEMA_BYTES)
        .unwrap_or(false);
    let presented_schema = if schema_is_bounded {
        schema.clone()
    } else {
        json!({"type": "object", "additionalProperties": true})
    };
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
            parameters: presented_schema,
        },
        header_map: tool_header_map(&schema)?,
        input_schema: schema,
        risk: tool_risk(tool),
        requires_write: tool_requires_write(tool),
    })
}

fn bounded(value: &str, max_chars: usize) -> String {
    let mut bounded: String = value
        .chars()
        .filter(|character| !crate::safety::is_disallowed_control(*character))
        .take(max_chars)
        .collect();
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

    #[test]
    fn mcp_argument_schema_validation_fails_closed() {
        let schema = json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["text"],
            "properties": {
                "text": {"type": "string"},
                "count": {"type": "integer"}
            }
        });
        assert!(validate_tool_arguments(&schema, &json!({"text": "x"})).is_ok());
        assert!(validate_tool_arguments(&schema, &json!({})).is_err());
        assert!(validate_tool_arguments(&schema, &json!({"text": 1})).is_err());
        assert!(validate_tool_arguments(&schema, &json!({"text": "x", "extra": true})).is_err());
    }

    #[test]
    fn x_mcp_header_annotations_are_validated_and_scalarized() {
        let schema = json!({
            "type": "object",
            "properties": {
                "region": {"type": "string", "x-mcp-header": "Region"},
                "attempt": {"type": "integer", "x-mcp-header": "Attempt"},
                "flag": {"type": "boolean", "x-mcp-header": "Flag"}
            }
        });
        let map = tool_header_map(&schema).unwrap();
        assert_eq!(
            map.get("region").map(String::as_str),
            Some("Mcp-Param-Region")
        );
        let headers =
            tool_header_values(&map, &json!({"region": "eu", "attempt": 2, "flag": true})).unwrap();
        assert!(headers.contains(&("Mcp-Param-Region".into(), "eu".into())));
        assert!(headers.contains(&("Mcp-Param-Attempt".into(), "2".into())));
        assert!(headers.contains(&("Mcp-Param-Flag".into(), "true".into())));
        let encoded = tool_header_values(&map, &json!({"region": "café"})).unwrap();
        assert!(encoded[0].1.starts_with("=?base64?"));
        assert!(encoded[0].1.ends_with("?="));
        assert!(
            tool_header_values(&map, &json!({"region": 1.5}))
                .unwrap()
                .iter()
                .any(|(_, value)| value == "1.5")
        );
        assert!(tool_header_values(&map, &json!({"region": ["nested"]})).is_err());
        assert!(
            tool_header_map(&json!({
                "type": "object",
                "properties": {"x": {"x-mcp-header": "Bad\nName"}}
            }))
            .is_err()
        );
    }

    #[test]
    fn tool_hashes_are_canonical_and_pin_files_round_trip() {
        let left = json!({
            "name": "echo",
            "description": "echo",
            "inputSchema": {"type": "object", "properties": {"b": {"type": "string"}, "a": {"type": "integer"}}}
        });
        let right = json!({
            "inputSchema": {"properties": {"a": {"type": "integer"}, "b": {"type": "string"}}, "type": "object"},
            "description": "echo",
            "name": "echo"
        });
        assert_eq!(
            tool_hashes(&[left]).unwrap(),
            tool_hashes(std::slice::from_ref(&right)).unwrap()
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pins.json");
        let pins = ToolPinFile {
            schema_version: 1,
            server: "fake".into(),
            tools: tool_hashes(std::slice::from_ref(&right)).unwrap(),
        };
        write_tool_pins(&path, &pins).unwrap();
        let loaded = read_tool_pins(&path).unwrap().unwrap();
        assert_eq!(loaded.tools, pins.tools);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    async fn fake_modern_http_server() -> (std::net::SocketAddr, Arc<std::sync::Mutex<Vec<String>>>)
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_for_server = captured.clone();
        let task_polls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let task_polls_for_server = task_polls.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let captured = captured_for_server.clone();
                let task_polls = task_polls_for_server.clone();
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
                        captured.lock().unwrap().push(headers.to_ascii_lowercase());
                        let message: Value =
                            serde_json::from_slice(&buffer[end + 4..end + 4 + length])
                                .unwrap_or(Value::Null);
                        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
                        let id = message.get("id").cloned().unwrap_or(Value::Null);
                        let result = match method {
                            "server/discover" => json!({
                                "protocolVersion": MODERN_PROTOCOL_VERSION,
                                "capabilities": {"tools": {}},
                                "serverInfo": {"name": "modern", "version": "1"},
                            }),
                            "tools/list" => json!({
                                "tools": [{
                                    "name": "echo",
                                    "description": "echo",
                                    "inputSchema": {"type": "object", "properties": {"text": {"type": "string", "x-mcp-header": "X-Test-Text"}}}
                                }]
                            }),
                            "tools/call" => {
                                let text = message
                                    .pointer("/params/arguments/text")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                match text {
                                    "input" => json!({
                                        "resultType": "input_required",
                                        "inputRequests": {
                                            "confirm": {
                                                "method": "elicitation/create",
                                                "params": {
                                                    "message": "Approve?",
                                                    "url": "https://example.com/secret?q=1"
                                                }
                                            }
                                        },
                                        "requestState": "opaque-state"
                                    }),
                                    "task" | "task-input" => json!({
                                        "resultType": "task",
                                        "taskId": if text == "task" { "task-1" } else { "task-input-1" },
                                        "status": "working",
                                        "ttlMs": 5000,
                                        "pollIntervalMs": 1,
                                        "createdAt": "2026-01-01T00:00:00Z",
                                        "lastUpdatedAt": "2026-01-01T00:00:00Z"
                                    }),
                                    _ => {
                                        json!({"content": [{"type": "text", "text": format!("echo:{text}")}]})
                                    }
                                }
                            }
                            "tasks/get" => {
                                let task_id = message
                                    .pointer("/params/taskId")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                if task_id.contains("input") {
                                    json!({
                                        "taskId": task_id,
                                        "status": "input_required",
                                        "inputRequests": {
                                            "confirm": {
                                                "method": "elicitation/create",
                                                "params": {"message": "Approve?"}
                                            }
                                        }
                                    })
                                } else if task_polls.fetch_add(1, Ordering::SeqCst) == 0 {
                                    json!({"taskId": task_id, "status": "working"})
                                } else {
                                    json!({
                                        "taskId": task_id,
                                        "status": "completed",
                                        "result": {"content": [{"type": "text", "text": "task:done"}]}
                                    })
                                }
                            }
                            "tasks/cancel" => json!({}),
                            other => json!({"error": {"code": -32601, "message": other}}),
                        };
                        let payload = if method == "server/discover"
                            || method == "tools/list"
                            || method == "tools/call"
                            || method == "tasks/get"
                            || method == "tasks/cancel"
                        {
                            json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
                        } else {
                            json!({"jsonrpc": "2.0", "id": id, "error": result["error"].clone()})
                                .to_string()
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                            payload.len()
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                        return;
                    }
                });
            }
        });
        (address, captured)
    }

    #[tokio::test]
    async fn modern_stateless_mcp_negotiates_and_sends_headers() {
        let (address, captured) = fake_modern_http_server().await;
        let server = McpServerConfig {
            name: "modern".into(),
            command: None,
            url: Some(format!("http://{address}/mcp")),
            env: BTreeMap::new(),
            bearer_token_env: None,
            bearer_token_file: None,
            token_exchange: None,
            oauth: None,
            timeout_seconds: None,
            tasks: false,
            pin_file: None,
        };
        let client = McpClient::spawn(&server).await.unwrap();
        assert!(client.modern.load(Ordering::SeqCst));
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        let tool = build_tool(client.clone(), "modern", &tools[0]).unwrap();
        let outcome = tool
            .execute(
                json!({"text": "hi"}),
                &PermissionSet {
                    allow_network: true,
                    ..PermissionSet::default()
                },
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.content, "echo:hi");
        let captured = captured.lock().unwrap();
        assert!(
            captured
                .iter()
                .any(|headers| headers.contains("mcp-method: server/discover"))
        );
        assert!(
            captured
                .iter()
                .any(|headers| headers.contains("mcp-name: echo"))
        );
        assert!(
            captured
                .iter()
                .any(|headers| headers.contains("mcp-param-x-test-text: hi")),
            "x-mcp-header value missing from captured headers: {captured:?}"
        );
        assert!(
            captured
                .iter()
                .any(|headers| headers.contains("mcp-protocol-version: 2026-07-28"))
        );
        assert!(
            !captured
                .iter()
                .any(|headers| headers.contains("mcp-session-id:"))
        );
    }

    fn modern_server(address: std::net::SocketAddr, tasks: bool) -> McpServerConfig {
        McpServerConfig {
            name: "modern".into(),
            command: None,
            url: Some(format!("http://{address}/mcp")),
            env: BTreeMap::new(),
            bearer_token_env: None,
            bearer_token_file: None,
            token_exchange: None,
            oauth: None,
            timeout_seconds: None,
            tasks,
            pin_file: None,
        }
    }

    #[tokio::test]
    async fn mrtr_input_required_fails_closed_without_echoing_state() {
        let (address, _) = fake_modern_http_server().await;
        let client = McpClient::spawn(&modern_server(address, false))
            .await
            .unwrap();
        let error = client
            .call_tool("echo", json!({"text": "input"}))
            .await
            .unwrap_err();
        let ToolError::InputRequired(message) = error else {
            panic!("expected input_required, got {error:?}");
        };
        assert!(message.contains("elicitation/create"), "{message}");
        assert!(
            message.contains("url-origin=https://example.com"),
            "{message}"
        );
        assert!(
            !message.contains("secret"),
            "URL path must not leak: {message}"
        );
        assert!(
            !message.contains("opaque-state"),
            "requestState must not leak"
        );
    }

    #[tokio::test]
    async fn tasks_poll_to_completion_when_enabled() {
        let (address, _) = fake_modern_http_server().await;
        let client = McpClient::spawn(&modern_server(address, true))
            .await
            .unwrap();
        let result = client
            .call_tool("echo", json!({"text": "task"}))
            .await
            .unwrap();
        assert_eq!(result["content"][0]["text"], "task:done");
    }

    #[tokio::test]
    async fn tasks_are_refused_when_not_enabled() {
        let (address, _) = fake_modern_http_server().await;
        let client = McpClient::spawn(&modern_server(address, false))
            .await
            .unwrap();
        let error = client
            .call_tool("echo", json!({"text": "task"}))
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("tasks is not enabled"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn in_task_input_required_fails_closed() {
        let (address, _) = fake_modern_http_server().await;
        let client = McpClient::spawn(&modern_server(address, true))
            .await
            .unwrap();
        let error = client
            .call_tool("echo", json!({"text": "task-input"}))
            .await
            .unwrap_err();
        let ToolError::InputRequired(message) = error else {
            panic!("expected input_required, got {error:?}");
        };
        assert!(
            message.contains("needs input during execution"),
            "{message}"
        );
        assert!(message.contains("elicitation/create"), "{message}");
    }

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
        assert!(tools[0].requires_write());
        let outcome = tools[0]
            .execute(
                json!({"text": "hello"}),
                &PermissionSet {
                    allow_network: true,
                    ..PermissionSet::default()
                },
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
                &PermissionSet {
                    allow_network: true,
                    ..PermissionSet::default()
                },
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
        assert!(echo.requires_write());
        let write = tools
            .iter()
            .find(|tool| tool.spec().name == "mcp_fake_write")
            .unwrap();
        assert!(write.requires_write());

        let outcome = echo
            .execute(
                json!({"text": "hello"}),
                &PermissionSet {
                    allow_network: true,
                    ..PermissionSet::default()
                },
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
