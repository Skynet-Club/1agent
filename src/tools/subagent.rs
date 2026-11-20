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

use std::collections::{BTreeMap, BTreeSet};

use super::spawn_ledger::{SpawnLedger, fingerprint as spawn_fingerprint};
#[cfg(test)]
use super::subagent_budget::TreeLimits;
use super::subagent_budget::{TREE_FILE_ENV, TreeBudget};
use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::subagent_registry::{
    RegistryGuard, SUBAGENT_PARENT_RUN_ENV, SUBAGENT_PARENT_SESSION_ENV, SUBAGENT_ROOT_PID_ENV,
    SUBAGENT_SPAWN_CALL_ENV, SubagentRecord, SubagentStatus,
};
use crate::usage_ledger::{UsageTotals, summarize_usage};

const MAX_TASK_CHARS: usize = 8 * 1024;
const MAX_DEPTH: u8 = 2;
const MAX_FANOUT: usize = 8;
const MAX_CHILD_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CHILD_ANSWER_BYTES: usize = 8 * 1024;
const DEPTH_ENV: &str = "ONEAGENT_SUBAGENT_DEPTH";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_TIMEOUT_SECS: u64 = 300;
const DEFAULT_MAX_STEPS: usize = 8;
const MAX_MAX_STEPS: usize = 20;
const MAX_ALLOWED_TOOLS: usize = 32;
const MAX_TOOL_NAME_CHARS: usize = 64;

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

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

fn parse_allowed_tools(value: Option<&Value>) -> Result<Option<BTreeSet<String>>, ToolError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let array = value.as_array().ok_or_else(|| {
        ToolError::InvalidArguments("allowed_tools must be an array of tool names".into())
    })?;
    if array.len() > MAX_ALLOWED_TOOLS {
        return Err(ToolError::InvalidArguments(format!(
            "allowed_tools must contain at most {MAX_ALLOWED_TOOLS} names"
        )));
    }
    let mut tools = BTreeSet::new();
    for item in array {
        let name = item.as_str().ok_or_else(|| {
            ToolError::InvalidArguments("allowed_tools entries must be strings".into())
        })?;
        let valid_start = name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if !valid_start
            || name.chars().count() > MAX_TOOL_NAME_CHARS
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        {
            return Err(ToolError::InvalidArguments(format!(
                "invalid tool name '{name}'"
            )));
        }
        tools.insert(name.to_string());
    }
    Ok(Some(tools))
}

