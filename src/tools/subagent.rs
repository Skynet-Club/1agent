//! Bounded child-agent spawning.
//!
//! The child is another `1agent run` process with its own journal, workspace
//! and budget. Depth is capped so an agent cannot spawn an unbounded tree.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const MAX_TASK_CHARS: usize = 8 * 1024;
const MAX_DEPTH: u8 = 2;
const DEPTH_ENV: &str = "ONEAGENT_SUBAGENT_DEPTH";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_TIMEOUT_SECS: u64 = 300;
const DEFAULT_MAX_STEPS: usize = 8;
const MAX_MAX_STEPS: usize = 20;

fn depth_from_env() -> u8 {
    std::env::var(DEPTH_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn cache_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".cache/1agent-tmp/subagents")
}

fn validate_task(value: &str) -> Result<(), ToolError> {
    if value.trim().is_empty() {
        return Err(ToolError::InvalidArguments("task must not be empty".into()));
    }
    if value.chars().count() > MAX_TASK_CHARS {
        return Err(ToolError::InvalidArguments(format!(
            "task must be at most {MAX_TASK_CHARS} characters"
        )));
    }
    if crate::safety::contains_disallowed_control(value) {
        return Err(ToolError::InvalidArguments(
            "task contains hidden or disallowed control characters".into(),
        ));
    }
    Ok(())
}

async fn resolve_workspace(
    arguments: &Value,
    permissions: &PermissionSet,
) -> Result<PathBuf, ToolError> {
    if let Some(path) = arguments.get("workspace").and_then(Value::as_str) {
        let canonical = tokio::fs::canonicalize(path)
            .await
            .map_err(|error| ToolError::InvalidArguments(format!("workspace: {error}")))?;
        let metadata = tokio::fs::metadata(&canonical)
            .await
            .map_err(|error| ToolError::InvalidArguments(format!("workspace: {error}")))?;
        if !metadata.is_dir() {
            return Err(ToolError::InvalidArguments(
                "workspace must be a directory".into(),
            ));
        }
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
                "workspace is outside the allowed read roots".into(),
            ));
        }
        return Ok(canonical);
    }
    let workspace = cache_root().join(Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&workspace)
        .await
        .map_err(|error| ToolError::Failed(format!("cannot create subagent workspace: {error}")))?;
    Ok(workspace)
}

fn provider_arguments() -> Result<Vec<String>, ToolError> {
    let base_url = std::env::var("AGENT_BASE_URL").ok();
    let model = std::env::var("AGENT_MODEL").ok();
    match (base_url, model) {
        (Some(base_url), Some(model)) if !base_url.is_empty() && !model.is_empty() => Ok(vec![
            "--provider".into(),
            "openai-compatible".into(),
            "--base-url".into(),
            base_url,
            "--model".into(),
            model,
        ]),
        (None, None) => Ok(Vec::new()),
        _ => Err(ToolError::Failed(
            "subagent needs both AGENT_BASE_URL and AGENT_MODEL, or neither".into(),
        )),
    }
}

pub struct SpawnAgentTool {
    program: PathBuf,
    depth: u8,
}

impl SpawnAgentTool {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            depth: depth_from_env(),
        }
    }

    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }
}

