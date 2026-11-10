//! Terminal tools backed by the Skynet Workshop session CLI.
//!
//! These tools create and drive a persistent PTY. They are high-risk and the
//! registry denies them unless the run has write permission. The session id and
//! every other argument are passed as separate argv elements; no shell is used
//! to build the command line.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::safety::{contains_disallowed_control, is_disallowed_control};

const DEFAULT_BIN: &str = "skynet-workshop-session";
const ENV_BIN: &str = "SKYNET_WORKSHOP_SESSION_BIN";
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_TEXT_CHARS: usize = 8 * 1024;
const MAX_SESSION_ID_CHARS: usize = 128;
const MAX_NAME_CHARS: usize = 64;
const MAX_EXEC_TIMEOUT_SECS: u64 = 300;
const DEFAULT_EXEC_TIMEOUT_SECS: u64 = 60;
const MAX_WAIT_MS: u64 = 30_000;

#[derive(Debug, Clone)]
struct TerminalCli {
    bin: PathBuf,
}

impl TerminalCli {
    fn new(bin: impl Into<PathBuf>) -> Self {
        Self { bin: bin.into() }
    }

    fn from_env() -> Self {
        let bin = std::env::var_os(ENV_BIN)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_BIN));
        Self::new(bin)
    }

    async fn run(
        &self,
        args: &[String],
        timeout: Duration,
    ) -> Result<std::process::Output, ToolError> {
        let mut command = tokio::process::Command::new(&self.bin);
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        match tokio::time::timeout(timeout, command.output()).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(ToolError::Failed(format!(
                    "terminal tools require Skynet Workshop; the current release supports Linux x86_64 only (missing {})",
                    self.bin.display()
                )))
            }
            Ok(Err(error)) => Err(ToolError::Failed(format!(
                "cannot run {}: {error}",
                self.bin.display()
            ))),
            Err(_) => Err(ToolError::TimedOut),
        }
    }
}

fn parse_response(output: &std::process::Output, cli: &TerminalCli) -> Result<Value, ToolError> {
    ensure_success(output, cli)?;
    serde_json::from_slice(&output.stdout).map_err(|error| {
        ToolError::Failed(format!("invalid {} response: {error}", cli.bin.display()))
    })
}

fn parse_exec_response(
    output: &std::process::Output,
    cli: &TerminalCli,
) -> Result<Value, ToolError> {
    serde_json::from_slice(&output.stdout).map_err(|error| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        ToolError::Failed(format!(
            "{} returned no JSON (exit {}): {error}; stderr: {}",
            cli.bin.display(),
            output.status,
            bounded_text(stderr.trim(), 2_000)
        ))
    })
}

fn ensure_success(output: &std::process::Output, cli: &TerminalCli) -> Result<(), ToolError> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    Err(ToolError::Failed(if detail.is_empty() {
        format!("{} exited {}", cli.bin.display(), output.status)
    } else {
        format!(
            "{} exited {}: {}",
            cli.bin.display(),
            output.status,
            bounded_text(detail, 2_000)
        )
    }))
}

fn bounded_text(value: &str, max_chars: usize) -> String {
    let mut bounded: String = value.chars().take(max_chars).collect();
    if value.chars().nth(max_chars).is_some() {
        bounded.push('…');
    }
    bounded
}

fn validate_session_id(value: &str) -> Result<(), ToolError> {
    if value.is_empty()
        || value.len() > MAX_SESSION_ID_CHARS
        || !value.starts_with("term-")
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ToolError::InvalidArguments(
            "session_id must look like term-<session> and contain only letters, digits, '-' or '_'"
                .into(),
        ));
    }
    Ok(())
}

fn required_session_id(arguments: &Value) -> Result<String, ToolError> {
    let value = arguments
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments("session_id is required".into()))?;
    validate_session_id(value)?;
    Ok(value.to_string())
}

fn optional_u64(arguments: &Value, key: &str) -> Result<Option<u64>, ToolError> {
    match arguments.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            ToolError::InvalidArguments(format!("{key} must be a non-negative integer"))
        }),
    }
}

