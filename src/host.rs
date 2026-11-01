use serde::{Deserialize, Serialize};

use crate::domain::{RunResult, TaskRequest};

pub const HOST_PROTOCOL_VERSION: u32 = 1;
pub const MAX_HOST_REQUEST_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub task: TaskRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HostResponse {
    pub protocol_version: u32,
    pub request_id: String,
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

impl HostResponse {
    pub fn success(request_id: impl Into<String>, result: RunResult) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(request_id: impl Into<String>, error: HostError) -> Self {
        Self {
            protocol_version: HOST_PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: false,
            result: None,
            error: Some(error),
        }
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
}
