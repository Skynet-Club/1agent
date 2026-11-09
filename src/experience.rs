use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::CognitiveMode;
use crate::domain::PROTOCOL_VERSION;
use crate::journal::{JournalEvent, JournalEventKind, audit_events};
use crate::safety::contains_disallowed_control;

const MAX_EXPERIENCE_TITLE_CHARS: usize = 200;
const MAX_EXPERIENCE_CONTENT_CHARS: usize = 4_000;
const MAX_EXPERIENCE_SCOPE_CHARS: usize = 200;
const MAX_EVIDENCE_REFERENCES: usize = 32;
const MAX_EXPERIENCE_ACTOR_CHARS: usize = 128;
const MAX_EXPERIENCE_REASON_CHARS: usize = 1_000;
const MAX_EXPERIENCE_EVENT_BYTES: usize = 64 * 1024;
pub const EXPERIENCE_AUDIT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExperienceAuditFindingCode {
    Io,
    InvalidJson,
    UnsupportedSchema,
    InvalidSequence,
    EventTooLarge,
    NonMonotonicTimestamp,
    InvalidCandidate,
    InvalidTransition,
    InvalidVerification,
    InvalidEvidence,
    InvalidMetadata,
    InvalidSupersession,
    OrphanedSupersession,
    SupersessionCycle,
    ExpiredActive,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceAuditFinding {
    pub code: ExperienceAuditFindingCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceAuditReport {
    pub schema_version: u32,
    pub ok: bool,
    pub event_count: usize,
    pub candidate_count: usize,
    #[serde(default)]
    pub status_counts: ExperienceStatusCounts,
    #[serde(default)]
    pub supersession_chains: usize,
    #[serde(default)]
    pub max_supersession_depth: usize,
    #[serde(default)]
    pub expired_active: usize,
    pub findings: Vec<ExperienceAuditFinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceStatusCounts {
    pub draft: usize,
    pub reviewed: usize,
    pub verified: usize,
    pub approved: usize,
    pub rejected: usize,
    pub active: usize,
    pub superseded: usize,
    pub degraded: usize,
    pub rolled_back: usize,
}

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
    Superseded,
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
    #[serde(default)]
    pub evidence_sha256: String,
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
        #[serde(default)]
        supersedes: Option<Uuid>,
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
                | ExperienceEventKind::Activated { id, .. }
                | ExperienceEventKind::Degraded { id, .. }
                | ExperienceEventKind::RolledBack { id, .. } => *id,
            };
            match event.event {
                ExperienceEventKind::Proposed { candidate } => {
                    validate_proposed_candidate(&candidate)?;
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
                ExperienceEventKind::Activated { id, supersedes } => {
                    let candidate = candidates
                        .get(&id)
                        .ok_or(ExperienceError::CandidateNotFound(id))?
                        .clone();
                    if candidate.supersedes != supersedes {
                        return Err(ExperienceError::SupersessionMismatch(id));
                    }
                    if let Some(previous_id) = supersedes {
                        let previous = candidates
                            .get(&previous_id)
                            .ok_or(ExperienceError::SupersededExperienceNotFound(previous_id))?;
                        if previous.kind != candidate.kind {
                            return Err(ExperienceError::SupersessionKindMismatch(previous_id));
                        }
                        if previous.scope != candidate.scope {
                            return Err(ExperienceError::SupersessionScopeMismatch(previous_id));
                        }
                        if candidate.version != previous.version.saturating_add(1) {
                            return Err(ExperienceError::SupersessionVersionMismatch {
                                previous: previous.version,
                                candidate: candidate.version,
                            });
                        }
                        if candidates.values().any(|other| {
                            other.id != previous_id
                                && other.status == ExperienceStatus::Active
                                && other.kind == candidate.kind
                                && other.scope == candidate.scope
                        }) {
                            return Err(ExperienceError::ActiveExperienceConflict);
                        }
                        transition(
                            &mut candidates,
                            previous_id,
                            &[ExperienceStatus::Active],
                            |previous| previous.status = ExperienceStatus::Superseded,
                        )?;
                    } else if candidates.values().any(|other| {
                        other.id != id
                            && other.status == ExperienceStatus::Active
                            && other.kind == candidate.kind
                            && other.scope == candidate.scope
                    }) {
                        return Err(ExperienceError::ActiveExperienceConflict);
                    }
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
        validate_candidate_shape(
            &spec.title,
            &spec.content,
            &spec.scope,
            spec.evidence_event_seqs.len(),
            &spec.applicable_modes,
        )?;

        let source_run_id = source_events
            .first()
            .map(|event| event.run_id)
            .ok_or(ExperienceError::NoEvidence)?;
        let unique_evidence_seqs: std::collections::BTreeSet<_> =
            spec.evidence_event_seqs.iter().copied().collect();
        if unique_evidence_seqs.len() != spec.evidence_event_seqs.len() {
            return Err(ExperienceError::DuplicateEvidenceSeq);
        }
        for seq in &spec.evidence_event_seqs {
            let Some(event) = source_events.iter().find(|event| event.seq == *seq) else {
                return Err(ExperienceError::UnknownEvidenceSeq(*seq));
            };
            if !is_evidence_event(&event.event) {
                return Err(ExperienceError::InvalidEvidenceKind(*seq));
            }
        }
        let evidence_sha256 = evidence_digest(source_events, &spec.evidence_event_seqs)?;

        let timestamp = now_unix_ms();
        if spec
            .expires_at_unix_ms
            .is_some_and(|expires_at| expires_at <= timestamp)
        {
            return Err(ExperienceError::ExpiredSpecification);
        }
        let candidates = self.candidates()?;
        let version = if let Some(previous_id) = spec.supersedes {
            supersession_target(&candidates, previous_id, spec.kind, &spec.scope)?
                .version
                .saturating_add(1)
        } else {
            1
        };
        let candidate = ExperienceCandidate {
            id: Uuid::new_v4(),
            version,
            kind: spec.kind,
            title: spec.title,
            content: spec.content,
            scope: spec.scope,
            source_run_id,
            evidence_event_seqs: spec.evidence_event_seqs,
            evidence_sha256,
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
        self.review_with_source(id, actor, None)
    }

    pub fn review_with_source(
        &mut self,
        id: Uuid,
        actor: &str,
        source_events: Option<&[JournalEvent]>,
    ) -> Result<(), ExperienceError> {
        let candidate = self.get(id)?;
        validate_evidence_source(&candidate, source_events)?;
        self.transition(id, &[ExperienceStatus::Draft], |_| {
            Ok(ExperienceEventKind::Reviewed {
                id,
                actor: validate_actor(actor)?.to_string(),
            })
        })
    }

    pub fn approve(&mut self, id: Uuid, actor: &str) -> Result<(), ExperienceError> {
        self.approve_with_source(id, actor, None)
    }

    pub fn approve_with_source(
        &mut self,
        id: Uuid,
        actor: &str,
        source_events: Option<&[JournalEvent]>,
    ) -> Result<(), ExperienceError> {
        let candidate = self.get(id)?;
        validate_evidence_source(&candidate, source_events)?;
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
        validate_bounded_text(
            &input.evidence,
            "verification evidence",
            MAX_EXPERIENCE_REASON_CHARS,
        )?;
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
        validate_bounded_text(reason, "reason", MAX_EXPERIENCE_REASON_CHARS)?;
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
        self.activate_with_source(id, None)
    }

    pub fn activate_with_source(
        &mut self,
        id: Uuid,
        source_events: Option<&[JournalEvent]>,
    ) -> Result<(), ExperienceError> {
        let candidates = self.candidates()?;
        let candidate = candidates
            .get(&id)
            .ok_or(ExperienceError::CandidateNotFound(id))?;
        if candidate.status != ExperienceStatus::Approved {
            return Err(ExperienceError::InvalidTransition {
                from: candidate.status,
                allowed_from: vec![ExperienceStatus::Approved],
            });
        }
        validate_evidence_source(candidate, source_events)?;
        if candidate
            .expires_at_unix_ms
            .is_some_and(|expires_at| expires_at <= now_unix_ms())
        {
            return Err(ExperienceError::ExpiredCandidate(id));
        }
        if let Some(previous_id) = candidate.supersedes {
            let previous =
                supersession_target(&candidates, previous_id, candidate.kind, &candidate.scope)?;
            if candidate.version != previous.version.saturating_add(1) {
                return Err(ExperienceError::SupersessionVersionMismatch {
                    previous: previous.version,
                    candidate: candidate.version,
                });
            }
            if candidates.values().any(|other| {
                other.id != previous_id
                    && other.status == ExperienceStatus::Active
                    && other.kind == candidate.kind
                    && other.scope == candidate.scope
            }) {
                return Err(ExperienceError::ActiveExperienceConflict);
            }
        } else if candidates.values().any(|other| {
            other.status == ExperienceStatus::Active
                && other.kind == candidate.kind
                && other.scope == candidate.scope
        }) {
            return Err(ExperienceError::ActiveExperienceConflict);
        }
        self.append(ExperienceEventKind::Activated {
            id,
            supersedes: candidate.supersedes,
        })
    }

    pub fn degrade(&mut self, id: Uuid, reason: &str) -> Result<(), ExperienceError> {
        validate_bounded_text(reason, "reason", MAX_EXPERIENCE_REASON_CHARS)?;
        self.transition(id, &[ExperienceStatus::Active], |_| {
            Ok(ExperienceEventKind::Degraded {
                id,
                reason: reason.to_string(),
            })
        })
    }

    pub fn rollback(&mut self, id: Uuid, reason: &str) -> Result<(), ExperienceError> {
        validate_bounded_text(reason, "reason", MAX_EXPERIENCE_REASON_CHARS)?;
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
        Ok(select_applicable(
            self.candidates()?.into_values().collect(),
            mode,
            scope,
            max_chars,
            false,
        ))
    }

    pub fn get(&self, id: Uuid) -> Result<ExperienceCandidate, ExperienceError> {
        self.candidates()?
            .remove(&id)
            .ok_or(ExperienceError::CandidateNotFound(id))
    }

    pub fn audit(&self) -> ExperienceAuditReport {
        let events = match self.read_events() {
            Ok(events) => events,
            Err(error) => {
                return ExperienceAuditReport {
                    schema_version: EXPERIENCE_AUDIT_SCHEMA_VERSION,
                    ok: false,
                    event_count: 0,
                    candidate_count: 0,
                    status_counts: ExperienceStatusCounts::default(),
                    supersession_chains: 0,
                    max_supersession_depth: 0,
                    expired_active: 0,
                    findings: vec![ExperienceAuditFinding {
                        code: experience_audit_code(&error),
                        message: error.to_string(),
                    }],
                };
            }
        };
        match self.candidates() {
            Ok(candidates) => {
                let (summary, findings) = audit_lifecycle(&candidates);
                ExperienceAuditReport {
                    schema_version: EXPERIENCE_AUDIT_SCHEMA_VERSION,
                    ok: findings.is_empty(),
                    event_count: events.len(),
                    candidate_count: candidates.len(),
                    status_counts: summary.status_counts,
                    supersession_chains: summary.supersession_chains,
                    max_supersession_depth: summary.max_supersession_depth,
                    expired_active: summary.expired_active,
                    findings,
                }
            }
            Err(error) => ExperienceAuditReport {
                schema_version: EXPERIENCE_AUDIT_SCHEMA_VERSION,
                ok: false,
                event_count: events.len(),
                candidate_count: 0,
                status_counts: ExperienceStatusCounts::default(),
                supersession_chains: 0,
                max_supersession_depth: 0,
                expired_active: 0,
                findings: vec![ExperienceAuditFinding {
                    code: experience_audit_code(&error),
                    message: error.to_string(),
                }],
            },
        }
    }

    pub fn validate_for_shadow(&self, id: Uuid) -> Result<ExperienceCandidate, ExperienceError> {
        let candidate = self.get(id)?;
        if candidate.status != ExperienceStatus::Reviewed {
            return Err(ExperienceError::InvalidTransition {
                from: candidate.status,
                allowed_from: vec![ExperienceStatus::Reviewed],
            });
        }
        Ok(candidate)
    }

    pub fn retrieve_shadow(
        &self,
        candidates: Vec<ExperienceCandidate>,
        mode: CognitiveMode,
        scope: &str,
        max_chars: usize,
    ) -> Vec<ExperienceCandidate> {
        select_applicable(candidates, mode, scope, max_chars, true)
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
        let bytes = serde_json::to_vec(&envelope)?;
        if bytes.len() > MAX_EXPERIENCE_EVENT_BYTES {
            return Err(ExperienceError::EventTooLarge(bytes.len()));
        }
        writer.write_all(&bytes)?;
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
        let mut previous_timestamp = None;
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line?;
            if line.len() > MAX_EXPERIENCE_EVENT_BYTES {
                return Err(ExperienceError::EventTooLarge(line.len()));
            }
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
            if let Some(previous) = previous_timestamp
                && event.timestamp_unix_ms < previous
            {
                return Err(ExperienceError::NonMonotonicTimestamp {
                    line: index + 1,
                    previous,
                    actual: event.timestamp_unix_ms,
                });
            }
            previous_timestamp = Some(event.timestamp_unix_ms);
            validate_transition_metadata(&event.event)?;
            events.push(event);
        }
        Ok(events)
    }
}

fn select_applicable(
    mut candidates: Vec<ExperienceCandidate>,
    mode: CognitiveMode,
    scope: &str,
    max_chars: usize,
    allow_reviewed: bool,
) -> Vec<ExperienceCandidate> {
    let now = now_unix_ms();
    candidates.retain(|candidate| {
        let status_allowed = candidate.status == ExperienceStatus::Active
            || (allow_reviewed && candidate.status == ExperienceStatus::Reviewed);
        status_allowed
            && candidate.applicable_modes.contains(&mode)
            && (candidate.scope == "*" || candidate.scope == scope)
            && candidate
                .expires_at_unix_ms
                .is_none_or(|expires_at| expires_at > now)
    });
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
    selected
}

fn validate_proposed_candidate(candidate: &ExperienceCandidate) -> Result<(), ExperienceError> {
    if candidate.status != ExperienceStatus::Draft || candidate.verification.is_some() {
        return Err(ExperienceError::InvalidProposedStatus);
    }
    if candidate.version == 0 {
        return Err(ExperienceError::InvalidCandidateVersion);
    }
    if candidate.source_run_id == Uuid::nil() {
        return Err(ExperienceError::InvalidSourceRunId);
    }
    if candidate.evidence_event_seqs.is_empty() {
        return Err(ExperienceError::NoEvidence);
    }
    let unique_sequences: std::collections::BTreeSet<_> =
        candidate.evidence_event_seqs.iter().copied().collect();
    if unique_sequences.len() != candidate.evidence_event_seqs.len() {
        return Err(ExperienceError::DuplicateEvidenceSeq);
    }
    if !candidate.evidence_sha256.is_empty()
        && (candidate.evidence_sha256.len() != 64
            || !candidate
                .evidence_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit()))
    {
        return Err(ExperienceError::InvalidEvidenceDigest);
    }
    validate_text(&candidate.title)?;
    validate_text(&candidate.content)?;
    validate_text(&candidate.scope)?;
    validate_candidate_shape(
        &candidate.title,
        &candidate.content,
        &candidate.scope,
        candidate.evidence_event_seqs.len(),
        &candidate.applicable_modes,
    )?;
    Ok(())
}

fn validate_candidate_shape(
    title: &str,
    content: &str,
    scope: &str,
    evidence_references: usize,
    modes: &[CognitiveMode],
) -> Result<(), ExperienceError> {
    if title.chars().count() > MAX_EXPERIENCE_TITLE_CHARS {
        return Err(ExperienceError::TextTooLong("title"));
    }
    if content.chars().count() > MAX_EXPERIENCE_CONTENT_CHARS {
        return Err(ExperienceError::TextTooLong("content"));
    }
    if scope.chars().count() > MAX_EXPERIENCE_SCOPE_CHARS {
        return Err(ExperienceError::TextTooLong("scope"));
    }
    if evidence_references > MAX_EVIDENCE_REFERENCES {
        return Err(ExperienceError::TooManyEvidenceReferences);
    }
    if modes.is_empty() {
        return Err(ExperienceError::EmptyApplicableModes);
    }
    let unique_modes = modes
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if unique_modes.len() != modes.len() {
        return Err(ExperienceError::DuplicateApplicableMode);
    }
    Ok(())
}

fn evidence_digest(
    source_events: &[JournalEvent],
    evidence_event_seqs: &[u64],
) -> Result<String, ExperienceError> {
    let events_by_seq = source_events
        .iter()
        .map(|event| (event.seq, event))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = evidence_event_seqs.to_vec();
    ordered.sort_unstable();
    let mut hasher = Sha256::new();
    hasher.update(b"1agent-experience-evidence-v1\0");
    for seq in ordered {
        let event = events_by_seq
            .get(&seq)
            .ok_or(ExperienceError::UnknownEvidenceSeq(seq))?;
        let bytes = serde_json::to_vec(event)?;
        hasher.update(seq.to_be_bytes());
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn is_evidence_event(event: &JournalEventKind) -> bool {
    matches!(
        event,
        JournalEventKind::ToolObserved { .. }
            | JournalEventKind::AnswerProduced { .. }
            | JournalEventKind::ProviderFailed { .. }
    )
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

fn supersession_target<'a>(
    candidates: &'a BTreeMap<Uuid, ExperienceCandidate>,
    previous_id: Uuid,
    kind: ExperienceKind,
    scope: &str,
) -> Result<&'a ExperienceCandidate, ExperienceError> {
    let previous = candidates
        .get(&previous_id)
        .ok_or(ExperienceError::SupersededExperienceNotFound(previous_id))?;
    if previous.kind != kind {
        return Err(ExperienceError::SupersessionKindMismatch(previous_id));
    }
    if previous.scope != scope {
        return Err(ExperienceError::SupersessionScopeMismatch(previous_id));
    }
    if previous.status != ExperienceStatus::Active {
        return Err(ExperienceError::SupersededExperienceNotActive(previous_id));
    }
    Ok(previous)
}

fn validate_evidence_source(
    candidate: &ExperienceCandidate,
    source_events: Option<&[JournalEvent]>,
) -> Result<(), ExperienceError> {
    if candidate.evidence_sha256.is_empty() {
        return Ok(());
    }
    let source_events = source_events.ok_or(ExperienceError::MissingSourceEvidence)?;
    let audit = audit_events(source_events);
    if !audit.ok {
        return Err(ExperienceError::UnauditedSource(audit.issues));
    }
    if source_events.first().map(|event| event.run_id) != Some(candidate.source_run_id) {
        return Err(ExperienceError::SourceRunMismatch);
    }
    let actual = evidence_digest(source_events, &candidate.evidence_event_seqs)?;
    if actual != candidate.evidence_sha256 {
        return Err(ExperienceError::EvidenceDigestMismatch);
    }
    Ok(())
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
    validate_bounded_text(trimmed, "actor", MAX_EXPERIENCE_ACTOR_CHARS)?;
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

fn validate_bounded_text(
    text: &str,
    label: &'static str,
    max_chars: usize,
) -> Result<(), ExperienceError> {
    validate_text(text)?;
    if text.chars().count() > max_chars {
        return Err(ExperienceError::TextTooLong(label));
    }
    Ok(())
}

fn validate_transition_metadata(event: &ExperienceEventKind) -> Result<(), ExperienceError> {
    match event {
        ExperienceEventKind::Reviewed { actor, .. }
        | ExperienceEventKind::Approved { actor, .. } => {
            validate_actor(actor)?;
        }
        ExperienceEventKind::Rejected { actor, reason, .. } => {
            validate_actor(actor)?;
            validate_bounded_text(reason, "reason", MAX_EXPERIENCE_REASON_CHARS)?;
        }
        ExperienceEventKind::Degraded { reason, .. }
        | ExperienceEventKind::RolledBack { reason, .. } => {
            validate_bounded_text(reason, "reason", MAX_EXPERIENCE_REASON_CHARS)?;
        }
        ExperienceEventKind::Verified { verification, .. } => {
            validate_bounded_text(
                &verification.evidence,
                "verification evidence",
                MAX_EXPERIENCE_REASON_CHARS,
            )?;
        }
        ExperienceEventKind::Proposed { .. } | ExperienceEventKind::Activated { .. } => {}
    }
    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn experience_audit_code(error: &ExperienceError) -> ExperienceAuditFindingCode {
    match error {
        ExperienceError::Io(_) => ExperienceAuditFindingCode::Io,
        ExperienceError::Json(_) => ExperienceAuditFindingCode::InvalidJson,
        ExperienceError::UnsupportedSchema(_) => ExperienceAuditFindingCode::UnsupportedSchema,
        ExperienceError::InvalidSequence { .. } => ExperienceAuditFindingCode::InvalidSequence,
        ExperienceError::EventTooLarge(_) => ExperienceAuditFindingCode::EventTooLarge,
        ExperienceError::NonMonotonicTimestamp { .. } => {
            ExperienceAuditFindingCode::NonMonotonicTimestamp
        }
        ExperienceError::DuplicateCandidate
        | ExperienceError::CandidateNotFound(_)
        | ExperienceError::InvalidProposedStatus
        | ExperienceError::InvalidCandidateVersion
        | ExperienceError::InvalidSourceRunId
        | ExperienceError::EmptyApplicableModes
        | ExperienceError::DuplicateApplicableMode => ExperienceAuditFindingCode::InvalidCandidate,
        ExperienceError::InvalidTransition { .. } => ExperienceAuditFindingCode::InvalidTransition,
        ExperienceError::InvalidVerification(_) | ExperienceError::VerificationFailed => {
            ExperienceAuditFindingCode::InvalidVerification
        }
        ExperienceError::NoEvidence
        | ExperienceError::DuplicateEvidenceSeq
        | ExperienceError::UnknownEvidenceSeq(_)
        | ExperienceError::MissingSourceEvidence
        | ExperienceError::SourceRunMismatch
        | ExperienceError::EvidenceDigestMismatch
        | ExperienceError::InvalidEvidenceDigest
        | ExperienceError::InvalidEvidenceKind(_) => ExperienceAuditFindingCode::InvalidEvidence,
        ExperienceError::InvalidText(_)
        | ExperienceError::TextTooLong(_)
        | ExperienceError::TooManyEvidenceReferences => ExperienceAuditFindingCode::InvalidMetadata,
        ExperienceError::ExpiredSpecification
        | ExperienceError::ExpiredCandidate(_)
        | ExperienceError::SupersededExperienceNotFound(_)
        | ExperienceError::SupersessionKindMismatch(_)
        | ExperienceError::SupersessionScopeMismatch(_)
        | ExperienceError::SupersededExperienceNotActive(_)
        | ExperienceError::SupersessionMismatch(_)
        | ExperienceError::SupersessionVersionMismatch { .. }
        | ExperienceError::ActiveExperienceConflict => {
            ExperienceAuditFindingCode::InvalidSupersession
        }
        ExperienceError::UnauditedSource(_) => ExperienceAuditFindingCode::InvalidEvidence,
    }
}

struct LifecycleAuditSummary {
    status_counts: ExperienceStatusCounts,
    supersession_chains: usize,
    max_supersession_depth: usize,
    expired_active: usize,
}

fn audit_lifecycle(
    candidates: &BTreeMap<Uuid, ExperienceCandidate>,
) -> (LifecycleAuditSummary, Vec<ExperienceAuditFinding>) {
    let now = now_unix_ms();
    let mut summary = LifecycleAuditSummary {
        status_counts: ExperienceStatusCounts::default(),
        supersession_chains: 0,
        max_supersession_depth: 0,
        expired_active: 0,
    };
    let mut findings = Vec::new();
    for candidate in candidates.values() {
        let count = match candidate.status {
            ExperienceStatus::Draft => &mut summary.status_counts.draft,
            ExperienceStatus::Reviewed => &mut summary.status_counts.reviewed,
            ExperienceStatus::Verified => &mut summary.status_counts.verified,
            ExperienceStatus::Approved => &mut summary.status_counts.approved,
            ExperienceStatus::Rejected => &mut summary.status_counts.rejected,
            ExperienceStatus::Active => &mut summary.status_counts.active,
            ExperienceStatus::Superseded => &mut summary.status_counts.superseded,
            ExperienceStatus::Degraded => &mut summary.status_counts.degraded,
            ExperienceStatus::RolledBack => &mut summary.status_counts.rolled_back,
        };
        *count += 1;
        if candidate.status == ExperienceStatus::Active
            && candidate
                .expires_at_unix_ms
                .is_some_and(|expires_at| expires_at <= now)
        {
            summary.expired_active += 1;
            findings.push(ExperienceAuditFinding {
                code: ExperienceAuditFindingCode::ExpiredActive,
                message: format!("active experience {} is expired", candidate.id),
            });
        }
    }

    for candidate in candidates
        .values()
        .filter(|candidate| candidate.supersedes.is_some())
    {
        summary.supersession_chains += 1;
        let mut visited = std::collections::BTreeSet::new();
        let mut current_id = Some(candidate.id);
        let mut depth = 0usize;
        while let Some(id) = current_id {
            if !visited.insert(id) {
                findings.push(ExperienceAuditFinding {
                    code: ExperienceAuditFindingCode::SupersessionCycle,
                    message: format!("supersession cycle includes experience {id}"),
                });
                break;
            }
            let Some(current) = candidates.get(&id) else {
                findings.push(ExperienceAuditFinding {
                    code: ExperienceAuditFindingCode::OrphanedSupersession,
                    message: format!("supersession references missing experience {id}"),
                });
                break;
            };
            let Some(previous_id) = current.supersedes else {
                break;
            };
            depth += 1;
            if depth > candidates.len() {
                findings.push(ExperienceAuditFinding {
                    code: ExperienceAuditFindingCode::SupersessionCycle,
                    message: format!("supersession chain from {} is cyclic", candidate.id),
                });
                break;
            }
            current_id = Some(previous_id);
        }
        summary.max_supersession_depth = summary.max_supersession_depth.max(depth);
    }
    (summary, findings)
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
    #[error("experience evidence sequences must be unique")]
    DuplicateEvidenceSeq,
    #[error("evidence event sequence {0} does not exist in source run")]
    UnknownEvidenceSeq(u64),
    #[error("evidence event sequence {0} is lifecycle metadata, not evidence")]
    InvalidEvidenceKind(u64),
    #[error("source events are required to validate bound experience evidence")]
    MissingSourceEvidence,
    #[error("source run does not match the candidate")]
    SourceRunMismatch,
    #[error("source evidence digest does not match the candidate")]
    EvidenceDigestMismatch,
    #[error("candidate {0} not found")]
    CandidateNotFound(Uuid),
    #[error("proposed candidate must be an unverified draft")]
    InvalidProposedStatus,
    #[error("candidate version must be greater than zero")]
    InvalidCandidateVersion,
    #[error("candidate source run id must not be nil")]
    InvalidSourceRunId,
    #[error("candidate evidence digest must be 64 hexadecimal characters")]
    InvalidEvidenceDigest,
    #[error("candidate must support at least one cognitive mode")]
    EmptyApplicableModes,
    #[error("duplicate candidate id")]
    DuplicateCandidate,
    #[error("invalid status transition from {from:?}; allowed: {allowed_from:?}")]
    InvalidTransition {
        from: ExperienceStatus,
        allowed_from: Vec<ExperienceStatus>,
    },
    #[error("invalid text: {0}")]
    InvalidText(&'static str),
    #[error("experience {0} exceeds the maximum length")]
    TextTooLong(&'static str),
    #[error("experience proposal has too many evidence references")]
    TooManyEvidenceReferences,
    #[error("experience proposal repeats an applicable mode")]
    DuplicateApplicableMode,
    #[error("invalid verification: {0}")]
    InvalidVerification(String),
    #[error("candidate verification failed held-out transfer gates")]
    VerificationFailed,
    #[error("experience expiration must be in the future")]
    ExpiredSpecification,
    #[error("expired experience {0} cannot be activated")]
    ExpiredCandidate(Uuid),
    #[error("superseded experience {0} not found")]
    SupersededExperienceNotFound(Uuid),
    #[error("superseded experience {0} has a different kind")]
    SupersessionKindMismatch(Uuid),
    #[error("superseded experience {0} has a different scope")]
    SupersessionScopeMismatch(Uuid),
    #[error("superseded experience {0} is not active")]
    SupersededExperienceNotActive(Uuid),
    #[error("activation event supersession does not match candidate {0}")]
    SupersessionMismatch(Uuid),
    #[error("superseding candidate version must be {previous}, got {candidate}")]
    SupersessionVersionMismatch { previous: u32, candidate: u32 },
    #[error("another active experience already exists for this kind and scope")]
    ActiveExperienceConflict,
    #[error("unsupported experience schema version {0}")]
    UnsupportedSchema(u32),
    #[error("invalid event sequence: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },
    #[error("experience event is too large: {0} bytes")]
    EventTooLarge(usize),
    #[error("non-monotonic timestamp at line {line}: previous {previous}, actual {actual}")]
    NonMonotonicTimestamp {
        line: usize,
        previous: u128,
        actual: u128,
    },
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
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: crate::domain::AnswerStatus::Completed,
                    conclusion: "done".into(),
                    claims: Vec::new(),
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
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

    fn write_experience_events(store: &ExperienceStore, events: &[ExperienceEvent]) {
        let mut content = String::new();
        for event in events {
            content.push_str(&serde_json::to_string(event).unwrap());
            content.push('\n');
        }
        std::fs::write(store.path(), content).unwrap();
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

    fn approve_and_activate(store: &mut ExperienceStore, id: Uuid, source_events: &[JournalEvent]) {
        store
            .review_with_source(id, "reviewer", Some(source_events))
            .unwrap();
        store.verify(id, verification()).unwrap();
        store
            .approve_with_source(id, "owner", Some(source_events))
            .unwrap();
        store.activate_with_source(id, Some(source_events)).unwrap();
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
        store
            .review_with_source(candidate.id, "reviewer", Some(&events))
            .unwrap();
        store.verify(candidate.id, verification()).unwrap();
        store
            .approve_with_source(candidate.id, "owner", Some(&events))
            .unwrap();
        store
            .activate_with_source(candidate.id, Some(&events))
            .unwrap();
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
    fn experience_evidence_must_be_observation_or_answer() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let mut lifecycle_only = spec();
        lifecycle_only.evidence_event_seqs = vec![0];
        assert!(matches!(
            store.propose(lifecycle_only, &events),
            Err(ExperienceError::InvalidEvidenceKind(0))
        ));

        let mut answer_evidence = spec();
        answer_evidence.evidence_event_seqs = vec![3];
        store.propose(answer_evidence, &events).unwrap();
    }

    #[test]
    fn rewritten_source_evidence_is_rejected_after_proposal() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let candidate = store.propose(spec(), &events).unwrap();
        assert_eq!(candidate.evidence_sha256.len(), 64);
        assert!(matches!(
            store.review(candidate.id, "reviewer"),
            Err(ExperienceError::MissingSourceEvidence)
        ));

        let mut rewritten = events.clone();
        let observation = rewritten
            .iter_mut()
            .find_map(|event| match &mut event.event {
                JournalEventKind::ToolObserved { observation } => Some(observation),
                _ => None,
            })
            .unwrap();
        observation.content = "rewritten fact".into();
        let mut journal = String::new();
        for event in rewritten {
            journal.push_str(&serde_json::to_string(&event).unwrap());
            journal.push('\n');
        }
        std::fs::write(dir.path().join("run.jsonl"), journal).unwrap();
        let rewritten = crate::journal::read_events(dir.path().join("run.jsonl")).unwrap();
        assert!(matches!(
            store.review_with_source(candidate.id, "reviewer", Some(&rewritten)),
            Err(ExperienceError::EvidenceDigestMismatch)
        ));
    }

    #[test]
    fn replay_rejects_tampered_proposal_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        store.propose(spec(), &events).unwrap();
        let path = store.path().to_path_buf();
        let original = std::fs::read_to_string(&path).unwrap();
        let mut event: ExperienceEvent = serde_json::from_str(original.trim()).unwrap();
        let ExperienceEventKind::Proposed { candidate } = &mut event.event else {
            panic!("expected proposed event");
        };
        candidate.evidence_sha256 = "not-a-digest".into();
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .unwrap();
        let invalid_digest = store.candidates();
        assert!(
            matches!(invalid_digest, Err(ExperienceError::InvalidEvidenceDigest)),
            "{invalid_digest:?}"
        );

        let mut event: ExperienceEvent = serde_json::from_str(original.trim()).unwrap();
        let ExperienceEventKind::Proposed { candidate } = &mut event.event else {
            panic!("expected proposed event");
        };
        candidate.status = ExperienceStatus::Active;
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            store.candidates(),
            Err(ExperienceError::InvalidProposedStatus)
        ));

        let mut event: ExperienceEvent = serde_json::from_str(original.trim()).unwrap();
        let ExperienceEventKind::Proposed { candidate } = &mut event.event else {
            panic!("expected proposed event");
        };
        candidate.version = 0;
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            store.candidates(),
            Err(ExperienceError::InvalidCandidateVersion)
        ));
    }

    #[test]
    fn replay_rejects_oversized_or_non_monotonic_transition_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let candidate = store.propose(spec(), &events).unwrap();
        store
            .review_with_source(candidate.id, "reviewer", Some(&events))
            .unwrap();
        let original = store.read_events().unwrap();

        let mut oversized_actor = original.clone();
        let ExperienceEventKind::Reviewed { actor, .. } = &mut oversized_actor[1].event else {
            panic!("expected reviewed event");
        };
        *actor = "x".repeat(MAX_EXPERIENCE_ACTOR_CHARS + 1);
        write_experience_events(&store, &oversized_actor);
        assert!(matches!(
            store.candidates(),
            Err(ExperienceError::TextTooLong("actor"))
        ));

        let mut regressed_time = original;
        regressed_time[1].timestamp_unix_ms = 0;
        write_experience_events(&store, &regressed_time);
        assert!(matches!(
            store.candidates(),
            Err(ExperienceError::NonMonotonicTimestamp { line: 2, .. })
        ));

        let large_dir = tempfile::tempdir().unwrap();
        let large_store = ExperienceStore::open(large_dir.path()).unwrap();
        std::fs::write(
            large_store.path(),
            format!("{}\n", "x".repeat(MAX_EXPERIENCE_EVENT_BYTES + 1)),
        )
        .unwrap();
        assert!(matches!(
            large_store.candidates(),
            Err(ExperienceError::EventTooLarge(bytes))
                if bytes > MAX_EXPERIENCE_EVENT_BYTES
        ));
    }

    #[test]
    fn active_candidate_can_be_degraded_and_rolled_back() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let candidate = store.propose(spec(), &events).unwrap();
        store
            .review_with_source(candidate.id, "reviewer", Some(&events))
            .unwrap();
        store.verify(candidate.id, verification()).unwrap();
        store
            .approve_with_source(candidate.id, "owner", Some(&events))
            .unwrap();
        store
            .activate_with_source(candidate.id, Some(&events))
            .unwrap();
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
    fn activation_rejects_competing_active_experience() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let first = store.propose(spec(), &events).unwrap();
        approve_and_activate(&mut store, first.id, &events);
        let mut second_spec = spec();
        second_spec.title = "Competing guidance".into();
        let second = store.propose(second_spec, &events).unwrap();
        store
            .review_with_source(second.id, "reviewer", Some(&events))
            .unwrap();
        store.verify(second.id, verification()).unwrap();
        store
            .approve_with_source(second.id, "owner", Some(&events))
            .unwrap();
        assert!(matches!(
            store.activate_with_source(second.id, Some(&events)),
            Err(ExperienceError::ActiveExperienceConflict)
        ));
    }

    #[test]
    fn supersession_replaces_prior_active_experience_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let first = store.propose(spec(), &events).unwrap();
        approve_and_activate(&mut store, first.id, &events);
        let mut replacement_spec = spec();
        replacement_spec.title = "Read evidence and timestamp it".into();
        replacement_spec.supersedes = Some(first.id);
        let replacement = store.propose(replacement_spec, &events).unwrap();
        assert_eq!(replacement.version, first.version + 1);
        approve_and_activate(&mut store, replacement.id, &events);

        let candidates = store.candidates().unwrap();
        assert_eq!(candidates[&first.id].status, ExperienceStatus::Superseded);
        assert_eq!(candidates[&replacement.id].status, ExperienceStatus::Active);
        let audit = store.audit();
        assert!(audit.ok, "{:?}", audit.findings);
        assert_eq!(audit.status_counts.active, 1);
        assert_eq!(audit.status_counts.superseded, 1);
        assert_eq!(audit.supersession_chains, 1);
        assert_eq!(audit.max_supersession_depth, 1);
        let active = store
            .retrieve_active(
                CognitiveMode::EvidenceResearch,
                "repository decisions",
                1_000,
            )
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, replacement.id);

        store
            .rollback(replacement.id, "replacement regressed")
            .unwrap();
        let candidates = store.candidates().unwrap();
        assert_eq!(candidates[&first.id].status, ExperienceStatus::Superseded);
        assert_eq!(
            candidates[&replacement.id].status,
            ExperienceStatus::RolledBack
        );
    }

    #[test]
    fn lifecycle_audit_detects_orphaned_and_cyclic_supersession() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let base = store.propose(spec(), &events).unwrap();

        let mut first = base.clone();
        first.id = Uuid::new_v4();
        first.supersedes = Some(Uuid::new_v4());
        let candidates = BTreeMap::from([(first.id, first)]);
        let (_, findings) = audit_lifecycle(&candidates);
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == ExperienceAuditFindingCode::OrphanedSupersession)
        );

        let mut first = base.clone();
        first.id = Uuid::new_v4();
        let mut second = base;
        second.id = Uuid::new_v4();
        first.supersedes = Some(second.id);
        second.supersedes = Some(first.id);
        let candidates = BTreeMap::from([(first.id, first), (second.id, second)]);
        let (_, findings) = audit_lifecycle(&candidates);
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == ExperienceAuditFindingCode::SupersessionCycle)
        );
    }

    #[test]
    fn lifecycle_audit_reports_expired_active_experience() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let mut short_lived = spec();
        short_lived.expires_at_unix_ms = Some(now_unix_ms().saturating_add(200));
        let candidate = store.propose(short_lived, &events).unwrap();
        approve_and_activate(&mut store, candidate.id, &events);
        std::thread::sleep(std::time::Duration::from_millis(250));
        let audit = store.audit();
        assert!(!audit.ok);
        assert_eq!(audit.expired_active, 1);
        assert!(
            audit
                .findings
                .iter()
                .any(|finding| finding.code == ExperienceAuditFindingCode::ExpiredActive)
        );
    }

    #[test]
    fn supersession_requires_matching_active_kind_and_scope() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let first = store.propose(spec(), &events).unwrap();
        approve_and_activate(&mut store, first.id, &events);

        let mut wrong_kind = spec();
        wrong_kind.kind = ExperienceKind::Memory;
        wrong_kind.supersedes = Some(first.id);
        assert!(matches!(
            store.propose(wrong_kind, &events),
            Err(ExperienceError::SupersessionKindMismatch(id)) if id == first.id
        ));

        let mut wrong_scope = spec();
        wrong_scope.scope = "finance".into();
        wrong_scope.supersedes = Some(first.id);
        assert!(matches!(
            store.propose(wrong_scope, &events),
            Err(ExperienceError::SupersessionScopeMismatch(id)) if id == first.id
        ));
    }

    #[test]
    fn expired_experience_cannot_be_proposed_or_activated() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let mut expired_spec = spec();
        expired_spec.expires_at_unix_ms = Some(now_unix_ms().saturating_sub(1));
        assert!(matches!(
            store.propose(expired_spec, &events),
            Err(ExperienceError::ExpiredSpecification)
        ));

        let mut short_lived = spec();
        short_lived.expires_at_unix_ms = Some(now_unix_ms().saturating_add(500));
        let candidate = store.propose(short_lived, &events).unwrap();
        store
            .review_with_source(candidate.id, "reviewer", Some(&events))
            .unwrap();
        store.verify(candidate.id, verification()).unwrap();
        store
            .approve_with_source(candidate.id, "owner", Some(&events))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(600));
        assert!(matches!(
            store.activate_with_source(candidate.id, Some(&events)),
            Err(ExperienceError::ExpiredCandidate(id)) if id == candidate.id
        ));
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
    fn experience_candidate_shape_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();

        let mut oversized_title = spec();
        oversized_title.title = "x".repeat(MAX_EXPERIENCE_TITLE_CHARS + 1);
        assert!(matches!(
            store.propose(oversized_title, &events),
            Err(ExperienceError::TextTooLong("title"))
        ));

        let mut oversized_content = spec();
        oversized_content.content = "x".repeat(MAX_EXPERIENCE_CONTENT_CHARS + 1);
        assert!(matches!(
            store.propose(oversized_content, &events),
            Err(ExperienceError::TextTooLong("content"))
        ));

        let mut oversized_scope = spec();
        oversized_scope.scope = "x".repeat(MAX_EXPERIENCE_SCOPE_CHARS + 1);
        assert!(matches!(
            store.propose(oversized_scope, &events),
            Err(ExperienceError::TextTooLong("scope"))
        ));

        let mut too_many_refs = spec();
        too_many_refs.evidence_event_seqs = vec![2; MAX_EVIDENCE_REFERENCES + 1];
        assert!(matches!(
            store.propose(too_many_refs, &events),
            Err(ExperienceError::TooManyEvidenceReferences)
        ));

        let mut duplicate_modes = spec();
        duplicate_modes.applicable_modes =
            vec![CognitiveMode::DirectAnswer, CognitiveMode::DirectAnswer];
        assert!(matches!(
            store.propose(duplicate_modes, &events),
            Err(ExperienceError::DuplicateApplicableMode)
        ));

        let mut no_modes = spec();
        no_modes.applicable_modes.clear();
        assert!(matches!(
            store.propose(no_modes, &events),
            Err(ExperienceError::EmptyApplicableModes)
        ));
    }

    #[test]
    fn audit_reports_clean_and_corrupt_experience_stores() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        store.propose(spec(), &events).unwrap();
        let report = store.audit();
        assert!(report.ok);
        assert_eq!(report.event_count, 1);
        assert_eq!(report.candidate_count, 1);
        assert!(report.findings.is_empty());

        let corrupt_dir = tempfile::tempdir().unwrap();
        let corrupt = ExperienceStore::open(corrupt_dir.path()).unwrap();
        std::fs::write(
            corrupt.path(),
            format!("{}\n", "x".repeat(MAX_EXPERIENCE_EVENT_BYTES + 1)),
        )
        .unwrap();
        let report = corrupt.audit();
        assert!(!report.ok);
        assert_eq!(
            report.findings[0].code,
            ExperienceAuditFindingCode::EventTooLarge
        );
        assert_eq!(
            serde_json::to_value(report.findings[0].code).unwrap(),
            serde_json::json!("event_too_large")
        );
    }

    #[test]
    fn retrieval_returns_only_active_scope_and_mode_matches() {
        let dir = tempfile::tempdir().unwrap();
        let events = source_events(dir.path());
        let mut store = ExperienceStore::open(dir.path().join("experience")).unwrap();
        let active = store.propose(spec(), &events).unwrap();
        store
            .review_with_source(active.id, "reviewer", Some(&events))
            .unwrap();
        store.verify(active.id, verification()).unwrap();
        store
            .approve_with_source(active.id, "owner", Some(&events))
            .unwrap();
        store
            .activate_with_source(active.id, Some(&events))
            .unwrap();
        let mut draft = spec();
        draft.title = "Draft only".into();
        draft.scope = "repository decisions".into();
        let draft = store.propose(draft, &events).unwrap();
        let mut wrong_scope = spec();
        wrong_scope.title = "Wrong scope".into();
        wrong_scope.scope = "finance".into();
        let wrong_scope = store.propose(wrong_scope, &events).unwrap();
        store
            .review_with_source(wrong_scope.id, "reviewer", Some(&events))
            .unwrap();
        store.verify(wrong_scope.id, verification()).unwrap();
        store
            .approve_with_source(wrong_scope.id, "owner", Some(&events))
            .unwrap();
        store
            .activate_with_source(wrong_scope.id, Some(&events))
            .unwrap();

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
        store
            .review_with_source(candidate.id, "reviewer", Some(&events))
            .unwrap();
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
