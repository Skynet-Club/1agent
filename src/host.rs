use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::{RunResult, TaskRequest};

pub const HOST_PROTOCOL_VERSION: u32 = 2;
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostCancel {
    pub protocol_version: u32,
    pub request_id: String,
    pub action: HostAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostCommand {
    Run(Box<HostRequest>),
    Cancel(HostCancel),
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
        if value.get("action").and_then(Value::as_str) == Some("cancel") {
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
        } else {
            let request: HostRequest = serde_json::from_value(value).map_err(invalid_json)?;
            validate_protocol(request.protocol_version)?;
            validate_request_id(&request.request_id)?;
            Ok(Self::Run(Box::new(request)))
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
            HostCommand::Run(_) => panic!("expected cancel command"),
        }
    }
}
