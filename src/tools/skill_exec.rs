//! Operator-allowlisted, read-only execution of external skill CLIs.
//!
//! A planner may need a requirement that lives outside the workspace, such as a
//! Lark document. The operator declares the CLI and the argument prefixes that
//! are read-only in `~/.1a/skill-exec.json`; this tool runs only those commands.
//! It never uses a shell, clears the environment except for a small allowlist,
//! and bounds time and output.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome, spawn_with_retry};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::safety::contains_disallowed_control;

const ENV_CONFIG: &str = "ONEAGENT_SKILL_EXEC";
const CONFIG_FILE: &str = "skill-exec.json";
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_COMMANDS: usize = 16;
const MAX_PREFIXES: usize = 32;
const MAX_ARGS: usize = 32;
const MAX_ARG_CHARS: usize = 4_096;
const MAX_STDOUT_CHARS: usize = 64_000;
const MAX_STDERR_CHARS: usize = 8_000;
const MAX_DESCRIPTION_CHARS: usize = 2_000;
const TOOL_TIMEOUT: Duration = Duration::from_secs(90);

const ENV_PASSTHROUGH: &[&str] = &[
    "HOME",
    "PATH",
    "LANG",
    "LC_ALL",
    "TZ",
    "TMPDIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "ALL_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillExecConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub commands: Vec<SkillExecCommand>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillExecCommand {
    pub name: String,
    pub bin: PathBuf,
    /// Argument prefixes that are read-only. The model may append flags after
    /// a prefix, but it cannot replace the prefix.
    #[serde(default)]
    pub read: Vec<Vec<String>>,
}

fn default_schema_version() -> u32 {
    1
}

pub struct SkillExecTool {
    commands: Vec<ResolvedCommand>,
}

struct ResolvedCommand {
    name: String,
    bin: PathBuf,
    read: Vec<Vec<String>>,
}

impl SkillExecTool {
    /// Load `~/.1a/skill-exec.json` (or `ONEAGENT_SKILL_EXEC`). A missing file
    /// means the capability is not configured.
    pub fn from_env() -> Result<Option<Self>, String> {
        let path = std::env::var_os(ENV_CONFIG)
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::runtime_home::default_home().join(CONFIG_FILE));
        Self::from_path(&path)
    }

    pub fn from_path(path: &Path) -> Result<Option<Self>, String> {
        if !path.is_file() {
            return Ok(None);
        }
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?;
        if metadata.len() > MAX_CONFIG_BYTES {
            return Err(format!("{} exceeds {MAX_CONFIG_BYTES} bytes", path.display()));
        }
        let bytes = std::fs::read(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let config: SkillExecConfig = serde_json::from_slice(&bytes)
            .map_err(|error| format!("invalid {}: {error}", path.display()))?;
        Self::new(config).map(Some)
    }

    pub fn new(config: SkillExecConfig) -> Result<Self, String> {
        if config.schema_version != 1 {
            return Err(format!(
                "unsupported skill-exec schema_version {}",
                config.schema_version
            ));
        }
        if config.commands.is_empty() {
            return Err("skill-exec must configure at least one command".into());
        }
        if config.commands.len() > MAX_COMMANDS {
            return Err(format!(
                "skill-exec allows at most {MAX_COMMANDS} commands"
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        let mut commands = Vec::with_capacity(config.commands.len());
        for command in config.commands {
            let name = command.name.trim().to_string();
            if name.is_empty()
                || name.chars().count() > 64
                || contains_disallowed_control(&name)
            {
                return Err("skill-exec command name is empty or unsafe".into());
            }
            if !names.insert(name.clone()) {
                return Err(format!("duplicate skill-exec command '{name}'"));
            }
            if command.read.is_empty() {
                return Err(format!("skill-exec command '{name}' has no read prefixes"));
            }
            if command.read.len() > MAX_PREFIXES {
                return Err(format!(
                    "skill-exec command '{name}' allows at most {MAX_PREFIXES} prefixes"
                ));
            }
            for prefix in &command.read {
                validate_argv(prefix).map_err(|error| error.to_string())?;
            }
            let bin = resolve_bin(&command.bin)?;
            commands.push(ResolvedCommand {
                name,
                bin,
                read: command.read,
            });
        }
        Ok(Self { commands })
    }

    fn describe(&self) -> String {
        let commands = self
            .commands
            .iter()
            .map(|command| {
                let prefixes = command
                    .read
                    .iter()
                    .map(|prefix| prefix.join(" "))
                    .collect::<Vec<_>>()
                    .join(" | ");
                format!("{} (allowed: {prefixes})", command.name)
            })
            .collect::<Vec<_>>()
            .join("; ");
        let description = format!(
            "Run a read-only command from an operator-approved skill CLI. No shell; \
             every argument is passed as argv. Configured: {commands}. Use the CLI's own \
             skill listing/read commands to learn usage, then run a read-only fetch. Write \
             and high-risk commands are rejected. Requires --allow-network."
        );
        bounded_chars(&description, MAX_DESCRIPTION_CHARS).0
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillExecArgs {
    skill: String,
    args: Vec<String>,
}

#[async_trait]
impl AgentTool for SkillExecTool {
    fn spec(&self) -> ToolSpec {
        let names = self
            .commands
            .iter()
            .map(|command| command.name.clone())
            .collect::<Vec<_>>();
        ToolSpec {
            name: "skill_exec".into(),
            description: self.describe(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "skill": {"type": "string", "enum": names},
                    "args": {
                        "type": "array",
                        "items": {"type": "string"},
                        "minItems": 1,
                        "maxItems": MAX_ARGS
                    }
                },
                "required": ["skill", "args"]
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn executes(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        TOOL_TIMEOUT
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: SkillExecArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        if !permissions.allow_network {
            return Err(ToolError::PermissionDenied(
                "skill_exec needs network permission; rerun `1agent master plan` with --allow-network"
                    .into(),
            ));
        }
        let command = self
            .commands
            .iter()
            .find(|command| command.name == args.skill)
            .ok_or_else(|| {
                let names = self
                    .commands
                    .iter()
                    .map(|command| command.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                ToolError::InvalidArguments(format!(
                    "unknown skill '{}'; configured: {names}",
                    args.skill
                ))
            })?;
        validate_argv(&args.args)?;
        let allowed = command
            .read
            .iter()
            .any(|prefix| args.args.starts_with(prefix));
        if !allowed {
            let prefixes = command
                .read
                .iter()
                .map(|prefix| prefix.join(" "))
                .collect::<Vec<_>>()
                .join(" | ");
            return Err(ToolError::InvalidArguments(format!(
                "skill '{}' does not allow '{}'; read prefixes: {prefixes}",
                command.name,
                args.args.join(" ")
            )));
        }

        let mut process = tokio::process::Command::new(&command.bin);
        process
            .args(&args.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env_clear();
        for key in ENV_PASSTHROUGH {
            if let Some(value) = std::env::var_os(key) {
                process.env(key, value);
            }
        }
        let output = async {
            let child = spawn_with_retry(&mut process).await?;
            child.wait_with_output().await
        };
        let output = match tokio::time::timeout(TOOL_TIMEOUT, output).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ToolError::Failed(format!(
                    "skill CLI not found: {}",
                    command.bin.display()
                )));
            }
            Ok(Err(error)) => {
                return Err(ToolError::Failed(format!(
                    "cannot run {}: {error}",
                    command.bin.display()
                )));
            }
            Err(_) => return Err(ToolError::TimedOut),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let (stdout, stdout_truncated) = bounded_chars(&stdout, MAX_STDOUT_CHARS);
        let (stderr, stderr_truncated) = bounded_chars(&stderr, MAX_STDERR_CHARS);
        if !output.status.success() {
            let mut message = format!(
                "{} {} exited {}",
                command.name,
                args.args.join(" "),
                output.status
            );
            if !stderr.is_empty() {
                message.push_str("\nstderr: ");
                message.push_str(&stderr);
            }
            if !stdout.is_empty() {
                message.push_str("\nstdout: ");
                message.push_str(&stdout);
            }
            return Err(ToolError::Failed(message));
        }

        let content = serde_json::to_string_pretty(&json!({
            "skill": command.name,
            "args": args.args,
            "exit": output.status.code(),
            "stdout": stdout,
            "stderr": stderr,
            "stdout_truncated": stdout_truncated,
            "stderr_truncated": stderr_truncated,
        }))
        .map_err(|error| ToolError::Failed(format!("cannot serialize skill output: {error}")))?;
        Ok(ToolOutcome {
            summary: format!("{} exited 0", command.name),
            content,
            metadata: BTreeMap::from([
                ("skill".into(), command.name.clone()),
                ("exit".into(), "0".into()),
            ]),
        })
    }
}

fn validate_argv(argv: &[String]) -> Result<(), ToolError> {
    if argv.is_empty() || argv.len() > MAX_ARGS {
        return Err(ToolError::InvalidArguments(format!(
            "args must contain 1..={MAX_ARGS} entries"
        )));
    }
    for arg in argv {
        if arg.is_empty() || arg.chars().count() > MAX_ARG_CHARS {
            return Err(ToolError::InvalidArguments(
                "each arg must be non-empty and at most 4096 chars".into(),
            ));
        }
        if contains_disallowed_control(arg) {
            return Err(ToolError::UnsafeContent(
                "args contain disallowed control characters".into(),
            ));
        }
    }
    Ok(())
}

fn resolve_bin(bin: &Path) -> Result<PathBuf, String> {
    let candidate = if bin.is_absolute() {
        bin.to_path_buf()
    } else {
        let path = std::env::var_os("PATH")
            .ok_or_else(|| format!("cannot resolve {}: PATH is not set", bin.display()))?;
        std::env::split_paths(&path)
            .map(|directory| directory.join(bin))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| format!("{} was not found in PATH", bin.display()))?
    };
    if !candidate.is_file() {
        return Err(format!("{} is not a file", candidate.display()));
    }
    // Keep symlinks intact so a CLI self-update is picked up on the next run.
    if candidate.is_absolute() {
        return Ok(candidate);
    }
    std::env::current_dir()
        .map(|directory| directory.join(&candidate))
        .map_err(|error| format!("cannot resolve {}: {error}", candidate.display()))
}

fn bounded_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut bounded: String = value.chars().take(max_chars).collect();
    let truncated = value.chars().nth(max_chars).is_some();
    if truncated {
        bounded.push('…');
    }
    (bounded, truncated)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("fake-skill");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn tool(bin: PathBuf, prefixes: Vec<Vec<&str>>) -> SkillExecTool {
        SkillExecTool::new(SkillExecConfig {
            schema_version: 1,
            commands: vec![SkillExecCommand {
                name: "fake".into(),
                bin,
                read: prefixes
                    .into_iter()
                    .map(|prefix| prefix.into_iter().map(str::to_string).collect())
                    .collect(),
            }],
        })
        .unwrap()
    }

    fn permissions(allow_network: bool) -> PermissionSet {
        PermissionSet {
            read_roots: Vec::new(),
            allow_network,
            allow_write: false,
        }
    }

    async fn run(
        tool: &SkillExecTool,
        arguments: Value,
        allow_network: bool,
    ) -> Result<ToolOutcome, ToolError> {
        tool.execute(arguments, &permissions(allow_network), &CancellationToken::new())
            .await
    }

    #[tokio::test]
    async fn runs_an_allowed_prefix_without_a_shell() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(dir.path(), "printf '%s\\n' \"$@\"");
        let tool = tool(bin, vec![vec!["print"]]);
        let outcome = run(
            &tool,
            json!({"skill": "fake", "args": ["print", "a;b", "$(touch should-not-exist)"]}),
            true,
        )
        .await
        .unwrap();
        assert!(outcome.content.contains("a;b"));
        assert!(outcome.content.contains("$(touch should-not-exist)"));
        assert!(!dir.path().join("should-not-exist").exists());
    }

    #[tokio::test]
    async fn rejects_an_unlisted_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(dir.path(), "printf '%s\\n' \"$@\"");
        let tool = tool(bin, vec![vec!["print"]]);
        let error = run(
            &tool,
            json!({"skill": "fake", "args": ["write", "x"]}),
            true,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, ToolError::InvalidArguments(_)));
    }

    #[tokio::test]
    async fn requires_network_permission() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(dir.path(), "printf '%s\\n' \"$@\"");
        let tool = tool(bin, vec![vec!["print"]]);
        let error = run(&tool, json!({"skill": "fake", "args": ["print"]}), false)
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn rejects_unknown_skills_and_control_characters() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(dir.path(), "printf '%s\\n' \"$@\"");
        let tool = tool(bin, vec![vec!["print"]]);
        let unknown = run(&tool, json!({"skill": "other", "args": ["print"]}), true)
            .await
            .unwrap_err();
        assert!(matches!(unknown, ToolError::InvalidArguments(_)));

        let control = run(
            &tool,
            json!({"skill": "fake", "args": ["print", "a\u{0000}b"]}),
            true,
        )
        .await
        .unwrap_err();
        assert!(matches!(control, ToolError::UnsafeContent(_)));
    }

    #[tokio::test]
    async fn truncates_large_output() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(dir.path(), "yes a | head -c 200000");
        let tool = tool(bin, vec![vec!["print"]]);
        let outcome = run(&tool, json!({"skill": "fake", "args": ["print"]}), true)
            .await
            .unwrap();
        assert!(outcome.content.contains("\"stdout_truncated\": true"));
    }

    #[test]
    fn missing_policy_is_none_and_malformed_policy_fails() {
        let dir = tempfile::tempdir().unwrap();
        assert!(SkillExecTool::from_path(&dir.path().join("missing.json"))
            .unwrap()
            .is_none());

        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(SkillExecTool::from_path(&path).is_err());
    }
}
