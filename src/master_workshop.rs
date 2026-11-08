use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::domain::PermissionSet;
use crate::master::{AgentAvailability, AgentProfile, ExecutionStatus, WorkItem};
use crate::master_coordination::{
    ExecutionObservation, ExecutionReceipt, ExecutionSubmission, ExecutionTarget,
};

const MAX_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_STDERR_BYTES: usize = 64 * 1024;
const MAX_AGENT_TASK_CHARS: usize = 8_000;
const MAX_AGENT_TTL_S: u64 = 86_400;

pub trait MasterExecutionPort {
    fn discover_agents(&self) -> Result<Vec<AgentProfile>, WorkshopAdapterError>;

    fn dispatch(
        &self,
        item: &WorkItem,
        profile: &AgentProfile,
        lease_generation: u64,
        idempotency_key: &str,
        now_unix_ms: u128,
    ) -> Result<ExecutionReceipt, WorkshopAdapterError>;
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkshopAdapterError {
    #[error("failed to start {command}: {message}")]
    Spawn { command: String, message: String },
    #[error("{command} exceeded its timeout")]
    Timeout { command: String },
    #[error("{command} failed with exit {exit}: {message}")]
    Command {
        command: String,
        exit: i32,
        message: String,
    },
    #[error("invalid Workshop response from {command}: {message}")]
    InvalidResponse { command: String, message: String },
    #[error("configured runtime command is empty for Agent '{0}'")]
    MissingRuntimeCommand(String),
    #[error("work item '{0}' has no approved assignment")]
    MissingAssignment(String),
    #[error("invalid Agent capability profile: {0}")]
    InvalidProfiles(String),
    #[error("production lease denied: {0}")]
    LeaseDenied(String),
    #[error("invalid Workshop request: {0}")]
    InvalidRequest(String),
}

struct ProductionLeaseGuard<'a> {
    adapter: &'a WorkshopCliAdapter,
    id: Option<String>,
}

impl<'a> ProductionLeaseGuard<'a> {
    fn new(adapter: &'a WorkshopCliAdapter, id: String) -> Self {
        Self {
            adapter,
            id: Some(id),
        }
    }

    fn take(mut self) -> Option<String> {
        self.id.take()
    }
}