async fn resolve_cwd(arguments: &Value, permissions: &PermissionSet) -> Result<PathBuf, ToolError> {
    let requested = match arguments.get("cwd").and_then(Value::as_str) {
        Some(cwd) => PathBuf::from(cwd),
        None => permissions
            .read_roots
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(".")),
    };
    let canonical = tokio::fs::canonicalize(&requested)
        .await
        .map_err(|error| ToolError::InvalidArguments(format!("cwd is not accessible: {error}")))?;
    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|error| ToolError::InvalidArguments(format!("cwd is not accessible: {error}")))?;
    if !metadata.is_dir() {
        return Err(ToolError::InvalidArguments(
            "cwd must point to a directory".into(),
        ));
    }
    if !permissions.read_roots.is_empty() {
        let mut allowed = false;
        for root in &permissions.read_roots {
            if let Ok(root) = tokio::fs::canonicalize(root).await
                && canonical.starts_with(&root)
            {
                allowed = true;
                break;
            }
        }
        if !allowed {
            return Err(ToolError::PermissionDenied(
                "cwd is outside the allowed read roots".into(),
            ));
        }
    }
    Ok(canonical)
}

fn validate_terminal_text(value: &str) -> Result<(), ToolError> {
    if value.is_empty() {
        return Err(ToolError::InvalidArguments("text must not be empty".into()));
    }
    if value.chars().count() > MAX_TEXT_CHARS {
        return Err(ToolError::InvalidArguments(format!(
            "text must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    if contains_disallowed_control(value) {
        return Err(ToolError::InvalidArguments(
            "text contains hidden or disallowed control characters".into(),
        ));
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<(), ToolError> {
    if value.is_empty()
        || value.len() > MAX_NAME_CHARS
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ToolError::InvalidArguments(
            "name must contain only letters, digits, '-', '_' or '.'".into(),
        ));
    }
    Ok(())
}

/// Strip ANSI/VT escape sequences and control characters from a PTY stream.
///
/// Newlines and tabs are preserved; carriage returns are dropped so CRLF input
/// becomes LF. This keeps tool observations safe to journal and render.
fn sanitize_terminal_output(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\u{001b}' => match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('\u{0040}'..='\u{007e}').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\u{0007}' {
                            break;
                        }
                        if next == '\u{001b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '\r' => {}
            '\n' | '\t' => output.push(character),
            character if character.is_control() || is_disallowed_control(character) => {}
            character => output.push(character),
        }
    }
    output
}

async fn read_artifact(path: &Path, max_bytes: usize) -> Result<(String, bool), ToolError> {
    if !path.is_absolute() {
        return Err(ToolError::Failed(
            "exec output path must be absolute".into(),
        ));
    }
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot stat exec output: {error}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ToolError::Failed(
            "exec output is not a regular file".into(),
        ));
    }
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot open exec output: {error}")))?;
    let mut bytes = Vec::new();
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot read exec output: {error}")))?;
    let truncated = bytes.len() > max_bytes;
    bytes.truncate(max_bytes);
    Ok((
        sanitize_terminal_output(&String::from_utf8_lossy(&bytes)),
        truncated,
    ))
}

fn session_metadata(session_id: &str) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("session_id".into(), session_id.into());
    metadata
}

/// Create a persistent terminal session.
pub struct TerminalCreateTool {
    cli: TerminalCli,
}

impl TerminalCreateTool {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: TerminalCli::new(bin),
        }
    }

    pub fn from_env() -> Self {
        Self {
            cli: TerminalCli::from_env(),
        }
    }
}