#[async_trait]
impl AgentTool for SpawnAgentTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "spawn_agent".into(),
            description: "Spawn a bounded child 1Agent run for a subtask. The child has its own journal, workspace and budget; results are returned as JSON. Depth is capped to prevent runaway trees. The child inherits the parent's write/network permissions and AGENT_* provider environment.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["task"],
                "properties": {
                    "task": {
                        "type": "string",
                        "maxLength": MAX_TASK_CHARS,
                        "description": "Self-contained subtask for the child agent."
                    },
                    "workspace": {
                        "type": "string",
                        "description": "Existing directory inside an allowed root. Omit to use an isolated cache workspace."
                    },
                    "max_steps": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_MAX_STEPS,
                        "description": "Child reasoning/tool steps, default 8."
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "minimum": 10,
                        "maximum": MAX_TIMEOUT_SECS,
                        "description": "Child timeout, default 120 seconds."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(MAX_TIMEOUT_SECS + 30)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if self.depth >= MAX_DEPTH {
            return Err(ToolError::Failed(format!(
                "subagent depth limit ({MAX_DEPTH}) reached"
            )));
        }
        let task = arguments
            .get("task")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("task is required".into()))?;
        validate_task(task)?;
        let max_steps = arguments
            .get("max_steps")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_STEPS as u64)
            .clamp(1, MAX_MAX_STEPS as u64) as usize;
        let timeout_seconds = arguments
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .clamp(10, MAX_TIMEOUT_SECS);
        let workspace = resolve_workspace(&arguments, permissions).await?;
        let journal_dir = workspace.join(".1agent-subagent-journal");

        let mut args = vec![
            "run".to_string(),
            "--task".to_string(),
            task.to_string(),
            "--json".to_string(),
            "--journal-dir".to_string(),
            journal_dir.display().to_string(),
            "--read-root".to_string(),
            workspace.display().to_string(),
            "--max-steps".to_string(),
            max_steps.to_string(),
            "--max-tool-calls".to_string(),
            (max_steps * 2).to_string(),
            "--max-provider-attempts".to_string(),
            "8".to_string(),
            "--timeout-seconds".to_string(),
            timeout_seconds.to_string(),
        ];
        if !permissions.allow_write {
            args.push("--read-only".into());
        }
        if permissions.allow_network {
            args.push("--allow-network".into());
        }
        args.extend(provider_arguments()?);

        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(&args)
            .current_dir(&workspace)
            .env(DEPTH_ENV, (self.depth + 1).to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let run = async {
            let child = super::spawn_with_retry(&mut command).await?;
            child.wait_with_output().await
        };
        let output =
            match tokio::time::timeout(Duration::from_secs(timeout_seconds + 20), run).await {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => {
                    return Err(ToolError::Failed(format!(
                        "cannot start subagent {}: {error}",
                        self.program.display()
                    )));
                }
                Err(_) => return Err(ToolError::TimedOut),
            };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Failed(format!(
                "subagent exited {}: {}",
                output.status,
                bounded(stderr.trim(), 2_000)
            )));
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| ToolError::Failed(format!("invalid subagent JSON: {error}")))?;
        let conclusion = value
            .get("answer")
            .and_then(|answer| answer.get("conclusion"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let stop_reason = value
            .get("stop_reason")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let child_journal = value
            .get("journal_path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let content = json!({
            "task": task,
            "stop_reason": stop_reason,
            "conclusion": conclusion,
            "workspace": workspace.display().to_string(),
            "journal_path": child_journal,
            "depth": self.depth + 1
        });
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("stop_reason".into(), stop_reason);
        metadata.insert("workspace".into(), workspace.display().to_string());
        if !child_journal.is_empty() {
            metadata.insert("journal_path".into(), child_journal);
        }
        Ok(ToolOutcome {
            summary: format!(
                "子 Agent 完成：{}",
                bounded(conclusion.lines().next().unwrap_or("无结论"), 160)
            ),
            content: content.to_string(),
            metadata,
        })
    }
}

fn bounded(value: &str, max_chars: usize) -> String {
    let mut bounded: String = value.chars().take(max_chars).collect();
    if value.chars().nth(max_chars).is_some() {
        bounded.push('…');
    }
    bounded
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    fn fake_program(dir: &Path) -> PathBuf {
        let path = dir.join("fake-1agent");
        let script = r#"#!/bin/sh
printf '%s\n' "$@" > "$0.args"
printf '{"run_id":"child","journal_path":"/tmp/child.jsonl","answer":{"schema_version":2,"status":"completed","conclusion":"child done","claims":[],"evidence":[],"assumptions":[],"constraints":[],"unknowns":[],"conflicts":[],"flip_conditions":[],"action_receipts":[]},"stop_reason":"completed"}\n'
"#;
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn permissions(root: &Path, allow_write: bool) -> PermissionSet {
        PermissionSet {
            read_roots: vec![std::fs::canonicalize(root).unwrap()],
            allow_network: false,
            allow_write,
        }
    }

    #[tokio::test]
    async fn spawns_a_read_only_child_and_returns_its_conclusion() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone()).with_depth(0);
        let outcome = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("child done"));
        let args = std::fs::read_to_string(format!("{}.args", program.display())).unwrap();
        assert!(args.contains("--read-only"));
        assert!(args.contains("--task"));
        assert!(args.contains("child task"));
        assert!(args.contains("--read-root"));
    }

    #[tokio::test]
    async fn rejects_workspace_outside_roots_and_depth_limit() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let tool = SpawnAgentTool::new(fake_program(root.path()));
        let error = tool
            .execute(
                json!({"task": "x", "workspace": outside.path()}),
                &permissions(root.path(), true),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));

        let limited = SpawnAgentTool::new(fake_program(root.path())).with_depth(MAX_DEPTH);
        let error = limited
            .execute(
                json!({"task": "x"}),
                &permissions(root.path(), true),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::Failed(_)));
    }
}
