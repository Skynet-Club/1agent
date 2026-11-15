//! Bounded child-agent spawning.
//!
//! The child is another `1agent run` process with its own journal, workspace
//! and budget. Depth is capped so an agent cannot spawn an unbounded tree.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::subagent_registry::{
    RegistryGuard, SUBAGENT_ROOT_PID_ENV, SubagentRecord, SubagentStatus,
};

const MAX_TASK_CHARS: usize = 8 * 1024;
const MAX_DEPTH: u8 = 2;
const MAX_FANOUT: usize = 8;
const MAX_CHILD_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
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
    spawned: AtomicUsize,
}

impl SpawnAgentTool {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            depth: depth_from_env(),
            spawned: AtomicUsize::new(0),
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
            description: "Spawn a bounded child 1Agent run for a subtask. The child has its own journal, workspace and budget; results are returned as JSON. Depth is capped to prevent runaway trees. Child write/network permissions are intersected with the parent's permissions, so allow_write/allow_network can only restrict; AGENT_* provider environment is inherited. The spawn itself is classified as write-capable because the child may write, so read-only mode, the kill switch and write-level Authority gates apply before any child starts.".into(),
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
                    },
                    "allow_write": {
                        "type": "boolean",
                        "description": "Intersected with the parent's write permission; false forces a read-only child."
                    },
                    "allow_network": {
                        "type": "boolean",
                        "description": "Intersected with the parent's network permission; false denies child network access."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
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
        let previous = self.spawned.fetch_add(1, Ordering::SeqCst);
        if previous >= MAX_FANOUT {
            return Err(ToolError::Failed(format!(
                "subagent fanout limit ({MAX_FANOUT}) reached"
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
        let child_write = permissions.allow_write
            && arguments
                .get("allow_write")
                .and_then(Value::as_bool)
                .unwrap_or(true);
        let child_network = permissions.allow_network
            && arguments
                .get("allow_network")
                .and_then(Value::as_bool)
                .unwrap_or(true);
        let root_pid = std::env::var(SUBAGENT_ROOT_PID_ENV)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_else(std::process::id);
        let mut registry = RegistryGuard::start(SubagentRecord::new(
            format!("sub-{}", Uuid::new_v4()),
            root_pid,
            std::process::id(),
            self.depth + 1,
            task,
            workspace.display().to_string(),
            journal_dir.display().to_string(),
            child_write,
            child_network,
            max_steps,
            timeout_seconds,
        ));

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
        if !child_write {
            args.push("--read-only".into());
        }
        if child_network {
            args.push("--allow-network".into());
        }
        match provider_arguments() {
            Ok(arguments) => args.extend(arguments),
            Err(error) => {
                registry.finish(SubagentStatus::Failed, Some(error.to_string()));
                return Err(error);
            }
        }

        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(&args)
            .current_dir(&workspace)
            .env(DEPTH_ENV, (self.depth + 1).to_string())
            .env(SUBAGENT_ROOT_PID_ENV, root_pid.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let child = match super::spawn_with_retry(&mut command).await {
            Ok(child) => child,
            Err(error) => {
                registry.finish(
                    SubagentStatus::Failed,
                    Some(format!("cannot start subagent: {error}")),
                );
                return Err(ToolError::Failed(format!(
                    "cannot start subagent {}: {error}",
                    self.program.display()
                )));
            }
        };
        if let Some(child_pid) = child.id() {
            registry.running(child_pid);
        }
        let output = match tokio::time::timeout(
            Duration::from_secs(timeout_seconds + 20),
            child.wait_with_output(),
        )
        .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                registry.finish(
                    SubagentStatus::Failed,
                    Some(format!("cannot wait for subagent: {error}")),
                );
                return Err(ToolError::Failed(format!(
                    "cannot start subagent {}: {error}",
                    self.program.display()
                )));
            }
            Err(_) => {
                registry.finish(SubagentStatus::TimedOut, Some("subagent timed out".into()));
                return Err(ToolError::TimedOut);
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            registry.finish(
                SubagentStatus::Failed,
                Some(format!("subagent exited {}", output.status)),
            );
            return Err(ToolError::Failed(format!(
                "subagent exited {}: {}",
                output.status,
                bounded(stderr.trim(), 2_000)
            )));
        }
        let value: Value = match serde_json::from_slice(&output.stdout) {
            Ok(value) => value,
            Err(error) => {
                registry.finish(
                    SubagentStatus::Failed,
                    Some(format!("invalid subagent JSON: {error}")),
                );
                return Err(ToolError::Failed(format!("invalid subagent JSON: {error}")));
            }
        };
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
        if stop_reason != "completed" {
            registry.finish(
                SubagentStatus::Failed,
                Some(format!("subagent stopped with {stop_reason}")),
            );
            return Err(ToolError::Failed(format!(
                "subagent stopped with {stop_reason}: {}",
                bounded(conclusion.lines().next().unwrap_or("no conclusion"), 500)
            )));
        }
        if !child_journal.is_empty() {
            let path = std::path::Path::new(&child_journal);
            if path.is_file() {
                let metadata = std::fs::metadata(path)
                    .map_err(|error| ToolError::Failed(format!("subagent journal: {error}")))?;
                if metadata.len() > MAX_CHILD_JOURNAL_BYTES {
                    return Err(ToolError::Failed(format!(
                        "subagent journal exceeds {MAX_CHILD_JOURNAL_BYTES} bytes"
                    )));
                }
                let events = crate::journal::read_events(path)
                    .map_err(|error| ToolError::Failed(format!("subagent journal: {error}")))?;
                let audit = crate::journal::audit_events(&events);
                if !audit.ok {
                    registry.finish(
                        SubagentStatus::Failed,
                        Some("subagent journal audit failed".into()),
                    );
                    return Err(ToolError::Failed(format!(
                        "subagent journal audit failed: {}",
                        audit.issues.join("; ")
                    )));
                }
            }
        }
        let content = json!({
            "task": task,
            "stop_reason": stop_reason,
            "conclusion": conclusion,
            "workspace": workspace.display().to_string(),
            "journal_path": child_journal,
            "depth": self.depth + 1,
            "allow_write": child_write,
            "allow_network": child_network
        });
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("stop_reason".into(), stop_reason);
        metadata.insert("workspace".into(), workspace.display().to_string());
        if !child_journal.is_empty() {
            metadata.insert("journal_path".into(), child_journal);
        }
        registry.finish(
            SubagentStatus::Completed,
            Some(bounded(conclusion.lines().next().unwrap_or("无结论"), 160)),
        );
        Ok(ToolOutcome {
            summary: format!(
                "队员完成：{}",
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

    fn fake_program_with_stop(dir: &Path, stop_reason: &str) -> PathBuf {
        let path = dir.join("fake-1agent");
        let script = r#"#!/bin/sh
printf '%s\n' "$@" > "$0.args"
printf '{"run_id":"child","journal_path":"","answer":{"schema_version":2,"status":"completed","conclusion":"child done","claims":[],"evidence":[],"assumptions":[],"constraints":[],"unknowns":[],"conflicts":[],"flip_conditions":[],"action_receipts":[]},"stop_reason":"STOP_REASON"}\n'
"#
        .replace("STOP_REASON", stop_reason);
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn fake_program(dir: &Path) -> PathBuf {
        fake_program_with_stop(dir, "completed")
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
    async fn requested_permissions_only_restrict_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone()).with_depth(0);
        let mut parent = permissions(dir.path(), true);
        parent.allow_network = false;
        let outcome = tool
            .execute(
                json!({
                    "task": "restricted child",
                    "workspace": workspace,
                    "allow_write": false,
                    "allow_network": true
                }),
                &parent,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("\"allow_write\":false"));
        assert!(outcome.content.contains("\"allow_network\":false"));
        let args = std::fs::read_to_string(format!("{}.args", program.display())).unwrap();
        assert!(args.contains("--read-only"));
        assert!(!args.contains("--allow-network"));

        // A child cannot widen permissions the parent does not have.
        let read_only_parent = permissions(dir.path(), false);
        let widened = tool
            .execute(
                json!({
                    "task": "widen attempt",
                    "workspace": workspace,
                    "allow_write": true,
                    "allow_network": true
                }),
                &read_only_parent,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(widened.content.contains("\"allow_write\":false"));
        assert!(widened.content.contains("\"allow_network\":false"));
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

    #[tokio::test]
    async fn parent_rejects_a_child_that_did_not_complete() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_with_stop(dir.path(), "needs_authorization");
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program).with_depth(0);
        let error = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::Failed(ref message) if message.contains("needs_authorization")),
            "unexpected error: {error:?}"
        );
    }

    #[tokio::test]
    async fn fanout_limit_is_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program).with_depth(0);
        let permissions = permissions(dir.path(), false);
        for _ in 0..MAX_FANOUT {
            tool.execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        }
        let error = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions,
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::Failed(ref message) if message.contains("fanout")),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn spawn_is_write_capable_so_read_only_gates_cannot_bypass_it() {
        let tool = SpawnAgentTool::new("1agent");
        assert!(tool.requires_write());
    }
}