impl Drop for ProductionLeaseGuard<'_> {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            self.adapter.release_production_lease(&id);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkshopAgentView {
    pub version: u32,
    pub agent_id: String,
    pub display_name: String,
    pub provider: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub sessions: Vec<String>,
    #[serde(default)]
    pub panes: Vec<u64>,
    pub status: String,
    #[serde(default)]
    pub task: Option<String>,
    pub registered_ms: u128,
    pub heartbeat_ms: u128,
    pub expires_ms: u128,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkshopJobSummary {
    pub job_id: String,
    pub status: String,
    #[serde(default)]
    pub task_count: usize,
    pub submitted_ms: u128,
    #[serde(default)]
    pub started_ms: Option<u128>,
    #[serde(default)]
    pub finished_ms: Option<u128>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkshopCliAdapter {
    prefix: String,
    timeout: Duration,
    open_gui: bool,
    artifact_dir: Option<PathBuf>,
    conversation_id: Option<String>,
    goal_id: Option<String>,
}

impl Default for WorkshopCliAdapter {
    fn default() -> Self {
        Self {
            prefix: "skynet-workshop".into(),
            timeout: Duration::from_secs(30),
            open_gui: true,
            artifact_dir: None,
            conversation_id: None,
            goal_id: None,
        }
    }
}

impl WorkshopCliAdapter {
    pub fn new(
        prefix: impl Into<String>,
        timeout: Duration,
        open_gui: bool,
    ) -> Result<Self, WorkshopAdapterError> {
        let prefix = prefix.into();
        if prefix.trim().is_empty() || prefix.chars().any(char::is_control) {
            return Err(WorkshopAdapterError::Spawn {
                command: prefix,
                message: "CLI prefix must be non-empty and contain no control characters".into(),
            });
        }
        Ok(Self {
            prefix,
            timeout,
            open_gui,
            artifact_dir: None,
            conversation_id: std::env::var("SKYTERM_MASTER_CONVERSATION_ID")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            goal_id: None,
        })
    }

    pub fn with_artifact_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.artifact_dir = Some(directory.into());
        self
    }

    pub fn with_goal_context(
        mut self,
        conversation_id: impl Into<String>,
        goal_id: impl Into<String>,
    ) -> Self {
        // The host-provided conversation id is authoritative; the trace id is
        // only a fallback for CLI-only runs.
        if self.conversation_id.is_none() {
            self.conversation_id = Some(conversation_id.into());
        }
        self.goal_id = Some(goal_id.into());
        self
    }

    pub fn list_agents(&self) -> Result<Vec<WorkshopAgentView>, WorkshopAdapterError> {
        let args = ["agents", "list", "--include-stale"];
        let output = self.run(&args, self.timeout)?;
        serde_json::from_slice(&output.stdout).map_err(|error| {
            WorkshopAdapterError::InvalidResponse {
                command: self.command_name(args[0]),
                message: error.to_string(),
            }
        })
    }

    pub fn agent_profiles(&self) -> Result<Vec<AgentProfile>, WorkshopAdapterError> {
        Ok(self
            .list_agents()?
            .into_iter()
            .map(WorkshopAgentView::into_profile)
            .collect())
    }

    pub fn merge_profiles(
        &self,
        configured: &[AgentProfile],
    ) -> Result<Vec<AgentProfile>, WorkshopAdapterError> {
        let registry = self.list_agents()?;
        let mut merged = Vec::with_capacity(configured.len().max(registry.len()));
        for configured in configured {
            if let Some(view) = registry
                .iter()
                .find(|view| view.agent_id == configured.agent_id)
            {
                let mut profile = configured.clone();
                profile.runtime = view.provider.clone();
                profile.display_name = Some(view.display_name.clone());
                profile.availability = if view.stale {
                    configured.availability
                } else if view.status == "busy" {
                    AgentAvailability::Busy
                } else {
                    AgentAvailability::Idle
                };
                profile.current_load = u32::from(profile.availability == AgentAvailability::Busy);
                for capability in &view.capabilities {
                    if !profile.capabilities.contains(capability) {
                        profile.capabilities.push(capability.clone());
                    }
                }
                merged.push(profile);
            } else {
                merged.push(configured.clone());
            }
        }
        for view in registry {
            if !merged
                .iter()
                .any(|profile| profile.agent_id == view.agent_id)
            {
                merged.push(view.into_profile());
            }
        }
        merged.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
        Ok(merged)
    }

    pub fn list_jobs(&self) -> Result<Vec<WorkshopJobSummary>, WorkshopAdapterError> {
        let args = ["job", "list"];
        let output = self.run(&args, self.timeout)?;
        serde_json::from_slice(&output.stdout).map_err(|error| {
            WorkshopAdapterError::InvalidResponse {
                command: self.command_name(args[0]),
                message: error.to_string(),
            }
        })
    }

    pub fn job_status(&self, job_id: &str) -> Result<Value, WorkshopAdapterError> {
        validate_text("job_id", job_id)?;
        let output = self.run(&["job", "status", job_id], self.timeout)?;
        parse_value("job", &output.stdout)
    }

    pub fn job_tasks(
        &self,
        job_id: &str,
        offset: usize,
        limit: usize,
        status: Option<&str>,
    ) -> Result<Value, WorkshopAdapterError> {
        validate_text("job_id", job_id)?;
        let offset = offset.to_string();
        let limit = limit.clamp(1, 200).to_string();
        let mut args = vec![
            "job".to_string(),
            "tasks".to_string(),
            job_id.to_string(),
            "--offset".into(),
            offset,
            "--limit".into(),
            limit,
        ];
        if let Some(status) = status {
            validate_text("status", status)?;
            args.push("--status".into());
            args.push(status.into());
        }
        let references = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = self.run(&references, self.timeout)?;
        parse_value("job", &output.stdout)
    }

    pub fn bounded_events(
        &self,
        since_unix_ms: Option<u128>,
        limit: usize,
    ) -> Result<Vec<Value>, WorkshopAdapterError> {
        let limit = limit.clamp(1, 200).to_string();
        let mut args = vec!["events".to_string(), "--limit".to_string(), limit];
        if let Some(since) = since_unix_ms {
            args.push("--since-ms".into());
            args.push(since.to_string());
        }
        let references = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = self.run(&references, self.timeout)?;
        output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
            .map(|line| {
                serde_json::from_slice(line).map_err(|error| {
                    WorkshopAdapterError::InvalidResponse {
                        command: self.command_name("events"),
                        message: error.to_string(),
                    }
                })
            })
            .collect()
    }

    pub fn dispatch(
        &self,
        item: &WorkItem,
        profile: &AgentProfile,
        lease_generation: u64,
        idempotency_key: &str,
        now_unix_ms: u128,
    ) -> Result<ExecutionReceipt, WorkshopAdapterError> {
        if profile.runtime_command.is_empty() {
            return Err(WorkshopAdapterError::MissingRuntimeCommand(
                profile.agent_id.clone(),
            ));
        }
        if item.title.len() > 128 {
            return Err(WorkshopAdapterError::InvalidRequest(
                "work item title exceeds the 128-byte Workshop limit".into(),
            ));
        }
        if !item.workspace.is_dir() {
            return Err(WorkshopAdapterError::Spawn {
                command: self.command_name("agent-run"),
                message: format!("workspace is not a directory: {}", item.workspace.display()),
            });
        }
        let _production_lease = match (&self.conversation_id, &self.goal_id) {
            (Some(conversation_id), Some(goal_id)) => Some(ProductionLeaseGuard::new(
                self,
                self.request_production_lease(
                    conversation_id,
                    goal_id,
                    item,
                    profile,
                    lease_generation,
                )?,
            )),
            _ => None,
        };
        let task = agent_task(item);
        validate_text("task", &task)?;
        let timeout_s = item.budget.timeout_ms.div_ceil(1000).max(1).to_string();
        let workspace = item.workspace.display().to_string();
        let mut args = vec![
            "agent-run".to_string(),
            "--agent".into(),
            profile.agent_id.clone(),
            "--provider".into(),
            profile.runtime.clone(),
            "--task".into(),
            item.title.clone(),
            "--cwd".into(),
            workspace,
            "--timeout".into(),
            timeout_s,
        ];
        if !self.open_gui {
            args.push("--no-gui".into());
        }
        args.push("--".into());
        args.extend(
            profile
                .runtime_command
                .iter()
                .map(|argument| shell_quote(argument)),
        );
        args.push(shell_quote(&task));
        let references = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = self.run_raw(
            &references,
            Duration::from_millis(item.budget.timeout_ms).saturating_add(Duration::from_secs(10)),
        )?;
        let response = match parse_value("agent-run", &output.stdout.bytes) {
            Ok(value) => value,
            Err(error) => {
                if !output.status.success() {
                    return Err(WorkshopAdapterError::Command {
                        command: self.command_name("agent-run"),
                        exit: output.status.code().unwrap_or(1),
                        message: String::from_utf8_lossy(&output.stderr.bytes)
                            .trim()
                            .to_string(),
                    });
                }
                return Err(error);
            }
        };
        if !output.status.success() && response.get("exec").is_none() {
            return Err(WorkshopAdapterError::Command {
                command: self.command_name("agent-run"),
                exit: output.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&output.stderr.bytes)
                    .trim()
                    .to_string(),
            });
        }
        if response.get("schema").and_then(Value::as_str) != Some("skynet.term.agent-run/v1") {
            return Err(WorkshopAdapterError::InvalidResponse {
                command: self.command_name("agent-run"),
                message: "response schema is not skynet.term.agent-run/v1".into(),
            });
        }
        let session_id = response.get("session_id").and_then(Value::as_str);
        let exec = response
            .get("exec")
            .ok_or_else(|| WorkshopAdapterError::InvalidResponse {
                command: self.command_name("agent-run"),
                message: "response has no exec object".into(),
            })?;
        let status = match exec.get("status").and_then(Value::as_str) {
            Some("ok") => ExecutionStatus::Completed,
            Some("timeout") => ExecutionStatus::TimedOut,
            Some("fail") => ExecutionStatus::Failed,
            Some("cancelled") => ExecutionStatus::Cancelled,
            Some("interrupted") => ExecutionStatus::Interrupted,
            Some(other) => {
                return Err(WorkshopAdapterError::InvalidResponse {
                    command: self.command_name("agent-run"),
                    message: format!("unknown execution status '{other}'"),
                });
            }
            None => {
                return Err(WorkshopAdapterError::InvalidResponse {
                    command: self.command_name("agent-run"),
                    message: "exec status is missing".into(),
                });
            }
        };
        let artifact = exec.get("artifact");
        let artifact_path = artifact
            .and_then(|value| value.get("path"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .into_iter()
            .collect::<Vec<_>>();
        let result_digest = artifact
            .and_then(|value| value.get("sha256"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let exit_code = exec
            .get("exit")
            .and_then(Value::as_i64)
            .map(|value| value.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        Ok(ExecutionReceipt {
            task_id: item.id.clone(),
            agent_id: profile.agent_id.clone(),
            lease_generation,
            idempotency_key: idempotency_key.into(),
            execution_target: ExecutionTarget {
                session_id: session_id.map(str::to_string),
                pane_id: response
                    .get("gui")
                    .and_then(|value| value.get("pane_id"))
                    .and_then(Value::as_u64),
                job_id: None,
            },
            status,
            exit_code,
            artifacts: artifact_path,
            result_digest,
            error: (status != ExecutionStatus::Completed)
                .then(|| "Workshop execution did not complete successfully".into()),
            started_unix_ms: now_unix_ms,
            finished_unix_ms: now_unix_ms
                + exec.get("duration_ms").and_then(Value::as_u64).unwrap_or(0) as u128,
        })
    }

    fn request_production_lease(
        &self,
        conversation_id: &str,
        goal_id: &str,
        item: &WorkItem,
        profile: &AgentProfile,
        generation: u64,
    ) -> Result<String, WorkshopAdapterError> {
        let ttl_ms = item.budget.timeout_ms.saturating_add(30_000).max(60_000);
        let generation = generation.to_string();
        let workspace = item.workspace.display().to_string();
        let mut args = vec![
            "lease".to_string(),
            "request".into(),
            "--conversation".into(),
            conversation_id.into(),
            "--goal".into(),
            goal_id.into(),
            "--task".into(),
            item.id.clone(),
            "--attempt".into(),
            generation.clone(),
            "--generation".into(),
            generation,
            "--workspace".into(),
            workspace,
            "--provider".into(),
            profile.runtime.clone(),
            "--ttl-seconds".into(),
            (ttl_ms / 1000).to_string(),
            "--json".into(),
        ];
        if item.permissions.allow_write {
            args.push("--write".into());
        }
        let references = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = self.run(&references, self.timeout)?;
        let value: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
            WorkshopAdapterError::InvalidResponse {
                command: self.command_name("lease"),
                message: error.to_string(),
            }
        })?;
        if value.get("ok") != Some(&Value::Bool(true)) {
            let error = value
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str)
                .unwrap_or("lease_denied");
            return Err(WorkshopAdapterError::LeaseDenied(error.into()));
        }
        value
            .get("lease")
            .and_then(|lease| lease.get("lease_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| WorkshopAdapterError::InvalidResponse {
                command: self.command_name("lease"),
                message: "lease response has no lease_id".into(),
            })
    }

    fn release_production_lease(&self, lease_id: &str) {
        let _ = self.run(
            &["lease", "release", "--id", lease_id, "--json"],
            self.timeout,
        );
    }

    fn release_production_lease_for_task(&self, task_id: &str, attempt: u64) {
        let Some(conversation_id) = self.conversation_id.as_deref() else {
            return;
        };
        if self.goal_id.is_none() {
            return;
        }
        let attempt = attempt.to_string();
        let _ = self.run(
            &[
                "lease",
                "release",
                "--conversation",
                conversation_id,
                "--task",
                task_id,
                "--attempt",
                &attempt,
                "--json",
            ],
            self.timeout,
        );
    }

    pub fn start(
        &self,
        item: &WorkItem,
        profile: &AgentProfile,
        lease_generation: u64,
        idempotency_key: &str,
        now_unix_ms: u128,
    ) -> Result<ExecutionSubmission, WorkshopAdapterError> {
        if profile.runtime_command.is_empty() {
            return Err(WorkshopAdapterError::MissingRuntimeCommand(
                profile.agent_id.clone(),
            ));
        }
        if !item.workspace.is_dir() {
            return Err(WorkshopAdapterError::Spawn {
                command: self.command_name("session"),
                message: format!("workspace is not a directory: {}", item.workspace.display()),
            });
        }
        let artifact_dir =
            self.artifact_dir
                .as_ref()
                .ok_or_else(|| WorkshopAdapterError::Spawn {
                    command: self.command_name("session"),
                    message: "asynchronous execution requires an artifact directory".into(),
                })?;
        create_private_dir(artifact_dir)?;
        let artifact_dir =
            artifact_dir
                .canonicalize()
                .map_err(|error| WorkshopAdapterError::Command {
                    command: artifact_dir.display().to_string(),
                    exit: 1,
                    message: error.to_string(),
                })?;
        let base = artifact_dir.join(format!("{}-{lease_generation}", safe_component(&item.id)));
        let output = base.with_extension("output");
        let exit_file = base.with_extension("exit");
        let workspace = item.workspace.display().to_string();
        let name = format!("1A · {} · {}", item.id, item.title);
        let task_label = format!("task_id={}", item.id);
        let attempt_label = format!("attempt={lease_generation}");
        let production_lease = match (&self.conversation_id, &self.goal_id) {
            (Some(conversation_id), Some(goal_id)) => Some(ProductionLeaseGuard::new(
                self,
                self.request_production_lease(
                    conversation_id,
                    goal_id,
                    item,
                    profile,
                    lease_generation,
                )?,
            )),
            _ => None,
        };
        let created = self.run(
            &[
                "session",
                "create",
                &workspace,
                "--name",
                &truncate_utf8(&name, 128),
                "--label",
                &task_label,
                "--label",
                &attempt_label,
            ],
            self.timeout,
        )?;
        let created = parse_value("session", &created.stdout)?;
        let session_id = created
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or_else(|| WorkshopAdapterError::InvalidResponse {
                command: self.command_name("session"),
                message: "create response has no session_id".into(),
            })?
            .to_string();
        validate_text("session_id", &session_id)?;
        let task = agent_task(item);
        validate_text("task", &task)?;
        let mut register = vec![
            "agents".to_string(),
            "register".into(),
            "--id".into(),
            profile.agent_id.clone(),
            "--provider".into(),
            profile.runtime.clone(),
            "--name".into(),
            profile
                .display_name
                .clone()
                .unwrap_or_else(|| profile.agent_id.clone()),
            "--session".into(),
            session_id.clone(),
            "--busy".into(),
            truncate_utf8(&task, 128),
            "--ttl".into(),
            item.budget
                .timeout_ms
                .div_ceil(1000)
                .saturating_add(30)
                .clamp(1, MAX_AGENT_TTL_S)
                .to_string(),
        ];
        for capability in &profile.capabilities {
            register.push("--capability".into());
            register.push(capability.clone());
        }
        let references = register.iter().map(String::as_str).collect::<Vec<_>>();
        self.run(&references, self.timeout)?;

        let command = {
            let mut values = profile
                .runtime_command
                .iter()
                .map(|argument| shell_quote(argument))
                .collect::<Vec<_>>();
            values.push(shell_quote(&task));
            format!(
                "mkdir -p {dir}; ( {command} ) > {output} 2>&1; oneagent_ec=$?; \
                 printf '%s\\n' \"$oneagent_ec\" > {exit_file}",
                dir = shell_quote(&artifact_dir.display().to_string()),
                command = values.join(" "),
                output = shell_quote(&output.display().to_string()),
                exit_file = shell_quote(&exit_file.display().to_string()),
            )
        };
        self.run(
            &["session", "write", &session_id, "--require-idle", &command],
            self.timeout,
        )?;
        if self.open_gui
            && let Err(error) = self.open_session(&session_id)
        {
            eprintln!("1agent master: GUI attach was not started: {error}");
        }
        let _ = production_lease.and_then(ProductionLeaseGuard::take);
        Ok(ExecutionSubmission {
            task_id: item.id.clone(),
            agent_id: profile.agent_id.clone(),
            lease_generation,
            idempotency_key: idempotency_key.into(),
            execution_target: ExecutionTarget {
                session_id: Some(session_id),
                pane_id: None,
                job_id: None,
            },
            submitted_unix_ms: now_unix_ms,
            deadline_unix_ms: now_unix_ms.saturating_add(u128::from(item.budget.timeout_ms)),
        })
    }

    pub fn observe_submission(
        &self,
        submission: &ExecutionSubmission,
        now_unix_ms: u128,
    ) -> Result<ExecutionObservation, WorkshopAdapterError> {
        let session_id = submission.execution_target.session_id.as_deref().ok_or(
            WorkshopAdapterError::MissingAssignment(submission.task_id.clone()),
        )?;
        let artifact_dir =
            self.artifact_dir
                .as_ref()
                .ok_or_else(|| WorkshopAdapterError::InvalidResponse {
                    command: self.command_name("session"),
                    message: "adapter has no artifact directory".into(),
                })?;
        let artifact_dir =
            artifact_dir
                .canonicalize()
                .map_err(|error| WorkshopAdapterError::Command {
                    command: artifact_dir.display().to_string(),
                    exit: 1,
                    message: error.to_string(),
                })?;
        let base = artifact_dir.join(format!(
            "{}-{}",
            safe_component(&submission.task_id),
            submission.lease_generation
        ));
        let output = base.with_extension("output");
        let exit_file = base.with_extension("exit");
        if exit_file.is_file() {
            let raw = std::fs::read_to_string(&exit_file).map_err(|error| {
                WorkshopAdapterError::Command {
                    command: self.command_name("session"),
                    exit: 1,
                    message: error.to_string(),
                }
            })?;
            let exit_code = raw.trim().parse::<i32>().ok();
            let (artifact_path, artifact_sha256) = file_artifact(&output)?;
            self.release_production_lease_for_task(
                &submission.task_id,
                submission.lease_generation,
            );
            self.mark_agent_idle(&submission.agent_id);
            return Ok(ExecutionObservation::Finished {
                receipt: Box::new(ExecutionReceipt {
                    task_id: submission.task_id.clone(),
                    agent_id: submission.agent_id.clone(),
                    lease_generation: submission.lease_generation,
                    idempotency_key: submission.idempotency_key.clone(),
                    execution_target: submission.execution_target.clone(),
                    status: if exit_code == Some(0) {
                        ExecutionStatus::Completed
                    } else {
                        ExecutionStatus::Failed
                    },
                    exit_code,
                    artifacts: vec![artifact_path],
                    result_digest: Some(artifact_sha256),
                    error: (exit_code != Some(0))
                        .then(|| format!("session command exited with {exit_code:?}")),
                    started_unix_ms: submission.submitted_unix_ms,
                    finished_unix_ms: now_unix_ms,
                }),
            });
        }
        if now_unix_ms >= submission.deadline_unix_ms {
            let _ = self.run(
                &["session", "kill", session_id, "--signal", "15"],
                self.timeout,
            );
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(status) = self.run(&["session", "status", session_id], self.timeout)
                && let Ok(status) = parse_value("session", &status.stdout)
                && status.get("running").and_then(Value::as_bool) == Some(true)
            {
                let _ = self.run(
                    &["session", "kill", session_id, "--signal", "9"],
                    self.timeout,
                );
            }
            self.release_production_lease_for_task(
                &submission.task_id,
                submission.lease_generation,
            );
            self.mark_agent_idle(&submission.agent_id);
            let (artifact_path, artifact_sha256) =
                file_artifact(&output).unwrap_or_else(|_| (String::new(), String::new()));
            return Ok(ExecutionObservation::Finished {
                receipt: Box::new(ExecutionReceipt {
                    task_id: submission.task_id.clone(),
                    agent_id: submission.agent_id.clone(),
                    lease_generation: submission.lease_generation,
                    idempotency_key: submission.idempotency_key.clone(),
                    execution_target: submission.execution_target.clone(),
                    status: ExecutionStatus::TimedOut,
                    exit_code: None,
                    artifacts: (!artifact_path.is_empty())
                        .then_some(artifact_path)
                        .into_iter()
                        .collect(),
                    result_digest: (!artifact_sha256.is_empty()).then_some(artifact_sha256),
                    error: Some("Workshop session exceeded the Work Item deadline".into()),
                    started_unix_ms: submission.submitted_unix_ms,
                    finished_unix_ms: now_unix_ms,
                }),
            });
        }
        if let Some(marker) = output_marker(&output)? {
            match marker {
                OutputMarker::Running => return Ok(ExecutionObservation::Running),
                OutputMarker::AwaitingInput(reason) => {
                    return Ok(ExecutionObservation::AwaitingInput { reason });
                }
                OutputMarker::Blocked(reason) => {
                    return Ok(ExecutionObservation::Blocked { reason });
                }
            }
        }
        let status = self.run(&["session", "status", session_id], self.timeout)?;
        let status = parse_value("session", &status.stdout)?;
        if status.get("running").and_then(Value::as_bool) == Some(true) {
            Ok(ExecutionObservation::Running)
        } else {
            Ok(ExecutionObservation::Blocked {
                reason: "Workshop session stopped without producing an exit receipt".into(),
            })
        }
    }

    pub fn cancel_session(&self, session_id: &str) -> Result<(), WorkshopAdapterError> {
        validate_text("session_id", session_id)?;
        let _ = self.run(
            &["session", "kill", session_id, "--signal", "15"],
            self.timeout,
        );
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(status) = self.run(&["session", "status", session_id], self.timeout)
            && let Ok(status) = parse_value("session", &status.stdout)
            && status.get("running").and_then(Value::as_bool) == Some(true)
        {
            let _ = self.run(
                &["session", "kill", session_id, "--signal", "9"],
                self.timeout,
            );
        }
        match self.run(&["session", "destroy", session_id], self.timeout) {
            Ok(_) => Ok(()),
            Err(WorkshopAdapterError::Command { message, .. })
                if message.contains("not found") || message.contains("not_found") =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub fn write_session(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<(), WorkshopAdapterError> {
        validate_text("session_id", session_id)?;
        validate_text("message", message)?;
        if message.len() > 64 * 1024 {
            return Err(WorkshopAdapterError::Spawn {
                command: self.command_name("session"),
                message: "message exceeds 64 KiB".into(),
            });
        }
        self.run(
            &["session", "write", session_id, "--require-idle", message],
            self.timeout,
        )?;
        Ok(())
    }

    fn mark_agent_idle(&self, agent_id: &str) {
        if let Err(error) = self.run(
            &["agents", "heartbeat", agent_id, "--idle", "--ttl", "60"],
            self.timeout,
        ) {
            eprintln!("1agent master: could not mark Agent '{agent_id}' idle: {error}");
        }
    }

    fn run(&self, args: &[&str], timeout: Duration) -> Result<BoundedOutput, WorkshopAdapterError> {
        let raw = self.run_raw(args, timeout)?;
        if !raw.status.success() {
            return Err(WorkshopAdapterError::Command {
                command: self.command_name(args.first().copied().unwrap_or_default()),
                exit: raw.status.code().unwrap_or(1),
                message: String::from_utf8_lossy(&raw.stderr.bytes)
                    .trim()
                    .to_string(),
            });
        }
        Ok(BoundedOutput {
            stdout: raw.stdout.bytes,
            _stdout_truncated: raw.stdout.truncated,
        })
    }

    fn run_raw(&self, args: &[&str], timeout: Duration) -> Result<RawOutput, WorkshopAdapterError> {
        let command_name = self.command_name(args.first().copied().unwrap_or_default());
        let mut command = Command::new(&command_name);
        command
            .args(args.iter().skip(1))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("SKYTERM_CLIENT_ID", "1agent-master");
        let mut child = spawn_with_retry(&mut command, &command_name)?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let stdout_reader = std::thread::spawn(move || read_bounded(stdout, MAX_STDOUT_BYTES));
        let stderr_reader = std::thread::spawn(move || read_bounded(stderr, MAX_STDERR_BYTES));
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(WorkshopAdapterError::Timeout {
                        command: command_name,
                    });
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(WorkshopAdapterError::Spawn {
                        command: command_name,
                        message: error.to_string(),
                    });
                }
            }
        };
        let stdout = stdout_reader.join().unwrap_or_default();
        let stderr = stderr_reader.join().unwrap_or_default();
        Ok(RawOutput {
            status,
            stdout,
            stderr,
        })
    }

    fn command_name(&self, suffix: &str) -> String {
        if suffix.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}-{suffix}", self.prefix)
        }
    }

    fn open_session(&self, session_id: &str) -> Result<(), WorkshopAdapterError> {
        Command::new(&self.prefix)
            .args(["--attach-session", session_id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| WorkshopAdapterError::Spawn {
                command: self.prefix.clone(),
                message: error.to_string(),
            })
    }
}

