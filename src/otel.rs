//! Minimal OpenTelemetry GenAI-style span projection over the run journal.
//!
//! This emits stable `gen_ai.*` attributes without adding an OTel SDK. A host
//! can forward the JSON spans to its own collector.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::StopReason;
use crate::journal::{JournalEvent, JournalEventKind};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Internal,
    Client,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    Unset,
    Ok,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OtelSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: SpanKind,
    pub start_unix_nano: u128,
    pub end_unix_nano: u128,
    pub status: SpanStatus,
    pub attributes: BTreeMap<String, String>,
}

/// Stable span id for a parent's `spawn_agent` tool span. The child computes
/// the same id from `ParentRunLinked`; both journals then join one trace.
pub fn spawn_span_id(parent_run_id: &str, spawn_call_id: &str) -> String {
    let digest = Sha256::digest(format!("{parent_run_id}:{spawn_call_id}").as_bytes());
    hashed_span_id(&digest)
}

fn child_root_span_id(run_id: uuid::Uuid) -> String {
    let digest = Sha256::digest(run_id.to_string().as_bytes());
    hashed_span_id(&digest)
}

/// Keep hashed span ids out of the low range used by sequential journal spans.
fn hashed_span_id(digest: &[u8]) -> String {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    format!("{:016x}", u64::from_be_bytes(bytes) | 0x8000_0000_0000_0000)
}

