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
}

#[derive(Debug, Clone, Default)]
pub struct GoalGraph {
    goals: BTreeMap<String, Goal>,
    commitments: BTreeMap<String, Commitment>,
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

    pub fn snapshot(&self) -> GoalSnapshot {
        GoalSnapshot {
            goals: self.goals.values().cloned().collect(),
            commitments: self.commitments.values().cloned().collect(),
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
            },
            now_ms,
        )
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
            GoalEventKind::CommitmentAdded { commitment } => {
                if self.commitments.contains_key(&commitment.commitment_id) {
                    return Err(GoalError::CommitmentExists(commitment.commitment_id));
                }
                self.commitments
                    .insert(commitment.commitment_id.clone(), commitment);
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