#[async_trait]
impl AgentTool for TerminalCreateTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "terminal_create".into(),
            description: "Create a persistent terminal (PTY) session through Skynet Workshop. Returns a session_id. Use terminal_write and terminal_output for interactive programs such as copilot or gh auth login; use terminal_exec for non-interactive commands. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "cwd": {
                        "type": "string",
                        "description": "Working directory. Must be inside an allowed read root."
                    },
                    "name": {
                        "type": "string",
                        "maxLength": MAX_NAME_CHARS,
                        "description": "Optional session name."
                    },
                    "rows": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 500,
                        "description": "Terminal rows, default 24."
                    },
                    "cols": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 500,
                        "description": "Terminal columns, default 80."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let cwd = resolve_cwd(&arguments, permissions).await?;
        let name = match arguments.get("name").and_then(Value::as_str) {
            Some(name) => {
                validate_name(name)?;
                Some(name.to_string())
            }
            None => None,
        };
        let rows = optional_u64(&arguments, "rows")?
            .unwrap_or(24)
            .clamp(1, 500);
        let cols = optional_u64(&arguments, "cols")?
            .unwrap_or(80)
            .clamp(1, 500);

        let mut args = vec!["create".to_string(), cwd.display().to_string()];
        if let Some(name) = &name {
            args.push("--name".into());
            args.push(name.clone());
        }
        args.push("--rows".into());
        args.push(rows.to_string());
        args.push("--cols".into());
        args.push(cols.to_string());

        let output = self.cli.run(&args, Duration::from_secs(30)).await?;
        let value = parse_response(&output, &self.cli)?;
        let session_id = value
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::Failed("workshop response is missing session_id".into()))?;
        validate_session_id(session_id)?;

        let content = json!({
            "session_id": session_id,
            "cwd": cwd.display().to_string(),
            "shell": value.get("shell").cloned().unwrap_or(Value::Null),
            "status": value.get("status").cloned().unwrap_or(Value::Null),
            "attach": format!("skynet-workshop-session attach {session_id}")
        });
        let mut metadata = session_metadata(session_id);
        metadata.insert("cwd".into(), cwd.display().to_string());
        Ok(ToolOutcome {
            summary: format!("终端会话已创建：{session_id}"),
            content: content.to_string(),
            metadata,
        })
    }
}

/// Type text into an existing terminal session.
pub struct TerminalWriteTool {
    cli: TerminalCli,
}

impl TerminalWriteTool {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: TerminalCli::new(bin),
        }
    }

    pub fn from_env() -> Self {
        Self {
            cli: TerminalCli::from_env(),
        }
    }
}

#[async_trait]
impl AgentTool for TerminalWriteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "terminal_write".into(),
            description: "Type text into an existing terminal session. By default Enter is appended, so the text runs as a command. Set submit=false to type without pressing Enter, for example when an interactive prompt asks for a value. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["session_id", "text"],
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session id returned by terminal_create."
                    },
                    "text": {
                        "type": "string",
                        "maxLength": MAX_TEXT_CHARS,
                        "description": "Text to type into the session."
                    },
                    "submit": {
                        "type": "boolean",
                        "description": "Append Enter. Default true."
                    },
                    "require_idle": {
                        "type": "boolean",
                        "description": "Refuse to write while the session is busy. Default false."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let session_id = required_session_id(&arguments)?;
        let text = arguments
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("text is required".into()))?;
        validate_terminal_text(text)?;
        let submit = arguments
            .get("submit")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let require_idle = arguments
            .get("require_idle")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut args = vec!["write".to_string(), session_id.clone()];
        if !submit {
            args.push("--raw".into());
        }
        if require_idle {
            args.push("--require-idle".into());
        }
        args.push(text.to_string());

        let output = self.cli.run(&args, Duration::from_secs(30)).await?;
        ensure_success(&output, &self.cli)?;

        let content = json!({
            "session_id": session_id,
            "chars": text.chars().count(),
            "submitted": submit
        });
        Ok(ToolOutcome {
            summary: format!("已向 {session_id} 写入 {} 个字符", text.chars().count()),
            content: content.to_string(),
            metadata: session_metadata(&session_id),
        })
    }
}

/// Read bounded output from a terminal session.
pub struct TerminalOutputTool {
    cli: TerminalCli,
}

impl TerminalOutputTool {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: TerminalCli::new(bin),
        }
    }

    pub fn from_env() -> Self {
        Self {
            cli: TerminalCli::from_env(),
        }
    }
}

