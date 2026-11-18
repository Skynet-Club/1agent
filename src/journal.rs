use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    AgentAnswer, CognitiveMode, InjectedExperience, ModelMessage, ModelRequest, ModelResponse,
    PROTOCOL_VERSION, RuntimeCheckpoint, RuntimeCheckpointDelta, StopReason, TaskRequest, ToolCall,
    ToolObservation, ToolSpec,
};
use crate::provider::{ProviderError, ProviderErrorCode};
use crate::reframe::{ReframeChoice, ReframeProposal};
use crate::router::{RouteObjective, RoutePlan, RoutePolicySources};
use crate::tools::ToolCapabilitySnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JournalEvent {
    pub schema_version: u32,
    pub run_id: Uuid,
    pub seq: u64,
    pub timestamp_unix_ms: u128,
    pub event: JournalEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ParentRunLink {
    pub parent_run_id: String,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    pub spawn_call_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderFailure {
    pub provider_id: String,
    pub model: String,
    pub code: ProviderErrorCode,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryOutcome {
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetryTiming {
    pub delay_ms: u64,
    pub elapsed_ms: u64,
    pub outcome: RetryOutcome,
}

impl ProviderFailure {
    pub fn from_error(
        provider_id: impl Into<String>,
        model: impl Into<String>,
        error: &ProviderError,
    ) -> Self {
        let mut message = error
            .message
            .chars()
            .map(|character| {
                if crate::safety::is_disallowed_control(character) {
                    '�'
                } else {
                    character
                }
            })
            .take(511)
            .collect::<String>();
        if error.message.chars().count() > 511 {
            message.push('…');
        }
        Self {
            provider_id: provider_id.into(),
            model: model.into(),
            code: error.code,
            retryable: error.retryable,
            message,
        }
    }
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
    /// A child run records the parent run and spawn call that created it.
    ParentRunLinked {
        link: ParentRunLink,
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
    ToolCapabilitiesAdvertised {
        snapshot: ToolCapabilitySnapshot,
    },
    /// The runtime's preflight resolution, recorded so the selected
    /// capability path is replayable and can feed path memory.
    CapabilityPreflightResolved {
        resolution: crate::capability_compiler::CapabilityResolution,
    },
    ModelRequested {
        request: ModelRequest,
    },
    /// Compact follow-up to the previous `ModelRequested`/`ModelRequestDelta`.
    /// Only the messages after `prefix_len` and changed tools are stored.
    ModelRequestDelta {
        delta: ModelRequestDelta,
    },
    /// First content fragment observed for a streaming provider attempt.
    StreamFirstToken {
        provider_id: String,
        model: String,
        elapsed_ms: u64,
    },
    ModelResponded {
        response: ModelResponse,
    },
    ProviderFailed {
        failure: ProviderFailure,
    },
    ProviderRetryScheduled {
        delay_ms: u64,
        next_provider_id: String,
        next_model: String,
    },
    ProviderRetryElapsed {
        timing: RetryTiming,
    },
    ToolCalled {
        call: ToolCall,
    },
    /// The model asked for tools that were not in the current request. The
    /// runtime recorded which names it granted or denied before the next
    /// model request.
    ToolsRequested {
        requested: Vec<String>,
        granted: Vec<String>,
        denied: Vec<ToolRequestDenial>,
        reason: String,
    },
    ToolObserved {
        observation: ToolObservation,
    },
    ContextCompacted {
        #[serde(default)]
        before_tokens: usize,
        #[serde(default)]
        after_tokens: usize,
        #[serde(default)]
        before_chars: usize,
        #[serde(default)]
        after_chars: usize,
        compacted_tool_results: usize,
    },
    Checkpoint {
        checkpoint: RuntimeCheckpoint,
    },
    CheckpointDelta {
        checkpoint: RuntimeCheckpointDelta,
    },
    ExperienceInjected {
        injected: Vec<InjectedExperience>,
    },
    AnswerProduced {
        answer: AgentAnswer,
    },
    /// A completed draft failed the evidence check and the model gets another
    /// bounded chance to gather evidence or downgrade the status.
    AnswerVerificationFailed {
        reason: String,
        round: usize,
    },
    /// The bounded evidence check still failed; the answer is downgraded to
    /// `needs_evidence` instead of being presented as complete.
    AnswerVerificationExhausted {
        reason: String,
        rounds: usize,
    },
    /// Side-effect tools ran but the completed answer has no accepted
    /// acceptance receipt; the runtime downgraded it to `conditional`.
    CompletionAcceptanceMissing {
        tools: Vec<String>,
        reason: String,
    },
    /// A delivered HTML artifact still contains unfinished placeholder text.
    CompletionArtifactUnfinished {
        paths: Vec<String>,
        markers: Vec<String>,
    },
    /// A page acceptance contract omitted mandatory responsive/design checks.
    CompletionDesignContractGap {
        missing: Vec<String>,
    },
    ReframeProposed {
        proposal: ReframeProposal,
        provider_id: String,
        model: String,
        prompt_sha256: String,
    },
    ReframeDecided {
        choice: ReframeChoice,
        actor: String,
        decided_at_unix_ms: u64,
    },
    RunStopped {
        reason: StopReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolRequestDenial {
    pub name: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModelRequestDelta {
    /// Seq of the previous `ModelRequested` or `ModelRequestDelta` event.
    pub base_seq: u64,
    /// Number of leading messages inherited unchanged from the base request.
    pub prefix_len: usize,
    /// Messages appended (or replacing the base tail) after `prefix_len`.
    pub messages: Vec<ModelMessage>,
    /// Full tool list when it changed; `None` inherits the base tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolSpec>>,
}

/// Writes full model requests the first time and deltas afterwards.
#[derive(Debug, Clone, Default)]
pub struct ModelRequestLog {
    last: Option<(u64, ModelRequest)>,
}

impl ModelRequestLog {
    pub fn append(&mut self, journal: &mut Journal, request: ModelRequest) -> std::io::Result<u64> {
        let seq = match &self.last {
            Some((base_seq, previous))
                if previous.run_id == request.run_id && previous.mode == request.mode =>
            {
                let prefix_len = previous
                    .messages
                    .iter()
                    .zip(request.messages.iter())
                    .take_while(|(left, right)| left == right)
                    .count();
                let tools = (previous.tools != request.tools).then(|| request.tools.clone());
                let delta = ModelRequestDelta {
                    base_seq: *base_seq,
                    prefix_len,
                    messages: request.messages[prefix_len..].to_vec(),
                    tools,
                };
                journal.append(JournalEventKind::ModelRequestDelta { delta })?
            }
            _ => journal.append(JournalEventKind::ModelRequested {
                request: request.clone(),
            })?,
        };
        self.last = Some((seq, request));
        Ok(seq)
    }
}

/// Rebuilds the model-visible request from full events and deltas.
#[derive(Debug, Clone, Default)]
pub struct ModelRequestChain {
    current: Option<ModelRequest>,
    base_seq: Option<u64>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ModelRequestChainError {
    #[error("model request delta has no base request")]
    MissingBase,
    #[error("model request delta base does not match the previous request")]
    InvalidBase,
    #[error("model request delta prefix exceeds the base request")]
    InvalidPrefix,
}

impl ModelRequestChain {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply an event; returns the reconstructed request for request events.
    pub fn apply(
        &mut self,
        event: &JournalEvent,
    ) -> Result<Option<&ModelRequest>, ModelRequestChainError> {
        match &event.event {
            JournalEventKind::ModelRequested { request } => {
                self.current = Some(request.clone());
                self.base_seq = Some(event.seq);
            }
            JournalEventKind::ModelRequestDelta { delta } => {
                let Some(base_seq) = self.base_seq else {
                    return Err(ModelRequestChainError::MissingBase);
                };
                if delta.base_seq != base_seq {
                    return Err(ModelRequestChainError::InvalidBase);
                }
                let Some(current) = self.current.as_mut() else {
                    return Err(ModelRequestChainError::MissingBase);
                };
                if delta.prefix_len > current.messages.len() {
                    return Err(ModelRequestChainError::InvalidPrefix);
                }
                current.messages.truncate(delta.prefix_len);
                current.messages.extend(delta.messages.iter().cloned());
                if let Some(tools) = &delta.tools {
                    current.tools = tools.clone();
                }
                self.base_seq = Some(event.seq);
            }
            _ => return Ok(None),
        }
        Ok(self.current.as_ref())
    }

    pub fn current(&self) -> Option<&ModelRequest> {
        self.current.as_ref()
    }
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
pub struct JournalAuditFinding {
    pub code: AuditFindingCode,
    pub event_index: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditFindingCode {
    UnsupportedProtocolVersion,
    NonSequentialEvent,
    RunIdChanged,
    DuplicateRunStart,
    DuplicateToolCall,
    DuplicateToolObservation,
    OrphanCheckpointDelta,
    OrphanModelRequestDelta,
    InvalidModelRequestDelta,
    RouteSelectionOutsidePlan,
    DuplicateRunStop,
    DuplicateAnswer,
    EmptyJournal,
    FirstEventNotRunStarted,
    UnpairedToolCall,
    MissingRunStop,
    AnswerValidationFailed,
    StopReasonMismatch,
    MissingAnswerForStop,
    ProviderFailureOutsideSelectedRoute,
    ProviderSucceededAndFailed,
    ProviderRetryOutsideRoutePlan,
    ProviderRetryTargetMismatch,
    ProviderRetryWithoutNextRoute,
    ProviderRetryDelayAboveCap,
    DuplicateProviderRetrySchedule,
    ProviderRetryOutcomeWithoutSchedule,
    ProviderRetryDelayMismatch,
    ProviderRetryElapsedAboveCap,
    ReframeProposalInvalid,
    ReframeDecisionWithoutProposal,
    ReframeDuplicateDecision,
    ReframeDecisionInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JournalAudit {
    pub ok: bool,
    pub event_count: usize,
    pub run_id: Option<Uuid>,
    pub terminal_reason: Option<StopReason>,
    #[serde(default)]
    pub answer_schema_version: Option<u32>,
    pub unresolved_tool_calls: Vec<String>,
    pub issues: Vec<String>,
    #[serde(default)]
    pub findings: Vec<JournalAuditFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReplayState {
    pub run_id: Uuid,
    pub request: Option<TaskRequest>,
    pub mode: Option<CognitiveMode>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub provider_failures: Vec<ProviderFailure>,
    #[serde(default)]
    pub retry_delays_ms: Vec<u64>,
    #[serde(default)]
    pub retry_timings: Vec<RetryTiming>,
    pub route_plan: Option<RoutePlan>,
    pub route_policy_sources: Option<RoutePolicySources>,
    pub tool_capabilities: Option<ToolCapabilitySnapshot>,
    pub answer: Option<AgentAnswer>,
    pub stop_reason: Option<StopReason>,
    pub tool_calls: Vec<ToolCall>,
    pub observations: Vec<ToolObservation>,
    pub checkpoint: Option<RuntimeCheckpoint>,
    pub client_request_id: Option<String>,
    #[serde(default)]
    pub parent_run_link: Option<ParentRunLink>,
    #[serde(default)]
    pub reframe_proposal: Option<ReframeProposal>,
    #[serde(default)]
    pub reframe_choice: Option<ReframeChoice>,
    #[serde(default)]
    pub reframe_actor: Option<String>,
    #[serde(default)]
    pub reframe_decided_at_unix_ms: Option<u64>,
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
    pub provider_failures: usize,
    pub failure_codes: Vec<ProviderErrorCode>,
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
            "\nProvider failures：count={} codes={:?}",
            self.provider_failures, self.failure_codes
        ));
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
    let mut provider_failures = 0;
    let mut failure_codes = Vec::new();
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
            JournalEventKind::ProviderFailed { failure } => {
                provider_failures += 1;
                if !failure_codes.contains(&failure.code) {
                    failure_codes.push(failure.code);
                }
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
        provider_failures,
        failure_codes,
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
    let mut reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut buffer = Vec::new();
    let mut line_number = 1;
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            break;
        }
        let line = buffer.strip_suffix(b"\n").unwrap_or(buffer.as_slice());
        if !line.iter().all(u8::is_ascii_whitespace) {
            let event = serde_json::from_slice(line).map_err(|source| JournalReadError::Json {
                line: line_number,
                source,
            })?;
            events.push(event);
        }
        line_number += 1;
    }
    Ok(events)
}

/// Byte-offset cursor for reading only events appended since the last read.
///
/// The cursor never consumes a line without a trailing newline, so a reader
/// that races a writer cannot parse half an event. `Journal::append` always
/// writes a newline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalCursor {
    offset: u64,
    line: usize,
}

impl Default for JournalCursor {
    fn default() -> Self {
        Self { offset: 0, line: 1 }
    }
}

impl JournalCursor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }
}

/// Parse complete, newline-terminated events starting at `cursor`.
///
/// Only the bytes consumed are advanced. A trailing partial line stays unread
/// and is parsed after the writer finishes it.
pub fn read_events_incremental(
    path: impl AsRef<Path>,
    cursor: &mut JournalCursor,
) -> Result<Vec<JournalEvent>, JournalReadError> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    if length < cursor.offset {
        return Err(JournalReadError::Truncated);
    }
    file.seek(SeekFrom::Start(cursor.offset))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let (events, consumed, lines) = parse_event_lines(&bytes, cursor.line, true)?;
    cursor.offset += consumed as u64;
    cursor.line += lines;
    Ok(events)
}

fn parse_event_lines(
    bytes: &[u8],
    first_line: usize,
    require_newline: bool,
) -> Result<(Vec<JournalEvent>, usize, usize), JournalReadError> {
    let mut events = Vec::new();
    let mut line_number = first_line;
    let mut lines = 0;
    let mut start = 0;
    while start < bytes.len() {
        let (line_end, has_newline) = match bytes[start..].iter().position(|byte| *byte == b'\n') {
            Some(offset) => (start + offset, true),
            None => (bytes.len(), false),
        };
        if !has_newline && require_newline {
            break;
        }
        let line = &bytes[start..line_end];
        if !line.iter().all(u8::is_ascii_whitespace) {
            let event = serde_json::from_slice(line).map_err(|source| JournalReadError::Json {
                line: line_number,
                source,
            })?;
            events.push(event);
        }
        line_number += 1;
        lines += 1;
        start = if has_newline { line_end + 1 } else { line_end };
    }
    Ok((events, start, lines))
}

pub fn audit_events(events: &[JournalEvent]) -> JournalAudit {
    let mut findings = Vec::new();
    let run_id = events.first().map(|event| event.run_id);
    let mut observed_calls = std::collections::BTreeMap::new();
    let mut answered_calls = std::collections::BTreeSet::new();
    let mut terminal_reason = None;
    let mut stop_event_index = None;
    let mut request = None;
    let mut answer = None;
    let mut answer_event_index = None;
    let mut current_route_plan: Option<RoutePlan> = None;
    let mut selected_route: Option<(String, String)> = None;
    let mut responded_route: Option<(String, String)> = None;
    let mut pending_retry: Option<(String, String, usize)> = None;
    let mut last_retry_schedule: Option<(u64, usize)> = None;
    let mut checkpoint_event_seq: Option<u64> = None;
    let mut model_request_base: Option<(u64, usize)> = None;

    for (index, event) in events.iter().enumerate() {
        if event.schema_version != PROTOCOL_VERSION {
            push_audit_finding(
                &mut findings,
                AuditFindingCode::UnsupportedProtocolVersion,
                Some(index),
                format!(
                    "event {index}: unsupported schema version {}",
                    event.schema_version
                ),
            );
        }
        if event.seq != index as u64 {
            push_audit_finding(
                &mut findings,
                AuditFindingCode::NonSequentialEvent,
                Some(index),
                format!("event {index}: expected seq {index}, got {}", event.seq),
            );
        }
        if Some(event.run_id) != run_id {
            push_audit_finding(
                &mut findings,
                AuditFindingCode::RunIdChanged,
                Some(index),
                format!("event {index}: run_id changed"),
            );
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
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::DuplicateRunStart,
                        Some(index),
                        format!("event {index}: duplicate run start"),
                    );
                }
                if matches!(&event.event, JournalEventKind::ResumeStarted { .. }) {
                    checkpoint_event_seq = Some(event.seq);
                }
                model_request_base = None;
            }
            JournalEventKind::Checkpoint { .. } => {
                checkpoint_event_seq = Some(event.seq);
            }
            JournalEventKind::CheckpointDelta { checkpoint } => {
                if checkpoint_event_seq != Some(checkpoint.base_seq) {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::OrphanCheckpointDelta,
                        Some(index),
                        format!(
                            "event {index}: checkpoint delta base {} does not match the previous checkpoint",
                            checkpoint.base_seq
                        ),
                    );
                }
                checkpoint_event_seq = Some(event.seq);
            }
            JournalEventKind::ModelRequested { request } => {
                model_request_base = Some((event.seq, request.messages.len()));
            }
            JournalEventKind::ModelRequestDelta { delta } => {
                match model_request_base {
                    None => push_audit_finding(
                        &mut findings,
                        AuditFindingCode::OrphanModelRequestDelta,
                        Some(index),
                        format!("event {index}: model request delta has no base request"),
                    ),
                    Some((base_seq, _)) if base_seq != delta.base_seq => push_audit_finding(
                        &mut findings,
                        AuditFindingCode::OrphanModelRequestDelta,
                        Some(index),
                        format!(
                            "event {index}: model request delta base {} does not match the previous request",
                            delta.base_seq
                        ),
                    ),
                    Some((_, base_len)) if delta.prefix_len > base_len => push_audit_finding(
                        &mut findings,
                        AuditFindingCode::InvalidModelRequestDelta,
                        Some(index),
                        format!(
                            "event {index}: model request delta prefix {} exceeds base length {base_len}",
                            delta.prefix_len
                        ),
                    ),
                    Some(_) => {}
                }
                model_request_base = Some((
                    event.seq,
                    delta.prefix_len.saturating_add(delta.messages.len()),
                ));
            }
            JournalEventKind::ToolCalled { call } => {
                if observed_calls.insert(call.id.clone(), index).is_some() {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::DuplicateToolCall,
                        Some(index),
                        format!("event {index}: duplicate tool call {}", call.id),
                    );
                }
            }
            JournalEventKind::ToolObserved { observation } => {
                if !answered_calls.insert(observation.call_id.clone()) {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::DuplicateToolObservation,
                        Some(index),
                        format!(
                            "event {index}: duplicate observation for {}",
                            observation.call_id
                        ),
                    );
                }
            }
            JournalEventKind::RoutePlanned { plan, .. } => {
                current_route_plan = Some(plan.clone());
            }
            JournalEventKind::RouteSelected {
                provider_id, model, ..
            } => {
                if let Some((expected_provider, expected_model, schedule_index)) =
                    pending_retry.take()
                    && (expected_provider != *provider_id || expected_model != *model)
                {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderRetryTargetMismatch,
                        Some(schedule_index),
                        format!(
                            "event {schedule_index}: scheduled retry {expected_provider}/{expected_model} was followed by {provider_id}/{model}"
                        ),
                    );
                }
                selected_route = Some((provider_id.clone(), model.clone()));
                responded_route = None;
                if let Some(plan) = &current_route_plan
                    && !plan.ranked.iter().any(|candidate| {
                        candidate.manifest.provider_id == *provider_id
                            && candidate.manifest.model == *model
                    })
                {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::RouteSelectionOutsidePlan,
                        Some(index),
                        format!(
                            "event {index}: selected route {provider_id}/{model} is not in the route plan"
                        ),
                    );
                }
            }
            JournalEventKind::ModelResponded { .. } => {
                responded_route = selected_route.clone();
            }
            JournalEventKind::ProviderRetryScheduled {
                delay_ms,
                next_provider_id,
                next_model,
            } => {
                last_retry_schedule = Some((*delay_ms, index));
                if *delay_ms > crate::domain::MAX_PROVIDER_RETRY_BACKOFF_MS {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderRetryDelayAboveCap,
                        Some(index),
                        format!(
                            "event {index}: retry delay {delay_ms} exceeds {} ms",
                            crate::domain::MAX_PROVIDER_RETRY_BACKOFF_MS
                        ),
                    );
                }
                let target_in_plan = current_route_plan.as_ref().is_some_and(|plan| {
                    plan.ranked.iter().any(|candidate| {
                        candidate.manifest.provider_id == *next_provider_id
                            && candidate.manifest.model == *next_model
                    })
                });
                if !target_in_plan {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderRetryOutsideRoutePlan,
                        Some(index),
                        format!(
                            "event {index}: retry target {next_provider_id}/{next_model} is not in the route plan"
                        ),
                    );
                }
                if pending_retry
                    .replace((next_provider_id.clone(), next_model.clone(), index))
                    .is_some()
                {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::DuplicateProviderRetrySchedule,
                        Some(index),
                        format!("event {index}: duplicate provider retry schedule"),
                    );
                }
            }
            JournalEventKind::ProviderRetryElapsed { timing } => {
                if timing.elapsed_ms > crate::domain::MAX_PROVIDER_RETRY_ELAPSED_MS {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderRetryElapsedAboveCap,
                        Some(index),
                        format!(
                            "event {index}: retry elapsed {} ms exceeds {} ms",
                            timing.elapsed_ms,
                            crate::domain::MAX_PROVIDER_RETRY_ELAPSED_MS
                        ),
                    );
                }
                match last_retry_schedule.take() {
                    Some((scheduled_delay, schedule_index))
                        if scheduled_delay != timing.delay_ms =>
                    {
                        push_audit_finding(
                            &mut findings,
                            AuditFindingCode::ProviderRetryDelayMismatch,
                            Some(index),
                            format!(
                                "event {index}: retry timing delay {} does not match schedule {scheduled_delay} at event {schedule_index}",
                                timing.delay_ms
                            ),
                        );
                    }
                    Some(_) => {}
                    None => push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderRetryOutcomeWithoutSchedule,
                        Some(index),
                        format!("event {index}: retry timing has no preceding schedule"),
                    ),
                }
            }
            JournalEventKind::ProviderFailed { failure } => {
                let failure_route = (failure.provider_id.clone(), failure.model.clone());
                if selected_route.as_ref() != Some(&failure_route) {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderFailureOutsideSelectedRoute,
                        Some(index),
                        format!(
                            "event {index}: provider failure {}/{} has no matching selected route",
                            failure.provider_id, failure.model
                        ),
                    );
                }
                if responded_route.as_ref() == Some(&failure_route) {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::ProviderSucceededAndFailed,
                        Some(index),
                        format!(
                            "event {index}: provider {}/{} responded and then failed in the same attempt",
                            failure.provider_id, failure.model
                        ),
                    );
                }
            }
            JournalEventKind::RunStopped { reason } => {
                stop_event_index = Some(index);
                if terminal_reason.replace(*reason).is_some() {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::DuplicateRunStop,
                        Some(index),
                        format!("event {index}: duplicate run stop"),
                    );
                }
            }
            JournalEventKind::AnswerProduced { answer: produced } => {
                answer_event_index = Some(index);
                if answer.replace(produced.clone()).is_some() {
                    push_audit_finding(
                        &mut findings,
                        AuditFindingCode::DuplicateAnswer,
                        Some(index),
                        format!("event {index}: duplicate answer"),
                    );
                }
            }
            _ => {}
        }
    }

    if events.is_empty() {
        push_audit_finding(
            &mut findings,
            AuditFindingCode::EmptyJournal,
            None,
            "journal is empty".into(),
        );
    }
    if !events.is_empty()
        && !matches!(
            events.first().map(|event| &event.event),
            Some(JournalEventKind::RunStarted { .. } | JournalEventKind::ResumeStarted { .. })
        )
    {
        push_audit_finding(
            &mut findings,
            AuditFindingCode::FirstEventNotRunStarted,
            Some(0),
            "first event is not run_started".into(),
        );
    }

    let unresolved_tool_calls = observed_calls
        .keys()
        .filter(|call_id| !answered_calls.contains(*call_id))
        .cloned()
        .collect();
    for call_id in &unresolved_tool_calls {
        push_audit_finding(
            &mut findings,
            AuditFindingCode::UnpairedToolCall,
            observed_calls.get(call_id).copied(),
            format!("tool call {call_id} has no observation"),
        );
    }
    if !events.is_empty() && terminal_reason.is_none() {
        push_audit_finding(
            &mut findings,
            AuditFindingCode::MissingRunStop,
            None,
            "journal has no run stop event".into(),
        );
    }
    if let (Some(request), Some(answer)) = (&request, &answer)
        && let Err(error) = answer.validate(request)
    {
        push_audit_finding(
            &mut findings,
            AuditFindingCode::AnswerValidationFailed,
            answer_event_index,
            format!("answer validation failed: {error}"),
        );
    }
    match (terminal_reason, answer.as_ref().map(|answer| answer.status)) {
        (Some(StopReason::Completed), Some(crate::domain::AnswerStatus::Completed))
        | (Some(StopReason::Completed), Some(crate::domain::AnswerStatus::Conditional))
        | (Some(StopReason::NeedsEvidence), Some(crate::domain::AnswerStatus::NeedsEvidence))
        | (Some(StopReason::BudgetExhausted), Some(crate::domain::AnswerStatus::NeedsEvidence))
        | (
            Some(StopReason::NeedsAuthorization),
            Some(crate::domain::AnswerStatus::NeedsAuthorization),
        )
        | (Some(StopReason::InvalidOutput), Some(crate::domain::AnswerStatus::Failed)) => {}
        (Some(reason), Some(status)) => push_audit_finding(
            &mut findings,
            AuditFindingCode::StopReasonMismatch,
            stop_event_index,
            format!("stop reason {reason:?} does not match answer status {status:?}"),
        ),
        (Some(StopReason::Completed), None)
        | (Some(StopReason::NeedsEvidence), None)
        | (Some(StopReason::NeedsAuthorization), None) => {
            push_audit_finding(
                &mut findings,
                AuditFindingCode::MissingAnswerForStop,
                stop_event_index,
                "successful or user-action stop has no answer".into(),
            );
        }
        _ => {}
    }

    if let Some((provider_id, model, schedule_index)) = pending_retry
        && !matches!(
            terminal_reason,
            Some(StopReason::Cancelled | StopReason::BudgetExhausted)
        )
    {
        push_audit_finding(
            &mut findings,
            AuditFindingCode::ProviderRetryWithoutNextRoute,
            Some(schedule_index),
            format!(
                "event {schedule_index}: scheduled retry to {provider_id}/{model} has no following route selection"
            ),
        );
    }

    audit_reframe(events, &mut findings);

    let issues = findings
        .iter()
        .map(|finding| finding.message.clone())
        .collect::<Vec<_>>();
    JournalAudit {
        ok: findings.is_empty(),
        event_count: events.len(),
        run_id,
        terminal_reason,
        answer_schema_version: answer.as_ref().map(|answer| answer.schema_version),
        unresolved_tool_calls,
        issues,
        findings,
    }
}