impl MasterExecutionPort for WorkshopCliAdapter {
    fn discover_agents(&self) -> Result<Vec<AgentProfile>, WorkshopAdapterError> {
        self.agent_profiles()
    }

    fn dispatch(
        &self,
        item: &WorkItem,
        profile: &AgentProfile,
        lease_generation: u64,
        idempotency_key: &str,
        now_unix_ms: u128,
    ) -> Result<ExecutionReceipt, WorkshopAdapterError> {
        WorkshopCliAdapter::dispatch(
            self,
            item,
            profile,
            lease_generation,
            idempotency_key,
            now_unix_ms,
        )
    }
}

impl WorkshopAgentView {
    pub fn into_profile(self) -> AgentProfile {
        AgentProfile {
            agent_id: self.agent_id,
            runtime: self.provider,
            display_name: Some(self.display_name),
            capabilities: self.capabilities,
            tools: Vec::new(),
            runtime_command: Vec::new(),
            permissions: PermissionSet::default(),
            availability: if self.stale {
                AgentAvailability::Offline
            } else if self.status == "busy" {
                AgentAvailability::Busy
            } else {
                AgentAvailability::Idle
            },
            current_load: u32::from(!self.stale && self.status == "busy"),
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: crate::domain::RiskLevel::Low,
        }
    }
}