#[async_trait]
impl AgentTool for TerminalOutputTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "terminal_output".into(),
            description: "Read bounded output from a terminal session. Use wait_ms to wait for a prompt or login code. Output is sanitized (ANSI escapes and control characters removed) and capped. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["session_id"],
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session id returned by terminal_create."
                    },
                    "offset": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Byte offset into the session output. Defaults to the tail."
                    },
                    "max_bytes": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_OUTPUT_BYTES,
                        "description": "Maximum bytes to return, default 16384."
                    },
                    "wait_ms": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": MAX_WAIT_MS,
                        "description": "Wait before reading, default 0."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(40)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let session_id = required_session_id(&arguments)?;
        let wait_ms = optional_u64(&arguments, "wait_ms")?.unwrap_or(0);
        if wait_ms > MAX_WAIT_MS {
            return Err(ToolError::InvalidArguments(format!(
                "wait_ms must be at most {MAX_WAIT_MS}"
            )));
        }
        if wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        }
        let max_bytes = optional_u64(&arguments, "max_bytes")?
            .unwrap_or(DEFAULT_OUTPUT_BYTES as u64)
            .clamp(1, MAX_OUTPUT_BYTES as u64) as usize;

        let status_args = vec!["status".to_string(), session_id.clone()];
        let status_output = self.cli.run(&status_args, Duration::from_secs(15)).await?;
        let status = parse_response(&status_output, &self.cli)?;
        let output_bytes = status
            .get("output_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let offset = match optional_u64(&arguments, "offset")? {
            Some(offset) if offset <= output_bytes => offset,
            Some(_) => {
                return Err(ToolError::InvalidArguments(
                    "offset is beyond the current session output".into(),
                ));
            }
            None => output_bytes.saturating_sub(max_bytes as u64),
        };

        let args = vec![
            "output".to_string(),
            session_id.clone(),
            "--offset".to_string(),
            offset.to_string(),
        ];
        let output = self.cli.run(&args, Duration::from_secs(20)).await?;
        ensure_success(&output, &self.cli)?;

        let raw = String::from_utf8_lossy(&output.stdout);
        let sanitized = sanitize_terminal_output(&raw);
        let truncated = sanitized.chars().count() > max_bytes;
        let text = bounded_text(&sanitized, max_bytes);
        let mut metadata = session_metadata(&session_id);
        metadata.insert("offset".into(), offset.to_string());
        metadata.insert("output_bytes".into(), output_bytes.to_string());
        metadata.insert("truncated".into(), truncated.to_string());

        let summary = if text.trim().is_empty() {
            format!("终端 {session_id} 暂无新输出")
        } else {
            format!(
                "终端输出：{} 个字符（offset {offset}）",
                text.chars().count()
            )
        };
        Ok(ToolOutcome {
            summary,
            content: text,
            metadata,
        })
    }
}

/// Run a non-interactive command in an existing terminal session.
pub struct TerminalExecTool {
    cli: TerminalCli,
}

impl TerminalExecTool {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: TerminalCli::new(bin),
        }
    }

    pub fn from_env() -> Self {
        Self {
            cli: TerminalCli::from_env(),
        }
    }
}

