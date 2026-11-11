//! Team handoff notes.
//!
//! A handoff carries a grounded summary, open items and evidence references
//! from one person to another. The store is append-only snapshots; the latest
//! snapshot per id wins. Only the addressee may accept, and a terminal handoff
//! cannot transition again.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HandoffError {
    #[error("handoff store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("handoff JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("handoff is invalid: {0}")]
    Invalid(String),
    #[error("handoff id already exists")]
    Duplicate,
    #[error("handoff was not found")]
    NotFound,
    #[error("only the addressee may accept this handoff")]
    Forbidden,
    #[error("handoff is not open")]
    Terminal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HandoffStatus {
    Open,
    Accepted,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub id: String,
    pub from: String,
    pub to: String,
    pub scope: String,
    pub summary: String,
    #[serde(default)]
    pub open_items: Vec<String>,
    pub evidence: Vec<String>,
    pub created_ms: u128,
    pub status: HandoffStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancelled_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancelled_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancelled_reason: Option<String>,
}

#[derive(Debug)]
pub struct HandoffStore {
    path: PathBuf,
    entries: BTreeMap<String, Handoff>,
}

impl HandoffStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, HandoffError> {
        let path = path.into();
        let mut entries = BTreeMap::new();
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let complete = if content.ends_with('\n') {
                    content.as_str()
                } else {
                    content.rsplit_once('\n').map_or("", |(head, _)| head)
                };
                for (index, line) in complete.lines().enumerate() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let handoff: Handoff = serde_json::from_str(line).map_err(|error| {
                        HandoffError::Invalid(format!("line {}: {error}", index + 1))
                    })?;
                    validate_static(&handoff)?;
                    entries.insert(handoff.id.clone(), handoff);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self { path, entries })
    }

    pub fn create(&mut self, handoff: Handoff) -> Result<&Handoff, HandoffError> {
        validate_static(&handoff)?;
        if handoff.status != HandoffStatus::Open
            || handoff.accepted_ms.is_some()
            || handoff.accepted_by.is_some()
            || handoff.cancelled_ms.is_some()
            || handoff.cancelled_by.is_some()
        {
            return Err(HandoffError::Invalid(
                "a new handoff must be open and unaccepted".into(),
            ));
        }
        if self.entries.contains_key(&handoff.id) {
            return Err(HandoffError::Duplicate);
        }
        let id = handoff.id.clone();
        self.append(&handoff)?;
        self.entries.insert(id.clone(), handoff);
        Ok(self.entries.get(&id).expect("handoff is present"))
    }

    pub fn accept(
        &mut self,
        id: &str,
        actor: &str,
        now_ms: u128,
    ) -> Result<&Handoff, HandoffError> {
        let Some(existing) = self.entries.get(id).cloned() else {
            return Err(HandoffError::NotFound);
        };
        if existing.to != actor {
            return Err(HandoffError::Forbidden);
        }
        if existing.status != HandoffStatus::Open {
            return Err(HandoffError::Terminal);
        }
        let mut accepted = existing;
        accepted.status = HandoffStatus::Accepted;
        accepted.accepted_ms = Some(now_ms);
        accepted.accepted_by = Some(actor.into());
        self.append(&accepted)?;
        self.entries.insert(id.into(), accepted);
        Ok(self.entries.get(id).expect("handoff is present"))
    }

    pub fn cancel(
        &mut self,
        id: &str,
        actor: &str,
        reason: &str,
        now_ms: u128,
    ) -> Result<&Handoff, HandoffError> {
        let Some(existing) = self.entries.get(id).cloned() else {
            return Err(HandoffError::NotFound);
        };
        if existing.from != actor && existing.to != actor {
            return Err(HandoffError::Forbidden);
        }
        if existing.status != HandoffStatus::Open {
            return Err(HandoffError::Terminal);
        }
        let mut cancelled = existing;
        cancelled.status = HandoffStatus::Cancelled;
        cancelled.cancelled_ms = Some(now_ms);
        cancelled.cancelled_by = Some(actor.into());
        cancelled.cancelled_reason = if reason.trim().is_empty() {
            None
        } else {
            Some(reason.trim().into())
        };
        self.append(&cancelled)?;
        self.entries.insert(id.into(), cancelled);
        Ok(self.entries.get(id).expect("handoff is present"))
    }

    pub fn list(&self) -> Vec<&Handoff> {
        let mut items: Vec<&Handoff> = self.entries.values().collect();
        items.sort_by(|left, right| {
            left.created_ms
                .cmp(&right.created_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        items
    }

    pub fn get(&self, id: &str) -> Option<&Handoff> {
        self.entries.get(id)
    }

    fn append(&self, handoff: &Handoff) -> Result<(), HandoffError> {
        let line = serde_json::to_vec(handoff)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&line)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(())
    }
}

fn validate_static(handoff: &Handoff) -> Result<(), HandoffError> {
    if handoff.id.trim().is_empty() {
        return Err(HandoffError::Invalid("id is empty".into()));
    }
    if handoff.from.trim().is_empty() || handoff.to.trim().is_empty() {
        return Err(HandoffError::Invalid("from and to are required".into()));
    }
    if handoff.from == handoff.to {
        return Err(HandoffError::Invalid(
            "from and to must be different people".into(),
        ));
    }
    if handoff.summary.trim().is_empty() {
        return Err(HandoffError::Invalid("summary is empty".into()));
    }
    if handoff.evidence.is_empty() || handoff.evidence.iter().any(|item| item.trim().is_empty()) {
        return Err(HandoffError::Invalid(
            "at least one non-empty evidence reference is required".into(),
        ));
    }
    if handoff.open_items.iter().any(|item| item.trim().is_empty()) {
        return Err(HandoffError::Invalid("open items must not be empty".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "1agent-handoff-{}-{name}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn handoff(id: &str) -> Handoff {
        Handoff {
            id: id.into(),
            from: "alice".into(),
            to: "bob".into(),
            scope: "release".into(),
            summary: "prepared the release candidate".into(),
            open_items: vec!["run the smoke test".into()],
            evidence: vec!["journal:run-1".into()],
            created_ms: 1,
            status: HandoffStatus::Open,
            accepted_ms: None,
            accepted_by: None,
            cancelled_ms: None,
            cancelled_by: None,
            cancelled_reason: None,
        }
    }

    #[test]
    fn create_and_accept_survive_reopen() {
        let path = temp_store("accept");
        let mut store = HandoffStore::open(&path).unwrap();
        store.create(handoff("h1")).unwrap();
        store.accept("h1", "bob", 2).unwrap();
        drop(store);

        let reopened = HandoffStore::open(&path).unwrap();
        let saved = reopened.get("h1").unwrap();
        assert_eq!(saved.status, HandoffStatus::Accepted);
        assert_eq!(saved.accepted_by.as_deref(), Some("bob"));
        assert_eq!(saved.accepted_ms, Some(2));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn only_the_addressee_can_accept_once() {
        let path = temp_store("forbidden");
        let mut store = HandoffStore::open(&path).unwrap();
        store.create(handoff("h1")).unwrap();
        assert!(matches!(
            store.accept("h1", "alice", 2),
            Err(HandoffError::Forbidden)
        ));
        store.accept("h1", "bob", 3).unwrap();
        assert!(matches!(
            store.accept("h1", "bob", 4),
            Err(HandoffError::Terminal)
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn either_party_can_cancel_an_open_handoff_once() {
        let path = temp_store("cancel");
        let mut store = HandoffStore::open(&path).unwrap();
        store.create(handoff("h1")).unwrap();
        let cancelled = store.cancel("h1", "bob", "not needed", 5).unwrap();
        assert_eq!(cancelled.status, HandoffStatus::Cancelled);
        assert_eq!(cancelled.cancelled_by.as_deref(), Some("bob"));
        assert_eq!(cancelled.cancelled_reason.as_deref(), Some("not needed"));
        assert!(cancelled.accepted_by.is_none());
        assert!(matches!(
            store.accept("h1", "bob", 6),
            Err(HandoffError::Terminal)
        ));
        assert!(matches!(
            store.cancel("h1", "alice", "", 7),
            Err(HandoffError::Terminal)
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn strangers_cannot_cancel_and_accepted_handoffs_are_terminal() {
        let path = temp_store("cancel-forbidden");
        let mut store = HandoffStore::open(&path).unwrap();
        store.create(handoff("h1")).unwrap();
        assert!(matches!(
            store.cancel("h1", "mallory", "", 5),
            Err(HandoffError::Forbidden)
        ));
        store.accept("h1", "bob", 6).unwrap();
        assert!(matches!(
            store.cancel("h1", "alice", "", 7),
            Err(HandoffError::Terminal)
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn duplicate_and_ungrounded_handoffs_are_rejected() {
        let path = temp_store("invalid");
        let mut store = HandoffStore::open(&path).unwrap();
        store.create(handoff("h1")).unwrap();
        assert!(matches!(
            store.create(handoff("h1")),
            Err(HandoffError::Duplicate)
        ));

        let mut no_evidence = handoff("h2");
        no_evidence.evidence.clear();
        assert!(store.create(no_evidence).is_err());

        let mut empty_summary = handoff("h3");
        empty_summary.summary = " ".into();
        assert!(store.create(empty_summary).is_err());

        let mut self_handoff = handoff("h4");
        self_handoff.to = "alice".into();
        assert!(store.create(self_handoff).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn list_is_sorted_and_unknown_accept_fails() {
        let path = temp_store("list");
        let mut store = HandoffStore::open(&path).unwrap();
        let mut later = handoff("h2");
        later.created_ms = 20;
        let mut earlier = handoff("h1");
        earlier.created_ms = 10;
        store.create(later).unwrap();
        store.create(earlier).unwrap();
        let ids: Vec<&str> = store.list().iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["h1", "h2"]);
        assert!(matches!(
            store.accept("missing", "bob", 1),
            Err(HandoffError::NotFound)
        ));
        std::fs::remove_file(&path).ok();
    }
}
