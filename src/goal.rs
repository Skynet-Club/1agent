//! Goals and commitments: durable, auditable intent for a standalone agent.
//!
//! The log is append-only and hash-chained. A commitment is a promise with an
//! optional due time; attention reads the same graph rather than keeping a
//! second reminder store.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_TEXT_CHARS: usize = 240;

#[derive(Debug, Error)]
pub enum GoalError {
    #[error("goal id must not be empty")]
    EmptyGoalId,
    #[error("goal owner must not be empty")]
    EmptyOwner,
    #[error("goal title must be 1..={MAX_TEXT_CHARS} characters")]
    InvalidTitle,
    #[error("commitment id must not be empty")]
    EmptyCommitmentId,
    #[error("commitment description must be 1..={MAX_TEXT_CHARS} characters")]
    InvalidDescription,
    #[error("goal {0} already exists")]
    GoalExists(String),
    #[error("goal {0} is not found")]
    UnknownGoal(String),
    #[error("commitment {0} already exists")]
    CommitmentExists(String),
    #[error("commitment {0} is not found")]
    UnknownCommitment(String),
    #[error("goal {0} is not active")]
    GoalNotActive(String),
    #[error("commitment {0} is not pending")]
    CommitmentNotPending(String),
    #[error("ticket id must not be empty")]
    EmptyTicketId,
    #[error("commitment {commitment_id} is already linked to ticket {ticket_id}")]
    CommitmentAlreadyLinked {
        commitment_id: String,
        ticket_id: String,
    },
    #[error("session id must not be empty")]
    EmptySessionId,
    #[error("run id must not be empty")]
    EmptyRunId,
    #[error("session {session_id} is already bound to active goal {goal_id}")]
    SessionAlreadyBound { session_id: String, goal_id: String },
    #[error("goal {goal_id} is already bound to session {session_id}")]
    GoalBoundToAnotherSession { goal_id: String, session_id: String },
    #[error("run {run_id} is already linked to goal {goal_id}")]
    RunAlreadyBound { run_id: String, goal_id: String },
    #[error("goal closure reason must be 1..={MAX_TEXT_CHARS} characters")]
    InvalidOutcomeReason,
    #[error("goal {goal_id} cannot be achieved: {reason}")]
    InsufficientEvidence { goal_id: String, reason: String },
    #[error("run {run_id} is not linked to goal {goal_id}")]
    EvidenceRunNotLinked { run_id: String, goal_id: String },
    #[error("goal {goal_id} cannot be achieved without a verification record for run {run_id}")]
    MissingVerification { goal_id: String, run_id: String },
    #[error(
        "verification {verification_id} belongs to run {evidence_run_id}, not goal run {answer_run_id}"
    )]
    VerificationRunMismatch {
        verification_id: String,
        evidence_run_id: String,
        answer_run_id: String,
    },
    #[error("verification evidence for goal {goal_id} is invalid: {reason}")]
    InvalidVerificationEvidence { goal_id: String, reason: String },
    #[error("goal {goal_id} still has open commitments: {commitment_ids:?}")]
    OpenCommitmentsRemain {
        goal_id: String,
        commitment_ids: Vec<String>,
    },
    #[error("goal {goal_id} has an unlisted pending commitment {commitment_id}")]
    OpenCommitmentUnlisted {
        goal_id: String,
        commitment_id: String,
    },
    #[error("commitment {commitment_id} does not belong to goal {goal_id}")]
    CommitmentGoalMismatch {
        commitment_id: String,
        goal_id: String,
    },
    #[error("invalid goal transition from {0:?} to {1:?}")]
    InvalidGoalTransition(GoalStatus, GoalStatus),
    #[error("goal log hash mismatch at {0}")]
    HashMismatch(u64),
    #[error("goal log sequence is not contiguous")]
    NonContiguousSequence,
    #[error(
        "goal log was modified by another writer (expected {expected_bytes} bytes, found {actual_bytes})"
    )]
    ConcurrentWrite {
        expected_bytes: u64,
        actual_bytes: u64,
    },
    #[error("goal log io: {0}")]
    Io(#[from] std::io::Error),
    #[error("goal log json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Paused,
    Achieved,
    Abandoned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentStatus {
    Pending,
    Kept,
    Missed,
    Cancelled,
}

impl std::str::FromStr for GoalStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "achieved" => Ok(Self::Achieved),
            "abandoned" => Ok(Self::Abandoned),
            other => Err(format!("unknown goal status {other}")),
        }
    }
}

