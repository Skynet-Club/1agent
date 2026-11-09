use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::domain::{RunResult, TaskRequest};

pub const HOST_PROTOCOL_VERSION: u32 = 3;
pub const MAX_HOST_REQUEST_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub task: TaskRequest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostAction {
    Run,
    Cancel,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostCancel {
    pub protocol_version: u32,
    pub request_id: String,
    pub action: HostAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostStatusRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub action: HostAction,
    #[serde(default)]
    pub target_request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostCommand {
    Run(Box<HostRequest>),
    Cancel(HostCancel),
    Status(Box<HostStatusRequest>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostRequestState {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostStatusSnapshot {
    pub active_requests: usize,
    pub completed_requests: u64,
    pub cancelled_requests: u64,
    pub failed_requests: u64,
    pub target_request_id: Option<String>,
    pub target_state: Option<HostRequestState>,
    pub target_journal_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub action: HostAction,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<RunResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HostError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<HostStatusSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HostError {
    pub code: String,
    pub message: String,
}

impl HostRequest {
    pub fn parse(line: &str) -> Result<Self, HostError> {
        if line.len() > MAX_HOST_REQUEST_BYTES {
            return Err(HostError {
                code: "request_too_large".into(),
                message: format!("request exceeds {MAX_HOST_REQUEST_BYTES} UTF-8 bytes"),
            });
        }
        let request: Self = serde_json::from_str(line).map_err(|error| HostError {
            code: "invalid_json".into(),
            message: error.to_string(),
        })?;
        if request.protocol_version != HOST_PROTOCOL_VERSION {
            return Err(HostError {
                code: "unsupported_protocol".into(),
                message: format!(
                    "expected protocol {}, got {}",
                    HOST_PROTOCOL_VERSION, request.protocol_version
                ),
            });
        }
        if request.request_id.trim().is_empty() {
            return Err(HostError {
                code: "missing_request_id".into(),
                message: "request_id must not be empty".into(),
            });
        }
        Ok(request)
    }
}

impl HostCommand {
    pub fn parse(line: &str) -> Result<Self, HostError> {
        validate_line_size(line)?;
        let value: Value = serde_json::from_str(line).map_err(invalid_json)?;
        match value.get("action").and_then(Value::as_str) {
            Some("cancel") => {
                let cancel: HostCancel = serde_json::from_value(value).map_err(invalid_json)?;
                validate_protocol(cancel.protocol_version)?;
                validate_request_id(&cancel.request_id)?;
                if cancel.action != HostAction::Cancel {
                    return Err(HostError {
                        code: "invalid_action".into(),
                        message: "expected cancel action".into(),
                    });
                }
                Ok(Self::Cancel(cancel))
            }
            Some("status") => {
                let status: HostStatusRequest =
                    serde_json::from_value(value).map_err(invalid_json)?;
                validate_protocol(status.protocol_version)?;
                validate_request_id(&status.request_id)?;
                if status.action != HostAction::Status {
                    return Err(HostError {
                        code: "invalid_action".into(),
                        message: "expected status action".into(),
                    });
                }
                Ok(Self::Status(Box::new(status)))
            }
            _ => {
                let request: HostRequest = serde_json::from_value(value).map_err(invalid_json)?;
                validate_protocol(request.protocol_version)?;
                validate_request_id(&request.request_id)?;
                Ok(Self::Run(Box::new(request)))
            }
        }
    }
}

impl HostResponse {
    pub fn success(request_id: impl Into<String>, result: RunResult) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            action: HostAction::Run,
            ok: true,
            result: Some(result),
            error: None,
            status: None,
        }
    }

    pub fn failure(request_id: impl Into<String>, error: HostError) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            action: HostAction::Run,
            ok: false,
            result: None,
            error: Some(error),
            status: None,
        }
    }

    pub fn cancelled(request_id: impl Into<String>) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            action: HostAction::Cancel,
            ok: true,
            result: None,
            error: None,
            status: None,
        }
    }

    pub fn control_failure(request_id: impl Into<String>, error: HostError) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            action: HostAction::Cancel,
            ok: false,
            result: None,
            error: Some(error),
            status: None,
        }
    }

    pub fn status(request_id: impl Into<String>, status: HostStatusSnapshot) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            action: HostAction::Status,
            ok: true,
            result: None,
            error: None,
            status: Some(status),
        }
    }
}

#[derive(Default)]
pub struct HostRegistry {
    active: HashMap<String, CancellationToken>,
    recent: VecDeque<RecentHostRequest>,
    completed: u64,
    cancelled: u64,
    failed: u64,
}

struct RecentHostRequest {
    request_id: String,
    state: HostRequestState,
    journal_path: Option<PathBuf>,
}

impl HostRegistry {
    pub fn reserve(
        &mut self,
        request_id: String,
        cancellation: CancellationToken,
    ) -> Result<(), HostError> {
        if self.active.contains_key(&request_id) {
            return Err(HostError {
                code: "duplicate_request".into(),
                message: "request id is already active".into(),
            });
        }
        self.active.insert(request_id, cancellation);
        Ok(())
    }