#[derive(Debug)]
struct BoundedOutput {
    stdout: Vec<u8>,
    _stdout_truncated: bool,
}

struct RawOutput {
    status: std::process::ExitStatus,
    stdout: LimitedReader,
    stderr: LimitedReader,
}

#[derive(Debug, Default)]
struct LimitedReader {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_bounded(mut reader: impl Read, limit: usize) -> LimitedReader {
    let mut output = LimitedReader::default();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                let remaining = limit.saturating_sub(output.bytes.len());
                output
                    .bytes
                    .extend_from_slice(&buffer[..read.min(remaining)]);
                output.truncated |= read > remaining;
            }
        }
    }
    output
}

fn spawn_with_retry(
    command: &mut Command,
    command_name: &str,
) -> Result<std::process::Child, WorkshopAdapterError> {
    for attempt in 0..20 {
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(error) if error.raw_os_error() == Some(26) && attempt < 19 => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                return Err(WorkshopAdapterError::Spawn {
                    command: command_name.into(),
                    message: error.to_string(),
                });
            }
        }
    }
    unreachable!("spawn loop either returns a child or an error")
}

fn parse_value(command: &str, bytes: &[u8]) -> Result<Value, WorkshopAdapterError> {
    serde_json::from_slice(bytes).map_err(|error| WorkshopAdapterError::InvalidResponse {
        command: format!("skynet-workshop-{command}"),
        message: error.to_string(),
    })
}

