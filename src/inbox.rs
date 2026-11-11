//! Agent Inbox: a durable, auditable queue of things the partner wants the
//! user to see, answer or approve.
//!
//! The store is append-only JSONL. Every status change is a new snapshot, so
//! replay keeps the latest state and the history remains inspectable.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("inbox io: {0}")]
    Io(#[from] std::io::Error),
    #[error("inbox json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("inbox item is invalid: {0}")]
    Invalid(String),
    #[error("inbox item already exists: {0}")]
    DuplicateId(String),
    #[error("inbox item not found: {0}")]
    UnknownItem(String),
    #[error("invalid inbox transition for {0}: {1:?} -> {2:?}")]
    InvalidTransition(String, InboxStatus, InboxStatus),
    #[error("snooze requires a future until_ms")]
    InvalidSnooze,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InboxKind {
    /// Informational: no decision required.
    Notify,
    /// The partner needs an answer.
    Question,
    /// The partner needs an approval or rejection.
    Review,
}

impl InboxKind {
    pub fn priority(self) -> u8 {
        match self {
            Self::Review => 0,
            Self::Question => 1,
            Self::Notify => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Question => "question",
            Self::Review => "review",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InboxStatus {
    Open,
    Snoozed,
    Approved,
    Rejected,
    Answered,
    Dismissed,
}

impl InboxStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Approved | Self::Rejected | Self::Answered | Self::Dismissed
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Snoozed => "snoozed",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Answered => "answered",
            Self::Dismissed => "dismissed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InboxItem {
    pub item_id: String,
    pub kind: InboxKind,
    pub title: String,
    pub why_now: String,
    pub source: String,
    pub created_ms: u128,
    pub updated_ms: u128,
    pub due_ms: Option<u128>,
    pub status: InboxStatus,
    pub snoozed_until_ms: Option<u128>,
    pub resolution: Option<String>,
}

impl InboxItem {
    pub fn new(
        item_id: impl Into<String>,
        kind: InboxKind,
        title: impl Into<String>,
        why_now: impl Into<String>,
        source: impl Into<String>,
        created_ms: u128,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            kind,
            title: title.into(),
            why_now: why_now.into(),
            source: source.into(),
            created_ms,
            updated_ms: created_ms,
            due_ms: None,
            status: InboxStatus::Open,
            snoozed_until_ms: None,
            resolution: None,
        }
    }

    pub fn with_due_ms(mut self, due_ms: u128) -> Self {
        self.due_ms = Some(due_ms);
        self
    }

    pub fn validate(&self) -> Result<(), InboxError> {
        if self.item_id.trim().is_empty()
            || self.title.trim().is_empty()
            || self.why_now.trim().is_empty()
            || self.source.trim().is_empty()
        {
            return Err(InboxError::Invalid(
                "item_id, title, why_now and source must be non-empty".into(),
            ));
        }
        if self.title.chars().count() > 500 {
            return Err(InboxError::Invalid("title is too long".into()));
        }
        if self.updated_ms < self.created_ms {
            return Err(InboxError::Invalid(
                "updated_ms must not precede created_ms".into(),
            ));
        }
        Ok(())
    }

    fn transition(
        &self,
        status: InboxStatus,
        at_ms: u128,
        resolution: Option<String>,
    ) -> Result<Self, InboxError> {
        let allowed = match self.status {
            InboxStatus::Open | InboxStatus::Snoozed => true,
            InboxStatus::Approved
            | InboxStatus::Rejected
            | InboxStatus::Answered
            | InboxStatus::Dismissed => false,
        };
        if !allowed {
            return Err(InboxError::InvalidTransition(
                self.item_id.clone(),
                self.status,
                status,
            ));
        }
        if at_ms < self.updated_ms {
            return Err(InboxError::Invalid(
                "status change time precedes the last update".into(),
            ));
        }
        let mut next = self.clone();
        next.status = status;
        next.updated_ms = at_ms;
        next.resolution = resolution;
        next.snoozed_until_ms = None;
        Ok(next)
    }
}

pub struct InboxStore {
    path: PathBuf,
    items: BTreeMap<String, InboxItem>,
}

impl InboxStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, InboxError> {
        let path = path.into();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            crate::jsonl::ensure_parent_private(&path)?;
        }
        let _lock = crate::jsonl::FileLock::exclusive(&path)?;
        let mut items = BTreeMap::new();
        if path.exists() {
            let file = std::fs::File::open(&path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let item: InboxItem = serde_json::from_str(&line)?;
                item.validate()?;
                items.insert(item.item_id.clone(), item);
            }
        }
        drop(_lock);
        Ok(Self { path, items })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn items(&self) -> impl Iterator<Item = &InboxItem> {
        self.items.values()
    }

    pub fn get(&self, item_id: &str) -> Option<&InboxItem> {
        self.items.get(item_id)
    }

    pub fn add(&mut self, item: InboxItem) -> Result<(), InboxError> {
        item.validate()?;
        if self.items.contains_key(&item.item_id) {
            return Err(InboxError::DuplicateId(item.item_id));
        }
        self.append(&item)?;
        self.items.insert(item.item_id.clone(), item);
        Ok(())
    }

    pub fn resolve(
        &mut self,
        item_id: &str,
        status: InboxStatus,
        at_ms: u128,
        resolution: Option<String>,
    ) -> Result<InboxItem, InboxError> {
        if !status.is_terminal() {
            return Err(InboxError::Invalid(
                "resolve requires approved, rejected, answered or dismissed".into(),
            ));
        }
        self.update(item_id, status, at_ms, resolution)
    }

    pub fn snooze(
        &mut self,
        item_id: &str,
        until_ms: u128,
        at_ms: u128,
    ) -> Result<InboxItem, InboxError> {
        let current = self
            .items
            .get(item_id)
            .ok_or_else(|| InboxError::UnknownItem(item_id.into()))?;
        if until_ms <= at_ms {
            return Err(InboxError::InvalidSnooze);
        }
        let mut next = current.transition(InboxStatus::Snoozed, at_ms, None)?;
        next.snoozed_until_ms = Some(until_ms);
        self.append(&next)?;
        self.items.insert(item_id.into(), next.clone());
        Ok(next)
    }

    pub fn reopen(&mut self, item_id: &str, at_ms: u128) -> Result<InboxItem, InboxError> {
        self.update(item_id, InboxStatus::Open, at_ms, None)
    }

    /// Open or due-snoozed items, most important first.
    pub fn open_sorted(&self, now_ms: u128) -> Vec<InboxItem> {
        let mut items = self
            .items
            .values()
            .filter(|item| match item.status {
                InboxStatus::Open => true,
                InboxStatus::Snoozed => item.snoozed_until_ms.is_some_and(|until| until <= now_ms),
                _ => false,
            })
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            left.kind
                .priority()
                .cmp(&right.kind.priority())
                .then_with(|| {
                    left.due_ms
                        .unwrap_or(u128::MAX)
                        .cmp(&right.due_ms.unwrap_or(u128::MAX))
                })
                .then_with(|| left.created_ms.cmp(&right.created_ms))
                .then_with(|| left.item_id.cmp(&right.item_id))
        });
        items
    }

    fn update(
        &mut self,
        item_id: &str,
        status: InboxStatus,
        at_ms: u128,
        resolution: Option<String>,
    ) -> Result<InboxItem, InboxError> {
        let current = self
            .items
            .get(item_id)
            .ok_or_else(|| InboxError::UnknownItem(item_id.into()))?;
        let next = current.transition(status, at_ms, resolution)?;
        self.append(&next)?;
        self.items.insert(item_id.into(), next.clone());
        Ok(next)
    }

    fn append(&self, item: &InboxItem) -> Result<(), InboxError> {
        let line = serde_json::to_string(item)?;
        crate::jsonl::append_batch(&self.path, &format!("{line}\n"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, kind: InboxKind, due: Option<u128>) -> InboxItem {
        let mut item = InboxItem::new(id, kind, format!("title {id}"), "why now", "test", 10);
        item.due_ms = due;
        item
    }

    #[test]
    fn add_and_reload_keeps_the_latest_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.jsonl");
        let mut store = InboxStore::open(&path).unwrap();
        store.add(item("i1", InboxKind::Review, Some(20))).unwrap();
        store.snooze("i1", 100, 30).expect("snooze an open item");
        drop(store);
        let reopened = InboxStore::open(&path).unwrap();
        let item = reopened.get("i1").unwrap();
        assert_eq!(item.status, InboxStatus::Snoozed);
        assert_eq!(item.snoozed_until_ms, Some(100));
    }

    #[test]
    fn duplicate_ids_and_invalid_items_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.jsonl");
        let mut store = InboxStore::open(&path).unwrap();
        store.add(item("i1", InboxKind::Notify, None)).unwrap();
        assert!(matches!(
            store.add(item("i1", InboxKind::Notify, None)),
            Err(InboxError::DuplicateId(_))
        ));
        let mut empty = item("i2", InboxKind::Notify, None);
        empty.title.clear();
        assert!(matches!(store.add(empty), Err(InboxError::Invalid(_))));
    }

    #[test]
    fn terminal_status_cannot_transition() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.jsonl");
        let mut store = InboxStore::open(&path).unwrap();
        store.add(item("i1", InboxKind::Review, None)).unwrap();
        store
            .resolve(
                "i1",
                InboxStatus::Approved,
                20,
                Some("approved by owner".into()),
            )
            .unwrap();
        assert!(matches!(
            store.snooze("i1", 100, 30),
            Err(InboxError::InvalidTransition(_, _, _))
        ));
    }

    #[test]
    fn snooze_hides_until_it_expires() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.jsonl");
        let mut store = InboxStore::open(&path).unwrap();
        store.add(item("i1", InboxKind::Question, None)).unwrap();
        store.snooze("i1", 100, 20).unwrap();
        assert!(store.open_sorted(50).is_empty());
        assert_eq!(store.open_sorted(100).len(), 1);
    }

    #[test]
    fn sorting_prioritizes_review_then_question_then_notify() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.jsonl");
        let mut store = InboxStore::open(&path).unwrap();
        store.add(item("n1", InboxKind::Notify, None)).unwrap();
        store.add(item("q1", InboxKind::Question, Some(5))).unwrap();
        store.add(item("r1", InboxKind::Review, Some(9))).unwrap();
        store.add(item("r2", InboxKind::Review, Some(1))).unwrap();
        let ids = store
            .open_sorted(0)
            .into_iter()
            .map(|item| item.item_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["r2", "r1", "q1", "n1"]);
    }
}