#[async_trait]
impl AgentTool for TerminalExecTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "terminal_exec".into(),
            description: "Run a non-interactive shell command in an existing terminal session and return its bounded output and exit code. Create a session with terminal_create first. Use terminal_write and terminal_output for interactive programs such as copilot or gh auth login. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["session_id", "command"],
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session id returned by terminal_create."
                    },
                    "command": {
                        "type": "string",
                        "maxLength": MAX_TEXT_CHARS,
                        "description": "Shell command to execute in the session."
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_EXEC_TIMEOUT_SECS,
                        "description": "Command timeout, default 60 seconds."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(MAX_EXEC_TIMEOUT_SECS + 15)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let session_id = required_session_id(&arguments)?;
        let command = arguments
            .get("command")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("command is required".into()))?;
        validate_terminal_text(command)?;
        let timeout_seconds =
            optional_u64(&arguments, "timeout_seconds")?.unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
        if timeout_seconds == 0 || timeout_seconds > MAX_EXEC_TIMEOUT_SECS {
            return Err(ToolError::InvalidArguments(format!(
                "timeout_seconds must be 1..={MAX_EXEC_TIMEOUT_SECS}"
            )));
        }

        let args = vec![
            "exec".to_string(),
            session_id.clone(),
            "--timeout".to_string(),
            timeout_seconds.to_string(),
            command.to_string(),
        ];
        let output = self
            .cli
            .run(&args, Duration::from_secs(timeout_seconds + 10))
            .await?;
        let value = parse_exec_response(&output, &self.cli)?;

        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let exit = value.get("exit").and_then(Value::as_i64);
        let duration_ms = value
            .get("duration_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let artifact_sha256 = value
            .get("artifact")
            .and_then(|artifact| artifact.get("sha256"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let artifact_path = value
            .get("artifact")
            .and_then(|artifact| artifact.get("path"))
            .and_then(Value::as_str)
            .or_else(|| value.get("output").and_then(Value::as_str))
            .ok_or_else(|| {
                ToolError::Failed("workshop exec response is missing the output path".into())
            })?;

        let (text, truncated) = read_artifact(Path::new(artifact_path), MAX_OUTPUT_BYTES).await?;
        let mut metadata = session_metadata(&session_id);
        metadata.insert("status".into(), status.into());
        if let Some(exit) = exit {
            metadata.insert("exit".into(), exit.to_string());
        }
        metadata.insert("duration_ms".into(), duration_ms.to_string());
        if !artifact_sha256.is_empty() {
            metadata.insert("artifact_sha256".into(), artifact_sha256.into());
        }
        metadata.insert("truncated".into(), truncated.to_string());

        let summary = match status {
            "ok" => format!("命令完成（exit {}）", exit.unwrap_or(0)),
            "timeout" => "命令超时".to_string(),
            other => format!("命令状态 {other}（exit {}）", exit.unwrap_or(-1)),
        };
        Ok(ToolOutcome {
            summary,
            content: text,
            metadata,
        })
    }
}

/// Destroy a terminal session.
pub struct TerminalDestroyTool {
    cli: TerminalCli,
}

impl TerminalDestroyTool {
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self {
            cli: TerminalCli::new(bin),
        }
    }

    pub fn from_env() -> Self {
        Self {
            cli: TerminalCli::from_env(),
        }
    }
}

#[async_trait]
impl AgentTool for TerminalDestroyTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "terminal_destroy".into(),
            description:
                "Destroy a terminal session and release its PTY. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["session_id"],
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session id returned by terminal_create."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let session_id = required_session_id(&arguments)?;
        let args = vec!["destroy".to_string(), session_id.clone()];
        let output = self.cli.run(&args, Duration::from_secs(15)).await?;
        ensure_success(&output, &self.cli)?;
        Ok(ToolOutcome {
            summary: format!("终端会话已销毁：{session_id}"),
            content: json!({"session_id": session_id, "destroyed": true}).to_string(),
            metadata: session_metadata(&session_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::domain::{ToolCall, ToolStatus};
    use crate::tools::ToolRegistry;

    fn write_permissions(root: &Path) -> PermissionSet {
        PermissionSet {
            read_roots: vec![std::fs::canonicalize(root).unwrap()],
            allow_network: false,
            allow_write: true,
        }
    }

    #[cfg(unix)]
    fn fake_cli(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join("fake-workshop-session");
        let script = r#"#!/bin/sh
set -eu
log="$0.log"
artifact="$0.artifact"
printf '%s\n' "$*" >> "$log"
case "$1" in
  create) printf '{"session_id":"term-1-2-3","cwd":"%s","shell":"/bin/sh","status":"detached","output_bytes":0}\n' "$2" ;;
  status) printf '{"session_id":"term-1-2-3","output_bytes":16}\n' ;;
  output) printf '\033[31mhello\033[0m\r\n' ;;
  exec) printf 'EXEC_OUT\n' > "$artifact"; printf '{"session_id":"term-1-2-3","status":"ok","exit":0,"duration_ms":5,"output":"%s","artifact":{"path":"%s","bytes":9,"sha256":"abc"}}\n' "$artifact" "$artifact" ;;
  write|destroy) printf '{"ok":true}\n' ;;
  *) echo "unexpected: $*" >&2; exit 2 ;;