fn audit_reframe(events: &[JournalEvent], findings: &mut Vec<JournalAuditFinding>) {
    let mut current: Option<ReframeProposal> = None;
    let mut current_provider: Option<String> = None;
    let mut decided = false;
    for (index, event) in events.iter().enumerate() {
        match &event.event {
            JournalEventKind::ReframeProposed {
                proposal,
                provider_id,
                ..
            } => {
                if let Err(error) = proposal.validate() {
                    push_audit_finding(
                        findings,
                        AuditFindingCode::ReframeProposalInvalid,
                        Some(index),
                        format!("event {index}: reframe proposal is invalid: {error}"),
                    );
                }
                current = Some(proposal.clone());
                current_provider = Some(provider_id.clone());
                decided = false;
            }
            JournalEventKind::ReframeDecided { choice, actor, .. } => {
                let Some(proposal) = &current else {
                    push_audit_finding(
                        findings,
                        AuditFindingCode::ReframeDecisionWithoutProposal,
                        Some(index),
                        format!("event {index}: decision without a preceding proposal"),
                    );
                    continue;
                };
                if decided {
                    push_audit_finding(
                        findings,
                        AuditFindingCode::ReframeDuplicateDecision,
                        Some(index),
                        format!("event {index}: second decision for the same proposal"),
                    );
                    continue;
                }
                if actor.trim().is_empty() || current_provider.as_deref() == Some(actor.as_str()) {
                    push_audit_finding(
                        findings,
                        AuditFindingCode::ReframeDecisionInvalid,
                        Some(index),
                        format!(
                            "event {index}: reframe actor must be non-empty and must not be the proposing provider"
                        ),
                    );
                }
                if let Err(error) = choice.validate_against(proposal) {
                    push_audit_finding(
                        findings,
                        AuditFindingCode::ReframeDecisionInvalid,
                        Some(index),
                        format!("event {index}: reframe decision is invalid: {error}"),
                    );
                }
                decided = true;
            }
            _ => {}
        }
    }
}

