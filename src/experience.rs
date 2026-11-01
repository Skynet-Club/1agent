use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::CognitiveMode;
use crate::domain::PROTOCOL_VERSION;
use crate::journal::{JournalEvent, audit_events};
use crate::safety::contains_disallowed_control;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceKind {
    Memory,
    Skill,
    RoutePolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceStatus {
    Draft,
    Reviewed,
    Verified,
    Approved,
    Rejected,
    Active,
    Degraded,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceVerificationInput {
    pub suite_id: String,
    pub suite_version: String,
    pub case_ids: Vec<String>,
    pub suite_sha256: String,
    pub training_group: String,
    pub evaluation_group: String,
    pub baseline_success_bps: u16,
    pub learned_success_bps: u16,
    pub baseline_unsupported_bps: u16,
    pub learned_unsupported_bps: u16,
    pub cost_delta_bps: i32,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceVerification {
    pub suite_id: String,
    pub suite_version: String,
    pub case_ids: Vec<String>,
    pub suite_sha256: String,
    pub training_group: String,
    pub evaluation_group: String,
    pub baseline_success_bps: u16,
    pub learned_success_bps: u16,
    pub baseline_unsupported_bps: u16,
    pub learned_unsupported_bps: u16,
    pub cost_delta_bps: i32,
    pub evidence: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceCandidateSpec {
    pub kind: ExperienceKind,
    pub title: String,
    pub content: String,
    pub scope: String,
    pub evidence_event_seqs: Vec<u64>,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default = "all_modes")]
    pub applicable_modes: Vec<CognitiveMode>,
    #[serde(default)]
    pub expires_at_unix_ms: Option<u128>,
    #[serde(default)]
    pub supersedes: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceCandidate {
    pub id: Uuid,
    pub version: u32,
    pub kind: ExperienceKind,
    pub title: String,
    pub content: String,
    pub scope: String,
    pub source_run_id: Uuid,
    pub evidence_event_seqs: Vec<u64>,
    pub sensitive: bool,
    pub applicable_modes: Vec<CognitiveMode>,
    pub expires_at_unix_ms: Option<u128>,
    pub supersedes: Option<Uuid>,
    pub status: ExperienceStatus,
    #[serde(default)]
    pub verification: Option<ExperienceVerification>,
    pub created_unix_ms: u128,
    pub updated_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ExperienceEvent {
    schema_version: u32,
    seq: u64,
    timestamp_unix_ms: u128,
    event: ExperienceEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum ExperienceEventKind {
    Proposed {
        candidate: Box<ExperienceCandidate>,
    },
    Reviewed {
        id: Uuid,
        actor: String,
    },
    Verified {
        id: Uuid,
        verification: ExperienceVerification,
    },
    Approved {
        id: Uuid,
        actor: String,
    },
    Rejected {
        id: Uuid,
        actor: String,
        reason: String,
    },
    Activated {
        id: Uuid,
    },
    Degraded {
        id: Uuid,
        reason: String,
    },
    RolledBack {
        id: Uuid,
        reason: String,
    },
}

pub struct ExperienceStore {
    path: PathBuf,
}

impl ExperienceStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, ExperienceError> {
        std::fs::create_dir_all(dir.as_ref())?;
        Ok(Self {
            path: dir.as_ref().join("events.jsonl"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn candidates(&self) -> Result<BTreeMap<Uuid, ExperienceCandidate>, ExperienceError> {
        let events = self.read_events()?;
        let mut candidates = BTreeMap::new();
        for event in events {
            let event_candidate_id = match &event.event {
                ExperienceEventKind::Proposed { candidate } => candidate.id,
                ExperienceEventKind::Reviewed { id, .. }
                | ExperienceEventKind::Verified { id, .. }
                | ExperienceEventKind::Approved { id, .. }
                | ExperienceEventKind::Rejected { id, .. }
                | ExperienceEventKind::Activated { id }
                | ExperienceEventKind::Degraded { id, .. }
                | ExperienceEventKind::RolledBack { id, .. } => *id,
            };
            match event.event {
                ExperienceEventKind::Proposed { candidate } => {
                    if candidates.insert(candidate.id, *candidate).is_some() {
                        return Err(ExperienceError::DuplicateCandidate);
                    }
                }
                ExperienceEventKind::Reviewed { id, .. } => {
                    transition(
                        &mut candidates,
                        id,
                        &[ExperienceStatus::Draft],
                        |candidate| {
                            candidate.status = ExperienceStatus::Reviewed;
                        },
                    )?;
                }
                ExperienceEventKind::Verified { id, verification } => {
                    transition(
                        &mut candidates,
                        id,
                        &[ExperienceStatus::Reviewed],
                        |candidate| {
                            candidate.status = ExperienceStatus::Verified;
                            candidate.verification = Some(verification.clone());
                        },
                    )?;
                }
                ExperienceEventKind::Approved { id, .. } => {
                    transition(
                        &mut candidates,
                        id,
                        &[ExperienceStatus::Verified],
                        |candidate| candidate.status = ExperienceStatus::Approved,
                    )?;
                }
                ExperienceEventKind::Rejected { id, .. } => {
                    transition(
                        &mut candidates,
                        id,
                        &[
                            ExperienceStatus::Draft,
                            ExperienceStatus::Reviewed,
                            ExperienceStatus::Verified,
                        ],
                        |candidate| candidate.status = ExperienceStatus::Rejected,
                    )?;
                }
                ExperienceEventKind::Activated { id } => {
                    transition(
                        &mut candidates,
                        id,
                        &[ExperienceStatus::Approved],
                        |candidate| candidate.status = ExperienceStatus::Active,
                    )?;
                }
                ExperienceEventKind::Degraded { id, .. } => {
                    transition(
                        &mut candidates,
                        id,
                        &[ExperienceStatus::Active],
                        |candidate| candidate.status = ExperienceStatus::Degraded,
                    )?;
                }
                ExperienceEventKind::RolledBack { id, .. } => {
                    transition(
                        &mut candidates,
                        id,
                        &[
                            ExperienceStatus::Approved,
                            ExperienceStatus::Active,
                            ExperienceStatus::Degraded,
                        ],
                        |candidate| candidate.status = ExperienceStatus::RolledBack,
                    )?;
                }
            }
            if let Some(candidate) = candidates.get_mut(&event_candidate_id) {
                candidate.updated_unix_ms = event.timestamp_unix_ms;
            }
        }
        Ok(candidates)
    }

    pub fn propose(
        &mut self,
        spec: ExperienceCandidateSpec,
        source_events: &[JournalEvent],
    ) -> Result<ExperienceCandidate, ExperienceError> {
        let audit = audit_events(source_events);
        if !audit.ok {
            return Err(ExperienceError::UnauditedSource(audit.issues));
        }
        if spec.evidence_event_seqs.is_empty() {
            return Err(ExperienceError::NoEvidence);
        }
        validate_text(&spec.title)?;
        validate_text(&spec.content)?;
        validate_text(&spec.scope)?;

        let source_run_id = source_events
            .first()
            .map(|event| event.run_id)
            .ok_or(ExperienceError::NoEvidence)?;
        let known_seqs: std::collections::BTreeSet<_> =
            source_events.iter().map(|event| event.seq).collect();
        for seq in &spec.evidence_event_seqs {
            if !known_seqs.contains(seq) {
                return Err(ExperienceError::UnknownEvidenceSeq(*seq));
            }
        }

        let timestamp = now_unix_ms();
        let candidate = ExperienceCandidate {
            id: Uuid::new_v4(),
            version: 1,
            kind: spec.kind,
            title: spec.title,
            content: spec.content,
            scope: spec.scope,
            source_run_id,
            evidence_event_seqs: spec.evidence_event_seqs,
            sensitive: spec.sensitive,
            applicable_modes: spec.applicable_modes,
            expires_at_unix_ms: spec.expires_at_unix_ms,
            supersedes: spec.supersedes,
            status: ExperienceStatus::Draft,
            verification: None,
            created_unix_ms: timestamp,
            updated_unix_ms: timestamp,
        };
        self.append(ExperienceEventKind::Proposed {
            candidate: Box::new(candidate.clone()),
        })?;
        Ok(candidate)
    }

    pub fn review(&mut self, id: Uuid, actor: &str) -> Result<(), ExperienceError> {
        self.transition(id, &[ExperienceStatus::Draft], |_| {
            Ok(ExperienceEventKind::Reviewed {
                id,
                actor: validate_actor(actor)?.to_string(),
            })
        })
    }

    pub fn approve(&mut self, id: Uuid, actor: &str) -> Result<(), ExperienceError> {
        self.transition(id, &[ExperienceStatus::Verified], |_| {
            Ok(ExperienceEventKind::Approved {
                id,
                actor: validate_actor(actor)?.to_string(),
            })
        })
    }

    pub fn verify(
        &mut self,
        id: Uuid,
        input: ExperienceVerificationInput,
    ) -> Result<(), ExperienceError> {
        validate_text(&input.suite_id)?;
        validate_text(&input.suite_version)?;
        validate_text(&input.training_group)?;
        validate_text(&input.evaluation_group)?;
        validate_text(&input.evidence)?;
        if input.case_ids.is_empty() {
            return Err(ExperienceError::InvalidVerification(
                "case_ids must not be empty".into(),
            ));
        }
        let unique_cases: std::collections::BTreeSet<_> = input.case_ids.iter().collect();
        if unique_cases.len() != input.case_ids.len() {
            return Err(ExperienceError::InvalidVerification(
                "case_ids must be unique".into(),
            ));
        }
        if input.suite_sha256.len() != 64
            || !input
                .suite_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit())
        {
            return Err(ExperienceError::InvalidVerification(
                "suite_sha256 must be 64 hexadecimal characters".into(),
            ));
        }
        if input.training_group == input.evaluation_group {
            return Err(ExperienceError::InvalidVerification(
                "training and evaluation groups must differ".into(),
            ));
        }
        if input.baseline_success_bps > 10_000
            || input.learned_success_bps > 10_000
            || input.baseline_unsupported_bps > 10_000
            || input.learned_unsupported_bps > 10_000
        {
            return Err(ExperienceError::InvalidVerification(
                "basis points must be between 0 and 10000".into(),
            ));
        }
        let passed = input.learned_success_bps > input.baseline_success_bps
            && input.learned_unsupported_bps <= input.baseline_unsupported_bps
            && input.cost_delta_bps <= 1_000;
        if !passed {
            return Err(ExperienceError::VerificationFailed);
        }
        let verification = ExperienceVerification {
            suite_id: input.suite_id,
            suite_version: input.suite_version,
            case_ids: input.case_ids,
            suite_sha256: input.suite_sha256,
            training_group: input.training_group,
            evaluation_group: input.evaluation_group,
            baseline_success_bps: input.baseline_success_bps,
            learned_success_bps: input.learned_success_bps,
            baseline_unsupported_bps: input.baseline_unsupported_bps,
            learned_unsupported_bps: input.learned_unsupported_bps,
            cost_delta_bps: input.cost_delta_bps,
            evidence: input.evidence,
            passed,
        };
        self.transition(id, &[ExperienceStatus::Reviewed], |_| {
            Ok(ExperienceEventKind::Verified { id, verification })
        })
    }

    pub fn reject(&mut self, id: Uuid, actor: &str, reason: &str) -> Result<(), ExperienceError> {
        validate_text(reason)?;
        self.transition(
            id,
            &[ExperienceStatus::Draft, ExperienceStatus::Reviewed],
            |_| {
                Ok(ExperienceEventKind::Rejected {
                    id,
                    actor: validate_actor(actor)?.to_string(),
                    reason: reason.to_string(),
                })
            },
        )
    }

    pub fn activate(&mut self, id: Uuid) -> Result<(), ExperienceError> {
        self.transition(id, &[ExperienceStatus::Approved], |_| {
            Ok(ExperienceEventKind::Activated { id })
        })
    }

    pub fn degrade(&mut self, id: Uuid, reason: &str) -> Result<(), ExperienceError> {
        validate_text(reason)?;
        self.transition(id, &[ExperienceStatus::Active], |_| {
            Ok(ExperienceEventKind::Degraded {
                id,
                reason: reason.to_string(),
            })
        })
    }

    pub fn rollback(&mut self, id: Uuid, reason: &str) -> Result<(), ExperienceError> {
        validate_text(reason)?;
        self.transition(
            id,
            &[
                ExperienceStatus::Approved,
                ExperienceStatus::Active,
                ExperienceStatus::Degraded,
            ],
            |_| {
                Ok(ExperienceEventKind::RolledBack {
                    id,
                    reason: reason.to_string(),
                })
            },
        )
    }

    pub fn retrieve_active(
        &self,
        mode: CognitiveMode,
        scope: &str,
        max_chars: usize,
    ) -> Result<Vec<ExperienceCandidate>, ExperienceError> {
        let now = now_unix_ms();
        let mut candidates: Vec<_> = self
            .candidates()?
            .into_values()
            .filter(|candidate| {
                candidate.status == ExperienceStatus::Active
                    && candidate.applicable_modes.contains(&mode)
                    && (candidate.scope == "*" || candidate.scope == scope)
                    && candidate
                        .expires_at_unix_ms
                        .is_none_or(|expires_at| expires_at > now)
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .updated_unix_ms
                .cmp(&left.updated_unix_ms)
                .then_with(|| left.id.cmp(&right.id))
        });

        let mut selected = Vec::new();
        let mut used_chars = 0usize;
        for candidate in candidates {
            let cost = candidate.title.chars().count() + candidate.content.chars().count();
            if used_chars + cost > max_chars {
                continue;
            }
            used_chars += cost;
            selected.push(candidate);
        }
        Ok(selected)
    }

    fn transition<F>(
        &mut self,
        id: Uuid,
        allowed_from: &[ExperienceStatus],
        build_event: F,
    ) -> Result<(), ExperienceError>
    where
        F: FnOnce(&ExperienceCandidate) -> Result<ExperienceEventKind, ExperienceError>,
    {
        let candidates = self.candidates()?;
        let candidate = candidates
            .get(&id)
            .ok_or(ExperienceError::CandidateNotFound(id))?;
        if !allowed_from.contains(&candidate.status) {
            return Err(ExperienceError::InvalidTransition {
                from: candidate.status,
                allowed_from: allowed_from.to_vec(),
            });
        }
        let event = build_event(candidate)?;
        self.append(event)
    }

    fn append(&mut self, event: ExperienceEventKind) -> Result<(), ExperienceError> {
        let events = self.read_events()?;
        let seq = events.len() as u64;
        let envelope = ExperienceEvent {
            schema_version: PROTOCOL_VERSION,
            seq,
            timestamp_unix_ms: now_unix_ms(),
            event,
        };
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &envelope)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    fn read_events(&self) -> Result<Vec<ExperienceEvent>, ExperienceError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file: File = File::open(&self.path)?;
        let mut events = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: ExperienceEvent = serde_json::from_str(&line)?;
            if event.schema_version != PROTOCOL_VERSION {
                return Err(ExperienceError::UnsupportedSchema(event.schema_version));
            }
            if event.seq != events.len() as u64 {
                return Err(ExperienceError::InvalidSequence {
                    expected: events.len() as u64,
                    actual: event.seq,
                });
            }
            events.push(event);
        }
        Ok(events)
    }
}

fn all_modes() -> Vec<CognitiveMode> {
    vec![
        CognitiveMode::DirectAnswer,
        CognitiveMode::EvidenceResearch,
        CognitiveMode::DecisionAnalysis,
        CognitiveMode::Execute,
        CognitiveMode::Diagnosis,
        CognitiveMode::Escalate,
    ]
}

fn transition<F>(
    candidates: &mut BTreeMap<Uuid, ExperienceCandidate>,
    id: Uuid,
    allowed_from: &[ExperienceStatus],
    apply: F,
) -> Result<(), ExperienceError>
where
    F: FnOnce(&mut ExperienceCandidate),
{
    let candidate = candidates
        .get_mut(&id)
        .ok_or(ExperienceError::CandidateNotFound(id))?;
    if !allowed_from.contains(&candidate.status) {
        return Err(ExperienceError::InvalidTransition {
            from: candidate.status,
            allowed_from: allowed_from.to_vec(),
        });
    }
    apply(candidate);
    Ok(())
}

fn validate_actor(actor: &str) -> Result<&str, ExperienceError> {
    let trimmed = actor.trim();
    if trimmed.is_empty() {
        return Err(ExperienceError::InvalidText("actor"));
    }
    Ok(trimmed)
}

fn validate_text(text: &str) -> Result<(), ExperienceError> {
    if text.trim().is_empty() {
        return Err(ExperienceError::InvalidText("empty text"));
    }
    if contains_disallowed_control(text) {
        return Err(ExperienceError::InvalidText(
            "invisible or bidirectional control character",
        ));
    }
    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, thiserror::Error)]
pub enum ExperienceError {
    #[error("experience store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("experience store JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("source run is not auditable: {0:?}")]
    UnauditedSource(Vec<String>),
    #[error("at least one evidence event is required")]
    NoEvidence,
    #[error("evidence event sequence {0} does not exist in source run")]
    UnknownEvidenceSeq(u64),
    #[error("candidate {0} not found")]
    CandidateNotFound(Uuid),
    #[error("duplicate candidate id")]
    DuplicateCandidate,
    #[error("invalid status transition from {from:?}; allowed: {allowed_from:?}")]
    InvalidTransition {
        from: ExperienceStatus,
        allowed_from: Vec<ExperienceStatus>,
    },
    #[error("invalid text: {0}")]
    InvalidText(&'static str),
    #[error("invalid verification: {0}")]
    InvalidVerification(String),
    #[error("candidate verification failed held-out transfer gates")]
    VerificationFailed,
    #[error("unsupported experience schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid event sequence: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentAnswer, CognitiveMode, TaskRequest, ToolObservation, ToolStatus};
    use crate::journal::{Journal, JournalEventKind};
    use std::collections::BTreeMap;

    fn source_events(dir: &Path) -> Vec<JournalEvent> {
        let path = dir.join("run.jsonl");
        let run_id = Uuid::new_v4();
        let mut journal = Journal::create(&path, run_id).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("learn", dir.to_path_buf()),
                mode: CognitiveMode::EvidenceResearch,
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolCalled {
                call: crate::domain::ToolCall {
                    id: "call-1".into(),
                    name: "read_file".into(),
                    arguments: serde_json::json!({"path":"evidence.txt"}),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "call-1".into(),
                    tool: "read_file".into(),
                    status: ToolStatus::Ok,
                    summary: "verified".into(),
                    content: "verified fact".into(),
                    error_code: None,
                    metadata: BTreeMap::new(),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: AgentAnswer {
                    status: crate::domain::AnswerStatus::Completed,
                    conclusion: "done".into(),
                    claims: Vec::new(),
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: crate::domain::StopReason::Completed,
            })
            .unwrap();
        drop(journal);
        crate::journal::read_events(path).unwrap()
    }

    fn spec() -> ExperienceCandidateSpec {
        ExperienceCandidateSpec {
            kind: ExperienceKind::Skill,
            title: "Read evidence before deciding".into(),
            content: "When a decision depends on repository facts, inspect the cited file first."
                .into(),
            scope: "repository decisions".into(),
            evidence_event_seqs: vec![2],
            sensitive: false,
            applicable_modes: all_modes(),
            expires_at_unix_ms: None,
            supersedes: None,
        }
    }

    fn verification() -> ExperienceVerificationInput {
        ExperienceVerificationInput {
            suite_id: "heldout-v1".into(),
            suite_version: "1".into(),
            case_ids: vec!["case-1".into()],
            suite_sha256: "a".repeat(64),
            training_group: "A".into(),
            evaluation_group: "B".into(),
            baseline_success_bps: 5_000,
            learned_success_bps: 6_000,
            baseline_unsupported_bps: 1_000,
            learned_unsupported_bps: 800,
            cost_delta_bps: 200,
            evidence: "evaluation report".into(),
        }
    }

    #[test]
    fn candidate_requires_review_and_approval_before_activation() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let candidate = store.propose(spec(), &events).unwrap();
        assert_eq!(candidate.status, ExperienceStatus::Draft);
        assert!(matches!(
            store.activate(candidate.id),
            Err(ExperienceError::InvalidTransition { .. })
        ));
        store.review(candidate.id, "reviewer").unwrap();
        store.verify(candidate.id, verification()).unwrap();
        store.approve(candidate.id, "owner").unwrap();
        store.activate(candidate.id).unwrap();
        let candidates = store.candidates().unwrap();
        assert_eq!(candidates[&candidate.id].status, ExperienceStatus::Active);
    }

    #[test]
    fn candidate_with_unknown_evidence_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let mut candidate = spec();
        candidate.evidence_event_seqs = vec![999];
        assert!(matches!(
            store.propose(candidate, &events),
            Err(ExperienceError::UnknownEvidenceSeq(999))
        ));
    }

    #[test]
    fn active_candidate_can_be_degraded_and_rolled_back() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let candidate = store.propose(spec(), &events).unwrap();
        store.review(candidate.id, "reviewer").unwrap();
        store.verify(candidate.id, verification()).unwrap();
        store.approve(candidate.id, "owner").unwrap();
        store.activate(candidate.id).unwrap();
        store.degrade(candidate.id, "held-out regression").unwrap();
        store
            .rollback(candidate.id, "reverted after regression")
            .unwrap();
        let candidates = store.candidates().unwrap();
        assert_eq!(
            candidates[&candidate.id].status,
            ExperienceStatus::RolledBack
        );
    }

    #[test]
    fn invisible_instructions_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let mut candidate = spec();
        candidate.content.push('\u{202E}');
        assert!(matches!(
            store.propose(candidate, &events),
            Err(ExperienceError::InvalidText(_))
        ));
    }

    #[test]
    fn retrieval_returns_only_active_scope_and_mode_matches() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let active = store.propose(spec(), &events).unwrap();
        store.review(active.id, "reviewer").unwrap();
        store.verify(active.id, verification()).unwrap();
        store.approve(active.id, "owner").unwrap();
        store.activate(active.id).unwrap();
        let mut draft = spec();
        draft.title = "Draft only".into();
        draft.scope = "repository decisions".into();
        let draft = store.propose(draft, &events).unwrap();
        let mut wrong_scope = spec();
        wrong_scope.title = "Wrong scope".into();
        wrong_scope.scope = "finance".into();
        let wrong_scope = store.propose(wrong_scope, &events).unwrap();
        store.review(wrong_scope.id, "reviewer").unwrap();
        store.verify(wrong_scope.id, verification()).unwrap();
        store.approve(wrong_scope.id, "owner").unwrap();
        store.activate(wrong_scope.id).unwrap();

        let retrieved = store
            .retrieve_active(
                crate::domain::CognitiveMode::EvidenceResearch,
                "repository decisions",
                1_000,
            )
            .unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].id, active.id);
        assert_ne!(retrieved[0].id, draft.id);
    }

    #[test]
    fn verification_rejects_leakage_or_increased_false_certainty() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let candidate = store.propose(spec(), &events).unwrap();
        store.review(candidate.id, "reviewer").unwrap();
        let mut leaked = verification();
        leaked.evaluation_group = leaked.training_group.clone();
        assert!(matches!(
            store.verify(candidate.id, leaked),
            Err(ExperienceError::InvalidVerification(_))
        ));
        let mut unsafe_confidence = verification();
        unsafe_confidence.learned_unsupported_bps = 1_200;
        assert!(matches!(
            store.verify(candidate.id, unsafe_confidence),
            Err(ExperienceError::VerificationFailed)
        ));
    }
}