impl std::str::FromStr for CommitmentStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "kept" => Ok(Self::Kept),
            "missed" => Ok(Self::Missed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(format!("unknown commitment status {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub goal_id: String,
    pub owner_id: String,
    pub title: String,
    pub status: GoalStatus,
    pub due_ms: Option<u128>,
    pub created_ms: u128,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Commitment {
    pub commitment_id: String,
    pub goal_id: String,
    pub description: String,
    pub status: CommitmentStatus,
    pub due_ms: Option<u128>,
    pub created_ms: u128,
    pub updated_ms: u128,
    pub closed_ms: Option<u128>,
    #[serde(default)]
    pub ticket_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalClosure {
    Achieved,
    Abandoned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalVerification {
    Passed,
    Failed,
    Unknown,
}

/// Provenance of the independent verification that justifies an achieved goal.
/// The goal log stores these identifiers so replay does not need the
/// verification store; a closure without this evidence fails closed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalVerificationEvidence {
    pub verification_id: String,
    pub run_id: String,
    pub executor_id: String,
    pub verifier_id: String,
    pub verified_at_ms: u128,
}

/// Evidence for closing a goal. `achieved` needs a run linked to the goal,
/// a passed verification, and the verification record that proves it;
/// `abandoned` needs a reason and must enumerate every pending commitment,
/// which is cancelled in the same hash-chained log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalOutcome {
    pub closure: GoalClosure,
    #[serde(default)]
    pub answer_run_id: Option<String>,
    pub verification: GoalVerification,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_evidence: Option<GoalVerificationEvidence>,
    #[serde(default)]
    pub open_commitment_ids: Vec<String>,
    pub reason: String,
}

/// Explicit Goal ↔ session/run/commitment binding. The fields are derived by
/// replaying the goal log; no second binding store exists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalBinding {
    pub goal_id: String,
    pub session_id: Option<String>,
    pub run_ids: Vec<String>,
    pub commitment_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum GoalEventKind {
    GoalCreated {
        goal: Goal,
    },
    GoalStatusChanged {
        goal_id: String,
        status: GoalStatus,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verification_evidence: Option<GoalVerificationEvidence>,
    },
    GoalSessionBound {
        goal_id: String,
        session_id: String,
    },
    GoalRunLinked {
        goal_id: String,
        run_id: String,
    },
    CommitmentAdded {
        commitment: Commitment,
    },
    CommitmentClosed {
        commitment_id: String,
        status: CommitmentStatus,
        note: String,
    },
    CommitmentLinkedToTicket {
        commitment_id: String,
        ticket_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalEvent {
    pub seq: u64,
    pub timestamp_unix_ms: u128,
    pub kind: GoalEventKind,
    pub previous_sha256: Option<String>,
    pub event_sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalSnapshot {
    pub goals: Vec<Goal>,
    pub commitments: Vec<Commitment>,
    #[serde(default)]
    pub bindings: Vec<GoalBinding>,
}

#[derive(Debug, Clone, Default)]
pub struct GoalGraph {
    goals: BTreeMap<String, Goal>,
    commitments: BTreeMap<String, Commitment>,
    bindings: BTreeMap<String, GoalBinding>,
    events: Vec<GoalEvent>,
    next_seq: u64,
    last_sha256: Option<String>,
}

impl GoalGraph {
    pub fn events(&self) -> &[GoalEvent] {
        &self.events
    }

    pub fn goal(&self, goal_id: &str) -> Option<&Goal> {
        self.goals.get(goal_id)
    }

    pub fn commitment(&self, commitment_id: &str) -> Option<&Commitment> {
        self.commitments.get(commitment_id)
    }

    pub fn binding(&self, goal_id: &str) -> Option<&GoalBinding> {
        self.bindings.get(goal_id)
    }

    /// The current goal binding for a session: prefer the active goal, and
    /// fall back to a historical binding when the goal is paused or closed.
    pub fn binding_for_session(&self, session_id: &str) -> Option<&GoalBinding> {
        let mut historical = None;
        for binding in self
            .bindings
            .values()
            .filter(|binding| binding.session_id.as_deref() == Some(session_id))
        {
            if self
                .goals
                .get(&binding.goal_id)
                .is_some_and(|goal| goal.status == GoalStatus::Active)
            {
                return Some(binding);
            }
            historical.get_or_insert(binding);
        }
        historical
    }

    pub fn binding_for_run(&self, run_id: &str) -> Option<&GoalBinding> {
        self.bindings
            .values()
            .find(|binding| binding.run_ids.iter().any(|run| run == run_id))
    }

    pub fn snapshot(&self) -> GoalSnapshot {
        GoalSnapshot {
            goals: self.goals.values().cloned().collect(),
            commitments: self.commitments.values().cloned().collect(),
            bindings: self.bindings.values().cloned().collect(),
        }
    }

    pub fn create_goal(
        &mut self,
        goal_id: impl Into<String>,
        owner_id: impl Into<String>,
        title: impl Into<String>,
        due_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let goal_id = goal_id.into();
        let owner_id = owner_id.into();
        let title = title.into();
        if goal_id.trim().is_empty() {
            return Err(GoalError::EmptyGoalId);
        }
        if owner_id.trim().is_empty() {
            return Err(GoalError::EmptyOwner);
        }
        validate_text(&title, GoalError::InvalidTitle)?;
        if self.goals.contains_key(&goal_id) {
            return Err(GoalError::GoalExists(goal_id));
        }
        self.append(
            GoalEventKind::GoalCreated {
                goal: Goal {
                    goal_id,
                    owner_id,
                    title,
                    status: GoalStatus::Active,
                    due_ms,
                    created_ms: now_ms,
                    updated_ms: now_ms,
                },
            },
            now_ms,
        )
    }

    pub fn set_goal_status(
        &mut self,
        goal_id: &str,
        status: GoalStatus,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let goal = self
            .goals
            .get(goal_id)
            .ok_or_else(|| GoalError::UnknownGoal(goal_id.into()))?;
        if !valid_goal_transition(goal.status, status) {
            return Err(GoalError::InvalidGoalTransition(goal.status, status));
        }
        self.append(
            GoalEventKind::GoalStatusChanged {
                goal_id: goal_id.into(),
                status,
                reason: reason.into(),
                verification_evidence: None,
            },
            now_ms,
        )
    }

    /// Bind an active goal to the session that pursues it. A session can have
    /// only one active goal at a time; historical bindings stay visible after
    /// the goal is paused or closed. Re-binding the same pair is idempotent.
    pub fn bind_session(
        &mut self,
        goal_id: &str,
        session_id: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let session_id = session_id.into();
        if session_id.trim().is_empty() {
            return Err(GoalError::EmptySessionId);
        }
        let goal = self
            .goals
            .get(goal_id)
            .ok_or_else(|| GoalError::UnknownGoal(goal_id.into()))?;
        if goal.status != GoalStatus::Active {
            return Err(GoalError::GoalNotActive(goal_id.into()));
        }
        if let Some(binding) = self.bindings.get(goal_id) {
            match binding.session_id.as_deref() {
                Some(existing) if existing == session_id => return Ok(()),
                Some(existing) => {
                    return Err(GoalError::GoalBoundToAnotherSession {
                        goal_id: goal_id.into(),
                        session_id: existing.into(),
                    });
                }
                None => {}
            }
        }
        if let Some(existing) = self.binding_for_session(&session_id)
            && existing.goal_id != goal_id
            && self
                .goals
                .get(&existing.goal_id)
                .is_some_and(|goal| goal.status == GoalStatus::Active)
        {
            return Err(GoalError::SessionAlreadyBound {
                session_id,
                goal_id: existing.goal_id.clone(),
            });
        }
        self.append(
            GoalEventKind::GoalSessionBound {
                goal_id: goal_id.into(),
                session_id,
            },
            now_ms,
        )
    }

    /// Attribute a run to an active goal. A run belongs to at most one goal;
    /// re-linking the same run to the same goal is idempotent.
    pub fn link_run(
        &mut self,
        goal_id: &str,
        run_id: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let run_id = run_id.into();
        if run_id.trim().is_empty() {
            return Err(GoalError::EmptyRunId);
        }
        let goal = self
            .goals
            .get(goal_id)
            .ok_or_else(|| GoalError::UnknownGoal(goal_id.into()))?;
        if goal.status != GoalStatus::Active {
            return Err(GoalError::GoalNotActive(goal_id.into()));
        }
        if let Some(existing) = self.binding_for_run(&run_id) {
            if existing.goal_id == goal_id {
                return Ok(());
            }
            return Err(GoalError::RunAlreadyBound {
                run_id,
                goal_id: existing.goal_id.clone(),
            });
        }
        self.append(
            GoalEventKind::GoalRunLinked {
                goal_id: goal_id.into(),
                run_id,
            },
            now_ms,
        )
    }

    /// Close a goal with explicit evidence. `achieved` is allowed only with a
    /// linked run and a passed verification; `abandoned` requires a reason and
    /// cancels every pending commitment of the goal in the same candidate
    /// graph, so a failed call persists nothing.
    pub fn close_with_evidence(
        &mut self,
        goal_id: &str,
        outcome: GoalOutcome,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let goal_status = self
            .goals
            .get(goal_id)
            .ok_or_else(|| GoalError::UnknownGoal(goal_id.into()))?
            .status;
        validate_text(&outcome.reason, GoalError::InvalidOutcomeReason)?;
        let pending = self
            .commitments
            .values()
            .filter(|commitment| {
                commitment.goal_id == goal_id && commitment.status == CommitmentStatus::Pending
            })
            .map(|commitment| commitment.commitment_id.clone())
            .collect::<Vec<_>>();
        for commitment_id in &outcome.open_commitment_ids {
            let commitment = self
                .commitments
                .get(commitment_id)
                .ok_or_else(|| GoalError::UnknownCommitment(commitment_id.clone()))?;
            if commitment.goal_id != goal_id {
                return Err(GoalError::CommitmentGoalMismatch {
                    commitment_id: commitment_id.clone(),
                    goal_id: goal_id.into(),
                });
            }
            if commitment.status != CommitmentStatus::Pending {
                return Err(GoalError::CommitmentNotPending(commitment_id.clone()));
            }
        }
        match outcome.closure {
            GoalClosure::Achieved => {
                if goal_status != GoalStatus::Active {
                    return Err(GoalError::GoalNotActive(goal_id.into()));
                }
                let run_id = outcome
                    .answer_run_id
                    .as_deref()
                    .filter(|run_id| !run_id.trim().is_empty())
                    .ok_or_else(|| GoalError::InsufficientEvidence {
                        goal_id: goal_id.into(),
                        reason: "a completed run id is required".into(),
                    })?;
                if self
                    .binding_for_run(run_id)
                    .map(|binding| binding.goal_id.as_str())
                    != Some(goal_id)
                {
                    return Err(GoalError::EvidenceRunNotLinked {
                        run_id: run_id.into(),
                        goal_id: goal_id.into(),
                    });
                }
                if outcome.verification != GoalVerification::Passed {
                    return Err(GoalError::InsufficientEvidence {
                        goal_id: goal_id.into(),
                        reason: "verification did not pass".into(),
                    });
                }
                if !pending.is_empty() || !outcome.open_commitment_ids.is_empty() {
                    return Err(GoalError::OpenCommitmentsRemain {
                        goal_id: goal_id.into(),
                        commitment_ids: pending,
                    });
                }
                let evidence = outcome.verification_evidence.as_ref().ok_or_else(|| {
                    GoalError::MissingVerification {
                        goal_id: goal_id.into(),
                        run_id: run_id.into(),
                    }
                })?;
                if evidence.verification_id.trim().is_empty() {
                    return Err(GoalError::InvalidVerificationEvidence {
                        goal_id: goal_id.into(),
                        reason: "verification id is empty".into(),
                    });
                }
                if evidence.run_id != run_id {
                    return Err(GoalError::VerificationRunMismatch {
                        verification_id: evidence.verification_id.clone(),
                        evidence_run_id: evidence.run_id.clone(),
                        answer_run_id: run_id.into(),
                    });
                }
                if evidence.executor_id.trim().is_empty() || evidence.verifier_id.trim().is_empty()
                {
                    return Err(GoalError::InvalidVerificationEvidence {
                        goal_id: goal_id.into(),
                        reason: "executor and verifier ids must be non-empty".into(),
                    });
                }
                if evidence.executor_id == evidence.verifier_id {
                    return Err(GoalError::InvalidVerificationEvidence {
                        goal_id: goal_id.into(),
                        reason: "executor and verifier must differ".into(),
                    });
                }
                self.append(
                    GoalEventKind::GoalStatusChanged {
                        goal_id: goal_id.into(),
                        status: GoalStatus::Achieved,
                        reason: outcome.reason,
                        verification_evidence: Some(evidence.clone()),
                    },
                    now_ms,
                )
            }
            GoalClosure::Abandoned => {
                if !valid_goal_transition(goal_status, GoalStatus::Abandoned) {
                    return Err(GoalError::InvalidGoalTransition(
                        goal_status,
                        GoalStatus::Abandoned,
                    ));
                }
                for commitment_id in &pending {
                    if !outcome.open_commitment_ids.contains(commitment_id) {
                        return Err(GoalError::OpenCommitmentUnlisted {
                            goal_id: goal_id.into(),
                            commitment_id: commitment_id.clone(),
                        });
                    }
                }
                let reason = outcome.reason;
                for commitment_id in &pending {
                    self.append(
                        GoalEventKind::CommitmentClosed {
                            commitment_id: commitment_id.clone(),
                            status: CommitmentStatus::Cancelled,
                            note: reason.clone(),
                        },
                        now_ms,
                    )?;
                }
                self.append(
                    GoalEventKind::GoalStatusChanged {
                        goal_id: goal_id.into(),
                        status: GoalStatus::Abandoned,
                        reason,
                        verification_evidence: None,
                    },
                    now_ms,
                )
            }
        }
    }

    pub fn add_commitment(
        &mut self,
        commitment_id: impl Into<String>,
        goal_id: &str,
        description: impl Into<String>,
        due_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let commitment_id = commitment_id.into();
        let description = description.into();
        if commitment_id.trim().is_empty() {
            return Err(GoalError::EmptyCommitmentId);
        }
        validate_text(&description, GoalError::InvalidDescription)?;
        let goal = self
            .goals
            .get(goal_id)
            .ok_or_else(|| GoalError::UnknownGoal(goal_id.into()))?;
        if goal.status != GoalStatus::Active {
            return Err(GoalError::GoalNotActive(goal_id.into()));
        }
        if self.commitments.contains_key(&commitment_id) {
            return Err(GoalError::CommitmentExists(commitment_id));
        }
        self.append(
            GoalEventKind::CommitmentAdded {
                commitment: Commitment {
                    commitment_id,
                    goal_id: goal_id.into(),
                    description,
                    status: CommitmentStatus::Pending,
                    due_ms,
                    created_ms: now_ms,
                    updated_ms: now_ms,
                    closed_ms: None,
                    ticket_id: None,
                },
            },
            now_ms,
        )
    }

    pub fn link_commitment_ticket(
        &mut self,
        commitment_id: &str,
        ticket_id: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let ticket_id = ticket_id.into();
        if ticket_id.trim().is_empty() {
            return Err(GoalError::EmptyTicketId);
        }
        let commitment = self
            .commitments
            .get(commitment_id)
            .ok_or_else(|| GoalError::UnknownCommitment(commitment_id.into()))?;
        if commitment.status != CommitmentStatus::Pending {
            return Err(GoalError::CommitmentNotPending(commitment_id.into()));
        }
        if let Some(existing) = &commitment.ticket_id {
            if existing == &ticket_id {
                return Ok(());
            }
            return Err(GoalError::CommitmentAlreadyLinked {
                commitment_id: commitment_id.into(),
                ticket_id: existing.clone(),
            });
        }
        self.append(
            GoalEventKind::CommitmentLinkedToTicket {
                commitment_id: commitment_id.into(),
                ticket_id,
            },
            now_ms,
        )
    }

    pub fn close_commitment(
        &mut self,
        commitment_id: &str,
        status: CommitmentStatus,
        note: impl Into<String>,
        now_ms: u128,
    ) -> Result<(), GoalError> {
        let commitment = self
            .commitments
            .get(commitment_id)
            .ok_or_else(|| GoalError::UnknownCommitment(commitment_id.into()))?;
        if commitment.status != CommitmentStatus::Pending {
            return Err(GoalError::CommitmentNotPending(commitment_id.into()));
        }
        if status == CommitmentStatus::Pending {
            return Err(GoalError::CommitmentNotPending(commitment_id.into()));
        }
        self.append(
            GoalEventKind::CommitmentClosed {
                commitment_id: commitment_id.into(),
                status,
                note: note.into(),
            },
            now_ms,
        )
    }

    pub fn replay(events: &[GoalEvent]) -> Result<Self, GoalError> {
        let mut graph = GoalGraph::default();
        for (index, event) in events.iter().enumerate() {
            let expected_seq = index as u64 + 1;
            if event.seq != expected_seq {
                return Err(GoalError::NonContiguousSequence);
            }
            if event.previous_sha256 != graph.last_sha256 {
                return Err(GoalError::HashMismatch(event.seq));
            }
            let expected_hash = hash_event(event)?;
            if event.event_sha256 != expected_hash {
                return Err(GoalError::HashMismatch(event.seq));
            }
            graph.apply(event.kind.clone(), event.timestamp_unix_ms)?;
            graph.next_seq = event.seq;
            graph.last_sha256 = Some(event.event_sha256.clone());
            graph.events.push(event.clone());
        }
        Ok(graph)
    }

    fn append(&mut self, kind: GoalEventKind, now_ms: u128) -> Result<(), GoalError> {
        let seq = self.next_seq + 1;
        let mut event = GoalEvent {
            seq,
            timestamp_unix_ms: now_ms,
            kind,
            previous_sha256: self.last_sha256.clone(),
            event_sha256: String::new(),
        };
        event.event_sha256 = hash_event(&event)?;
        self.apply(event.kind.clone(), now_ms)?;
        self.last_sha256 = Some(event.event_sha256.clone());
        self.next_seq = seq;
        self.events.push(event);
        Ok(())
    }

    fn apply(&mut self, kind: GoalEventKind, at_ms: u128) -> Result<(), GoalError> {
        match kind {
            GoalEventKind::GoalCreated { goal } => {
                if self.goals.contains_key(&goal.goal_id) {
                    return Err(GoalError::GoalExists(goal.goal_id));
                }
                self.bindings
                    .entry(goal.goal_id.clone())
                    .or_insert_with(|| GoalBinding {
                        goal_id: goal.goal_id.clone(),
                        session_id: None,
                        run_ids: Vec::new(),
                        commitment_ids: Vec::new(),
                    });
                self.goals.insert(goal.goal_id.clone(), goal);
            }
            GoalEventKind::GoalStatusChanged {
                goal_id, status, ..
            } => {
                let goal = self
                    .goals
                    .get_mut(&goal_id)
                    .ok_or_else(|| GoalError::UnknownGoal(goal_id.clone()))?;
                if !valid_goal_transition(goal.status, status) {
                    return Err(GoalError::InvalidGoalTransition(goal.status, status));
                }
                goal.status = status;
                goal.updated_ms = at_ms;
            }
            GoalEventKind::GoalSessionBound {
                goal_id,
                session_id,
            } => {
                let goal = self
                    .goals
                    .get(&goal_id)
                    .ok_or_else(|| GoalError::UnknownGoal(goal_id.clone()))?;
                if goal.status != GoalStatus::Active {
                    return Err(GoalError::GoalNotActive(goal_id));
                }
                if let Some(binding) = self.bindings.get(&goal_id)
                    && let Some(existing) = &binding.session_id
                {
                    if existing == &session_id {
                        return Ok(());
                    }
                    return Err(GoalError::GoalBoundToAnotherSession {
                        goal_id,
                        session_id: existing.clone(),
                    });
                }
                if let Some(existing) = self
                    .bindings
                    .values()
                    .find(|binding| {
                        binding.session_id.as_deref() == Some(session_id.as_str())
                            && binding.goal_id != goal_id
                    })
                    .filter(|binding| {
                        self.goals
                            .get(&binding.goal_id)
                            .is_some_and(|goal| goal.status == GoalStatus::Active)
                    })
                {
                    return Err(GoalError::SessionAlreadyBound {
                        session_id,
                        goal_id: existing.goal_id.clone(),
                    });
                }
                self.bindings
                    .entry(goal_id.clone())
                    .or_insert_with(|| GoalBinding {
                        goal_id,
                        session_id: None,
                        run_ids: Vec::new(),
                        commitment_ids: Vec::new(),
                    })
                    .session_id = Some(session_id);
            }
            GoalEventKind::GoalRunLinked { goal_id, run_id } => {
                let goal = self
                    .goals
                    .get(&goal_id)
                    .ok_or_else(|| GoalError::UnknownGoal(goal_id.clone()))?;
                if goal.status != GoalStatus::Active {
                    return Err(GoalError::GoalNotActive(goal_id));
                }
                if let Some(existing) = self
                    .bindings
                    .values()
                    .find(|binding| binding.run_ids.iter().any(|run| run == &run_id))
                {
                    if existing.goal_id == goal_id {
                        return Ok(());
                    }
                    return Err(GoalError::RunAlreadyBound {
                        run_id,
                        goal_id: existing.goal_id.clone(),
                    });
                }
                self.bindings
                    .entry(goal_id.clone())
                    .or_insert_with(|| GoalBinding {
                        goal_id,
                        session_id: None,
                        run_ids: Vec::new(),
                        commitment_ids: Vec::new(),
                    })
                    .run_ids
                    .push(run_id);
            }
            GoalEventKind::CommitmentAdded { commitment } => {
                if self.commitments.contains_key(&commitment.commitment_id) {
                    return Err(GoalError::CommitmentExists(commitment.commitment_id));
                }
                let goal_id = commitment.goal_id.clone();
                if !self.goals.contains_key(&goal_id) {
                    return Err(GoalError::UnknownGoal(goal_id));
                }
                let commitment_id = commitment.commitment_id.clone();
                self.commitments
                    .insert(commitment.commitment_id.clone(), commitment);
                if let Some(binding) = self.bindings.get_mut(&goal_id) {
                    binding.commitment_ids.push(commitment_id);
                }
            }
            GoalEventKind::CommitmentClosed {
                commitment_id,
                status,
                ..
            } => {
                let commitment = self
                    .commitments
                    .get_mut(&commitment_id)
                    .ok_or_else(|| GoalError::UnknownCommitment(commitment_id.clone()))?;
                if commitment.status != CommitmentStatus::Pending {
                    return Err(GoalError::CommitmentNotPending(commitment_id));
                }
                commitment.status = status;
                commitment.updated_ms = at_ms;
                commitment.closed_ms = Some(at_ms);
            }
            GoalEventKind::CommitmentLinkedToTicket {
                commitment_id,
                ticket_id,
            } => {
                let commitment = self
                    .commitments
                    .get_mut(&commitment_id)
                    .ok_or_else(|| GoalError::UnknownCommitment(commitment_id.clone()))?;
                if commitment.status != CommitmentStatus::Pending {
                    return Err(GoalError::CommitmentNotPending(commitment_id));
                }
                if let Some(existing) = &commitment.ticket_id {
                    if existing == &ticket_id {
                        return Ok(());
                    }
                    return Err(GoalError::CommitmentAlreadyLinked {
                        commitment_id,
                        ticket_id: existing.clone(),
                    });
                }
                commitment.ticket_id = Some(ticket_id);
                commitment.updated_ms = at_ms;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct GoalStore {
    path: PathBuf,
    next_seq: u64,
    persisted_bytes: u64,
    graph: GoalGraph,
}

impl GoalStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, GoalError> {
        let path = path.into();
        let _lock = crate::jsonl::FileLock::exclusive(&path)?;
        crate::jsonl::repair_torn_tail(&path)?;
        let mut events = Vec::new();
        if path.exists() {
            use std::io::{BufRead, BufReader};
            let file = std::fs::File::open(&path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                events.push(serde_json::from_str::<GoalEvent>(&line)?);
            }
        }
        let graph = GoalGraph::replay(&events)?;
        let next_seq = events.last().map(|event| event.seq).unwrap_or(0);
        let persisted_bytes = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        drop(_lock);
        Ok(Self {
            path,
            next_seq,
            persisted_bytes,
            graph,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn graph(&self) -> &GoalGraph {
        &self.graph
    }

    pub fn reload(&mut self) -> Result<(), GoalError> {
        let path = self.path.clone();
        *self = Self::open(path)?;
        Ok(())
    }

    pub fn persist(&mut self) -> Result<usize, GoalError> {
        let events = self.graph.events.clone();
        match append_events(
            &self.path,
            &mut self.next_seq,
            &mut self.persisted_bytes,
            &events,
        ) {
            Ok(count) => Ok(count),
            Err(error @ GoalError::ConcurrentWrite { .. }) => {
                let _ = self.reload();
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    pub fn with_graph_mut<R>(
        &mut self,
        operation: impl FnOnce(&mut GoalGraph) -> Result<R, GoalError>,
    ) -> Result<R, GoalError> {
        let mut candidate = self.graph.clone();
        let result = operation(&mut candidate)?;
        match append_events(
            &self.path,
            &mut self.next_seq,
            &mut self.persisted_bytes,
            &candidate.events,
        ) {
            Ok(_) => {
                self.graph = candidate;
                Ok(result)
            }
            Err(error @ GoalError::ConcurrentWrite { .. }) => {
                let _ = self.reload();
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}

/// Bind a session to a goal through the store's hash-chained log.
pub fn bind_session_goal(
    store: &mut GoalStore,
    goal_id: &str,
    session_id: &str,
    now_ms: u128,
) -> Result<(), GoalError> {
    store.with_graph_mut(|graph| graph.bind_session(goal_id, session_id, now_ms))
}

/// Attribute a run to a goal through the store's hash-chained log.
pub fn link_run_to_goal(
    store: &mut GoalStore,
    goal_id: &str,
    run_id: &str,
    now_ms: u128,
) -> Result<(), GoalError> {
    store.with_graph_mut(|graph| graph.link_run(goal_id, run_id, now_ms))
}

/// Close a goal with evidence through the store's hash-chained log.
pub fn close_goal_with_evidence(
    store: &mut GoalStore,
    goal_id: &str,
    outcome: GoalOutcome,
    now_ms: u128,
) -> Result<(), GoalError> {
    store.with_graph_mut(|graph| graph.close_with_evidence(goal_id, outcome, now_ms))
}

fn append_events(
    path: &Path,
    next_seq: &mut u64,
    persisted_bytes: &mut u64,
    events: &[GoalEvent],
) -> Result<usize, GoalError> {
    let new_events: Vec<&GoalEvent> = events
        .iter()
        .filter(|event| event.seq > *next_seq)
        .collect();
    for (index, event) in new_events.iter().enumerate() {
        if event.seq != *next_seq + 1 + index as u64 {
            return Err(GoalError::NonContiguousSequence);
        }
    }
    if new_events.is_empty() {
        return Ok(0);
    }
    let mut buffer = String::new();
    for event in &new_events {
        buffer.push_str(&serde_json::to_string(event)?);
        buffer.push('\n');
    }
    *persisted_bytes =
        crate::jsonl::append_batch_locked(path, *persisted_bytes, &buffer).map_err(|error| {
            match error {
                crate::jsonl::LockedAppendError::Io(error) => GoalError::Io(error),
                crate::jsonl::LockedAppendError::ConcurrentWrite {
                    expected_bytes,
                    actual_bytes,
                } => GoalError::ConcurrentWrite {
                    expected_bytes,
                    actual_bytes,
                },
            }
        })?;
    *next_seq = new_events
        .last()
        .map(|event| event.seq)
        .unwrap_or(*next_seq);
    Ok(new_events.len())
}

fn valid_goal_transition(from: GoalStatus, to: GoalStatus) -> bool {
    matches!(
        (from, to),
        (
            GoalStatus::Active,
            GoalStatus::Paused | GoalStatus::Achieved | GoalStatus::Abandoned
        ) | (
            GoalStatus::Paused,
            GoalStatus::Active | GoalStatus::Abandoned
        )
    )
}

fn validate_text(value: &str, error: GoalError) -> Result<(), GoalError> {
    if value.trim().is_empty() || value.chars().count() > MAX_TEXT_CHARS {
        Err(error)
    } else {
        Ok(())
    }
}

fn hash_event(event: &GoalEvent) -> Result<String, GoalError> {
    #[derive(Serialize)]
    struct Payload<'a> {
        seq: u64,
        timestamp_unix_ms: u128,
        kind: &'a GoalEventKind,
        previous_sha256: &'a Option<String>,
    }
    let payload = Payload {
        seq: event.seq,
        timestamp_unix_ms: event.timestamp_unix_ms,
        kind: &event.kind,
        previous_sha256: &event.previous_sha256,
    };
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&payload)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verification_evidence(run_id: &str) -> GoalVerificationEvidence {
        GoalVerificationEvidence {
            verification_id: format!("verification-{run_id}"),
            run_id: run_id.into(),
            executor_id: "executor-1".into(),
            verifier_id: "verifier-1".into(),
            verified_at_ms: 99,
        }
    }

    #[test]
    fn goal_and_commitment_lifecycle_replays() {
        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "ship 1Agent", Some(1_000), 10)
            .unwrap();
        graph
            .add_commitment("commit-1", "goal-1", "write release notes", Some(500), 11)
            .unwrap();
        graph
            .link_commitment_ticket("commit-1", "ticket-1", 12)
            .unwrap();
        graph
            .close_commitment("commit-1", CommitmentStatus::Kept, "done", 20)
            .unwrap();
        graph
            .set_goal_status("goal-1", GoalStatus::Achieved, "shipped", 30)
            .unwrap();
        assert_eq!(graph.goal("goal-1").unwrap().status, GoalStatus::Achieved);
        assert_eq!(
            graph.commitment("commit-1").unwrap().status,
            CommitmentStatus::Kept
        );
        assert_eq!(graph.commitment("commit-1").unwrap().closed_ms, Some(20));
        assert_eq!(
            graph.commitment("commit-1").unwrap().ticket_id.as_deref(),
            Some("ticket-1")
        );
        let replayed = GoalGraph::replay(graph.events()).unwrap();
        assert_eq!(replayed.snapshot(), graph.snapshot());
    }

    #[test]
    fn invalid_transitions_and_unknown_ids_fail_closed() {
        let mut graph = GoalGraph::default();
        assert!(matches!(
            graph
                .create_goal("", "owner-1", "title", None, 1)
                .unwrap_err(),
            GoalError::EmptyGoalId
        ));
        graph
            .create_goal("goal-1", "owner-1", "title", None, 1)
            .unwrap();
        assert!(matches!(
            graph
                .add_commitment("commit-1", "missing", "x", None, 2)
                .unwrap_err(),
            GoalError::UnknownGoal(id) if id == "missing"
        ));
        graph
            .add_commitment("commit-1", "goal-1", "x", None, 2)
            .unwrap();
        graph
            .close_commitment("commit-1", CommitmentStatus::Kept, "done", 3)
            .unwrap();
        assert!(matches!(
            graph
                .close_commitment("commit-1", CommitmentStatus::Kept, "again", 4)
                .unwrap_err(),
            GoalError::CommitmentNotPending(id) if id == "commit-1"
        ));
        graph
            .set_goal_status("goal-1", GoalStatus::Achieved, "done", 5)
            .unwrap();
        assert!(matches!(
            graph
                .set_goal_status("goal-1", GoalStatus::Active, "reopen", 6)
                .unwrap_err(),
            GoalError::InvalidGoalTransition(_, _)
        ));
    }

    #[test]
    fn stale_writer_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut first = GoalStore::open(&path).unwrap();
        let mut second = GoalStore::open(&path).unwrap();
        first
            .with_graph_mut(|graph| graph.create_goal("goal-1", "owner-1", "ship", None, 1))
            .unwrap();
        let error = second
            .with_graph_mut(|graph| graph.create_goal("goal-2", "owner-1", "other", None, 2))
            .unwrap_err();
        assert!(matches!(
            error,
            GoalError::ConcurrentWrite {
                expected_bytes: 0,
                actual_bytes: _
            }
        ));
        second
            .with_graph_mut(|graph| graph.create_goal("goal-2", "owner-1", "other", None, 2))
            .unwrap();
        let reopened = GoalStore::open(&path).unwrap();
        assert_eq!(reopened.graph().snapshot().goals.len(), 2);
    }

    #[test]
    fn goal_binding_tracks_session_runs_and_commitments() {
        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "session:ses-1", "ship", None, 1)
            .unwrap();
        graph
            .add_commitment("commit-1", "goal-1", "write notes", None, 2)
            .unwrap();
        graph
            .add_commitment("commit-2", "goal-1", "run tests", None, 3)
            .unwrap();
        graph.bind_session("goal-1", "ses-1", 4).unwrap();
        graph.link_run("goal-1", "run-1", 5).unwrap();
        graph.link_run("goal-1", "run-2", 6).unwrap();

        let binding = graph.binding("goal-1").unwrap();
        assert_eq!(binding.session_id.as_deref(), Some("ses-1"));
        assert_eq!(binding.run_ids, ["run-1", "run-2"]);
        assert_eq!(binding.commitment_ids, ["commit-1", "commit-2"]);
        assert_eq!(
            graph.binding_for_session("ses-1").unwrap().goal_id,
            "goal-1"
        );
        assert_eq!(graph.binding_for_run("run-2").unwrap().goal_id, "goal-1");

        let replayed = GoalGraph::replay(graph.events()).unwrap();
        assert_eq!(replayed.snapshot(), graph.snapshot());
    }

    #[test]
    fn binding_negative_cases_fail_closed() {
        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "one", None, 1)
            .unwrap();
        graph
            .create_goal("goal-2", "owner-1", "two", None, 2)
            .unwrap();
        assert!(matches!(
            graph.bind_session("missing", "ses-1", 3).unwrap_err(),
            GoalError::UnknownGoal(id) if id == "missing"
        ));
        assert!(matches!(
            graph.bind_session("goal-1", "  ", 3).unwrap_err(),
            GoalError::EmptySessionId
        ));
        assert!(matches!(
            graph.link_run("goal-1", "", 3).unwrap_err(),
            GoalError::EmptyRunId
        ));

        graph.bind_session("goal-1", "ses-1", 4).unwrap();
        let events_after_bind = graph.events().len();
        graph.bind_session("goal-1", "ses-1", 5).unwrap();
        assert_eq!(graph.events().len(), events_after_bind);
        assert!(matches!(
            graph.bind_session("goal-1", "ses-2", 6).unwrap_err(),
            GoalError::GoalBoundToAnotherSession { .. }
        ));
        assert!(matches!(
            graph.bind_session("goal-2", "ses-1", 7).unwrap_err(),
            GoalError::SessionAlreadyBound { .. }
        ));

        graph.link_run("goal-1", "run-1", 8).unwrap();
        let events_after_link = graph.events().len();
        graph.link_run("goal-1", "run-1", 9).unwrap();
        assert_eq!(graph.events().len(), events_after_link);
        assert!(matches!(
            graph.link_run("goal-2", "run-1", 10).unwrap_err(),
            GoalError::RunAlreadyBound { .. }
        ));

        graph
            .set_goal_status("goal-2", GoalStatus::Achieved, "done", 11)
            .unwrap();
        assert!(matches!(
            graph.bind_session("goal-2", "ses-3", 12).unwrap_err(),
            GoalError::GoalNotActive(id) if id == "goal-2"
        ));
        assert!(matches!(
            graph.link_run("goal-2", "run-2", 13).unwrap_err(),
            GoalError::GoalNotActive(id) if id == "goal-2"
        ));

        // A closed goal keeps its history but frees the session for a new goal.
        graph
            .set_goal_status("goal-1", GoalStatus::Achieved, "done", 14)
            .unwrap();
        graph
            .create_goal("goal-3", "owner-1", "three", None, 15)
            .unwrap();
        graph.bind_session("goal-3", "ses-1", 16).unwrap();
        assert_eq!(
            graph.binding_for_session("ses-1").unwrap().goal_id,
            "goal-3"
        );
        assert_eq!(graph.binding("goal-1").unwrap().run_ids, ["run-1"]);
    }

    #[test]
    fn store_level_binding_helpers_persist_and_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::open(&path).unwrap();
        store
            .with_graph_mut(|graph| graph.create_goal("goal-1", "owner-1", "ship", None, 1))
            .unwrap();
        bind_session_goal(&mut store, "goal-1", "ses-1", 2).unwrap();
        link_run_to_goal(&mut store, "goal-1", "run-1", 3).unwrap();
        link_run_to_goal(&mut store, "goal-1", "run-2", 4).unwrap();
        drop(store);

        let reopened = GoalStore::open(&path).unwrap();
        let binding = reopened.graph().binding("goal-1").unwrap();
        assert_eq!(binding.session_id.as_deref(), Some("ses-1"));
        assert_eq!(binding.run_ids, ["run-1", "run-2"]);
        assert_eq!(
            reopened.graph().binding_for_run("run-2").unwrap().goal_id,
            "goal-1"
        );
    }

    #[test]
    fn legacy_goal_snapshot_without_bindings_is_readable() {
        let legacy = serde_json::json!({
            "goals": [],
            "commitments": []
        });
        let snapshot: GoalSnapshot = serde_json::from_value(legacy).unwrap();
        assert!(snapshot.bindings.is_empty());
    }

    #[test]
    fn close_with_evidence_achieves_only_with_a_linked_run_and_passed_verification() {
        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "ship", None, 1)
            .unwrap();
        graph.bind_session("goal-1", "ses-1", 2).unwrap();
        graph.link_run("goal-1", "run-1", 3).unwrap();
        graph
            .add_commitment("commit-1", "goal-1", "tests", None, 4)
            .unwrap();
        graph
            .close_commitment("commit-1", CommitmentStatus::Kept, "passed", 5)
            .unwrap();
        let outcome = GoalOutcome {
            closure: GoalClosure::Achieved,
            answer_run_id: Some("run-1".into()),
            verification: GoalVerification::Passed,
            verification_evidence: Some(verification_evidence("run-1")),
            open_commitment_ids: Vec::new(),
            reason: "tests and artifacts verified".into(),
        };
        graph
            .close_with_evidence("goal-1", outcome.clone(), 6)
            .unwrap();
        assert_eq!(graph.goal("goal-1").unwrap().status, GoalStatus::Achieved);
        let replayed = GoalGraph::replay(graph.events()).unwrap();
        assert_eq!(replayed.snapshot(), graph.snapshot());
        assert!(matches!(
            graph.close_with_evidence("goal-1", outcome, 7).unwrap_err(),
            GoalError::GoalNotActive(id) if id == "goal-1"
        ));
    }

    #[test]
    fn close_with_evidence_rejects_unproven_achieved() {
        fn outcome(
            run: Option<&str>,
            verification: GoalVerification,
            open: &[&str],
            reason: &str,
        ) -> GoalOutcome {
            GoalOutcome {
                closure: GoalClosure::Achieved,
                answer_run_id: run.map(str::to_string),
                verification,
                verification_evidence: None,
                open_commitment_ids: open.iter().map(|id| id.to_string()).collect(),
                reason: reason.into(),
            }
        }

        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "ship", None, 1)
            .unwrap();
        graph.link_run("goal-1", "run-1", 2).unwrap();
        graph
            .add_commitment("commit-1", "goal-1", "tests", None, 3)
            .unwrap();
        graph
            .create_goal("goal-2", "owner-1", "other", None, 4)
            .unwrap();
        graph
            .add_commitment("commit-2", "goal-2", "other tests", None, 5)
            .unwrap();

        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(None, GoalVerification::Passed, &[], "done"),
                    6
                )
                .unwrap_err(),
            GoalError::InsufficientEvidence { .. }
        ));
        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(Some("run-missing"), GoalVerification::Passed, &[], "done"),
                    7
                )
                .unwrap_err(),
            GoalError::EvidenceRunNotLinked { run_id, .. } if run_id == "run-missing"
        ));
        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(Some("run-1"), GoalVerification::Failed, &[], "done"),
                    8
                )
                .unwrap_err(),
            GoalError::InsufficientEvidence { .. }
        ));
        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(Some("run-1"), GoalVerification::Passed, &[], "done"),
                    9
                )
                .unwrap_err(),
            GoalError::OpenCommitmentsRemain { commitment_ids, .. }
                if commitment_ids == ["commit-1"]
        ));
        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(Some("run-1"), GoalVerification::Passed, &["missing"], "done"),
                    10
                )
                .unwrap_err(),
            GoalError::UnknownCommitment(id) if id == "missing"
        ));
        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(Some("run-1"), GoalVerification::Passed, &["commit-2"], "done"),
                    11
                )
                .unwrap_err(),
            GoalError::CommitmentGoalMismatch { commitment_id, .. }
                if commitment_id == "commit-2"
        ));
        assert!(matches!(
            graph
                .close_with_evidence(
                    "goal-1",
                    outcome(Some("run-1"), GoalVerification::Passed, &[], ""),
                    12
                )
                .unwrap_err(),
            GoalError::InvalidOutcomeReason
        ));
        assert_eq!(graph.goal("goal-1").unwrap().status, GoalStatus::Active);
    }

    #[test]
    fn abandoned_closure_requires_reason_and_cancels_every_pending_commitment() {
        let abandoned = |open: &[&str], reason: &str| GoalOutcome {
            closure: GoalClosure::Abandoned,
            answer_run_id: None,
            verification: GoalVerification::Unknown,
            verification_evidence: None,
            open_commitment_ids: open.iter().map(|id| id.to_string()).collect(),
            reason: reason.into(),
        };

        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "ship", None, 1)
            .unwrap();
        graph
            .add_commitment("commit-1", "goal-1", "notes", None, 2)
            .unwrap();
        graph
            .add_commitment("commit-2", "goal-1", "tests", None, 3)
            .unwrap();
        graph
            .close_commitment("commit-2", CommitmentStatus::Missed, "late", 4)
            .unwrap();
        assert!(matches!(
            graph
                .close_with_evidence("goal-1", abandoned(&[], "scope dropped"), 5)
                .unwrap_err(),
            GoalError::OpenCommitmentUnlisted { commitment_id, .. }
                if commitment_id == "commit-1"
        ));
        assert!(matches!(
            graph
                .close_with_evidence("goal-1", abandoned(&["commit-2"], "scope dropped"), 6)
                .unwrap_err(),
            GoalError::CommitmentNotPending(id) if id == "commit-2"
        ));
        graph
            .close_with_evidence("goal-1", abandoned(&["commit-1"], "scope dropped"), 7)
            .unwrap();
        assert_eq!(graph.goal("goal-1").unwrap().status, GoalStatus::Abandoned);
        assert_eq!(
            graph.commitment("commit-1").unwrap().status,
            CommitmentStatus::Cancelled
        );
        assert_eq!(graph.commitment("commit-1").unwrap().closed_ms, Some(7));
        assert_eq!(
            graph.commitment("commit-2").unwrap().status,
            CommitmentStatus::Missed
        );
        let replayed = GoalGraph::replay(graph.events()).unwrap();
        assert_eq!(replayed.snapshot(), graph.snapshot());

        graph
            .create_goal("goal-2", "owner-1", "paused goal", None, 8)
            .unwrap();
        graph
            .set_goal_status("goal-2", GoalStatus::Paused, "waiting", 9)
            .unwrap();
        graph
            .close_with_evidence("goal-2", abandoned(&[], "cancelled"), 10)
            .unwrap();
        assert_eq!(graph.goal("goal-2").unwrap().status, GoalStatus::Abandoned);
        assert!(matches!(
            graph
                .close_with_evidence("goal-2", abandoned(&[], "again"), 11)
                .unwrap_err(),
            GoalError::InvalidGoalTransition(GoalStatus::Abandoned, GoalStatus::Abandoned)
        ));
    }

    #[test]
    fn close_with_evidence_requires_matching_independent_verification() {
        fn graph() -> GoalGraph {
            let mut graph = GoalGraph::default();
            graph
                .create_goal("goal-1", "owner-1", "ship", None, 1)
                .unwrap();
            graph.link_run("goal-1", "run-1", 2).unwrap();
            graph
        }
        let base = GoalOutcome {
            closure: GoalClosure::Achieved,
            answer_run_id: Some("run-1".into()),
            verification: GoalVerification::Passed,
            verification_evidence: None,
            open_commitment_ids: Vec::new(),
            reason: "verified".into(),
        };

        assert!(matches!(
            graph().close_with_evidence("goal-1", base.clone(), 3).unwrap_err(),
            GoalError::MissingVerification { goal_id, run_id }
                if goal_id == "goal-1" && run_id == "run-1"
        ));

        let mut mismatched = base.clone();
        mismatched.verification_evidence = Some(verification_evidence("run-2"));
        assert!(matches!(
            graph().close_with_evidence("goal-1", mismatched, 3).unwrap_err(),
            GoalError::VerificationRunMismatch {
                evidence_run_id,
                answer_run_id,
                ..
            } if evidence_run_id == "run-2" && answer_run_id == "run-1"
        ));

        let mut self_verified = base.clone();
        let mut evidence = verification_evidence("run-1");
        evidence.verifier_id = evidence.executor_id.clone();
        self_verified.verification_evidence = Some(evidence);
        assert!(matches!(
            graph().close_with_evidence("goal-1", self_verified, 3).unwrap_err(),
            GoalError::InvalidVerificationEvidence { reason, .. }
                if reason.contains("differ")
        ));

        let mut valid = base;
        valid.verification_evidence = Some(verification_evidence("run-1"));
        let mut graph = graph();
        graph.close_with_evidence("goal-1", valid, 3).unwrap();
        assert_eq!(graph.goal("goal-1").unwrap().status, GoalStatus::Achieved);
        let closed = graph
            .events()
            .iter()
            .find_map(|event| match &event.kind {
                GoalEventKind::GoalStatusChanged {
                    status: GoalStatus::Achieved,
                    verification_evidence,
                    ..
                } => verification_evidence.clone(),
                _ => None,
            })
            .expect("closure evidence is stored in the goal event");
        assert_eq!(closed.verification_id, "verification-run-1");
    }

    #[test]
    fn store_level_close_helper_persists_and_replays() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::open(&path).unwrap();
        store
            .with_graph_mut(|graph| graph.create_goal("goal-1", "owner-1", "ship", None, 1))
            .unwrap();
        bind_session_goal(&mut store, "goal-1", "ses-1", 2).unwrap();
        link_run_to_goal(&mut store, "goal-1", "run-1", 3).unwrap();
        close_goal_with_evidence(
            &mut store,
            "goal-1",
            GoalOutcome {
                closure: GoalClosure::Achieved,
                answer_run_id: Some("run-1".into()),
                verification: GoalVerification::Passed,
                verification_evidence: Some(verification_evidence("run-1")),
                open_commitment_ids: Vec::new(),
                reason: "verified".into(),
            },
            4,
        )
        .unwrap();
        drop(store);

        let reopened = GoalStore::open(&path).unwrap();
        assert_eq!(
            reopened.graph().goal("goal-1").unwrap().status,
            GoalStatus::Achieved
        );
        assert_eq!(reopened.graph().events().len(), 4);
    }

    #[test]
    fn commitment_lifecycle_negatives_fail_closed() {
        let mut graph = GoalGraph::default();
        graph
            .create_goal("goal-1", "owner-1", "ship", None, 1)
            .unwrap();
        graph
            .create_goal("goal-2", "owner-1", "other", None, 2)
            .unwrap();
        assert!(matches!(
            graph
                .add_commitment("", "goal-1", "notes", None, 3)
                .unwrap_err(),
            GoalError::EmptyCommitmentId
        ));
        assert!(matches!(
            graph
                .add_commitment("commit-1", "goal-1", " ", None, 3)
                .unwrap_err(),
            GoalError::InvalidDescription
        ));
        graph
            .add_commitment("commit-1", "goal-1", "notes", None, 4)
            .unwrap();
        assert!(matches!(
            graph
                .add_commitment("commit-1", "goal-1", "again", None, 5)
                .unwrap_err(),
            GoalError::CommitmentExists(id) if id == "commit-1"
        ));
        assert!(matches!(
            graph
                .add_commitment("commit-1", "goal-2", "other notes", None, 5)
                .unwrap_err(),
            GoalError::CommitmentExists(id) if id == "commit-1"
        ));

        graph
            .link_commitment_ticket("commit-1", "ticket-1", 6)
            .unwrap();
        let events_after_link = graph.events().len();
        graph
            .link_commitment_ticket("commit-1", "ticket-1", 7)
            .unwrap();
        assert_eq!(graph.events().len(), events_after_link);
        assert!(matches!(
            graph
                .link_commitment_ticket("commit-1", "ticket-2", 8)
                .unwrap_err(),
            GoalError::CommitmentAlreadyLinked { commitment_id, ticket_id }
                if commitment_id == "commit-1" && ticket_id == "ticket-1"
        ));
        assert!(matches!(
            graph
                .link_commitment_ticket("commit-1", " ", 8)
                .unwrap_err(),
            GoalError::EmptyTicketId
        ));
        assert!(matches!(
            graph
                .link_commitment_ticket("missing", "ticket-1", 8)
                .unwrap_err(),
            GoalError::UnknownCommitment(id) if id == "missing"
        ));

        assert!(matches!(
            graph
                .close_commitment("commit-1", CommitmentStatus::Pending, "noop", 9)
                .unwrap_err(),
            GoalError::CommitmentNotPending(id) if id == "commit-1"
        ));
        graph
            .close_commitment("commit-1", CommitmentStatus::Kept, "done", 10)
            .unwrap();
        assert!(matches!(
            graph
                .close_commitment("commit-1", CommitmentStatus::Missed, "again", 11)
                .unwrap_err(),
            GoalError::CommitmentNotPending(id) if id == "commit-1"
        ));
        assert!(matches!(
            graph
                .link_commitment_ticket("commit-1", "ticket-1", 12)
                .unwrap_err(),
            GoalError::CommitmentNotPending(id) if id == "commit-1"
        ));
        assert!(matches!(
            graph
                .close_commitment("missing", CommitmentStatus::Kept, "x", 12)
                .unwrap_err(),
            GoalError::UnknownCommitment(id) if id == "missing"
        ));

        graph
            .set_goal_status("goal-2", GoalStatus::Achieved, "done", 13)
            .unwrap();
        assert!(matches!(
            graph
                .add_commitment("commit-3", "goal-2", "late", None, 14)
                .unwrap_err(),
            GoalError::GoalNotActive(id) if id == "goal-2"
        ));

        let replayed = GoalGraph::replay(graph.events()).unwrap();
        assert_eq!(replayed.snapshot(), graph.snapshot());
    }

    #[test]
    fn store_persists_and_rejects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = GoalStore::open(&path).unwrap();
        store
            .with_graph_mut(|graph| {
                graph.create_goal("goal-1", "owner-1", "ship", None, 1)?;
                graph.add_commitment("commit-1", "goal-1", "test", None, 2)
            })
            .unwrap();
        drop(store);
        let reopened = GoalStore::open(&path).unwrap();
        assert_eq!(reopened.graph().snapshot().goals.len(), 1);
        assert_eq!(reopened.graph().snapshot().commitments.len(), 1);
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"ship\"", "\"hacked\"");
        std::fs::write(&path, text).unwrap();
        assert!(GoalStore::open(&path).is_err());
    }
}
