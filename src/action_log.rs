//! Hash-chained action log for security-relevant operations.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionLogError {
    #[error("action actor is empty")]
    EmptyActor,
    #[error("action target is empty")]
    EmptyTarget,
    #[error("action event hash mismatch at {0}")]
    HashMismatch(u64),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Approve,
    Retry,
    Takeover,
    ReleaseTakeover,
    SideEffect,
    RevokeMandate,
    Cancel,
    SkillActivate,
    MemoryForget,
}

impl ActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Retry => "retry",
            Self::Takeover => "takeover",
            Self::ReleaseTakeover => "release_takeover",
            Self::SideEffect => "side_effect",
            Self::RevokeMandate => "revoke_mandate",
            Self::Cancel => "cancel",
            Self::SkillActivate => "skill_activate",
            Self::MemoryForget => "memory_forget",
        }
    }
}

impl std::str::FromStr for ActionKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "approve" => Ok(Self::Approve),
            "retry" => Ok(Self::Retry),
            "takeover" => Ok(Self::Takeover),
            "release_takeover" => Ok(Self::ReleaseTakeover),
            "side_effect" => Ok(Self::SideEffect),
            "revoke_mandate" => Ok(Self::RevokeMandate),
            "cancel" => Ok(Self::Cancel),
            "skill_activate" => Ok(Self::SkillActivate),
            "memory_forget" => Ok(Self::MemoryForget),
            other => Err(format!("unknown action kind {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionActorCount {
    pub actor: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionKindCount {
    pub kind: ActionKind,
    pub count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLogSummary {
    pub total: u64,
    pub actors: Vec<ActionActorCount>,
    pub kinds: Vec<ActionKindCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionLogEntry {
    pub seq: u64,
    pub entry_id: String,
    pub timestamp_unix_ms: u128,
    pub actor: String,
    pub kind: ActionKind,
    pub target: String,
    pub outcome: String,
    pub reason: String,
    pub previous_sha256: Option<String>,
    pub entry_sha256: String,
}

#[derive(Debug, Clone, Default)]
pub struct ActionLog {
    entries: Vec<ActionLogEntry>,
    next_seq: u64,
    last_sha256: Option<String>,
}

impl ActionLog {
    pub fn from_entries(entries: Vec<ActionLogEntry>) -> Result<Self, ActionLogError> {
        let log = Self {
            next_seq: entries.last().map(|entry| entry.seq).unwrap_or(0),
            last_sha256: entries.last().map(|entry| entry.entry_sha256.clone()),
            entries,
        };
        log.verify()?;
        Ok(log)
    }

    pub fn record(
        &mut self,
        actor: impl Into<String>,
        kind: ActionKind,
        target: impl Into<String>,
        outcome: impl Into<String>,
        reason: impl Into<String>,
        now_ms: u128,
    ) -> Result<ActionLogEntry, ActionLogError> {
        let actor = actor.into();
        let target = target.into();
        if actor.trim().is_empty() {
            return Err(ActionLogError::EmptyActor);
        }
        if target.trim().is_empty() {
            return Err(ActionLogError::EmptyTarget);
        }
        let mut entry = ActionLogEntry {
            seq: self.next_seq + 1,
            entry_id: Uuid::new_v4().to_string(),
            timestamp_unix_ms: now_ms,
            actor,
            kind,
            target,
            outcome: outcome.into(),
            reason: reason.into(),
            previous_sha256: self.last_sha256.clone(),
            entry_sha256: String::new(),
        };
        entry.entry_sha256 = hash_entry(&entry)?;
        self.next_seq = entry.seq;
        self.last_sha256 = Some(entry.entry_sha256.clone());
        self.entries.push(entry.clone());
        Ok(entry)
    }

    pub fn verify(&self) -> Result<(), ActionLogError> {
        let mut previous: Option<String> = None;
        for (expected, entry) in (1u64..).zip(&self.entries) {
            if entry.seq != expected
                || entry.previous_sha256.as_deref() != previous.as_deref()
                || entry.entry_sha256 != hash_entry(entry)?
            {
                return Err(ActionLogError::HashMismatch(entry.seq));
            }
            previous = Some(entry.entry_sha256.clone());
        }
        Ok(())
    }

    pub fn entries(&self) -> &[ActionLogEntry] {
        &self.entries
    }

    pub fn query(
        &self,
        actor: Option<&str>,
        kind: Option<ActionKind>,
        target: Option<&str>,
        limit: usize,
    ) -> Vec<&ActionLogEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                actor.is_none_or(|actor| entry.actor == actor)
                    && kind.is_none_or(|kind| entry.kind == kind)
                    && target.is_none_or(|target| entry.target == target)
            })
            .take(limit)
            .collect()
    }

    pub fn query_page(
        &self,
        actor: Option<&str>,
        kind: Option<ActionKind>,
        target: Option<&str>,
        after_seq: Option<u64>,
        limit: usize,
    ) -> Vec<&ActionLogEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                after_seq.is_none_or(|after| entry.seq > after)
                    && actor.is_none_or(|actor| entry.actor == actor)
                    && kind.is_none_or(|kind| entry.kind == kind)
                    && target.is_none_or(|target| entry.target == target)
            })
            .take(limit)
            .collect()
    }

    pub fn summary(&self) -> ActionLogSummary {
        let mut actors = BTreeMap::<String, u64>::new();
        let mut kinds = BTreeMap::<ActionKind, u64>::new();
        for entry in &self.entries {
            *actors.entry(entry.actor.clone()).or_insert(0) += 1;
            *kinds.entry(entry.kind).or_insert(0) += 1;
        }
        ActionLogSummary {
            total: self.entries.len() as u64,
            actors: actors
                .into_iter()
                .map(|(actor, count)| ActionActorCount { actor, count })
                .collect(),
            kinds: kinds
                .into_iter()
                .map(|(kind, count)| ActionKindCount { kind, count })
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Serialize)]
struct ActionHashPayload<'a> {
    seq: u64,
    entry_id: &'a str,
    timestamp_unix_ms: u128,
    actor: &'a str,
    kind: ActionKind,
    target: &'a str,
    outcome: &'a str,
    reason: &'a str,
    previous_sha256: &'a Option<String>,
}