pub fn spans_from_events(events: &[JournalEvent]) -> Vec<OtelSpan> {
    let Some(first) = events.first() else {
        return Vec::new();
    };
    let parent_link = events.iter().find_map(|event| match &event.event {
        JournalEventKind::ParentRunLinked { link } => Some(link),
        _ => None,
    });
    let trace_id = parent_link
        .and_then(|link| uuid::Uuid::parse_str(&link.parent_run_id).ok())
        .map(|run_id| format!("{:032x}", run_id.as_u128()))
        .unwrap_or_else(|| format!("{:032x}", first.run_id.as_u128()));
    let root_span_id = match parent_link {
        Some(_) => child_root_span_id(first.run_id),
        None => "0000000000000001".to_string(),
    };
    let parent_span_id =
        parent_link.map(|link| spawn_span_id(&link.parent_run_id, &link.spawn_call_id));
    let conversation_id = parent_link
        .and_then(|link| link.parent_session_id.clone())
        .unwrap_or_else(|| first.run_id.to_string());
    let root_start = first.timestamp_unix_ms;
    let root_end = events
        .last()
        .map(|event| event.timestamp_unix_ms)
        .unwrap_or(root_start);
    let root_status = events
        .iter()
        .find_map(|event| match &event.event {
            JournalEventKind::RunStopped { reason } => Some(status_for_reason(*reason)),
            _ => None,
        })
        .unwrap_or(SpanStatus::Unset);
    let mut root_attributes = BTreeMap::new();
    root_attributes.insert("gen_ai.operation.name".into(), "invoke_agent".into());
    root_attributes.insert("gen_ai.agent.name".into(), "1Agent".into());
    root_attributes.insert("gen_ai.conversation.id".into(), conversation_id.clone());
    if let Some(link) = parent_link {
        root_attributes.insert(
            "gen_ai.subagent.parent_run_id".into(),
            link.parent_run_id.clone(),
        );
        root_attributes.insert(
            "gen_ai.subagent.spawn_call_id".into(),
            link.spawn_call_id.clone(),
        );
    }
    if let Some(JournalEventKind::RunStarted { mode, .. }) = events
        .iter()
        .find(|event| matches!(event.event, JournalEventKind::RunStarted { .. }))
        .map(|event| &event.event)
    {
        root_attributes.insert("gen_ai.request.mode".into(), format!("{mode:?}"));
    }

    let mut spans = vec![OtelSpan {
        trace_id: trace_id.clone(),
        span_id: root_span_id.clone(),
        parent_span_id,
        name: "invoke_agent 1Agent".into(),
        kind: SpanKind::Internal,
        start_unix_nano: ms_to_nano(root_start),
        end_unix_nano: ms_to_nano(root_end),
        status: root_status,
        attributes: root_attributes,
    }];

    let mut pending_request: Option<(u128, u64)> = None;
    let mut next_span = 2u64;
    for event in events {
        match &event.event {
            JournalEventKind::ModelRequested { .. }
            | JournalEventKind::ModelRequestDelta { .. } => {
                pending_request = Some((event.timestamp_unix_ms, event.seq));
            }
            JournalEventKind::ModelResponded { response } => {
                let start = pending_request
                    .map(|(timestamp, _)| timestamp)
                    .unwrap_or(event.timestamp_unix_ms);
                let mut attributes = BTreeMap::new();
                attributes.insert("gen_ai.operation.name".into(), "chat".into());
                attributes.insert("gen_ai.provider.name".into(), response.provider_id.clone());
                attributes.insert("gen_ai.request.model".into(), response.model.clone());
                attributes.insert(
                    "gen_ai.usage.input_tokens".into(),
                    response.usage.input_tokens.to_string(),
                );
                attributes.insert(
                    "gen_ai.usage.output_tokens".into(),
                    response.usage.output_tokens.to_string(),
                );
                attributes.insert(
                    "gen_ai.usage.cost_micros".into(),
                    response.usage.cost_micros.to_string(),
                );
                attributes.insert("gen_ai.conversation.id".into(), conversation_id.clone());
                spans.push(OtelSpan {
                    trace_id: trace_id.clone(),
                    span_id: format!("{next_span:016x}"),
                    parent_span_id: Some(root_span_id.clone()),
                    name: format!("chat {}/{}", response.provider_id, response.model),
                    kind: SpanKind::Client,
                    start_unix_nano: ms_to_nano(start),
                    end_unix_nano: ms_to_nano(event.timestamp_unix_ms),
                    status: SpanStatus::Ok,
                    attributes,
                });
                next_span += 1;
                pending_request = None;
            }
            _ => {}
        }
    }

    let mut pending_tools = BTreeMap::<String, (u128, String)>::new();
    for event in events {
        match &event.event {
            JournalEventKind::ToolCalled { call } => {
                pending_tools.insert(
                    call.id.clone(),
                    (event.timestamp_unix_ms, call.name.clone()),
                );
            }
            JournalEventKind::ToolObserved { observation } => {
                let (start, tool_name) = pending_tools
                    .remove(&observation.call_id)
                    .unwrap_or((event.timestamp_unix_ms, observation.tool.clone()));
                let mut attributes = BTreeMap::new();
                attributes.insert("gen_ai.operation.name".into(), "execute_tool".into());
                attributes.insert("gen_ai.tool.name".into(), tool_name.clone());
                attributes.insert("gen_ai.tool.call_id".into(), observation.call_id.clone());
                attributes.insert(
                    "gen_ai.tool.status".into(),
                    format!("{:?}", observation.status),
                );
                let span_id = if tool_name == "spawn_agent" {
                    spawn_span_id(&first.run_id.to_string(), &observation.call_id)
                } else {
                    let span_id = format!("{next_span:016x}");
                    next_span += 1;
                    span_id
                };
                spans.push(OtelSpan {
                    trace_id: trace_id.clone(),
                    span_id,
                    parent_span_id: Some(root_span_id.clone()),
                    name: format!("execute_tool {tool_name}"),
                    kind: SpanKind::Client,
                    start_unix_nano: ms_to_nano(start),
                    end_unix_nano: ms_to_nano(event.timestamp_unix_ms),
                    status: if matches!(observation.status, crate::domain::ToolStatus::Ok) {
                        SpanStatus::Ok
                    } else {
                        SpanStatus::Error
                    },
                    attributes,
                });
            }
            _ => {}
        }
    }

    spans
}

fn status_for_reason(reason: StopReason) -> SpanStatus {
    match reason {
        StopReason::Completed => SpanStatus::Ok,
        StopReason::NeedsAuthorization | StopReason::NeedsEvidence => SpanStatus::Unset,
        _ => SpanStatus::Error,
    }
}