fn validate_text(field: &str, value: &str) -> Result<(), WorkshopAdapterError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(WorkshopAdapterError::Spawn {
            command: field.into(),
            message: "must be non-empty and contain no control characters".into(),
        });
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn agent_task(item: &WorkItem) -> String {
    truncate_utf8(
        &format!(
            "{}: {}. Progress protocol: print [1A:AWAITING_INPUT] followed by the question \
             when human input is required; print [1A:RUNNING] after work resumes; print \
             [1A:BLOCKED] followed by the reason when no safe progress is possible.",
            item.title, item.objective
        ),
        MAX_AGENT_TASK_CHARS,
    )
}

fn safe_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.') {
                byte as char
            } else {
                '_'
            }
        })
        .collect()
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    let mut end = 0;
    for character in value.chars() {
        let next = end + character.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    value[..end].to_string()
}

fn create_private_dir(path: &Path) -> Result<(), WorkshopAdapterError> {
    std::fs::create_dir_all(path).map_err(|error| WorkshopAdapterError::Command {
        command: path.display().to_string(),
        exit: 1,
        message: error.to_string(),
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(
            |error| WorkshopAdapterError::Command {
                command: path.display().to_string(),
                exit: 1,
                message: error.to_string(),
            },
        )?;
    }
    Ok(())
}

fn file_artifact(path: &Path) -> Result<(String, String), WorkshopAdapterError> {
    let mut file = std::fs::File::open(path).map_err(|error| WorkshopAdapterError::Command {
        command: path.display().to_string(),
        exit: 1,
        message: error.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| WorkshopAdapterError::Command {
                command: path.display().to_string(),
                exit: 1,
                message: error.to_string(),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok((path.display().to_string(), hex::encode(hasher.finalize())))
}

#[derive(Debug, PartialEq, Eq)]
enum OutputMarker {
    Running,
    AwaitingInput(String),
    Blocked(String),
}

fn output_marker(path: &Path) -> Result<Option<OutputMarker>, WorkshopAdapterError> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut file = std::fs::File::open(path).map_err(|error| WorkshopAdapterError::Command {
        command: path.display().to_string(),
        exit: 1,
        message: error.to_string(),
    })?;
    let length = file
        .metadata()
        .map_err(|error| WorkshopAdapterError::Command {
            command: path.display().to_string(),
            exit: 1,
            message: error.to_string(),
        })?
        .len();
    file.seek(SeekFrom::Start(length.saturating_sub(64 * 1024)))
        .map_err(|error| WorkshopAdapterError::Command {
            command: path.display().to_string(),
            exit: 1,
            message: error.to_string(),
        })?;
    let mut bytes = Vec::with_capacity(64 * 1024);
    file.take(64 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|error| WorkshopAdapterError::Command {
            command: path.display().to_string(),
            exit: 1,
            message: error.to_string(),
        })?;
    let text = String::from_utf8_lossy(&bytes);
    let running = text.rfind("[1A:RUNNING]");
    let awaiting = text.rfind("[1A:AWAITING_INPUT]");
    let blocked = text.rfind("[1A:BLOCKED]");
    let latest = [
        running.map(|index| (index, OutputMarkerKind::Running)),
        awaiting.map(|index| (index, OutputMarkerKind::AwaitingInput)),
        blocked.map(|index| (index, OutputMarkerKind::Blocked)),
    ]
    .into_iter()
    .flatten()
    .max_by_key(|(index, _)| *index);
    let Some((index, kind)) = latest else {
        return Ok(None);
    };
    let marker_len = match kind {
        OutputMarkerKind::Running => "[1A:RUNNING]".len(),
        OutputMarkerKind::AwaitingInput => "[1A:AWAITING_INPUT]".len(),
        OutputMarkerKind::Blocked => "[1A:BLOCKED]".len(),
    };
    let reason = text[index + marker_len..]
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(Some(match kind {
        OutputMarkerKind::Running => OutputMarker::Running,
        OutputMarkerKind::AwaitingInput => OutputMarker::AwaitingInput(reason),
        OutputMarkerKind::Blocked => OutputMarker::Blocked(reason),
    }))
}

#[derive(Debug, Clone, Copy)]
enum OutputMarkerKind {
    Running,
    AwaitingInput,
    Blocked,
}

pub fn load_agent_profiles(path: &Path) -> Result<Vec<AgentProfile>, WorkshopAdapterError> {
    let bytes = std::fs::read(path).map_err(|error| WorkshopAdapterError::Spawn {
        command: path.display().to_string(),
        message: error.to_string(),
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| WorkshopAdapterError::InvalidProfiles(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RiskLevel;
    use crate::master::{WorkBudget, WorkItemStatus};
    use tempfile::tempdir;

    #[test]
    fn registry_profiles_merge_configuration_without_inventing_tools() {
        let adapter = WorkshopCliAdapter::default();
        let registry = WorkshopAgentView {
            version: 1,
            agent_id: "worker".into(),
            display_name: "Worker".into(),
            provider: "pi".into(),
            capabilities: vec!["rust".into(), "test".into()],
            sessions: vec!["term-1".into()],
            panes: vec![3],
            status: "idle".into(),
            task: None,
            registered_ms: 1,
            heartbeat_ms: 1,
            expires_ms: u128::MAX,
            stale: false,
        };
        let profile = registry.into_profile();
        assert_eq!(profile.availability, AgentAvailability::Idle);
        assert!(profile.tools.is_empty());
        assert_eq!(profile.observed_success_permille, None);
        let _ = adapter;
    }

    #[test]
    fn configured_runtime_can_relaunch_after_a_stale_registry_heartbeat() {
        let root = tempdir().unwrap();
        let prefix = root.path().join("fake");
        write_executable(
            &root.path().join("fake-agents"),
            r#"#!/bin/sh
printf '%s\n' '[{"version":1,"agent_id":"worker","display_name":"Worker","provider":"pi","capabilities":["rust"],"sessions":[],"panes":[],"status":"idle","registered_ms":1,"heartbeat_ms":1,"expires_ms":2,"stale":true}]'
"#,
        );
        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap();
        let configured = AgentProfile {
            agent_id: "worker".into(),
            runtime: "pi".into(),
            display_name: None,
            capabilities: vec!["rust".into()],
            tools: vec!["shell".into()],
            runtime_command: vec!["pi".into()],
            permissions: PermissionSet::default(),
            availability: AgentAvailability::Idle,
            current_load: 0,
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: RiskLevel::Low,
        };
        let merged = adapter.merge_profiles(&[configured]).unwrap();
        assert_eq!(merged[0].availability, AgentAvailability::Idle);
        let discovered = adapter.agent_profiles().unwrap();
        assert_eq!(discovered[0].availability, AgentAvailability::Offline);
    }

    #[test]
    fn fake_workshop_cli_is_bounded_and_dispatches_real_contract_shape() {
        let root = tempdir().unwrap();
        let prefix = root.path().join("fake");
        write_executable(
            &root.path().join("fake-agents"),
            r#"#!/bin/sh
printf '%s\n' '[{"version":1,"agent_id":"worker","display_name":"Worker","provider":"pi","capabilities":["rust"],"sessions":[],"panes":[],"status":"idle","registered_ms":1,"heartbeat_ms":1,"expires_ms":9999999999999,"stale":false}]'
"#,
        );
        write_executable(
            &root.path().join("fake-agent-run"),
            r#"#!/bin/sh
printf '%s\n' '{"schema":"skynet.term.agent-run/v1","agent":{"agent_id":"worker"},"session_id":"term-test","gui":{"started":false},"exec":{"status":"ok","exit":0,"duration_ms":12,"artifact":{"path":"/tmp/out","sha256":"abc"}}}'
"#,
        );
        write_executable(
            &root.path().join("fake-lease"),
            "#!/bin/sh\ncommand=$1; shift; printf '%s\\n' \"$@\" > \"$0.$command.args\"; if [ \"$command\" = request ]; then printf '%s\\n' '{\"ok\":true,\"lease\":{\"lease_id\":\"lease-test\"}}'; else printf '%s\\n' '{\"ok\":true}'; fi\n",
        );
        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap()
                .with_goal_context("master-test", "goal-test");
        let profiles = adapter.agent_profiles().unwrap();
        assert_eq!(profiles[0].agent_id, "worker");
        let mut profile = profiles[0].clone();
        profile.runtime_command = vec!["pi".into(), "--print".into()];
        profile.tools = vec!["shell".into()];
        let workspace = tempdir().unwrap();
        let item = WorkItem {
            id: "task-1".into(),
            title: "Test".into(),
            objective: "Run tests".into(),
            dependencies: Vec::new(),
            required_capabilities: vec!["rust".into()],
            required_tools: vec!["shell".into()],
            permissions: PermissionSet::default(),
            acceptance_criteria: vec!["tests pass".into()],
            expected_artifacts: vec!["out.txt".into()],
            workspace: workspace.path().into(),
            budget: WorkBudget::default(),
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        };
        let receipt = adapter
            .dispatch(&item, &profile, 1, "plan-1:task-1:1", 10)
            .unwrap();
        let output = receipt
            .artifacts
            .first()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default();
        assert_eq!(
            receipt.status,
            ExecutionStatus::Completed,
            "exit={:?} error={:?} output={output}",
            receipt.exit_code,
            receipt.error
        );
        assert_eq!(
            receipt.execution_target.session_id.as_deref(),
            Some("term-test")
        );
        let release_args = std::fs::read_to_string(root.path().join("fake-lease.release.args"))
            .expect("synchronous dispatch must release its production lease");
        assert!(release_args.contains("lease-test"), "{release_args}");
        assert!(release_args.contains("--id"), "{release_args}");
    }

    #[test]
    fn async_session_start_and_poll_preserve_target_and_artifact_hash() {
        let root = tempdir().unwrap();
        let prefix = root.path().join("fake");
        write_executable(
            &root.path().join("fake-agents"),
            "#!/bin/sh\nprintf '%s\\n' '[]'\n",
        );
        write_executable(
            &root.path().join("fake-session"),
            r#"#!/bin/sh
command=$1
shift
case "$command" in
  create)
    printf '%s\n' "$@" > "$0.args"
    printf '%s\n' '{"session_id":"term-async"}'
    ;;
  write)
    session=$1
    shift
    if [ "${1-}" = "--require-idle" ]; then
      shift
    fi
    /bin/bash -c "$*"
    ;;
  status)
    printf '%s\n' '{"session_id":"term-async","running":false}'
    ;;
  kill)
    printf '%s\n' '{"signaled":true}'
    ;;
  *)
    printf '%s\n' '{}'
    ;;
esac
"#,
        );
        write_executable(
            &root.path().join("fake-lease"),
            r#"#!/bin/sh
command=$1
shift
printf '%s\n' "$@" > "$0.$command.args"
if [ "$command" = "request" ]; then
  printf '%s\n' '{"ok":true,"lease":{"lease_id":"lease-test"}}'
else
  printf '%s\n' '{"ok":true}'
fi
"#,
        );
        let artifact_dir = root.path().join("executions");
        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap()
                .with_artifact_dir(&artifact_dir)
                .with_goal_context("master-test", "goal-test");
        let workspace = tempdir().unwrap();
        let item = WorkItem {
            id: "async-task".into(),
            title: "Async".into(),
            objective: "Create result.txt".into(),
            dependencies: Vec::new(),
            required_capabilities: vec!["shell".into()],
            required_tools: vec!["shell".into()],
            permissions: PermissionSet::default(),
            acceptance_criteria: vec!["result exists".into()],
            expected_artifacts: vec!["result.txt".into()],
            workspace: workspace.path().into(),
            budget: WorkBudget {
                timeout_ms: 2_000,
                ..WorkBudget::default()
            },
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        };
        let profile = AgentProfile {
            agent_id: "worker".into(),
            runtime: "shell".into(),
            display_name: None,
            capabilities: vec!["shell".into()],
            tools: vec!["shell".into()],
            runtime_command: vec![
                "sh".into(),
                "-c".into(),
                format!(
                    "printf async > '{}'",
                    workspace.path().join("result.txt").display()
                ),
            ],
            permissions: PermissionSet::default(),
            availability: AgentAvailability::Idle,
            current_load: 0,
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: RiskLevel::Low,
        };
        let submission = adapter
            .start(&item, &profile, 2, "plan:async-task:2", 100)
            .unwrap();
        assert_eq!(
            submission.execution_target.session_id.as_deref(),
            Some("term-async")
        );
        let create_args = std::fs::read_to_string(root.path().join("fake-session.args")).unwrap();
        assert!(create_args.contains("task_id=async-task"), "{create_args}");
        assert!(create_args.contains("attempt=2"), "{create_args}");
        let lease_args =
            std::fs::read_to_string(root.path().join("fake-lease.request.args")).unwrap();
        assert!(lease_args.contains("--conversation"), "{lease_args}");
        assert!(lease_args.contains("master-test"), "{lease_args}");
        assert!(lease_args.contains("--goal"), "{lease_args}");
        assert!(lease_args.contains("goal-test"), "{lease_args}");
        assert!(lease_args.contains("async-task"), "{lease_args}");
        assert!(
            !root.path().join("fake-lease.release.args").exists(),
            "lease must stay active while the session is running"
        );
        assert!(workspace.path().join("result.txt").is_file());
        let observation = adapter.observe_submission(&submission, 200).unwrap();
        let ExecutionObservation::Finished { receipt } = observation else {
            panic!("expected a final receipt");
        };
        let release_args =
            std::fs::read_to_string(root.path().join("fake-lease.release.args")).unwrap();
        assert!(release_args.contains("master-test"), "{release_args}");
        assert!(release_args.contains("async-task"), "{release_args}");
        assert!(release_args.contains("--attempt"), "{release_args}");
        assert!(release_args.contains('2'), "{release_args}");
        let receipt = *receipt;
        let output = receipt
            .artifacts
            .first()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default();
        assert_eq!(
            receipt.status,
            ExecutionStatus::Completed,
            "exit={:?} error={:?} output={output}",
            receipt.exit_code,
            receipt.error
        );
        assert_eq!(receipt.exit_code, Some(0));
        assert_eq!(receipt.artifacts.len(), 1);
        assert_ne!(receipt.result_digest.as_deref(), Some(""));
    }

    #[test]
    fn output_marker_uses_the_last_explicit_progress_signal() {
        let root = tempdir().unwrap();
        let output = root.path().join("output");
        std::fs::write(
            &output,
            "[1A:AWAITING_INPUT] approve deployment\n\
             [1A:RUNNING]\n\
             [1A:BLOCKED] missing token\n",
        )
        .unwrap();
        assert_eq!(
            output_marker(&output).unwrap(),
            Some(OutputMarker::Blocked("missing token".into()))
        );
        std::fs::write(&output, "[1A:BLOCKED] missing token\n[1A:RUNNING]\n").unwrap();
        assert_eq!(output_marker(&output).unwrap(), Some(OutputMarker::Running));
        let mut large = "x".repeat(128 * 1024);
        large.push_str("\n[1A:AWAITING_INPUT] approve tail\n");
        std::fs::write(&output, large).unwrap();
        assert_eq!(
            output_marker(&output).unwrap(),
            Some(OutputMarker::AwaitingInput("approve tail".into()))
        );
    }

    #[test]
    fn expired_session_is_killed_and_reported_as_timed_out() {
        let root = tempdir().unwrap();
        let prefix = root.path().join("fake");
        write_executable(
            &root.path().join("fake-agents"),
            "#!/bin/sh\nprintf '%s\\n' '[]'\n",
        );
        write_executable(
            &root.path().join("fake-session"),
            r#"#!/bin/sh
if [ "$1" = "status" ]; then
  printf '%s\n' '{"session_id":"term-slow","running":true}'
else
  printf '%s\n' '{"signaled":true}'
fi
"#,
        );
        let artifact_dir = root.path().join("executions");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap()
                .with_artifact_dir(&artifact_dir);
        let submission = ExecutionSubmission {
            task_id: "slow".into(),
            agent_id: "worker".into(),
            lease_generation: 1,
            idempotency_key: "plan:slow:1".into(),
            execution_target: ExecutionTarget {
                session_id: Some("term-slow".into()),
                pane_id: None,
                job_id: None,
            },
            submitted_unix_ms: 10,
            deadline_unix_ms: 100,
        };
        let ExecutionObservation::Finished { receipt } =
            adapter.observe_submission(&submission, 101).unwrap()
        else {
            panic!("expected timeout receipt");
        };
        assert_eq!(receipt.status, ExecutionStatus::TimedOut);
        assert_eq!(receipt.exit_code, None);
    }

    #[test]
    fn nonzero_agent_run_with_valid_receipt_keeps_failure_evidence() {
        let root = tempdir().unwrap();
        let prefix = root.path().join("fake");
        write_executable(
            &root.path().join("fake-agent-run"),
            r#"#!/bin/sh
printf '%s\n' '{"schema":"skynet.term.agent-run/v1","agent":{"agent_id":"worker"},"session_id":"term-failed","gui":{"started":false},"exec":{"status":"fail","exit":7,"duration_ms":12,"artifact":{"path":"/tmp/out","sha256":"abc"}}}'
exit 7
"#,
        );
        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap();
        let profile = AgentProfile {
            agent_id: "worker".into(),
            runtime: "pi".into(),
            display_name: None,
            capabilities: vec!["rust".into()],
            tools: Vec::new(),
            runtime_command: vec!["pi".into()],
            permissions: PermissionSet::default(),
            availability: AgentAvailability::Idle,
            current_load: 0,
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: RiskLevel::Low,
        };
        let item = WorkItem {
            id: "task-1".into(),
            title: "Fail".into(),
            objective: "Fail".into(),
            dependencies: Vec::new(),
            required_capabilities: Vec::new(),
            required_tools: Vec::new(),
            permissions: PermissionSet::default(),
            acceptance_criteria: Vec::new(),
            expected_artifacts: Vec::new(),
            workspace: root.path().into(),
            budget: WorkBudget::default(),
            deadline_unix_ms: None,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        };
        let receipt = adapter.dispatch(&item, &profile, 1, "key", 10).unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Failed);
        assert_eq!(receipt.exit_code, Some(7));
        assert_eq!(
            receipt.execution_target.session_id.as_deref(),
            Some("term-failed")
        );
        assert_eq!(receipt.result_digest.as_deref(), Some("abc"));
    }

    #[test]
    fn cancel_session_destroys_the_session() {
        let root = tempdir().unwrap();
        let prefix = root.path().join("fake");
        write_executable(
            &root.path().join("fake-session"),
            "#!/bin/sh\ncommand=$1; shift\nprintf '%s %s\\n' \"$command\" \"$*\" >> \"$0.args\"\nif [ \"$command\" = status ]; then printf '%s\\n' '{\"running\":false}'; else printf '%s\\n' '{\"ok\":true}'; fi\n",
        );
        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap();
        adapter.cancel_session("term-test").unwrap();
        let args = std::fs::read_to_string(root.path().join("fake-session.args")).unwrap();
        assert!(args.contains("destroy term-test"), "{args}");
    }

    fn write_executable(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
}
