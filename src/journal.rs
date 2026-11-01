use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    AgentAnswer, CognitiveMode, InjectedExperience, ModelRequest, ModelResponse, PROTOCOL_VERSION,
    RuntimeCheckpoint, StopReason, TaskRequest, ToolCall, ToolObservation,
};
use crate::router::{RouteObjective, RoutePlan, RoutePolicySources};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JournalEvent {
    pub schema_version: u32,
    pub run_id: Uuid,
    pub seq: u64,
    pub timestamp_unix_ms: u128,
    pub event: JournalEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum JournalEventKind {
    RunStarted {
        request: TaskRequest,
        mode: CognitiveMode,
    },
    ClientRequestLinked {
        client_request_id: String,
    },
    ResumeStarted {
        original_run_id: Uuid,
        checkpoint_seq: u64,
        request: TaskRequest,
        checkpoint: RuntimeCheckpoint,
    },
    RouteSelected {
        provider_id: String,
        model: String,
        reason: String,
    },
    RoutePlanned {
        plan: RoutePlan,
        sources: RoutePolicySources,
    },
    ModelRequested {
        request: ModelRequest,
    },
    ModelResponded {
        response: ModelResponse,
    },
    ToolCalled {
        call: ToolCall,
    },
    ToolObserved {
        observation: ToolObservation,
    },
    ContextCompacted {
        before_chars: usize,
        after_chars: usize,
        compacted_tool_results: usize,
    },
    Checkpoint {
        checkpoint: RuntimeCheckpoint,
    },
    ExperienceInjected {
        injected: Vec<InjectedExperience>,
    },
    AnswerProduced {
        answer: AgentAnswer,
    },
    RunStopped {
        reason: StopReason,
    },
}

pub struct Journal {
    run_id: Uuid,
    next_seq: u64,
    path: PathBuf,
    writer: BufWriter<File>,
}

impl Journal {
    pub fn create(path: impl AsRef<Path>, run_id: Uuid) -> std::io::Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(path.as_ref())?;
        Ok(Self {
            run_id,
            next_seq: 0,
            path: path.as_ref().to_path_buf(),
            writer: BufWriter::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&mut self, event: JournalEventKind) -> std::io::Result<u64> {
        let seq = self.next_seq;
        let envelope = JournalEvent {
            schema_version: PROTOCOL_VERSION,
            run_id: self.run_id,
            seq,
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            event,
        };
        serde_json::to_writer(&mut self.writer, &envelope)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        self.next_seq += 1;
        Ok(seq)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JournalAudit {
    pub ok: bool,
    pub event_count: usize,
    pub run_id: Option<Uuid>,
    pub terminal_reason: Option<StopReason>,
    pub unresolved_tool_calls: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReplayState {
    pub run_id: Uuid,
    pub request: Option<TaskRequest>,
    pub mode: Option<CognitiveMode>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub route_plan: Option<RoutePlan>,
    pub route_policy_sources: Option<RoutePolicySources>,
    pub answer: Option<AgentAnswer>,
    pub stop_reason: Option<StopReason>,
    pub tool_calls: Vec<ToolCall>,
    pub observations: Vec<ToolObservation>,
    pub checkpoint: Option<RuntimeCheckpoint>,
    pub client_request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteTraceSummary {
    pub mode: CognitiveMode,
    pub objective: RouteObjective,
    pub candidates: usize,
    pub rejected: usize,
    pub selected_provider_id: Option<String>,
    pub selected_model: Option<String>,
    pub selection_reason: Option<String>,
    pub fallback_attempts: usize,
    pub policy_sources: RoutePolicySources,
}

impl RouteTraceSummary {
    pub fn render(&self) -> String {
        let mut output = format!(
            "路由：mode={:?} objective={:?} candidates={} rejected={} fallback_attempts={}",
            self.mode, self.objective, self.candidates, self.rejected, self.fallback_attempts
        );
        if let (Some(provider_id), Some(model)) = (&self.selected_provider_id, &self.selected_model)
        {
            output.push_str(&format!("\n路由选择：{provider_id}/{model}"));
        }
        if let Some(reason) = &self.selection_reason {
            output.push_str(&format!("\n路由原因：{}", bounded_chars(reason, 240)));
        }
        output.push_str(&format!(
            "\n策略来源：objective={:?} latency={:?} unit_cost={:?}",
            self.policy_sources.objective,
            self.policy_sources.latency_budget,
            self.policy_sources.unit_cost_budget
        ));
        output
    }
}

pub fn summarize_route_trace(events: &[JournalEvent]) -> Option<RouteTraceSummary> {
    let mut route_plan = None;
    let mut selected = None;
    let mut sources = None;
    let mut fallback_attempts = 0;
    let mut first_attempt_phase = true;
    for event in events {
        match &event.event {
            JournalEventKind::RoutePlanned {
                plan,
                sources: value,
            } => {
                route_plan = Some(plan);
                sources = Some(value);
            }
            JournalEventKind::RouteSelected {
                provider_id,
                model,
                reason,
            } if selected.is_none() => {
                selected = Some((provider_id, model, reason));
                fallback_attempts = 1;
            }
            JournalEventKind::RouteSelected { .. } if first_attempt_phase => {
                fallback_attempts += 1;
            }
            JournalEventKind::ModelResponded { .. } => {
                first_attempt_phase = false;
            }
            _ => {}
        }
    }
    let plan = route_plan?;
    let sources = sources?.clone();
    Some(RouteTraceSummary {
        mode: plan.request.mode,
        objective: plan.request.objective,
        candidates: plan.ranked.len(),
        rejected: plan.rejected.len(),
        selected_provider_id: selected.map(|value| value.0.clone()),
        selected_model: selected.map(|value| value.1.clone()),
        selection_reason: selected.map(|value| value.2.clone()),
        fallback_attempts,
        policy_sources: sources,
    })
}

fn bounded_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.into();
    }
    let mut bounded: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    bounded.push('…');
    bounded
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunHistoryEntry {
    pub run_id: Option<Uuid>,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_unix_ms: u128,
    pub stop_reason: Option<StopReason>,
    pub client_request_id: Option<String>,
}

pub fn scan_history(
    journal_dir: impl AsRef<Path>,
    limit: usize,
) -> Result<Vec<RunHistoryEntry>, std::io::Error> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(journal_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let markers = read_history_markers(&path)?;
        let modified_unix_ms = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .unwrap_or_default()
            .as_millis();
        entries.push(RunHistoryEntry {
            run_id: markers.run_id,
            path,
            size_bytes: metadata.len(),
            modified_unix_ms,
            stop_reason: markers.stop_reason,
            client_request_id: markers.client_request_id,
        });
    }
    entries.sort_by(|left, right| {
        right
            .modified_unix_ms
            .cmp(&left.modified_unix_ms)
            .then_with(|| right.path.cmp(&left.path))
    });
    entries.truncate(limit.min(200));
    Ok(entries)
}

struct HistoryMarkers {
    run_id: Option<Uuid>,
    stop_reason: Option<StopReason>,
    client_request_id: Option<String>,
}

fn read_history_markers(path: &Path) -> Result<HistoryMarkers, std::io::Error> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let head_window = length.min(65_536);
    file.seek(SeekFrom::Start(0))?;
    let mut head = Vec::with_capacity(head_window as usize);
    Read::by_ref(&mut file)
        .take(head_window)
        .read_to_end(&mut head)?;
    let head_text = String::from_utf8_lossy(&head);
    let mut client_request_id = None;
    for line in head_text.lines() {
        let Ok(event) = serde_json::from_str::<JournalEvent>(line) else {
            continue;
        };
        if let JournalEventKind::ClientRequestLinked {
            client_request_id: value,
        } = event.event
        {
            client_request_id = Some(value);
            break;
        }
    }

    file.seek(SeekFrom::Start(length.saturating_sub(length.min(65_536))))?;
    let window = length.min(65_536);
    let mut bytes = Vec::with_capacity(window as usize);
    Read::by_ref(&mut file)
        .take(window)
        .read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut run_id = None;
    let mut stop_reason = None;
    for line in text.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<JournalEvent>(line) else {
            continue;
        };
        run_id = Some(event.run_id);
        if let JournalEventKind::RunStopped { reason } = event.event {
            stop_reason = Some(reason);
        }
        break;
    }
    Ok(HistoryMarkers {
        run_id,
        stop_reason,
        client_request_id,
    })
}

pub fn read_events(path: impl AsRef<Path>) -> Result<Vec<JournalEvent>, JournalReadError> {
    let file = File::open(path).map_err(JournalReadError::Io)?;
    let mut events = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(JournalReadError::Io)?;
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str(&line).map_err(|source| JournalReadError::Json {
            line: index + 1,
            source,
        })?;
        events.push(event);
    }
    Ok(events)
}

pub fn audit_events(events: &[JournalEvent]) -> JournalAudit {
    let mut issues = Vec::new();
    let run_id = events.first().map(|event| event.run_id);
    let mut observed_calls = std::collections::BTreeSet::new();
    let mut answered_calls = std::collections::BTreeSet::new();
    let mut terminal_reason = None;
    let mut request = None;
    let mut answer = None;
    let mut current_route_plan: Option<RoutePlan> = None;

    for (index, event) in events.iter().enumerate() {
        if event.schema_version != PROTOCOL_VERSION {
            issues.push(format!(
                "event {index}: unsupported schema version {}",
                event.schema_version
            ));
        }
        if event.seq != index as u64 {
            issues.push(format!(
                "event {index}: expected seq {index}, got {}",
                event.seq
            ));
        }
        if Some(event.run_id) != run_id {
            issues.push(format!("event {index}: run_id changed"));
        }
        match &event.event {
            JournalEventKind::RunStarted {
                request: started_request,
                ..
            }
            | JournalEventKind::ResumeStarted {
                request: started_request,
                ..
            } => {
                if request.replace(started_request.clone()).is_some() {
                    issues.push(format!("event {index}: duplicate run start"));
                }
            }
            JournalEventKind::ToolCalled { call } => {
                if !observed_calls.insert(call.id.clone()) {
                    issues.push(format!("event {index}: duplicate tool call {}", call.id));
                }
            }
            JournalEventKind::ToolObserved { observation } => {
                if !answered_calls.insert(observation.call_id.clone()) {
                    issues.push(format!(
                        "event {index}: duplicate observation for {}",
                        observation.call_id
                    ));
                }
            }
            JournalEventKind::RoutePlanned { plan, .. } => {
                current_route_plan = Some(plan.clone());
            }
            JournalEventKind::RouteSelected {
                provider_id, model, ..
            } => {
                if let Some(plan) = &current_route_plan
                    && !plan.ranked.iter().any(|candidate| {
                        candidate.manifest.provider_id == *provider_id
                            && candidate.manifest.model == *model
                    })
                {
                    issues.push(format!(
                        "event {index}: selected route {provider_id}/{model} is not in the route plan"
                    ));
                }
            }
            JournalEventKind::RunStopped { reason }
                if terminal_reason.replace(*reason).is_some() =>
            {
                issues.push(format!("event {index}: duplicate run stop"));
            }
            JournalEventKind::AnswerProduced { answer: produced }
                if answer.replace(produced.clone()).is_some() =>
            {
                issues.push(format!("event {index}: duplicate answer"));
            }
            _ => {}
        }
    }

    if events.is_empty() {
        issues.push("journal is empty".into());
    }
    if !events.is_empty()
        && !matches!(
            events.first().map(|event| &event.event),
            Some(JournalEventKind::RunStarted { .. } | JournalEventKind::ResumeStarted { .. })
        )
    {
        issues.push("first event is not run_started".into());
    }

    let unresolved_tool_calls = observed_calls
        .difference(&answered_calls)
        .cloned()
        .collect();
    for call_id in &unresolved_tool_calls {
        issues.push(format!("tool call {call_id} has no observation"));
    }
    if !events.is_empty() && terminal_reason.is_none() {
        issues.push("journal has no run stop event".into());
    }
    if let (Some(request), Some(answer)) = (&request, &answer)
        && let Err(error) = answer.validate(request)
    {
        issues.push(format!("answer validation failed: {error}"));
    }
    match (terminal_reason, answer.as_ref().map(|answer| answer.status)) {
        (Some(StopReason::Completed), Some(crate::domain::AnswerStatus::Completed))
        | (Some(StopReason::Completed), Some(crate::domain::AnswerStatus::Conditional))
        | (Some(StopReason::NeedsEvidence), Some(crate::domain::AnswerStatus::NeedsEvidence))
        | (
            Some(StopReason::NeedsAuthorization),
            Some(crate::domain::AnswerStatus::NeedsAuthorization),
        )
        | (Some(StopReason::InvalidOutput), Some(crate::domain::AnswerStatus::Failed)) => {}
        (Some(reason), Some(status)) => issues.push(format!(
            "stop reason {reason:?} does not match answer status {status:?}"
        )),
        (Some(StopReason::Completed), None)
        | (Some(StopReason::NeedsEvidence), None)
        | (Some(StopReason::NeedsAuthorization), None) => {
            issues.push("successful or user-action stop has no answer".into());
        }
        _ => {}
    }

    JournalAudit {
        ok: issues.is_empty(),
        event_count: events.len(),
        run_id,
        terminal_reason,
        unresolved_tool_calls,
        issues,
    }
}

pub fn replay_events(events: &[JournalEvent]) -> Result<ReplayState, String> {
    let run_id = events
        .first()
        .map(|event| event.run_id)
        .ok_or_else(|| "journal is empty".to_string())?;
    let mut state = ReplayState {
        run_id,
        request: None,
        mode: None,
        provider_id: None,
        model: None,
        route_plan: None,
        route_policy_sources: None,
        answer: None,
        stop_reason: None,
        tool_calls: Vec::new(),
        observations: Vec::new(),
        checkpoint: None,
        client_request_id: None,
    };

    for event in events {
        match &event.event {
            JournalEventKind::RunStarted { request, mode } => {
                state.request = Some(request.clone());
                state.mode = Some(*mode);
            }
            JournalEventKind::ClientRequestLinked {
                client_request_id: value,
            } => state.client_request_id = Some(value.clone()),
            JournalEventKind::ResumeStarted {
                request,
                checkpoint,
                ..
            } => {
                state.request = Some(request.clone());
                state.mode = Some(checkpoint.mode);
                state.checkpoint = Some(checkpoint.clone());
            }
            JournalEventKind::RouteSelected {
                provider_id, model, ..
            } => {
                state.provider_id = Some(provider_id.clone());
                state.model = Some(model.clone());
            }
            JournalEventKind::RoutePlanned { plan, sources } => {
                state.route_plan = Some(plan.clone());
                state.route_policy_sources = Some(sources.clone());
            }
            JournalEventKind::ToolCalled { call } => state.tool_calls.push(call.clone()),
            JournalEventKind::ToolObserved { observation } => {
                state.observations.push(observation.clone())
            }
            JournalEventKind::Checkpoint { checkpoint } => {
                state.checkpoint = Some(checkpoint.clone())
            }
            JournalEventKind::AnswerProduced { answer } => state.answer = Some(answer.clone()),
            JournalEventKind::RunStopped { reason } => state.stop_reason = Some(*reason),
            JournalEventKind::ContextCompacted { .. }
            | JournalEventKind::ExperienceInjected { .. }
            | JournalEventKind::ModelRequested { .. }
            | JournalEventKind::ModelResponded { .. } => {}
        }
    }

    Ok(state)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ResumablePrefix {
    pub original_run_id: Uuid,
    pub checkpoint_seq: u64,
    pub request: TaskRequest,
    pub checkpoint: RuntimeCheckpoint,
}

pub fn find_resumable_prefix(events: &[JournalEvent]) -> Result<ResumablePrefix, ResumeError> {
    if events.is_empty() {
        return Err(ResumeError::Empty);
    }
    let original_run_id = events[0].run_id;
    let mut request = None;
    let mut checkpoint = None;
    let mut checkpoint_seq = 0;
    let mut calls = std::collections::BTreeSet::new();
    let mut observations = std::collections::BTreeSet::new();

    for (index, event) in events.iter().enumerate() {
        if event.seq != index as u64 || event.run_id != original_run_id {
            return Err(ResumeError::InvalidSequenceOrIdentity);
        }
        match &event.event {
            JournalEventKind::RunStarted {
                request: value,
                mode,
            } => {
                request = Some(value.clone());
                if *mode == CognitiveMode::Execute && !value.permissions.allow_write {
                    return Err(ResumeError::UnsafeExecuteMode);
                }
            }
            JournalEventKind::ResumeStarted {
                request: value,
                checkpoint: value_checkpoint,
                ..
            } => {
                request = Some(value.clone());
                checkpoint = Some(value_checkpoint.clone());
                checkpoint_seq = event.seq;
            }
            JournalEventKind::ToolCalled { call } => {
                if !calls.insert(call.id.clone()) {
                    return Err(ResumeError::InvalidToolPairing);
                }
            }
            JournalEventKind::ToolObserved { observation } => {
                if !observations.insert(observation.call_id.clone()) {
                    return Err(ResumeError::InvalidToolPairing);
                }
            }
            JournalEventKind::Checkpoint { checkpoint: value } => {
                checkpoint = Some(value.clone());
                checkpoint_seq = event.seq;
            }
            JournalEventKind::RunStopped { .. } | JournalEventKind::AnswerProduced { .. } => {
                return Err(ResumeError::AlreadyTerminal);
            }
            _ => {}
        }
    }
    if calls != observations {
        return Err(ResumeError::InvalidToolPairing);
    }
    Ok(ResumablePrefix {
        original_run_id,
        checkpoint_seq,
        request: request.ok_or(ResumeError::MissingRunStart)?,
        checkpoint: checkpoint.ok_or(ResumeError::NoCheckpoint)?,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum JournalReadError {
    #[error("journal I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("journal line {line} is invalid: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResumeError {
    #[error("journal is empty")]
    Empty,
    #[error("journal sequence or run identity is invalid")]
    InvalidSequenceOrIdentity,
    #[error("run is already terminal")]
    AlreadyTerminal,
    #[error("tool calls and observations are not paired")]
    InvalidToolPairing,
    #[error("run start event is missing")]
    MissingRunStart,
    #[error("no checkpoint is available")]
    NoCheckpoint,
    #[error("execute-mode run is not safely resumable")]
    UnsafeExecuteMode,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AnswerStatus, ToolStatus};
    use crate::router::{
        PolicyValueSource, RouteObjective, RoutePlan, RoutePolicySources, RouteRequest,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn answer() -> AgentAnswer {
        AgentAnswer {
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        }
    }

    #[test]
    fn journal_replays_and_pairs_tools() {
        let dir = tempfile::tempdir().unwrap();
        let run_id = Uuid::new_v4();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, run_id).unwrap();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        journal
            .append(JournalEventKind::RunStarted {
                request,
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        let call = ToolCall {
            id: "call-1".into(),
            name: "read_file".into(),
            arguments: serde_json::json!({"path":"x"}),
        };
        journal
            .append(JournalEventKind::ToolCalled { call: call.clone() })
            .unwrap();
        journal
            .append(JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: call.id,
                    tool: call.name,
                    status: ToolStatus::Ok,
                    summary: "ok".into(),
                    content: "content".into(),
                    error_code: None,
                    metadata: BTreeMap::new(),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced { answer: answer() })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(journal);

        let events = read_events(&path).unwrap();
        let audit = audit_events(&events);
        assert!(audit.ok, "{:?}", audit.issues);
        assert_eq!(
            find_resumable_prefix(&events),
            Err(ResumeError::AlreadyTerminal)
        );
        let state = replay_events(&events).unwrap();
        assert_eq!(state.tool_calls.len(), 1);
        assert_eq!(state.observations.len(), 1);
        assert_eq!(state.stop_reason, Some(StopReason::Completed));
    }

    #[test]
    fn audit_detects_unpaired_tool_call() {
        let run_id = Uuid::new_v4();
        let events = vec![
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 0,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("answer", PathBuf::from(".")),
                    mode: CognitiveMode::DirectAnswer,
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 1,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: "call-1".into(),
                        name: "read_file".into(),
                        arguments: serde_json::json!({}),
                    },
                },
            },
        ];
        let audit = audit_events(&events);
        assert!(!audit.ok);
        assert_eq!(audit.unresolved_tool_calls, vec!["call-1"]);
        assert!(audit.terminal_reason.is_none());
    }

    #[test]
    fn audit_rejects_route_selection_outside_the_plan() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let plan = RoutePlan {
            request: RouteRequest {
                mode: CognitiveMode::DirectAnswer,
                risk: crate::domain::RiskLevel::Low,
                requires_tools: false,
                requires_structured_output: true,
                allow_remote: false,
                objective: RouteObjective::Quality,
                max_provider_p95_latency_ms: None,
                max_unit_cost_micros_per_k: None,
            },
            ranked: Vec::new(),
            rejected: Vec::new(),
        };
        let events = vec![
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 0,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStarted {
                    request,
                    mode: CognitiveMode::DirectAnswer,
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 1,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RoutePlanned {
                    plan,
                    sources: RoutePolicySources {
                        objective: PolicyValueSource::RuntimeDefault,
                        latency_budget: PolicyValueSource::Unset,
                        unit_cost_budget: PolicyValueSource::Unset,
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RouteSelected {
                    provider_id: "unplanned".into(),
                    model: "model".into(),
                    reason: "tampered".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 3,
                timestamp_unix_ms: 0,
                event: JournalEventKind::AnswerProduced { answer: answer() },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 4,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStopped {
                    reason: StopReason::Completed,
                },
            },
        ];
        let audit = audit_events(&events);
        assert!(!audit.ok);
        assert!(
            audit
                .issues
                .iter()
                .any(|issue| issue.contains("is not in the route plan"))
        );
    }

    #[test]
    fn audit_rejects_unknown_claim_references() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let events = vec![
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 0,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStarted {
                    request: request.clone(),
                    mode: CognitiveMode::DirectAnswer,
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 1,
                timestamp_unix_ms: 0,
                event: JournalEventKind::AnswerProduced {
                    answer: AgentAnswer {
                        status: AnswerStatus::Completed,
                        conclusion: "done".into(),
                        claims: Vec::new(),
                        evidence: vec![crate::domain::Evidence {
                            source: "fixture".into(),
                            statement: "unsupported".into(),
                            claim_ids: vec!["missing".into()],
                        }],
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        flip_conditions: Vec::new(),
                        action_receipts: Vec::new(),
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStopped {
                    reason: StopReason::Completed,
                },
            },
        ];
        let audit = audit_events(&events);
        assert!(!audit.ok);
        assert!(
            audit
                .issues
                .iter()
                .any(|issue| issue.contains("unknown claim id 'missing'"))
        );
    }

    #[test]
    fn history_scans_tail_without_loading_full_journal() {
        let dir = tempfile::tempdir().unwrap();
        let run_id = Uuid::new_v4();
        let path = dir.path().join("completed.jsonl");
        let mut journal = Journal::create(&path, run_id).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("answer", PathBuf::from(".")),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(JournalEventKind::ClientRequestLinked {
                client_request_id: "client-1".into(),
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::BudgetExhausted,
            })
            .unwrap();
        drop(journal);
        let history = scan_history(dir.path(), 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].run_id, Some(run_id));
        assert_eq!(history[0].stop_reason, Some(StopReason::BudgetExhausted));
        assert_eq!(history[0].client_request_id.as_deref(), Some("client-1"));
    }
}
