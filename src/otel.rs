//! Minimal OpenTelemetry GenAI-style span projection over the run journal.
//!
//! This emits stable `gen_ai.*` attributes without adding an OTel SDK. A host
//! can forward the JSON spans to its own collector.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

pub fn spans_from_events(events: &[JournalEvent]) -> Vec<OtelSpan> {
    let Some(first) = events.first() else {
        return Vec::new();
    };
    let trace_id = format!("{:032x}", first.run_id.as_u128());
    let root_span_id = "0000000000000001".to_string();
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
    root_attributes.insert("gen_ai.conversation.id".into(), first.run_id.to_string());
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
        parent_span_id: None,
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
                attributes.insert("gen_ai.conversation.id".into(), event.run_id.to_string());
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
                spans.push(OtelSpan {
                    trace_id: trace_id.clone(),
                    span_id: format!("{next_span:016x}"),
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
                next_span += 1;
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
    use super::*;
    use uuid::Uuid;

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
}