    pub fn cancel(&mut self, request_id: &str) -> bool {
        self.active
            .get(request_id)
            .map(CancellationToken::cancel)
            .is_some()
    }

    pub fn finish(
        &mut self,
        request_id: &str,
        state: HostRequestState,
        journal_path: Option<PathBuf>,
    ) {
        self.active.remove(request_id);
        match state {
            HostRequestState::Completed => self.completed += 1,
            HostRequestState::Cancelled => self.cancelled += 1,
            HostRequestState::Failed => self.failed += 1,
            HostRequestState::Running => return,
        }
        self.recent.push_back(RecentHostRequest {
            request_id: request_id.to_string(),
            state,
            journal_path,
        });
        while self.recent.len() > 256 {
            self.recent.pop_front();
        }
    }

    pub fn snapshot(&self, target_request_id: Option<&str>) -> HostStatusSnapshot {
        let target = target_request_id.and_then(|target| {
            if self.active.contains_key(target) {
                Some((HostRequestState::Running, None))
            } else {
                self.recent
                    .iter()
                    .rev()
                    .find(|entry| entry.request_id == target)
                    .map(|entry| (entry.state, entry.journal_path.clone()))
            }
        });
        let (target_state, target_journal_path) = target
            .map(|(state, path)| (Some(state), path))
            .unwrap_or((None, None));
        HostStatusSnapshot {
            active_requests: self.active.len(),
            completed_requests: self.completed,
            cancelled_requests: self.cancelled,
            failed_requests: self.failed,
            target_request_id: target_request_id.map(str::to_string),
            target_state,
            target_journal_path,
        }
    }
}

fn validate_line_size(line: &str) -> Result<(), HostError> {
    if line.len() > MAX_HOST_REQUEST_BYTES {
        return Err(HostError {
            code: "request_too_large".into(),
            message: format!("request exceeds {MAX_HOST_REQUEST_BYTES} UTF-8 bytes"),
        });
    }
    Ok(())
}

fn validate_protocol(protocol_version: u32) -> Result<(), HostError> {
    if protocol_version != HOST_PROTOCOL_VERSION {
        return Err(HostError {
            code: "unsupported_protocol".into(),
            message: format!(
                "expected protocol {}, got {}",
                HOST_PROTOCOL_VERSION, protocol_version
            ),
        });
    }
    Ok(())
}

fn validate_request_id(request_id: &str) -> Result<(), HostError> {
    if request_id.trim().is_empty() {
        return Err(HostError {
            code: "missing_request_id".into(),
            message: "request_id must not be empty".into(),
        });
    }
    Ok(())
}

fn invalid_json(error: serde_json::Error) -> HostError {
    HostError {
        code: "invalid_json".into(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskRequest;
    use std::path::PathBuf;

    fn request() -> HostRequest {
        HostRequest {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: "req-1".into(),
            task: TaskRequest::from_objective("answer", PathBuf::from(".")),
        }
    }

    #[test]
    fn host_request_round_trips() {
        let request = request();
        let line = serde_json::to_string(&request).unwrap();
        assert_eq!(HostRequest::parse(&line).unwrap(), request);
    }

    #[test]
    fn host_request_rejects_unknown_protocol() {
        let mut request = request();
        request.protocol_version = 999;
        let line = serde_json::to_string(&request).unwrap();
        assert_eq!(
            HostRequest::parse(&line).unwrap_err().code,
            "unsupported_protocol"
        );
    }

    #[test]
    fn host_command_parses_cancel_message() {
        let line = serde_json::json!({
            "protocol_version": HOST_PROTOCOL_VERSION,
            "request_id": "req-1",
            "action": "cancel"
        })
        .to_string();
        match HostCommand::parse(&line).unwrap() {
            HostCommand::Cancel(cancel) => {
                assert_eq!(cancel.request_id, "req-1");
                assert_eq!(cancel.action, HostAction::Cancel);
            }
            HostCommand::Run(_) | HostCommand::Status(_) => {
                panic!("expected cancel command")
            }
        }
    }

    #[test]
    fn registry_tracks_active_and_terminal_requests() {
        let mut registry = HostRegistry::default();
        let token = CancellationToken::new();
        registry.reserve("req-1".into(), token).unwrap();
        let running = registry.snapshot(Some("req-1"));
        assert_eq!(running.active_requests, 1);
        assert_eq!(running.target_state, Some(HostRequestState::Running));
        registry.finish(
            "req-1",
            HostRequestState::Completed,
            Some(PathBuf::from("req-1.jsonl")),
        );
        let completed = registry.snapshot(Some("req-1"));
        assert_eq!(completed.active_requests, 0);
        assert_eq!(completed.completed_requests, 1);
        assert_eq!(completed.target_state, Some(HostRequestState::Completed));
        assert_eq!(
            completed.target_journal_path,
            Some(PathBuf::from("req-1.jsonl"))
        );
        assert_eq!(registry.snapshot(Some("missing")).target_state, None);
    }
}