fn hash_entry(entry: &ActionLogEntry) -> Result<String, ActionLogError> {
    let payload = ActionHashPayload {
        seq: entry.seq,
        entry_id: &entry.entry_id,
        timestamp_unix_ms: entry.timestamp_unix_ms,
        actor: &entry.actor,
        kind: entry.kind,
        target: &entry.target,
        outcome: &entry.outcome,
        reason: &entry.reason,
        previous_sha256: &entry.previous_sha256,
    };
    let bytes =
        serde_json::to_vec(&payload).map_err(|_| ActionLogError::HashMismatch(entry.seq))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_chain_and_query_filters() {
        let mut log = ActionLog::default();
        log.record("owner", ActionKind::Approve, "ticket-1", "allowed", "ok", 1)
            .unwrap();
        log.record(
            "owner",
            ActionKind::Retry,
            "ticket-1",
            "allowed",
            "retry",
            2,
        )
        .unwrap();
        log.record(
            "worker",
            ActionKind::SideEffect,
            "ticket-1",
            "applied",
            "write",
            3,
        )
        .unwrap();
        log.verify().unwrap();
        assert_eq!(log.query(Some("owner"), None, None, 10).len(), 2);
        assert_eq!(
            log.query(None, Some(ActionKind::SideEffect), Some("ticket-1"), 10)
                .len(),
            1
        );
        assert_eq!(
            log.query_page(None, None, None, Some(1), 10)
                .iter()
                .map(|entry| entry.seq)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
        let summary = log.summary();
        assert_eq!(summary.total, 3);
        assert_eq!(
            summary
                .actors
                .iter()
                .find(|actor| actor.actor == "owner")
                .unwrap()
                .count,
            2
        );
        assert_eq!(summary.kinds.len(), 3);
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn tampering_and_empty_fields_fail_closed() {
        let mut log = ActionLog::default();
        log.record("owner", ActionKind::Approve, "ticket-1", "allowed", "ok", 1)
            .unwrap();
        let mut tampered = log.entries().to_vec();
        tampered[0].actor = "attacker".into();
        let forged = ActionLog {
            entries: tampered,
            next_seq: 1,
            last_sha256: None,
        };
        assert!(matches!(
            forged.verify().unwrap_err(),
            ActionLogError::HashMismatch(1)
        ));
        assert_eq!(
            log.record(" ", ActionKind::Approve, "ticket-1", "allowed", "", 2)
                .unwrap_err(),
            ActionLogError::EmptyActor
        );
        assert_eq!(
            log.record("owner", ActionKind::Approve, " ", "allowed", "", 2)
                .unwrap_err(),
            ActionLogError::EmptyTarget
        );
    }
}