fn ms_to_nano(ms: u128) -> u128 {
    ms.saturating_mul(1_000_000)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::{
        CognitiveMode, ModelResponse, ModelTurn, PROTOCOL_VERSION, TaskRequest, ToolCall,
        ToolObservation, ToolStatus, Usage,
    };
    use crate::journal::ParentRunLink;
    use uuid::Uuid;

    fn event(
        run_id: Uuid,
        seq: u64,
        timestamp_unix_ms: u128,
        kind: JournalEventKind,
    ) -> JournalEvent {
        JournalEvent {
            schema_version: PROTOCOL_VERSION,
            run_id,
            seq,
            timestamp_unix_ms,
            event: kind,
        }
    }

    #[test]
    fn empty_events_emit_no_spans() {
        assert!(spans_from_events(&[]).is_empty());
    }

    #[test]
    fn trace_id_is_stable_for_a_run() {
        let run_id = Uuid::parse_str("00000000-0000-0000-0000-00000000002a").unwrap();
        assert_eq!(
            format!("{:032x}", run_id.as_u128()),
            "0000000000000000000000000000002a"
        );
    }

    #[test]
    fn child_root_span_joins_the_parent_spawn_span_in_one_trace() {
        let parent_run = Uuid::new_v4();
        let child_run = Uuid::new_v4();
        let parent_events = vec![
            event(
                parent_run,
                0,
                10,
                JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("parent", PathBuf::from(".")),
                    mode: CognitiveMode::DirectAnswer,
                },
            ),
            event(
                parent_run,
                1,
                11,
                JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: "call-1".into(),
                        name: "spawn_agent".into(),
                        arguments: serde_json::json!({}),
                    },
                },
            ),
            event(
                parent_run,
                2,
                12,
                JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id: "call-1".into(),
                        tool: "spawn_agent".into(),
                        status: ToolStatus::Ok,
                        summary: "ok".into(),
                        content: String::new(),
                        error_code: None,
                        metadata: BTreeMap::new(),
                    },
                },
            ),
            event(
                parent_run,
                3,
                13,
                JournalEventKind::RunStopped {
                    reason: StopReason::Completed,
                },
            ),
        ];
        let child_events = vec![
            event(
                child_run,
                0,
                20,
                JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("child", PathBuf::from(".")),
                    mode: CognitiveMode::DirectAnswer,
                },
            ),
            event(
                child_run,
                1,
                21,
                JournalEventKind::ParentRunLinked {
                    link: ParentRunLink {
                        parent_run_id: parent_run.to_string(),
                        parent_session_id: Some("session-1".into()),
                        spawn_call_id: "call-1".into(),
                    },
                },
            ),
            event(
                child_run,
                2,
                22,
                JournalEventKind::ModelResponded {
                    response: ModelResponse {
                        provider_id: "fake".into(),
                        model: "fake-model".into(),
                        turn: ModelTurn::ToolCalls { calls: Vec::new() },
                        usage: Usage::default(),
                    },
                },
            ),
            event(
                child_run,
                3,
                23,
                JournalEventKind::RunStopped {
                    reason: StopReason::Completed,
                },
            ),
        ];

        let parent_spans = spans_from_events(&parent_events);
        let child_spans = spans_from_events(&child_events);
        let spawn = parent_spans
            .iter()
            .find(|span| span.name == "execute_tool spawn_agent")
            .expect("parent spawn span");
        assert_eq!(
            spawn.span_id,
            spawn_span_id(&parent_run.to_string(), "call-1")
        );
        let child_root = child_spans
            .iter()
            .find(|span| span.name == "invoke_agent 1Agent")
            .expect("child root span");
        assert_eq!(
            child_root.trace_id,
            format!("{:032x}", parent_run.as_u128())
        );
        assert_eq!(
            child_root.parent_span_id.as_deref(),
            Some(spawn.span_id.as_str())
        );
        assert_ne!(child_root.span_id, "0000000000000001");
        assert_eq!(
            child_root.attributes.get("gen_ai.conversation.id"),
            Some(&"session-1".to_string())
        );
        let chat = child_spans
            .iter()
            .find(|span| span.name.starts_with("chat "))
            .expect("child chat span");
        assert_eq!(chat.trace_id, child_root.trace_id);
        assert_eq!(
            chat.parent_span_id.as_deref(),
            Some(child_root.span_id.as_str())
        );
        assert_eq!(
            chat.attributes.get("gen_ai.conversation.id"),
            Some(&"session-1".to_string())
        );
    }
}