fn push_audit_finding(
    findings: &mut Vec<JournalAuditFinding>,
    code: AuditFindingCode,
    event_index: Option<usize>,
    message: String,
) {
    findings.push(JournalAuditFinding {
        code,
        event_index,
        message,
    });
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
        provider_failures: Vec::new(),
        retry_delays_ms: Vec::new(),
        retry_timings: Vec::new(),
        route_plan: None,
        route_policy_sources: None,
        tool_capabilities: None,
        answer: None,
        stop_reason: None,
        tool_calls: Vec::new(),
        observations: Vec::new(),
        checkpoint: None,
        client_request_id: None,
        parent_run_link: None,
        reframe_proposal: None,
        reframe_choice: None,
        reframe_actor: None,
        reframe_decided_at_unix_ms: None,
    };
    let mut checkpoint_seq = 0u64;

    for event in events {
        match &event.event {
            JournalEventKind::RunStarted { request, mode } => {
                state.request = Some(request.clone());
                state.mode = Some(*mode);
            }
            JournalEventKind::ClientRequestLinked {
                client_request_id: value,
            } => state.client_request_id = Some(value.clone()),
            JournalEventKind::ParentRunLinked { link } => {
                state.parent_run_link = Some(link.clone());
            }
            JournalEventKind::ReframeProposed { proposal, .. } => {
                state.reframe_proposal = Some(proposal.clone());
            }
            JournalEventKind::ReframeDecided {
                choice,
                actor,
                decided_at_unix_ms,
            } => {
                state.reframe_choice = Some(choice.clone());
                state.reframe_actor = Some(actor.clone());
                state.reframe_decided_at_unix_ms = Some(*decided_at_unix_ms);
            }
            JournalEventKind::ResumeStarted {
                request,
                checkpoint,
                ..
            } => {
                state.request = Some(request.clone());
                state.mode = Some(checkpoint.mode);
                state.checkpoint = Some(checkpoint.clone());
                checkpoint_seq = event.seq;
            }
            JournalEventKind::RouteSelected {
                provider_id, model, ..
            } => {
                state.provider_id = Some(provider_id.clone());
                state.model = Some(model.clone());
            }
            JournalEventKind::ProviderFailed { failure } => {
                state.provider_failures.push(failure.clone());
            }
            JournalEventKind::ProviderRetryScheduled { delay_ms, .. } => {
                state.retry_delays_ms.push(*delay_ms);
            }
            JournalEventKind::ProviderRetryElapsed { timing } => {
                state.retry_timings.push(*timing);
            }
            JournalEventKind::RoutePlanned { plan, sources } => {
                state.route_plan = Some(plan.clone());
                state.route_policy_sources = Some(sources.clone());
            }
            JournalEventKind::ToolCapabilitiesAdvertised { snapshot } => {
                state.tool_capabilities = Some(snapshot.clone());
            }
            JournalEventKind::ToolCalled { call } => state.tool_calls.push(call.clone()),
            JournalEventKind::ToolObserved { observation } => {
                state.observations.push(observation.clone())
            }
            JournalEventKind::Checkpoint { checkpoint } => {
                state.checkpoint = Some(checkpoint.clone());
                checkpoint_seq = event.seq;
            }
            JournalEventKind::CheckpointDelta { checkpoint: delta } => {
                let Some(current) = state.checkpoint.as_mut() else {
                    return Err(ResumeError::MissingCheckpointBase.to_string());
                };
                apply_checkpoint_delta(current, delta, event.seq, &mut checkpoint_seq)
                    .map_err(|error| error.to_string())?;
            }
            JournalEventKind::AnswerProduced { answer } => state.answer = Some(answer.clone()),
            JournalEventKind::RunStopped { reason } => state.stop_reason = Some(*reason),
            JournalEventKind::ContextCompacted { .. }
            | JournalEventKind::CapabilityPreflightResolved { .. }
            | JournalEventKind::ExperienceInjected { .. }
            | JournalEventKind::ModelRequested { .. }
            | JournalEventKind::ModelRequestDelta { .. }
            | JournalEventKind::ModelResponded { .. }
            | JournalEventKind::StreamFirstToken { .. }
            | JournalEventKind::ToolsRequested { .. }
            | JournalEventKind::AnswerVerificationFailed { .. }
            | JournalEventKind::AnswerVerificationExhausted { .. }
            | JournalEventKind::CompletionAcceptanceMissing { .. }
            | JournalEventKind::CompletionArtifactUnfinished { .. }
            | JournalEventKind::CompletionDesignContractGap { .. } => {}
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

fn apply_checkpoint_delta(
    checkpoint: &mut RuntimeCheckpoint,
    delta: &RuntimeCheckpointDelta,
    event_seq: u64,
    checkpoint_seq: &mut u64,
) -> Result<(), ResumeError> {
    if delta.base_seq != *checkpoint_seq {
        return Err(ResumeError::InvalidCheckpointDelta);
    }
    checkpoint.messages.extend(delta.messages.iter().cloned());
    checkpoint.tool_calls_used = delta.tool_calls_used;
    checkpoint.provider_attempts_used = delta.provider_attempts_used;
    checkpoint.unavailable_routes = delta.unavailable_routes.clone();
    checkpoint.next_step = delta.next_step;
    checkpoint.elapsed_ms = delta.elapsed_ms;
    checkpoint.route_plan_sha256 = delta.route_plan_sha256.clone();
    checkpoint.route_policy_sources = delta.route_policy_sources.clone();
    checkpoint.route_objective = delta.route_objective;
    checkpoint.tool_capabilities_sha256 = delta.tool_capabilities_sha256.clone();
    *checkpoint_seq = event_seq;
    Ok(())
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
    let mut last_tool_seq = None;

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
                last_tool_seq = Some(event.seq);
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
            JournalEventKind::CheckpointDelta { checkpoint: delta } => {
                let Some(current) = checkpoint.as_mut() else {
                    return Err(ResumeError::MissingCheckpointBase);
                };
                apply_checkpoint_delta(current, delta, event.seq, &mut checkpoint_seq)?;
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
    if let Some(seq) = last_tool_seq
        && seq > checkpoint_seq
    {
        // The tool already ran, but its step was never checkpointed. Resuming
        // from the earlier checkpoint would let the model repeat the side
        // effect with a fresh call id; fail closed instead.
        return Err(ResumeError::UncheckpointedToolCall);
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
    #[error("journal shrank below the read cursor")]
    Truncated,
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
    #[error("a checkpoint delta has no base checkpoint")]
    MissingCheckpointBase,
    #[error("a checkpoint delta does not extend the previous checkpoint")]
    InvalidCheckpointDelta,
    #[error("execute-mode run is not safely resumable")]
    UnsafeExecuteMode,
    #[error("a tool ran after the last checkpoint; resuming could repeat its side effect")]
    UncheckpointedToolCall,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AnswerStatus, Evidence, ModelResponse, ModelTurn, ToolStatus, Usage};
    use crate::router::{
        PolicyValueSource, RouteObjective, RoutePlan, RoutePolicySources, RouteRequest,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn answer() -> AgentAnswer {
        AgentAnswer {
            task_need: None,
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "answer".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        }
    }

    #[test]
    fn tagged_events_with_evidence_timestamps_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("answer", PathBuf::from(".")),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        let mut answer = answer();
        answer.evidence = vec![Evidence {
            source: "fixture".into(),
            statement: "timestamped evidence".into(),
            observed_unix_ms: Some(0),
            expires_unix_ms: Some(1),
            claim_ids: Vec::new(),
        }];
        journal
            .append(JournalEventKind::ModelResponded {
                response: ModelResponse {
                    provider_id: "provider".into(),
                    model: "model".into(),
                    turn: ModelTurn::Final { answer },
                    usage: Usage::default(),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::ReframeDecided {
                choice: ReframeChoice::Deferred,
                actor: "user".into(),
                decided_at_unix_ms: 1_793_000_000_000,
            })
            .unwrap();
        drop(journal);

        let events = read_events(&path).unwrap();
        assert_eq!(events.len(), 3);
        match &events[1].event {
            JournalEventKind::ModelResponded { response } => match &response.turn {
                ModelTurn::Final { answer } => {
                    assert_eq!(
                        (
                            answer.evidence[0].observed_unix_ms,
                            answer.evidence[0].expires_unix_ms
                        ),
                        (Some(0), Some(1))
                    );
                }
                other => panic!("expected a final turn, got {other:?}"),
            },
            other => panic!("expected model_responded, got {other:?}"),
        }
        assert!(matches!(
            events[2].event,
            JournalEventKind::ReframeDecided {
                decided_at_unix_ms: 1_793_000_000_000,
                ..
            }
        ));
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
            audit.answer_schema_version,
            Some(crate::domain::ANSWER_SCHEMA_VERSION)
        );
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
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::RouteSelectionOutsidePlan
                && finding.event_index == Some(2)
        }));
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
                        task_need: None,
                        schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                        status: AnswerStatus::Completed,
                        conclusion: "done".into(),
                        claims: Vec::new(),
                        evidence: vec![crate::domain::Evidence {
                            source: "fixture".into(),
                            statement: "unsupported".into(),
                            observed_unix_ms: None,
                            expires_unix_ms: None,
                            claim_ids: vec!["missing".into()],
                        }],
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        conflicts: Vec::new(),
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
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::AnswerValidationFailed
                && finding.event_index == Some(1)
        }));
    }

    #[test]
    fn legacy_journal_answer_defaults_to_schema_version_one() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let events: Vec<JournalEvent> = serde_json::from_value(serde_json::json!([
            {
                "schema_version": PROTOCOL_VERSION,
                "run_id": run_id,
                "seq": 0,
                "timestamp_unix_ms": 0,
                "event": {
                    "type": "run_started",
                    "data": {"request": request, "mode": "direct_answer"}
                }
            },
            {
                "schema_version": PROTOCOL_VERSION,
                "run_id": run_id,
                "seq": 1,
                "timestamp_unix_ms": 0,
                "event": {
                    "type": "answer_produced",
                    "data": {
                        "answer": {
                            "status": "conditional",
                            "conclusion": "legacy answer"
                        }
                    }
                }
            },
            {
                "schema_version": PROTOCOL_VERSION,
                "run_id": run_id,
                "seq": 2,
                "timestamp_unix_ms": 0,
                "event": {"type": "run_stopped", "data": {"reason": "completed"}}
            }
        ]))
        .unwrap();

        let audit = audit_events(&events);
        assert!(audit.ok, "{:?}", audit.issues);
        assert_eq!(
            audit.answer_schema_version,
            Some(crate::domain::LEGACY_ANSWER_SCHEMA_VERSION)
        );
    }

    #[test]
    fn audit_rejects_unsupported_answer_schema_version() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let mut unsupported = answer();
        unsupported.schema_version = crate::domain::ANSWER_SCHEMA_VERSION + 1;
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
                event: JournalEventKind::AnswerProduced {
                    answer: unsupported,
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
        assert_eq!(
            audit.answer_schema_version,
            Some(crate::domain::ANSWER_SCHEMA_VERSION + 1)
        );
        assert!(
            audit
                .issues
                .iter()
                .any(|issue| issue.contains("unsupported answer schema version"))
        );
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::AnswerValidationFailed
                && finding.event_index == Some(1)
        }));
    }

    #[test]
    fn audit_rejects_incomplete_current_answer_decision() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let mut incomplete = answer();
        incomplete.status = AnswerStatus::Conditional;
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
                event: JournalEventKind::AnswerProduced { answer: incomplete },
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
                .any(|issue| issue.contains("conditional answer requires"))
        );
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::AnswerValidationFailed
                && finding.event_index == Some(1)
        }));
    }

    #[test]
    fn audit_findings_have_stable_codes_and_event_indexes() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
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
                event: JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: "call-1".into(),
                        name: "read_file".into(),
                        arguments: serde_json::json!({"path": "x"}),
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 0,
                event: JournalEventKind::AnswerProduced { answer: answer() },
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
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::DuplicateAnswer && finding.event_index == Some(3)
        }));
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::UnpairedToolCall && finding.event_index == Some(1)
        }));
        let json = serde_json::to_value(&audit).unwrap();
        assert!(json["findings"].as_array().is_some_and(|findings| {
            findings
                .iter()
                .any(|finding| finding["code"] == "duplicate_answer" && finding["event_index"] == 3)
        }));
    }

    #[test]
    fn audit_detects_contradictory_provider_failures() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
        let failure = ProviderFailure {
            provider_id: "provider-a".into(),
            model: "model-a".into(),
            code: ProviderErrorCode::ProviderUnavailable,
            retryable: true,
            message: "unavailable".into(),
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
                event: JournalEventKind::ProviderFailed {
                    failure: failure.clone(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RouteSelected {
                    provider_id: "provider-a".into(),
                    model: "model-a".into(),
                    reason: "selected".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 3,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ModelResponded {
                    response: ModelResponse {
                        provider_id: "provider-a".into(),
                        model: "model-a".into(),
                        turn: crate::domain::ModelTurn::Final { answer: answer() },
                        usage: crate::domain::Usage::default(),
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 4,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ProviderFailed { failure },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 5,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStopped {
                    reason: StopReason::ProviderError,
                },
            },
        ];

        let audit = audit_events(&events);
        assert!(!audit.ok);
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::ProviderFailureOutsideSelectedRoute
                && finding.event_index == Some(1)
        }));
        assert!(audit.findings.iter().any(|finding| {
            finding.code == AuditFindingCode::ProviderSucceededAndFailed
                && finding.event_index == Some(4)
        }));
        let replay = replay_events(&events).unwrap();
        assert_eq!(replay.provider_failures.len(), 2);
    }

    #[test]
    fn audit_detects_invalid_or_stalled_retry_schedules() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
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
                event: JournalEventKind::ProviderRetryScheduled {
                    delay_ms: 2_000,
                    next_provider_id: "provider-a".into(),
                    next_model: "model-a".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RouteSelected {
                    provider_id: "provider-b".into(),
                    model: "model-b".into(),
                    reason: "unexpected".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 3,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ProviderRetryScheduled {
                    delay_ms: 50,
                    next_provider_id: "provider-c".into(),
                    next_model: "model-c".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 4,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ProviderRetryScheduled {
                    delay_ms: 50,
                    next_provider_id: "provider-d".into(),
                    next_model: "model-d".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 5,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStopped {
                    reason: StopReason::ProviderError,
                },
            },
        ];

        let audit = audit_events(&events);
        assert!(!audit.ok);
        for expected in [
            AuditFindingCode::ProviderRetryDelayAboveCap,
            AuditFindingCode::ProviderRetryOutsideRoutePlan,
            AuditFindingCode::ProviderRetryTargetMismatch,
            AuditFindingCode::DuplicateProviderRetrySchedule,
            AuditFindingCode::ProviderRetryWithoutNextRoute,
        ] {
            assert!(
                audit
                    .findings
                    .iter()
                    .any(|finding| finding.code == expected),
                "missing {expected:?}: {:?}",
                audit.findings
            );
        }
    }

    #[test]
    fn audit_detects_invalid_retry_timing_outcomes() {
        let run_id = Uuid::new_v4();
        let request = TaskRequest::from_objective("answer", PathBuf::from("."));
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
                event: JournalEventKind::ProviderRetryScheduled {
                    delay_ms: 50,
                    next_provider_id: "provider-a".into(),
                    next_model: "model-a".into(),
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 2,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ProviderRetryElapsed {
                    timing: RetryTiming {
                        delay_ms: 100,
                        elapsed_ms: 3_000,
                        outcome: RetryOutcome::Completed,
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 3,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ProviderRetryElapsed {
                    timing: RetryTiming {
                        delay_ms: 0,
                        elapsed_ms: 0,
                        outcome: RetryOutcome::Cancelled,
                    },
                },
            },
            JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq: 4,
                timestamp_unix_ms: 0,
                event: JournalEventKind::RunStopped {
                    reason: StopReason::ProviderError,
                },
            },
        ];

        let audit = audit_events(&events);
        assert!(!audit.ok);
        for expected in [
            AuditFindingCode::ProviderRetryDelayMismatch,
            AuditFindingCode::ProviderRetryElapsedAboveCap,
            AuditFindingCode::ProviderRetryOutcomeWithoutSchedule,
        ] {
            assert!(
                audit
                    .findings
                    .iter()
                    .any(|finding| finding.code == expected),
                "missing {expected:?}: {:?}",
                audit.findings
            );
        }
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

#[cfg(test)]
mod reframe_journal_tests {
    use super::*;
    use crate::domain::{AnswerStatus, CognitiveMode, ToolStatus};
    use crate::reframe::{
        ReframeChoice, ReframeProposal, RejectedReason, RequestRole, StopRule, StopRuleSource,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn proposal() -> ReframeProposal {
        ReframeProposal {
            schema_version: crate::reframe::REFRAME_SCHEMA_VERSION,
            stated_request: "我要买个电钻".into(),
            classification: RequestRole::Means,
            confidence_bps: Some(8_200),
            question: Some("打孔是为了穿线、挂东西，还是施工改造？".into()),
            options: vec!["穿线".into(), "挂东西".into(), "施工改造".into()],
            added_costs: Default::default(),
            hypotheses: vec!["联网".into()],
            substitutes: vec!["无线路由器".into()],
            stop_rule: StopRule {
                verifiable: true,
                deliverable: true,
                better_substitute_exists: true,
            },
            stop_rule_source: StopRuleSource::Asserted,
        }
    }

    fn answer() -> AgentAnswer {
        AgentAnswer {
            task_need: None,
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: "已记录".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        }
    }

    fn journal_with(extras: Vec<JournalEventKind>) -> Vec<JournalEvent> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("我要买个电钻", PathBuf::from(".")),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        for extra in extras {
            journal.append(extra).unwrap();
        }
        journal
            .append(JournalEventKind::AnswerProduced { answer: answer() })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        read_events(&path).unwrap()
    }

    fn proposed() -> JournalEventKind {
        JournalEventKind::ReframeProposed {
            proposal: proposal(),
            provider_id: "deterministic".into(),
            model: "offline-v1".into(),
            prompt_sha256: "a".repeat(64),
        }
    }

    fn decided(choice: ReframeChoice) -> JournalEventKind {
        JournalEventKind::ReframeDecided {
            choice,
            actor: "user".into(),
            decided_at_unix_ms: 1_793_000_000_000,
        }
    }

    fn codes(events: &[JournalEvent]) -> Vec<AuditFindingCode> {
        audit_events(events)
            .findings
            .into_iter()
            .map(|finding| finding.code)
            .collect()
    }

    #[test]
    fn replay_recovers_the_proposal_and_the_decision() {
        let events = journal_with(vec![
            proposed(),
            decided(ReframeChoice::Accepted {
                option_index: 0,
                substitute_index: Some(0),
            }),
        ]);
        let state = replay_events(&events).unwrap();
        assert_eq!(state.reframe_proposal, Some(proposal()));
        assert_eq!(
            state.reframe_choice,
            Some(ReframeChoice::Accepted {
                option_index: 0,
                substitute_index: Some(0),
            })
        );
    }

    #[test]
    fn audit_flags_a_decision_without_a_proposal() {
        let events = journal_with(vec![decided(ReframeChoice::Deferred)]);
        assert!(codes(&events).contains(&AuditFindingCode::ReframeDecisionWithoutProposal));
    }

    #[test]
    fn audit_flags_a_second_decision() {
        let events = journal_with(vec![
            proposed(),
            decided(ReframeChoice::Deferred),
            decided(ReframeChoice::Deferred),
        ]);
        assert!(codes(&events).contains(&AuditFindingCode::ReframeDuplicateDecision));
    }

    #[test]
    fn audit_flags_an_empty_or_provider_actor() {
        for actor in ["", "deterministic"] {
            let events = journal_with(vec![
                proposed(),
                JournalEventKind::ReframeDecided {
                    choice: ReframeChoice::Deferred,
                    actor: actor.into(),
                    decided_at_unix_ms: 1_793_000_000_000,
                },
            ]);
            assert!(
                codes(&events).contains(&AuditFindingCode::ReframeDecisionInvalid),
                "actor {actor:?} should be rejected"
            );
        }
    }

    #[test]
    fn audit_flags_an_out_of_range_option() {
        let events = journal_with(vec![
            proposed(),
            decided(ReframeChoice::Accepted {
                option_index: 9,
                substitute_index: None,
            }),
        ]);
        assert!(codes(&events).contains(&AuditFindingCode::ReframeDecisionInvalid));
    }

    #[test]
    fn a_rejected_decision_is_clean() {
        let events = journal_with(vec![
            proposed(),
            decided(ReframeChoice::Rejected {
                reason: RejectedReason::PreferLiteral,
            }),
        ]);
        let audit = audit_events(&events);
        assert!(audit.ok, "unexpected findings: {:?}", audit.issues);
    }

    #[test]
    fn incremental_reads_only_new_events() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("answer", PathBuf::from(".")),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        let mut cursor = JournalCursor::new();
        let first = read_events_incremental(&path, &mut cursor).unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(
            cursor.offset(),
            std::fs::metadata(&path).unwrap().len(),
            "cursor should consume every complete line"
        );

        journal
            .append(JournalEventKind::ClientRequestLinked {
                client_request_id: "req-1".into(),
            })
            .unwrap();
        let second = read_events_incremental(&path, &mut cursor).unwrap();
        assert_eq!(second.len(), 1);
        assert!(matches!(
            second[0].event,
            JournalEventKind::ClientRequestLinked { .. }
        ));
        assert!(
            read_events_incremental(&path, &mut cursor)
                .unwrap()
                .is_empty()
        );
        assert_eq!(read_events(&path).unwrap().len(), 3);
    }

    #[test]
    fn incremental_read_holds_a_partial_trailing_line() {
        let workspace = tempfile::tempdir().unwrap();
        let complete_path = workspace.path().join("complete.jsonl");
        let mut journal = Journal::create(&complete_path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::ClientRequestLinked {
                client_request_id: "first".into(),
            })
            .unwrap();
        journal
            .append(JournalEventKind::ClientRequestLinked {
                client_request_id: "second".into(),
            })
            .unwrap();
        drop(journal);
        let events = read_events(&complete_path).unwrap();
        let content = std::fs::read_to_string(&complete_path).unwrap();
        let mut lines = content.lines();
        let first_line = lines.next().unwrap();
        let second_line = lines.next().unwrap();
        let split = second_line.len() / 2;

        let partial_path = workspace.path().join("partial.jsonl");
        std::fs::write(
            &partial_path,
            format!("{first_line}\n{}", &second_line[..split]),
        )
        .unwrap();
        let mut cursor = JournalCursor::new();
        let before = read_events_incremental(&partial_path, &mut cursor).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(cursor.offset(), first_line.len() as u64 + 1);

        let mut file = OpenOptions::new().append(true).open(&partial_path).unwrap();
        writeln!(file, "{}", &second_line[split..]).unwrap();
        file.flush().unwrap();
        let after = read_events_incremental(&partial_path, &mut cursor).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].seq, events[1].seq);
    }

    #[test]
    fn incremental_read_reports_the_global_line_number() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("broken.jsonl");
        let event = JournalEvent {
            schema_version: PROTOCOL_VERSION,
            run_id: Uuid::new_v4(),
            seq: 0,
            timestamp_unix_ms: 0,
            event: JournalEventKind::ClientRequestLinked {
                client_request_id: "first".into(),
            },
        };
        std::fs::write(
            &path,
            format!(
                "{}\n{{\"broken\":true}}\n",
                serde_json::to_string(&event).unwrap()
            ),
        )
        .unwrap();
        let mut cursor = JournalCursor::new();
        let error = read_events_incremental(&path, &mut cursor).unwrap_err();
        match error {
            JournalReadError::Json { line, .. } => assert_eq!(line, 2),
            other => panic!("expected a JSON error, got {other:?}"),
        }
    }

    #[test]
    fn incremental_read_rejects_a_shrunk_journal() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::ClientRequestLinked {
                client_request_id: "first".into(),
            })
            .unwrap();
        let mut cursor = JournalCursor::new();
        read_events_incremental(&path, &mut cursor).unwrap();
        drop(journal);
        std::fs::write(&path, "").unwrap();
        assert!(matches!(
            read_events_incremental(&path, &mut cursor),
            Err(JournalReadError::Truncated)
        ));
    }

    #[test]
    #[ignore = "timing probe; run with --ignored --nocapture"]
    fn journal_full_parse_at_one_million_tokens() {
        // ~4 MiB of ASCII is about 1M tokens at the runtime's 4-chars/token
        // estimate, which makes this the SA97-10 acceptance size.
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("one-million-tokens.jsonl");
        let run_id = Uuid::new_v4();
        let total = 4_000u64;
        let mut body = String::new();
        for seq in 0..total {
            let event = JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ClientRequestLinked {
                    client_request_id: format!("{seq:08}-{}", "x".repeat(1000)),
                },
            };
            body.push_str(&serde_json::to_string(&event).unwrap());
            body.push('\n');
        }
        std::fs::write(&path, body).unwrap();
        let started = std::time::Instant::now();
        let events = read_events(&path).unwrap();
        let full = started.elapsed();
        assert_eq!(events.len() as u64, total);
        let started = std::time::Instant::now();
        let replay = replay_events(&events).unwrap();
        let replay_time = started.elapsed();
        assert_eq!(replay.run_id, run_id);

        let mut cursor = JournalCursor::new();
        let consumed = read_events_incremental(&path, &mut cursor).unwrap();
        assert_eq!(consumed.len() as u64, total);
        let mut samples = Vec::new();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        for offset in 0..20u64 {
            let seq = total + offset;
            let event = JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ClientRequestLinked {
                    client_request_id: format!("new-{seq}"),
                },
            };
            writeln!(file, "{}", serde_json::to_string(&event).unwrap()).unwrap();
            file.flush().unwrap();
            let started = std::time::Instant::now();
            let new = read_events_incremental(&path, &mut cursor).unwrap();
            samples.push(started.elapsed());
            assert_eq!(new.len(), 1);
        }
        samples.sort();
        let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
        eprintln!(
            "journal bytes={} events={total} full={full:?} replay={replay_time:?} incremental_p95={p95:?}",
            std::fs::metadata(&path).unwrap().len()
        );
        assert!(
            full < std::time::Duration::from_millis(50),
            "full parse took {full:?}"
        );
        assert!(
            p95 < std::time::Duration::from_millis(50),
            "incremental p95 took {p95:?}"
        );
    }

    fn journal_prefix(extras: Vec<JournalEventKind>) -> Vec<JournalEvent> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("answer", PathBuf::from(".")),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        for extra in extras {
            journal.append(extra).unwrap();
        }
        drop(journal);
        read_events(&path).unwrap()
    }

    fn checkpoint() -> RuntimeCheckpoint {
        RuntimeCheckpoint {
            mode: CognitiveMode::DirectAnswer,
            messages: vec![crate::domain::ModelMessage::user("hello")],
            tool_calls_used: 0,
            provider_attempts_used: 0,
            unavailable_routes: Vec::new(),
            next_step: 1,
            elapsed_ms: 1,
            route_plan_sha256: None,
            route_policy_sources: None,
            route_objective: None,
            tool_capabilities_sha256: None,
        }
    }

    fn checkpoint_delta(
        base_seq: u64,
        messages: Vec<crate::domain::ModelMessage>,
    ) -> JournalEventKind {
        JournalEventKind::CheckpointDelta {
            checkpoint: RuntimeCheckpointDelta {
                base_seq,
                messages,
                tool_calls_used: 0,
                provider_attempts_used: 0,
                unavailable_routes: Vec::new(),
                next_step: 2,
                elapsed_ms: 2,
                route_plan_sha256: None,
                route_policy_sources: None,
                route_objective: None,
                tool_capabilities_sha256: None,
            },
        }
    }

    #[test]
    fn checkpoint_delta_extends_the_previous_checkpoint() {
        let events = journal_prefix(vec![
            JournalEventKind::Checkpoint {
                checkpoint: checkpoint(),
            },
            checkpoint_delta(
                1,
                vec![crate::domain::ModelMessage::assistant("", Vec::new())],
            ),
        ]);
        let prefix = find_resumable_prefix(&events).unwrap();
        assert_eq!(prefix.checkpoint_seq, 2);
        assert_eq!(prefix.checkpoint.messages.len(), 2);
        assert_eq!(prefix.checkpoint.next_step, 2);
    }

    #[test]
    fn resume_rejects_a_tool_call_after_the_last_checkpoint() {
        let call = ToolCall {
            id: "call-1".into(),
            name: "write_file".into(),
            arguments: serde_json::json!({"path": "x"}),
        };
        let events = journal_prefix(vec![
            JournalEventKind::Checkpoint {
                checkpoint: checkpoint(),
            },
            JournalEventKind::ToolCalled { call: call.clone() },
            JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: call.id,
                    tool: call.name,
                    status: ToolStatus::Ok,
                    summary: "wrote".into(),
                    content: String::new(),
                    error_code: None,
                    metadata: BTreeMap::new(),
                },
            },
        ]);
        assert!(matches!(
            find_resumable_prefix(&events),
            Err(ResumeError::UncheckpointedToolCall)
        ));
    }

    #[test]
    fn checkpoint_delta_with_a_wrong_base_is_rejected() {
        let events = journal_prefix(vec![
            JournalEventKind::Checkpoint {
                checkpoint: checkpoint(),
            },
            checkpoint_delta(0, vec![crate::domain::ModelMessage::user("stale")]),
        ]);
        assert!(matches!(
            find_resumable_prefix(&events),
            Err(ResumeError::InvalidCheckpointDelta)
        ));
    }

    #[test]
    fn audit_flags_an_orphan_checkpoint_delta() {
        let events = journal_with(vec![checkpoint_delta(0, Vec::new())]);
        assert!(codes(&events).contains(&AuditFindingCode::OrphanCheckpointDelta));
    }

    fn model_request(
        run_id: Uuid,
        messages: Vec<ModelMessage>,
        tools: Vec<ToolSpec>,
    ) -> ModelRequest {
        ModelRequest {
            run_id,
            mode: CognitiveMode::DirectAnswer,
            messages,
            tools,
        }
    }

    fn model_request_delta(
        base_seq: u64,
        prefix_len: usize,
        messages: Vec<ModelMessage>,
    ) -> JournalEventKind {
        JournalEventKind::ModelRequestDelta {
            delta: ModelRequestDelta {
                base_seq,
                prefix_len,
                messages,
                tools: None,
            },
        }
    }

    #[test]
    fn model_request_deltas_reconstruct_the_full_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.jsonl");
        let mut journal = Journal::create(&path, Uuid::new_v4()).unwrap();
        let run_id = Uuid::new_v4();
        let tool = ToolSpec {
            name: "read".into(),
            description: "read a file".into(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let first = model_request(run_id, vec![ModelMessage::user("one")], vec![tool.clone()]);
        let second = model_request(
            run_id,
            vec![
                ModelMessage::user("one"),
                ModelMessage::assistant("", Vec::new()),
            ],
            vec![tool],
        );
        let repeated = second.clone();
        let mut log = ModelRequestLog::default();
        log.append(&mut journal, first.clone()).unwrap();
        log.append(&mut journal, second.clone()).unwrap();
        log.append(&mut journal, repeated.clone()).unwrap();
        drop(journal);

        let events = read_events(&path).unwrap();
        assert!(matches!(
            events[0].event,
            JournalEventKind::ModelRequested { .. }
        ));
        assert!(matches!(
            events[1].event,
            JournalEventKind::ModelRequestDelta { .. }
        ));
        assert!(matches!(
            events[2].event,
            JournalEventKind::ModelRequestDelta { .. }
        ));
        let mut chain = ModelRequestChain::new();
        let reconstructed = events
            .iter()
            .filter_map(|event| chain.apply(event).ok().flatten().cloned())
            .collect::<Vec<_>>();
        assert_eq!(reconstructed, vec![first, second.clone(), repeated]);
        assert!(
            serde_json::to_vec(&events[1]).unwrap().len()
                < serde_json::to_vec(&second).unwrap().len()
        );
    }

    #[test]
    fn audit_flags_an_orphan_model_request_delta() {
        let events = journal_with(vec![model_request_delta(
            0,
            0,
            vec![ModelMessage::user("stale")],
        )]);
        assert!(codes(&events).contains(&AuditFindingCode::OrphanModelRequestDelta));
    }

    #[test]
    fn audit_flags_a_model_request_delta_prefix_overflow() {
        let events = journal_with(vec![
            JournalEventKind::ModelRequested {
                request: model_request(Uuid::new_v4(), vec![ModelMessage::user("one")], Vec::new()),
            },
            model_request_delta(1, 5, Vec::new()),
        ]);
        assert!(codes(&events).contains(&AuditFindingCode::InvalidModelRequestDelta));
    }

    #[test]
    #[ignore = "timing probe; run with --ignored --nocapture"]
    fn incremental_read_benchmark_on_a_large_journal() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("large.jsonl");
        let run_id = Uuid::new_v4();
        let total = 20_000u64;
        let mut body = String::new();
        for seq in 0..total {
            let event = JournalEvent {
                schema_version: PROTOCOL_VERSION,
                run_id,
                seq,
                timestamp_unix_ms: 0,
                event: JournalEventKind::ClientRequestLinked {
                    client_request_id: format!("{seq:08}-{}", "x".repeat(1000)),
                },
            };
            body.push_str(&serde_json::to_string(&event).unwrap());
            body.push('\n');
        }
        std::fs::write(&path, body).unwrap();

        let started = std::time::Instant::now();
        let all = read_events(&path).unwrap();
        let full = started.elapsed();
        assert_eq!(all.len() as u64, total);

        let mut cursor = JournalCursor::new();
        let started = std::time::Instant::now();
        let first = read_events_incremental(&path, &mut cursor).unwrap();
        let first_read = started.elapsed();
        assert_eq!(first.len() as u64, total);

        let extra = JournalEvent {
            schema_version: PROTOCOL_VERSION,
            run_id,
            seq: total,
            timestamp_unix_ms: 0,
            event: JournalEventKind::ClientRequestLinked {
                client_request_id: "new".into(),
            },
        };
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(file, "{}", serde_json::to_string(&extra).unwrap()).unwrap();
        file.flush().unwrap();

        let started = std::time::Instant::now();
        let new = read_events_incremental(&path, &mut cursor).unwrap();
        let incremental = started.elapsed();
        assert_eq!(new.len(), 1);
        eprintln!(
            "journal bytes={} full={full:?} first={first_read:?} incremental={incremental:?}",
            std::fs::metadata(&path).unwrap().len()
        );
        assert!(
            incremental < first_read,
            "incremental read should be cheaper than the first full read"
        );
    }
}