esac
"#;
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[test]
    fn sanitizer_strips_ansi_carriage_returns_and_controls() {
        let raw = "\u{1b}]0;title\u{7}\u{1b}[31mred\u{1b}[0m\r\nline\u{8}\u{2}";
        assert_eq!(sanitize_terminal_output(raw), "red\nline");
    }

    #[test]
    fn session_ids_reject_option_like_values() {
        for value in ["", "--help", "../term-1", "term-1/2", "term-1 2"] {
            assert!(validate_session_id(value).is_err(), "{value:?}");
        }
        assert!(validate_session_id("term-1794224805342-6602-65").is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn create_returns_a_session_and_rejects_cwd_outside_roots() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_cli(dir.path());
        let tool = TerminalCreateTool::new(fake.clone());
        let permissions = write_permissions(dir.path());
        let cancellation = CancellationToken::new();

        let outcome = tool
            .execute(
                json!({"cwd": dir.path(), "name": "probe"}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        assert!(outcome.summary.contains("term-1-2-3"));
        assert!(outcome.content.contains("attach"));
        let log = std::fs::read_to_string(format!("{}.log", fake.display())).unwrap();
        assert!(log.contains("create"));
        assert!(log.contains("--name probe"));

        let outside = tempfile::tempdir().unwrap();
        let error = tool
            .execute(json!({"cwd": outside.path()}), &permissions, &cancellation)
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_and_output_round_trip_through_the_cli() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_cli(dir.path());
        let permissions = write_permissions(dir.path());
        let cancellation = CancellationToken::new();

        let write = TerminalWriteTool::new(fake.clone());
        let outcome = write
            .execute(
                json!({"session_id": "term-1-2-3", "text": "echo hi"}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        assert!(outcome.summary.contains('7'));
        let log = std::fs::read_to_string(format!("{}.log", fake.display())).unwrap();
        assert!(log.contains("write term-1-2-3 echo hi"));

        let output = TerminalOutputTool::new(fake);
        let outcome = output
            .execute(
                json!({"session_id": "term-1-2-3"}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        assert_eq!(outcome.content.trim(), "hello");
        assert!(!outcome.content.contains('\u{1b}'));
        assert_eq!(outcome.metadata.get("offset").unwrap(), "0");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn exec_returns_the_artifact_content_and_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let fake = fake_cli(dir.path());
        let permissions = write_permissions(dir.path());
        let cancellation = CancellationToken::new();
        let tool = TerminalExecTool::new(fake);
        let outcome = tool
            .execute(
                json!({"session_id": "term-1-2-3", "command": "echo EXEC_OUT"}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        assert_eq!(outcome.content.trim(), "EXEC_OUT");
        assert_eq!(outcome.metadata.get("exit").unwrap(), "0");
        assert_eq!(outcome.metadata.get("status").unwrap(), "ok");
    }

    #[tokio::test]
    async fn missing_workshop_backend_is_actionable() {
        let dir = tempfile::tempdir().unwrap();
        let tool = TerminalCreateTool::new(dir.path().join("missing-skynet-workshop-session"));
        let permissions = write_permissions(dir.path());
        let error = tool
            .execute(
                json!({"cwd": dir.path()}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        match error {
            ToolError::Failed(message) => {
                assert!(message.contains("Skynet Workshop"), "{message}");
                assert!(message.contains("Linux x86_64"), "{message}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn registry_denies_terminal_tools_without_write_permission() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = ToolRegistry::default();
        registry.register(Arc::new(TerminalExecTool::new(fake_cli(dir.path()))));
        let call = ToolCall {
            id: "call-1".into(),
            name: "terminal_exec".into(),
            arguments: json!({"session_id": "term-1-2-3", "command": "true"}),
        };
        let observation = registry
            .execute(&call, &PermissionSet::default(), &CancellationToken::new())
            .await;
        assert_eq!(observation.status, ToolStatus::PermissionDenied);
        assert_eq!(observation.error_code.as_deref(), Some("write_not_allowed"));
    }
}