fn child_allowed_tools(
    parent: Option<&BTreeSet<String>>,
    requested: Option<BTreeSet<String>>,
) -> Option<BTreeSet<String>> {
    match (parent, requested) {
        (Some(parent), Some(requested)) => Some(parent.intersection(&requested).cloned().collect()),
        (Some(parent), None) => Some(parent.clone()),
        (None, requested) => requested,
    }
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

#[cfg(unix)]
fn kill_spawn_tree(child_pid: Option<u32>, depth: u8) {
    let Some(pid) = child_pid else {
        return;
    };
    // A root spawn puts each direct child in its own process group, so a group
    // kill also stops depth-2 grandchildren. A depth-2 child is a leaf and is
    // killed directly by kill_on_drop.
    if depth == 0 {
        // SAFETY: a negative pid signals the child's process group, not ours.
        unsafe {
            libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn kill_spawn_tree(_child_pid: Option<u32>, _depth: u8) {}

#[cfg(all(unix, any(target_os = "linux", target_os = "android")))]
fn set_parent_death_signal(command: &mut tokio::process::Command) {
    let parent = std::process::id();
    // SAFETY: pre_exec runs in the forked child before exec. prctl/getppid are
    // async-signal-safe and the closure performs no allocation.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() as u32 != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

#[cfg(not(all(unix, any(target_os = "linux", target_os = "android"))))]
fn set_parent_death_signal(_command: &mut tokio::process::Command) {}

pub struct SpawnAgentTool {
    program: PathBuf,
    depth: u8,
    spawned: AtomicUsize,
    /// A completed child must leave a readable journal the parent can audit.
    require_journal: bool,
    /// Identifies this tool when no root run id is available.
    instance_id: Uuid,
    #[cfg(test)]
    tree_override: Option<(PathBuf, TreeLimits)>,
}

impl SpawnAgentTool {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            depth: depth_from_env(),
            spawned: AtomicUsize::new(0),
            require_journal: true,
            instance_id: Uuid::new_v4(),
            #[cfg(test)]
            tree_override: None,
        }
    }

    #[cfg(test)]
    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    #[cfg(test)]
    pub fn with_tree_budget(mut self, path: PathBuf, limits: TreeLimits) -> Self {
        self.tree_override = Some((path, limits));
        self
    }

    /// Legacy fake programs in tests report no journal. Production keeps the
    /// fail-closed default.
    #[cfg(test)]
    pub fn allow_missing_journal(mut self) -> Self {
        self.require_journal = false;
        self
    }

    fn tree_budget(&self, run_id: Option<&str>) -> Result<TreeBudget, String> {
        #[cfg(test)]
        if let Some((path, limits)) = &self.tree_override {
            return Ok(TreeBudget::at(path.clone(), *limits));
        }
        TreeBudget::resolve(self.instance_id, run_id, &cache_root())
    }
}

#[async_trait]
impl AgentTool for SpawnAgentTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "spawn_agent".into(),
            description: "Spawn a bounded child 1Agent run for a subtask. The child has its own journal, workspace and budget; results are returned as JSON. Depth is capped to prevent runaway trees. Child write/network permissions are intersected with the parent's permissions, so allow_write/allow_network can only restrict; AGENT_* provider environment is inherited. The spawn itself is classified as write- and execute-capable because the child may write or run terminal tools, so read-only mode, the kill switch and write/execute-level Authority gates apply before any child starts.".into(),
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
                    },
                    "allowed_tools": {
                        "type": "array",
                        "maxItems": MAX_ALLOWED_TOOLS,
                        "items": {"type": "string", "minLength": 1, "maxLength": MAX_TOOL_NAME_CHARS},
                        "description": "Exact child tool allowlist. Intersected with the parent's allowlist; an empty array denies all tools. Omit to inherit the parent's allowlist or allow the default set."
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

    fn executes(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(MAX_TIMEOUT_SECS + 30)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        cancellation: &CancellationToken,
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
        let requested_allowed_tools = parse_allowed_tools(arguments.get("allowed_tools"))?;
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
        let spawn_call_id = super::TOOL_CALL_ID.try_with(Clone::clone).ok();
        let run_context = super::RUN_CONTEXT.try_with(Clone::clone).ok();
        let allowed_tools = child_allowed_tools(
            run_context
                .as_ref()
                .and_then(|context| context.allowed_tools.as_ref()),
            requested_allowed_tools,
        );
        let ledger = match (run_context.as_ref(), spawn_call_id.as_deref()) {
            (Some(context), Some(call_id)) => Some(
                SpawnLedger::new(&cache_root(), &context.run_id, call_id)
                    .map_err(|error| ToolError::Failed(format!("spawn idempotency: {error}")))?,
            ),
            _ => None,
        };
        let fingerprint = ledger.as_ref().map(|_| {
            spawn_fingerprint(&json!({
                "task": task,
                // An omitted workspace is generated per attempt; key on the
                // argument so a replay does not create a second workspace.
                "workspace": arguments
                    .get("workspace")
                    .and_then(Value::as_str)
                    .unwrap_or("auto"),
                "max_steps": max_steps,
                "timeout_seconds": timeout_seconds,
                "allow_write": child_write,
                "allow_network": child_network,
                "allowed_tools": allowed_tools,
                "read_roots": permissions
                    .read_roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect::<Vec<_>>(),
            }))
        });
        if let (Some(ledger), Some(fingerprint)) = (&ledger, &fingerprint)
            && let Some(mut outcome) = ledger
                .replay(fingerprint)
                .map_err(|error| ToolError::Failed(format!("spawn idempotency: {error}")))?
        {
            outcome
                .metadata
                .insert("idempotent_replay".into(), "true".into());
            return Ok(outcome);
        }
        let previous = self.spawned.fetch_add(1, Ordering::SeqCst);
        if previous >= MAX_FANOUT {
            return Err(ToolError::Failed(format!(
                "subagent fanout limit ({MAX_FANOUT}) reached"
            )));
        }
        let workspace = resolve_workspace(&arguments, permissions).await?;
        let journal_dir = workspace.join(".1agent-subagent-journal");
        let root_pid = std::env::var(SUBAGENT_ROOT_PID_ENV)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or_else(std::process::id);
        let mut record = SubagentRecord::new(
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
        );
        record.spawn_call_id = spawn_call_id.clone();
        if let Some(context) = &run_context {
            record.parent_run_id = Some(context.run_id.clone());
            record.parent_session_id = context.session_id.clone();
        }
        record.goal_id = run_context
            .as_ref()
            .and_then(|context| context.goal_id.clone())
            .or_else(|| nonempty_env("ONEAGENT_GOAL_ID"));
        record.work_item_id = run_context
            .as_ref()
            .and_then(|context| context.work_item_id.clone())
            .or_else(|| nonempty_env("ONEAGENT_WORK_ITEM_ID"));
        record.delegation_id = match (&record.parent_run_id, &spawn_call_id) {
            (Some(run_id), Some(call_id)) => Some(format!("{run_id}:{call_id}")),
            _ => None,
        };
        let tree = self
            .tree_budget(run_context.as_ref().map(|context| context.run_id.as_str()))
            .map_err(|error| ToolError::Failed(format!("tree budget: {error}")))?;
        let reservation_id = spawn_call_id
            .clone()
            .unwrap_or_else(|| format!("spawn-{}", Uuid::new_v4()));
        let mut reservation = tree
            .reserve(reservation_id)
            .map_err(|error| ToolError::Failed(format!("tree budget: {error}")))?;
        let mut registry = RegistryGuard::start(record);

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
        match &allowed_tools {
            Some(tools) if tools.is_empty() => args.push("--deny-all-tools".into()),
            Some(tools) => {
                for tool in tools {
                    args.push("--allowed-tools".into());
                    args.push(tool.clone());
                }
            }
            None => {}
        }
        match provider_arguments() {
            Ok(arguments) => args.extend(arguments),
            Err(error) => {
                registry.finish(SubagentStatus::Failed, Some(error.to_string()));
                return Err(ToolError::FailedWithMetadata {
                    message: error.to_string(),
                    metadata: spawn_failure_metadata(
                        "failed",
                        run_context.as_ref(),
                        spawn_call_id.as_deref(),
                        &workspace,
                        &journal_dir,
                        None,
                    ),
                });
            }
        }

        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(&args)
            .current_dir(&workspace)
            .env(DEPTH_ENV, (self.depth + 1).to_string())
            .env(SUBAGENT_ROOT_PID_ENV, root_pid.to_string())
            .env(TREE_FILE_ENV, tree.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        if self.depth == 0 {
            command.process_group(0);
        }
        set_parent_death_signal(&mut command);
        if let Some(context) = &run_context {
            command.env(SUBAGENT_PARENT_RUN_ENV, &context.run_id);
            if let Some(session_id) = &context.session_id {
                command.env(SUBAGENT_PARENT_SESSION_ENV, session_id);
            }
        }
        if let Some(call_id) = &spawn_call_id {
            command.env(SUBAGENT_SPAWN_CALL_ENV, call_id);
        }
        let child = match super::spawn_with_retry(&mut command).await {
            Ok(child) => child,
            Err(error) => {
                registry.finish(
                    SubagentStatus::Failed,
                    Some(format!("cannot start subagent: {error}")),
                );
                return Err(ToolError::FailedWithMetadata {
                    message: format!("cannot start subagent {}: {error}", self.program.display()),
                    metadata: spawn_failure_metadata(
                        "failed",
                        run_context.as_ref(),
                        spawn_call_id.as_deref(),
                        &workspace,
                        &journal_dir,
                        None,
                    ),
                });
            }
        };
        let child_pid = child.id();
        if let Some(pid) = child_pid {
            registry.running(pid);
            reservation.set_child_pid(pid);
        }
        let waited = tokio::select! {
            _ = cancellation.cancelled() => {
                kill_spawn_tree(child_pid, self.depth);
                registry.finish(
                    SubagentStatus::Cancelled,
                    Some("parent cancelled the spawn".into()),
                );
                return Err(ToolError::CancelledWithMetadata(spawn_failure_metadata(
                    "cancelled",
                    run_context.as_ref(),
                    spawn_call_id.as_deref(),
                    &workspace,
                    &journal_dir,
                    None,
                )));
            }
            waited = tokio::time::timeout(
                Duration::from_secs(timeout_seconds + 20),
                child.wait_with_output(),
            ) => waited,
        };
        let output = match waited {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                kill_spawn_tree(child_pid, self.depth);
                registry.finish(
                    SubagentStatus::Failed,
                    Some(format!("cannot wait for subagent: {error}")),
                );
                return Err(ToolError::FailedWithMetadata {
                    message: format!("cannot start subagent {}: {error}", self.program.display()),
                    metadata: spawn_failure_metadata(
                        "failed",
                        run_context.as_ref(),
                        spawn_call_id.as_deref(),
                        &workspace,
                        &journal_dir,
                        None,
                    ),
                });
            }
            Err(_) => {
                kill_spawn_tree(child_pid, self.depth);
                registry.finish(SubagentStatus::TimedOut, Some("subagent timed out".into()));
                return Err(ToolError::TimedOutWithMetadata(spawn_failure_metadata(
                    "timed_out",
                    run_context.as_ref(),
                    spawn_call_id.as_deref(),
                    &workspace,
                    &journal_dir,
                    None,
                )));
            }
        };
        // The direct child has exited; stop any descendants it left behind.
        kill_spawn_tree(child_pid, self.depth);
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            registry.finish(
                SubagentStatus::Failed,
                Some(format!("subagent exited {}", output.status)),
            );
            let mut metadata = spawn_failure_metadata(
                "failed",
                run_context.as_ref(),
                spawn_call_id.as_deref(),
                &workspace,
                &journal_dir,
                None,
            );
            metadata.insert("exit_status".into(), output.status.to_string());
            return Err(ToolError::FailedWithMetadata {
                message: format!(
                    "subagent exited {}: {}",
                    output.status,
                    bounded(stderr.trim(), 2_000)
                ),
                metadata,
            });
        }
        let value: Value = match serde_json::from_slice(&output.stdout) {
            Ok(value) => value,
            Err(error) => {
                registry.finish(
                    SubagentStatus::Failed,
                    Some(format!("invalid subagent JSON: {error}")),
                );
                return Err(ToolError::FailedWithMetadata {
                    message: format!("invalid subagent JSON: {error}"),
                    metadata: spawn_failure_metadata(
                        "failed",
                        run_context.as_ref(),
                        spawn_call_id.as_deref(),
                        &workspace,
                        &journal_dir,
                        None,
                    ),
                });
            }
        };
        let child_run_id = value
            .get("run_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(child_run_id) = &child_run_id {
            registry.set_child_run_id(child_run_id);
        }
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
        // Read once: usage feeds the tree budget, and the same events feed the
        // completed-child audit below.
        let journal_events = if child_journal.is_empty() {
            None
        } else {
            let path = std::path::Path::new(&child_journal);
            path.is_file().then(|| read_child_journal(path))
        };
        let child_usage = journal_events
            .as_ref()
            .and_then(|events| events.as_ref().ok())
            .map(|events| summarize_usage(events).totals)
            .unwrap_or_default();
        reservation.finish(&child_usage);
        let answer_value = value.get("answer").cloned();
        let child_answer = bounded_child_answer(answer_value.as_ref(), &conclusion);
        let answer_truncated = child_answer
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let claims_count = answer_array_len(answer_value.as_ref(), "claims");
        let evidence_count = answer_array_len(answer_value.as_ref(), "evidence");
        let conflicts_count = answer_array_len(answer_value.as_ref(), "conflicts");
        if stop_reason != "completed" {
            registry.finish(
                SubagentStatus::Failed,
                Some(format!("subagent stopped with {stop_reason}")),
            );
            let mut metadata = spawn_failure_metadata(
                "failed",
                run_context.as_ref(),
                spawn_call_id.as_deref(),
                &workspace,
                &journal_dir,
                child_run_id.as_deref(),
            );
            metadata.insert("stop_reason".into(), stop_reason.clone());
            insert_usage_metadata(&mut metadata, &child_usage);
            return Err(ToolError::FailedWithMetadata {
                message: format!(
                    "subagent stopped with {stop_reason}: {}",
                    bounded(conclusion.lines().next().unwrap_or("no conclusion"), 500)
                ),
                metadata,
            });
        }
        if self.require_journal
            && (child_journal.is_empty() || !std::path::Path::new(&child_journal).is_file())
        {
            registry.finish(
                SubagentStatus::Failed,
                Some("subagent journal is missing".into()),
            );
            return Err(spawn_journal_error(
                "completed child must provide a readable journal",
                run_context.as_ref(),
                spawn_call_id.as_deref(),
                &workspace,
                &journal_dir,
                child_run_id.as_deref(),
            ));
        }
        if let Some(events) = &journal_events {
            let events = events.as_ref().map_err(|error| {
                spawn_journal_error(
                    error,
                    run_context.as_ref(),
                    spawn_call_id.as_deref(),
                    &workspace,
                    &journal_dir,
                    child_run_id.as_deref(),
                )
            })?;
            let audit = crate::journal::audit_events(events);
            if !audit.ok {
                registry.finish(
                    SubagentStatus::Failed,
                    Some("subagent journal audit failed".into()),
                );
                return Err(spawn_journal_error(
                    &format!("audit failed: {}", audit.issues.join("; ")),
                    run_context.as_ref(),
                    spawn_call_id.as_deref(),
                    &workspace,
                    &journal_dir,
                    child_run_id.as_deref(),
                ));
            }
        }
        let content = json!({
            "task": task,
            "stop_reason": stop_reason,
            "conclusion": conclusion,
            "answer": child_answer,
            "answer_truncated": answer_truncated,
            "usage": {
                "model_calls": child_usage.calls,
                "input_tokens": child_usage.input_tokens,
                "output_tokens": child_usage.output_tokens,
                "cost_micros": child_usage.cost_micros
            },
            "workspace": workspace.display().to_string(),
            "journal_path": child_journal,
            "depth": self.depth + 1,
            "allow_write": child_write,
            "allow_network": child_network
        });
        let mut metadata = spawn_failure_metadata(
            "completed",
            run_context.as_ref(),
            spawn_call_id.as_deref(),
            &workspace,
            &journal_dir,
            child_run_id.as_deref(),
        );
        metadata.insert("stop_reason".into(), stop_reason);
        metadata.insert(
            "child_status".into(),
            answer_value
                .as_ref()
                .and_then(|answer| answer.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
        );
        metadata.insert("claims_count".into(), claims_count.to_string());
        metadata.insert("evidence_count".into(), evidence_count.to_string());
        metadata.insert("conflicts_count".into(), conflicts_count.to_string());
        metadata.insert("answer_truncated".into(), answer_truncated.to_string());
        insert_usage_metadata(&mut metadata, &child_usage);
        if !child_journal.is_empty() {
            metadata.insert("journal_path".into(), child_journal);
        }
        registry.finish(
            SubagentStatus::Completed,
            Some(bounded(conclusion.lines().next().unwrap_or("无结论"), 160)),
        );
        let outcome = ToolOutcome {
            summary: format!(
                "队员完成：{}",
                bounded(conclusion.lines().next().unwrap_or("无结论"), 160)
            ),
            content: content.to_string(),
            metadata,
        };
        if let (Some(ledger), Some(fingerprint)) = (&ledger, &fingerprint) {
            ledger
                .record(fingerprint, &outcome)
                .map_err(|error| ToolError::Failed(format!("spawn idempotency: {error}")))?;
        }
        Ok(outcome)
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_failure_metadata(
    status: &str,
    run_context: Option<&super::RunContext>,
    spawn_call_id: Option<&str>,
    workspace: &std::path::Path,
    journal_dir: &std::path::Path,
    child_run_id: Option<&str>,
) -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();
    metadata.insert("status".into(), status.into());
    metadata.insert("workspace".into(), workspace.display().to_string());
    metadata.insert("journal_dir".into(), journal_dir.display().to_string());
    if let Some(context) = run_context {
        metadata.insert("parent_run_id".into(), context.run_id.clone());
        if let Some(session_id) = &context.session_id {
            metadata.insert("parent_session_id".into(), session_id.clone());
        }
    }
    if let Some(call_id) = spawn_call_id {
        metadata.insert("spawn_call_id".into(), call_id.into());
    }
    if let Some(child_run_id) = child_run_id {
        metadata.insert("child_run_id".into(), child_run_id.into());
    }
    metadata
}

#[allow(clippy::too_many_arguments)]
fn spawn_journal_error(
    message: &str,
    run_context: Option<&super::RunContext>,
    spawn_call_id: Option<&str>,
    workspace: &std::path::Path,
    journal_dir: &std::path::Path,
    child_run_id: Option<&str>,
) -> ToolError {
    ToolError::FailedWithMetadata {
        message: format!("subagent journal: {message}"),
        metadata: spawn_failure_metadata(
            "failed",
            run_context,
            spawn_call_id,
            workspace,
            journal_dir,
            child_run_id,
        ),
    }
}

fn read_child_journal(path: &std::path::Path) -> Result<Vec<crate::journal::JournalEvent>, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_CHILD_JOURNAL_BYTES {
        return Err(format!("exceeds {MAX_CHILD_JOURNAL_BYTES} bytes"));
    }
    crate::journal::read_events(path).map_err(|error| error.to_string())
}

fn insert_usage_metadata(metadata: &mut BTreeMap<String, String>, usage: &UsageTotals) {
    metadata.insert("usage_model_calls".into(), usage.calls.to_string());
    metadata.insert("usage_input_tokens".into(), usage.input_tokens.to_string());
    metadata.insert(
        "usage_output_tokens".into(),
        usage.output_tokens.to_string(),
    );
    metadata.insert("usage_cost_micros".into(), usage.cost_micros.to_string());
}

fn answer_array_len(answer: Option<&Value>, key: &str) -> usize {
    answer
        .and_then(|answer| answer.get(key))
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

/// Keep the child answer bounded in the tool result. The full answer remains
/// in the child journal; a truncated result keeps counts so the parent still
/// knows what was omitted.
fn bounded_child_answer(answer: Option<&Value>, conclusion: &str) -> Value {
    let Some(answer) = answer else {
        return json!({"status": "unknown", "conclusion": bounded(conclusion, 2_000)});
    };
    if answer.to_string().len() <= MAX_CHILD_ANSWER_BYTES {
        return answer.clone();
    }
    json!({
        "status": answer.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "conclusion": bounded(conclusion, 2_000),
        "truncated": true,
        "counts": {
            "claims": answer_array_len(Some(answer), "claims"),
            "evidence": answer_array_len(Some(answer), "evidence"),
            "conflicts": answer_array_len(Some(answer), "conflicts"),
            "unknowns": answer_array_len(Some(answer), "unknowns"),
        }
    })
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
    use crate::domain::{
        ANSWER_SCHEMA_VERSION, AgentAnswer, AnswerStatus, Claim, ClaimStatus, CognitiveMode,
        ModelResponse, ModelTurn, StopReason, TaskRequest, ToolStatus, Usage,
    };
    use crate::journal::{Journal, JournalEventKind};
    use crate::tools::ToolRegistry;

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

    fn fake_program_counted(dir: &Path) -> PathBuf {
        write_program(
            dir,
            "fake-counted",
            r#"#!/bin/sh
printf 'run\n' >> "$0.count"
printf '{"run_id":"child-idem","journal_path":"","answer":{"schema_version":2,"status":"completed","conclusion":"child done","claims":[],"evidence":[],"assumptions":[],"constraints":[],"unknowns":[],"conflicts":[],"flip_conditions":[],"action_receipts":[]},"stop_reason":"completed"}\n'
"#,
        )
    }

    fn fake_program_pgid(dir: &Path) -> PathBuf {
        write_program(
            dir,
            "fake-pgid",
            r#"#!/bin/sh
pgid=$(ps -o pgid= -p $$ | tr -d ' ')
printf '%s' "$pgid" > "$0.pgid"
printf '{"run_id":"child-pgid","journal_path":"","answer":{"schema_version":2,"status":"completed","conclusion":"child done","claims":[],"evidence":[],"assumptions":[],"constraints":[],"unknowns":[],"conflicts":[],"flip_conditions":[],"action_receipts":[]},"stop_reason":"completed"}\n'
"#,
        )
    }

    fn fake_program_spawning_grandchild(dir: &Path) -> PathBuf {
        write_program(
            dir,
            "fake-grandchild",
            r#"#!/bin/sh
sh -c 'sleep 60' &
printf '%s' "$!" > "$0.grandchild"
wait
"#,
        )
    }

    fn record_for_workspace(workspace: &Path) -> SubagentRecord {
        let expected = std::fs::canonicalize(workspace)
            .unwrap()
            .display()
            .to_string();
        let registry = std::fs::read_to_string(crate::subagent_registry::registry_path()).unwrap();
        registry
            .lines()
            .filter_map(|line| serde_json::from_str::<SubagentRecord>(line).ok())
            .rfind(|record| record.workspace == expected)
            .expect("spawned record")
    }

    fn write_program(dir: &Path, name: &str, script: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    fn fake_program_exiting(dir: &Path, code: i32) -> PathBuf {
        write_program(
            dir,
            "fake-exit",
            &format!("#!/bin/sh\nprintf 'out\\n'\nprintf 'err\\n' >&2\nexit {code}\n"),
        )
    }

    fn fake_program_sleeping(dir: &Path) -> PathBuf {
        write_program(dir, "fake-sleep", "#!/bin/sh\nsleep 60\n")
    }

    fn fake_program_dumping_env(dir: &Path) -> PathBuf {
        write_program(
            dir,
            "fake-env",
            r#"#!/bin/sh
printf '%s\n' "$ONEAGENT_PARENT_RUN_ID" "$ONEAGENT_PARENT_SESSION_ID" "$ONEAGENT_SPAWN_CALL_ID" "$ONEAGENT_SUBAGENT_TREE_FILE" > "$0.env"
printf '{"run_id":"child-env","journal_path":"","answer":{"schema_version":2,"status":"completed","conclusion":"env child done","claims":[],"evidence":[],"assumptions":[],"constraints":[],"unknowns":[],"conflicts":[],"flip_conditions":[],"action_receipts":[]},"stop_reason":"completed"}\n'
"#,
        )
    }

    fn permissions(root: &Path, allow_write: bool) -> PermissionSet {
        PermissionSet {
            read_roots: vec![std::fs::canonicalize(root).unwrap()],
            allow_network: false,
            allow_write,
        }
    }

    fn run_context(allowed_tools: Option<&[&str]>) -> crate::tools::RunContext {
        crate::tools::RunContext {
            run_id: format!("run-{}", Uuid::new_v4()),
            session_id: None,
            goal_id: None,
            work_item_id: None,
            allowed_tools: allowed_tools
                .map(|tools| tools.iter().map(|tool| tool.to_string()).collect()),
        }
    }

    async fn spawn_with_allowlist(
        dir: &Path,
        program: &Path,
        workspace: &Path,
        context: crate::tools::RunContext,
        allowed_tools: Value,
    ) -> Result<ToolOutcome, ToolError> {
        let tool = SpawnAgentTool::new(program.to_path_buf())
            .with_depth(0)
            .allow_missing_journal();
        crate::tools::RUN_CONTEXT
            .scope(context, async {
                tool.execute(
                    json!({
                        "task": "child task",
                        "workspace": workspace,
                        "allowed_tools": allowed_tools,
                    }),
                    &permissions(dir, true),
                    &CancellationToken::new(),
                )
                .await
            })
            .await
    }

    #[tokio::test]
    async fn spawns_a_read_only_child_and_returns_its_conclusion() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
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
    async fn child_tool_allowlist_is_intersected_with_the_parent() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let outcome = spawn_with_allowlist(
            dir.path(),
            &program,
            &workspace,
            run_context(Some(&["read_file", "echo"])),
            json!(["read_file", "write_file"]),
        )
        .await
        .unwrap();
        assert!(outcome.content.contains("child done"));
        let args = std::fs::read_to_string(format!("{}.args", program.display())).unwrap();
        let args = args.lines().collect::<Vec<_>>();
        assert!(args.contains(&"--allowed-tools"));
        assert!(args.contains(&"read_file"));
        assert!(!args.contains(&"write_file"));
        assert!(!args.contains(&"echo"));
        assert!(!args.contains(&"--deny-all-tools"));
    }

    #[tokio::test]
    async fn omitted_child_allowlist_inherits_the_parent_allowlist() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
        crate::tools::RUN_CONTEXT
            .scope(run_context(Some(&["read_file"])), async {
                tool.execute(
                    json!({"task": "child task", "workspace": workspace}),
                    &permissions(dir.path(), true),
                    &CancellationToken::new(),
                )
                .await
            })
            .await
            .unwrap();
        let args = std::fs::read_to_string(format!("{}.args", program.display())).unwrap();
        assert!(args.lines().any(|arg| arg == "read_file"));
        assert!(!args.contains("write_file"));
    }

    #[tokio::test]
    async fn empty_child_allowlist_denies_all_tools() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        spawn_with_allowlist(
            dir.path(),
            &program,
            &workspace,
            run_context(None),
            json!([]),
        )
        .await
        .unwrap();
        let args = std::fs::read_to_string(format!("{}.args", program.display())).unwrap();
        assert!(args.lines().any(|arg| arg == "--deny-all-tools"));
        assert!(!args.contains("--allowed-tools"));
    }

    #[tokio::test]
    async fn invalid_child_allowlist_is_rejected_before_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal();
        for invalid in [
            json!(["bad tool"]),
            json!([""]),
            json!(["ok", 3]),
            json!(["--allow-write"]),
        ] {
            let error = tool
                .execute(
                    json!({
                        "task": "child task",
                        "workspace": workspace.display().to_string(),
                        "allowed_tools": invalid,
                    }),
                    &permissions(dir.path(), true),
                    &CancellationToken::new(),
                )
                .await
                .unwrap_err();
            assert!(matches!(error, ToolError::InvalidArguments(_)));
        }
    }

    #[tokio::test]
    async fn registry_records_parent_session_run_and_spawn_call_lineage() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let mut registry = ToolRegistry::default();
        registry.register(std::sync::Arc::new(
            SpawnAgentTool::new(program)
                .with_depth(0)
                .allow_missing_journal(),
        ));
        let call_id = format!("spawn-call-{}", Uuid::new_v4());
        let context = crate::tools::RunContext {
            run_id: format!("run-parent-{}", Uuid::new_v4()),
            session_id: Some(format!("ses-{}", Uuid::new_v4())),
            goal_id: Some("goal-lineage".into()),
            work_item_id: Some("wi-lineage".into()),
            allowed_tools: None,
        };
        let observation = crate::tools::RUN_CONTEXT
            .scope(context.clone(), async {
                registry
                    .execute(
                        &crate::domain::ToolCall {
                            id: call_id.clone(),
                            name: "spawn_agent".into(),
                            arguments: json!({
                                "task": "child task",
                                "workspace": workspace,
                                "allow_write": false
                            }),
                        },
                        &permissions(dir.path(), true),
                        &CancellationToken::new(),
                    )
                    .await
            })
            .await;
        assert_eq!(observation.status, ToolStatus::Ok, "{observation:?}");
        assert_eq!(
            observation.metadata.get("parent_run_id"),
            Some(&context.run_id)
        );
        assert_eq!(
            observation.metadata.get("parent_session_id"),
            context.session_id.as_ref()
        );
        assert_eq!(
            observation.metadata.get("child_run_id").map(String::as_str),
            Some("child")
        );

        let expected_workspace = std::fs::canonicalize(&workspace)
            .unwrap()
            .display()
            .to_string();
        let registry_text =
            std::fs::read_to_string(crate::subagent_registry::registry_path()).unwrap();
        let record = registry_text
            .lines()
            .filter_map(|line| serde_json::from_str::<SubagentRecord>(line).ok())
            .rfind(|record| record.workspace == expected_workspace)
            .expect("spawned record");
        assert_eq!(record.spawn_call_id.as_deref(), Some(call_id.as_str()));
        assert_eq!(record.child_run_id.as_deref(), Some("child"));
        assert_eq!(
            record.parent_run_id.as_deref(),
            Some(context.run_id.as_str())
        );
        assert_eq!(
            record.parent_session_id.as_deref(),
            context.session_id.as_deref()
        );
        assert_eq!(record.goal_id.as_deref(), Some("goal-lineage"));
        assert_eq!(record.work_item_id.as_deref(), Some("wi-lineage"));
        let expected_delegation = format!("{}:{}", context.run_id, call_id);
        assert_eq!(
            record.delegation_id.as_deref(),
            Some(expected_delegation.as_str())
        );
    }

    #[tokio::test]
    async fn concurrent_spawns_keep_their_parent_context() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace_a = dir.path().join("workspace-a");
        let workspace_b = dir.path().join("workspace-b");
        std::fs::create_dir_all(&workspace_a).unwrap();
        std::fs::create_dir_all(&workspace_b).unwrap();
        let tool_a = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
        let tool_b = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal();
        let permissions_a = permissions(dir.path(), false);
        let permissions_b = permissions(dir.path(), false);
        let cancellation_a = CancellationToken::new();
        let cancellation_b = CancellationToken::new();
        let context_a = crate::tools::RunContext {
            run_id: format!("run-a-{}", Uuid::new_v4()),
            session_id: Some(format!("ses-a-{}", Uuid::new_v4())),
            goal_id: None,
            work_item_id: None,
            allowed_tools: None,
        };
        let context_b = crate::tools::RunContext {
            run_id: format!("run-b-{}", Uuid::new_v4()),
            session_id: Some(format!("ses-b-{}", Uuid::new_v4())),
            goal_id: None,
            work_item_id: None,
            allowed_tools: None,
        };

        let future_a = crate::tools::RUN_CONTEXT.scope(
            context_a.clone(),
            crate::tools::TOOL_CALL_ID.scope(
                "call-a".to_string(),
                tool_a.execute(
                    json!({"task": "task a", "workspace": workspace_a}),
                    &permissions_a,
                    &cancellation_a,
                ),
            ),
        );
        let future_b = crate::tools::RUN_CONTEXT.scope(
            context_b.clone(),
            crate::tools::TOOL_CALL_ID.scope(
                "call-b".to_string(),
                tool_b.execute(
                    json!({"task": "task b", "workspace": workspace_b}),
                    &permissions_b,
                    &cancellation_b,
                ),
            ),
        );
        let (result_a, result_b) = tokio::join!(future_a, future_b);
        result_a.unwrap();
        result_b.unwrap();

        let registry_text =
            std::fs::read_to_string(crate::subagent_registry::registry_path()).unwrap();
        let records = registry_text
            .lines()
            .filter_map(|line| serde_json::from_str::<SubagentRecord>(line).ok())
            .collect::<Vec<_>>();
        let record_a = records
            .iter()
            .rfind(|record| {
                record.workspace
                    == std::fs::canonicalize(&workspace_a)
                        .unwrap()
                        .display()
                        .to_string()
            })
            .expect("record a");
        let record_b = records
            .iter()
            .rfind(|record| {
                record.workspace
                    == std::fs::canonicalize(&workspace_b)
                        .unwrap()
                        .display()
                        .to_string()
            })
            .expect("record b");
        assert_eq!(
            record_a.parent_run_id.as_deref(),
            Some(context_a.run_id.as_str())
        );
        assert_eq!(
            record_a.parent_session_id.as_deref(),
            context_a.session_id.as_deref()
        );
        assert_eq!(record_a.spawn_call_id.as_deref(), Some("call-a"));
        assert_eq!(
            record_b.parent_run_id.as_deref(),
            Some(context_b.run_id.as_str())
        );
        assert_eq!(
            record_b.parent_session_id.as_deref(),
            context_b.session_id.as_deref()
        );
        assert_eq!(record_b.spawn_call_id.as_deref(), Some("call-b"));
    }

    #[tokio::test]
    async fn spawn_outside_a_run_context_keeps_parent_ids_empty() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal();
        tool.execute(
            json!({"task": "out-of-run child", "workspace": workspace}),
            &permissions(dir.path(), false),
            &CancellationToken::new(),
        )
        .await
        .unwrap();

        let expected_workspace = std::fs::canonicalize(&workspace)
            .unwrap()
            .display()
            .to_string();
        let registry_text =
            std::fs::read_to_string(crate::subagent_registry::registry_path()).unwrap();
        let record = registry_text
            .lines()
            .filter_map(|line| serde_json::from_str::<SubagentRecord>(line).ok())
            .rfind(|record| record.workspace == expected_workspace)
            .expect("spawned record");
        assert!(record.parent_run_id.is_none());
        assert!(record.parent_session_id.is_none());
    }

    #[tokio::test]
    async fn requested_permissions_only_restrict_the_child() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
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
        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal();
        let error = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                &error,
                ToolError::FailedWithMetadata { message, metadata }
                    if message.contains("needs_authorization")
                        && metadata.get("child_run_id").map(String::as_str) == Some("child")
                        && metadata.get("stop_reason").map(String::as_str)
                            == Some("needs_authorization")
            ),
            "unexpected error: {error:?}"
        );
    }

    #[tokio::test]
    async fn failed_spawn_observation_keeps_parent_lineage() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_exiting(dir.path(), 7);
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let mut registry = ToolRegistry::default();
        registry.register(std::sync::Arc::new(
            SpawnAgentTool::new(program)
                .with_depth(0)
                .allow_missing_journal(),
        ));
        let call_id = format!("spawn-fail-{}", Uuid::new_v4());
        let context = crate::tools::RunContext {
            run_id: format!("run-fail-{}", Uuid::new_v4()),
            session_id: Some(format!("ses-fail-{}", Uuid::new_v4())),
            goal_id: None,
            work_item_id: None,
            allowed_tools: None,
        };
        let observation = crate::tools::RUN_CONTEXT
            .scope(context.clone(), async {
                registry
                    .execute(
                        &crate::domain::ToolCall {
                            id: call_id.clone(),
                            name: "spawn_agent".into(),
                            arguments: json!({"task": "child task", "workspace": workspace}),
                        },
                        &permissions(dir.path(), true),
                        &CancellationToken::new(),
                    )
                    .await
            })
            .await;
        assert_eq!(observation.status, ToolStatus::Failed);
        assert_eq!(observation.error_code.as_deref(), Some("tool_failed"));
        assert_eq!(
            observation.metadata.get("status").map(String::as_str),
            Some("failed")
        );
        assert_eq!(
            observation.metadata.get("parent_run_id"),
            Some(&context.run_id)
        );
        assert_eq!(
            observation.metadata.get("parent_session_id"),
            context.session_id.as_ref()
        );
        assert_eq!(
            observation
                .metadata
                .get("spawn_call_id")
                .map(String::as_str),
            Some(call_id.as_str())
        );
        assert!(observation.metadata.contains_key("exit_status"));

        let registry_text =
            std::fs::read_to_string(crate::subagent_registry::registry_path()).unwrap();
        let expected_workspace = std::fs::canonicalize(&workspace)
            .unwrap()
            .display()
            .to_string();
        let record = registry_text
            .lines()
            .filter_map(|line| serde_json::from_str::<SubagentRecord>(line).ok())
            .rfind(|record| record.workspace == expected_workspace)
            .expect("failed spawn record");
        assert_eq!(record.status, SubagentStatus::Failed);
        assert_eq!(
            record.parent_run_id.as_deref(),
            Some(context.run_id.as_str())
        );
        assert_eq!(
            record.parent_session_id.as_deref(),
            context.session_id.as_deref()
        );
    }

    #[tokio::test]
    async fn cancelled_spawn_is_structured_and_marked_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_sleeping(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let child_permissions = permissions(dir.path(), false);
        let context = crate::tools::RunContext {
            run_id: format!("run-cancel-{}", Uuid::new_v4()),
            session_id: Some(format!("ses-cancel-{}", Uuid::new_v4())),
            goal_id: None,
            work_item_id: None,
            allowed_tools: None,
        };
        let (result, _) = tokio::join!(
            crate::tools::RUN_CONTEXT.scope(
                context.clone(),
                tool.execute(
                    json!({"task": "slow child", "workspace": workspace}),
                    &child_permissions,
                    &cancellation,
                ),
            ),
            async {
                tokio::time::sleep(Duration::from_millis(150)).await;
                cancel.cancel();
            }
        );
        let error = result.unwrap_err();
        assert!(
            matches!(
                &error,
                ToolError::CancelledWithMetadata(metadata)
                    if metadata.get("status").map(String::as_str) == Some("cancelled")
                        && metadata.get("parent_run_id") == Some(&context.run_id)
            ),
            "unexpected error: {error:?}"
        );
        let expected_workspace = std::fs::canonicalize(&workspace)
            .unwrap()
            .display()
            .to_string();
        let registry_text =
            std::fs::read_to_string(crate::subagent_registry::registry_path()).unwrap();
        let record = registry_text
            .lines()
            .filter_map(|line| serde_json::from_str::<SubagentRecord>(line).ok())
            .rfind(|record| record.workspace == expected_workspace)
            .expect("cancelled spawn record");
        assert_eq!(record.status, SubagentStatus::Cancelled);
        assert_eq!(
            record.parent_run_id.as_deref(),
            Some(context.run_id.as_str())
        );
    }

    #[tokio::test]
    async fn child_receives_parent_lineage_environment() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_dumping_env(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tree_path = dir.path().join("tree.json");
        let mut registry = ToolRegistry::default();
        registry.register(std::sync::Arc::new(
            SpawnAgentTool::new(program.clone())
                .with_depth(0)
                .allow_missing_journal()
                .with_tree_budget(tree_path.clone(), TreeLimits::default()),
        ));
        let call_id = format!("spawn-env-{}", Uuid::new_v4());
        let session_id = format!("ses-env-{}", Uuid::new_v4());
        let context = crate::tools::RunContext {
            run_id: format!("run-env-{}", Uuid::new_v4()),
            session_id: Some(session_id.clone()),
            goal_id: None,
            work_item_id: None,
            allowed_tools: None,
        };
        let observation = crate::tools::RUN_CONTEXT
            .scope(context.clone(), async {
                registry
                    .execute(
                        &crate::domain::ToolCall {
                            id: call_id.clone(),
                            name: "spawn_agent".into(),
                            arguments: json!({"task": "env child", "workspace": workspace}),
                        },
                        &permissions(dir.path(), true),
                        &CancellationToken::new(),
                    )
                    .await
            })
            .await;
        assert_eq!(observation.status, ToolStatus::Ok, "{observation:?}");
        let env = std::fs::read_to_string(format!("{}.env", program.display())).unwrap();
        let lines = env.lines().collect::<Vec<_>>();
        assert_eq!(
            lines,
            [
                context.run_id.as_str(),
                session_id.as_str(),
                call_id.as_str(),
                tree_path.to_str().unwrap()
            ]
        );
    }

    #[tokio::test]
    async fn fanout_limit_is_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal();
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

    #[tokio::test]
    async fn completed_child_without_journal_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
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
            matches!(error, ToolError::FailedWithMetadata { ref message, .. } if message.contains("journal")),
            "unexpected error: {error:?}"
        );
    }

    #[tokio::test]
    async fn completed_child_with_valid_journal_preserves_structured_answer() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        let journal_path = dir.path().join("child.jsonl");
        let mut journal = Journal::create(&journal_path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("child task", std::path::PathBuf::from(".")),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(JournalEventKind::ModelResponded {
                response: ModelResponse {
                    provider_id: "fake-provider".into(),
                    model: "fake-model".into(),
                    turn: ModelTurn::ToolCalls { calls: Vec::new() },
                    usage: Usage {
                        input_tokens: 100,
                        output_tokens: 50,
                        cost_micros: 7,
                        ..Usage::default()
                    },
                },
            })
            .unwrap();
        let answer = AgentAnswer {
            schema_version: ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "child done".into(),
            claims: vec![Claim {
                id: "c1".into(),
                text: "42 is even".into(),
                status: ClaimStatus::Verified,
                sources: vec!["child-journal".into()],
                scope: None,
            }],
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
            task_need: None,
        };
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: answer.clone(),
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(journal);

        let output = json!({
            "run_id": "child-valid",
            "journal_path": journal_path.display().to_string(),
            "answer": answer,
            "stop_reason": "completed"
        });
        std::fs::write(dir.path().join("child-output.json"), output.to_string()).unwrap();
        let program = write_program(
            dir.path(),
            "fake-valid",
            &format!(
                "#!/bin/sh\ncat '{}'\n",
                dir.path().join("child-output.json").display()
            ),
        );

        let tree_path = dir.path().join("tree.json");
        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .with_tree_budget(tree_path.clone(), TreeLimits::default());
        let outcome = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(
            outcome.content.contains("\"claims\""),
            "{}",
            outcome.content
        );
        assert!(
            outcome.content.contains("42 is even"),
            "{}",
            outcome.content
        );
        assert!(outcome.content.contains("\"usage\""), "{}", outcome.content);
        assert_eq!(
            outcome.metadata.get("claims_count").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            outcome.metadata.get("child_status").map(String::as_str),
            Some("completed")
        );
        assert_eq!(
            outcome
                .metadata
                .get("usage_input_tokens")
                .map(String::as_str),
            Some("100")
        );
        assert_eq!(
            outcome
                .metadata
                .get("usage_output_tokens")
                .map(String::as_str),
            Some("50")
        );
        assert_eq!(
            outcome
                .metadata
                .get("usage_cost_micros")
                .map(String::as_str),
            Some("7")
        );
        let state: Value =
            serde_json::from_str(&std::fs::read_to_string(&tree_path).unwrap()).unwrap();
        assert_eq!(state["spent_cost_micros"], 7);
        assert_eq!(state["spent_tokens"], 150);
        assert_eq!(state["active"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn root_spawn_starts_a_new_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_pgid(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
        tool.execute(
            json!({"task": "child task", "workspace": workspace}),
            &permissions(dir.path(), false),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let pgid = std::fs::read_to_string(format!("{}.pgid", program.display())).unwrap();
        let record = record_for_workspace(&workspace);
        assert_eq!(
            record.child_pid.map(|pid| pid.to_string()),
            Some(pgid.trim().to_string())
        );
    }

    #[tokio::test]
    async fn deeper_spawn_stays_in_its_ancestor_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_pgid(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(1)
            .allow_missing_journal();
        tool.execute(
            json!({"task": "child task", "workspace": workspace}),
            &permissions(dir.path(), false),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let pgid = std::fs::read_to_string(format!("{}.pgid", program.display())).unwrap();
        // SAFETY: getpgid(0) reads the test process group; no signal is sent.
        let expected = unsafe { libc::getpgid(0) };
        assert_eq!(pgid.trim(), expected.to_string());
    }

    #[tokio::test]
    async fn cancelling_a_root_spawn_kills_its_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_spawning_grandchild(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let child_permissions = permissions(dir.path(), false);
        let (result, _) = tokio::join!(
            tool.execute(
                json!({"task": "spawn a grandchild", "workspace": workspace}),
                &child_permissions,
                &cancellation,
            ),
            async {
                tokio::time::sleep(Duration::from_millis(200)).await;
                cancel.cancel();
            }
        );
        assert!(result.is_err(), "cancelled spawn must not report success");
        let grandchild: u32 = std::fs::read_to_string(format!("{}.grandchild", program.display()))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while crate::subagent_registry::pid_alive(grandchild)
            && std::time::Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            !crate::subagent_registry::pid_alive(grandchild),
            "grandchild {grandchild} survived the process-group kill"
        );
    }

    #[tokio::test]
    async fn replaying_the_same_spawn_call_returns_the_cached_child() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program_counted(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tool = SpawnAgentTool::new(program.clone())
            .with_depth(0)
            .allow_missing_journal();
        let context = crate::tools::RunContext {
            run_id: format!("run-idem-{}", Uuid::new_v4()),
            session_id: None,
            goal_id: None,
            work_item_id: None,
            allowed_tools: None,
        };
        let call_id = format!("call-idem-{}", Uuid::new_v4());
        let permissions = permissions(dir.path(), false);
        let args = json!({"task": "child task", "workspace": workspace});
        let first = crate::tools::RUN_CONTEXT
            .scope(
                context.clone(),
                crate::tools::TOOL_CALL_ID.scope(
                    call_id.clone(),
                    tool.execute(args.clone(), &permissions, &CancellationToken::new()),
                ),
            )
            .await
            .unwrap();
        assert!(!first.metadata.contains_key("idempotent_replay"));
        let second = crate::tools::RUN_CONTEXT
            .scope(
                context.clone(),
                crate::tools::TOOL_CALL_ID.scope(
                    call_id.clone(),
                    tool.execute(args.clone(), &permissions, &CancellationToken::new()),
                ),
            )
            .await
            .unwrap();
        assert_eq!(
            second.metadata.get("idempotent_replay").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            second.metadata.get("child_run_id"),
            first.metadata.get("child_run_id")
        );
        let runs = std::fs::read_to_string(format!("{}.count", program.display())).unwrap();
        assert_eq!(runs.lines().count(), 1);

        let conflict = crate::tools::RUN_CONTEXT
            .scope(
                context.clone(),
                crate::tools::TOOL_CALL_ID.scope(
                    call_id.clone(),
                    tool.execute(
                        json!({"task": "different task", "workspace": workspace}),
                        &permissions,
                        &CancellationToken::new(),
                    ),
                ),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                conflict,
                ToolError::Failed(ref message) if message.contains("spawn idempotency")
            ),
            "unexpected error: {conflict:?}"
        );

        let error = crate::tools::RUN_CONTEXT
            .scope(
                crate::tools::RunContext {
                    run_id: format!("run-idem-{}", Uuid::new_v4()),
                    session_id: None,
                    goal_id: None,
                    work_item_id: None,
                    allowed_tools: None,
                },
                crate::tools::TOOL_CALL_ID.scope(
                    "call-idem-other".to_string(),
                    tool.execute(
                        json!({"task": "child task", "workspace": workspace}),
                        &permissions,
                        &CancellationToken::new(),
                    ),
                ),
            )
            .await;
        assert!(error.is_ok(), "a new run id is a new key");
        let runs = std::fs::read_to_string(format!("{}.count", program.display())).unwrap();
        assert_eq!(runs.lines().count(), 2);
    }

    #[tokio::test]
    async fn tree_budget_rejects_spawn_when_concurrency_is_full() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tree_path = dir.path().join("tree.json");
        let limits = TreeLimits {
            max_agents: 1,
            max_cost_micros: None,
            max_tokens: None,
        };
        let budget = crate::tools::subagent_budget::TreeBudget::at(tree_path.clone(), limits);
        std::mem::forget(budget.reserve("held".into()).unwrap());

        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal()
            .with_tree_budget(tree_path, limits);
        let error = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                ToolError::Failed(ref message)
                    if message.contains("tree budget") && message.contains("concurrency")
            ),
            "unexpected error: {error:?}"
        );
    }

    #[tokio::test]
    async fn tree_budget_rejects_spawn_after_cost_limit() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_program(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let tree_path = dir.path().join("tree.json");
        let limits = TreeLimits {
            max_agents: 4,
            max_cost_micros: Some(10),
            max_tokens: None,
        };
        let budget = crate::tools::subagent_budget::TreeBudget::at(tree_path.clone(), limits);
        let mut first = budget.reserve("first".into()).unwrap();
        first.finish(&UsageTotals {
            calls: 1,
            input_tokens: 5,
            output_tokens: 5,
            cost_micros: 10,
            zero_cost_calls: 0,
        });

        let tool = SpawnAgentTool::new(program)
            .with_depth(0)
            .allow_missing_journal()
            .with_tree_budget(tree_path, limits);
        let error = tool
            .execute(
                json!({"task": "child task", "workspace": workspace}),
                &permissions(dir.path(), false),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                ToolError::Failed(ref message)
                    if message.contains("tree budget") && message.contains("cost budget")
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn oversized_child_answer_is_truncated_with_counts() {
        let answer = json!({
            "status": "completed",
            "conclusion": "x".repeat(MAX_CHILD_ANSWER_BYTES + 1),
            "claims": [1, 2, 3],
            "evidence": [1],
            "conflicts": [],
            "unknowns": [1, 2],
        });
        let bounded = bounded_child_answer(Some(&answer), "short conclusion");
        assert_eq!(bounded["truncated"], true);
        assert_eq!(bounded["counts"]["claims"], 3);
        assert_eq!(bounded["counts"]["evidence"], 1);
        assert_eq!(bounded["counts"]["unknowns"], 2);
        assert_eq!(bounded["conclusion"], "short conclusion");
    }

    #[test]
    fn spawn_is_write_capable_so_read_only_gates_cannot_bypass_it() {
        let tool = SpawnAgentTool::new("1agent");
        assert!(tool.requires_write());
    }
}
