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

use crate::alerts::AlertRecord;
use crate::attention::{AttentionCandidate, AttentionKind};
use crate::handoff::{Handoff, HandoffStatus};

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

impl std::str::FromStr for InboxKind {
    type Err = InboxError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "notify" => Ok(Self::Notify),
            "question" => Ok(Self::Question),
            "review" => Ok(Self::Review),
            _ => Err(InboxError::Invalid(
                "inbox kind must be notify, question or review".into(),
            )),
        }
    }
}

impl std::str::FromStr for InboxStatus {
    type Err = InboxError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "snoozed" => Ok(Self::Snoozed),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "answered" => Ok(Self::Answered),
            "dismissed" => Ok(Self::Dismissed),
            _ => Err(InboxError::Invalid(
                "inbox status must be open, snoozed, approved, rejected, answered or dismissed"
                    .into(),
            )),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InboxSyncReport {
    pub scanned: usize,
    pub added: usize,
    pub skipped: usize,
    pub dry_run: bool,
    pub added_items: Vec<InboxItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxAction {
    Resolve {
        item_id: String,
        status: InboxStatus,
        note: Option<String>,
    },
    Snooze {
        item_id: String,
        until_ms: u128,
    },
}

/// Parse the inline action form used by the TUI:
/// `resolve <id> <approved|rejected|answered|dismissed> [note]`
/// or `snooze <id> <until_ms>`.
pub fn parse_action(args: &str) -> Result<InboxAction, String> {
    let mut parts = args.split_whitespace();
    match parts.next() {
        Some("resolve") => {
            let item_id = parts.next().ok_or("resolve needs an item id")?;
            let status_text = parts.next().ok_or("resolve needs a status")?;
            let status: InboxStatus = status_text
                .parse()
                .map_err(|error: InboxError| error.to_string())?;
            if !status.is_terminal() {
                return Err(
                    "resolve status must be approved, rejected, answered or dismissed".into(),
                );
            }
            let note = {
                let rest = parts.collect::<Vec<_>>().join(" ");
                if rest.is_empty() { None } else { Some(rest) }
            };
            Ok(InboxAction::Resolve {
                item_id: item_id.into(),
                status,
                note,
            })
        }
        Some("snooze") => {
            let item_id = parts.next().ok_or("snooze needs an item id")?;
            let until = parts.next().ok_or("snooze needs until_ms")?;
            let until_ms: u128 = until
                .parse()
                .map_err(|_| "until_ms must be a non-negative integer".to_string())?;
            if parts.next().is_some() {
                return Err("snooze takes an item id and until_ms only".into());
            }
            Ok(InboxAction::Snooze {
                item_id: item_id.into(),
                until_ms,
            })
        }
        Some(other) => Err(format!("unknown inbox action: {other}")),
        None => Err("empty inbox action".into()),
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

    /// Merge projected items into the store, skipping ids that already exist.
    /// `apply = false` reports what would be added without writing.
    pub fn sync(
        &mut self,
        items: impl IntoIterator<Item = InboxItem>,
        apply: bool,
    ) -> Result<InboxSyncReport, InboxError> {
        let mut report = InboxSyncReport {
            scanned: 0,
            added: 0,
            skipped: 0,
            dry_run: !apply,
            added_items: Vec::new(),
        };
        for item in items {
            item.validate()?;
            report.scanned = report.scanned.saturating_add(1);
            if self.items.contains_key(&item.item_id) {
                report.skipped = report.skipped.saturating_add(1);
                continue;
            }
            if apply {
                self.append(&item)?;
                self.items.insert(item.item_id.clone(), item.clone());
            }
            report.added = report.added.saturating_add(1);
            report.added_items.push(item);
        }
        Ok(report)
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

    pub fn apply_action(
        &mut self,
        action: &InboxAction,
        at_ms: u128,
    ) -> Result<InboxItem, InboxError> {
        match action {
            InboxAction::Resolve {
                item_id,
                status,
                note,
            } => self.resolve(item_id, *status, at_ms, note.clone()),
            InboxAction::Snooze { item_id, until_ms } => self.snooze(item_id, *until_ms, at_ms),
        }
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

/// Project attention candidates into inbox items. Overdue or missed
/// commitments need a decision; ordinary due reminders are informational.
pub fn project_attention(candidates: &[AttentionCandidate], now_ms: u128) -> Vec<InboxItem> {
    candidates
        .iter()
        .map(|candidate| {
            let kind = match candidate.kind {
                AttentionKind::CommitmentOverdue | AttentionKind::CommitmentMissed => {
                    InboxKind::Review
                }
                AttentionKind::CommitmentDue | AttentionKind::GoalDue => InboxKind::Notify,
            };
            InboxItem {
                item_id: format!("attention:{}", candidate.id),
                kind,
                title: candidate.title.clone(),
                why_now: candidate.why_now.clone(),
                source: format!("attention:{:?}", candidate.kind),
                created_ms: now_ms,
                updated_ms: now_ms,
                due_ms: candidate.due_ms,
                status: InboxStatus::Open,
                snoozed_until_ms: None,
                resolution: None,
            }
        })
        .collect()
}

/// Project open handoffs addressed to one person. A handoff needs review
/// because accepting it is a decision.
pub fn project_handoffs(handoffs: &[&Handoff], recipient: &str, now_ms: u128) -> Vec<InboxItem> {
    handoffs
        .iter()
        .filter(|handoff| handoff.status == HandoffStatus::Open && handoff.to == recipient)
        .map(|handoff| {
            let title: String = format!("Handoff from {}: {}", handoff.from, handoff.summary)
                .chars()
                .take(500)
                .collect();
            InboxItem {
                item_id: format!("handoff:{}", handoff.id),
                kind: InboxKind::Review,
                title,
                why_now: format!(
                    "open handoff in {} awaits your acceptance",
                    if handoff.scope.is_empty() {
                        "no scope"
                    } else {
                        &handoff.scope
                    }
                ),
                source: "handoffs".into(),
                created_ms: handoff.created_ms.min(now_ms),
                updated_ms: handoff.created_ms.min(now_ms),
                due_ms: None,
                status: InboxStatus::Open,
                snoozed_until_ms: None,
                resolution: None,
            }
        })
        .collect()
}

/// Project alert records into inbox items. Failed/timeout alerts need a
/// decision; health notices are informational.
pub fn project_alerts(alerts: &[AlertRecord], now_ms: u128) -> Vec<InboxItem> {
    alerts
        .iter()
        .map(|alert| {
            let kind = if alert.kind.contains("fail") || alert.kind.contains("error") {
                InboxKind::Review
            } else {
                InboxKind::Notify
            };
            InboxItem {
                item_id: format!("alert:{}", alert.alert_id),
                kind,
                title: alert.message.clone(),
                why_now: format!("alert kind: {}", alert.kind),
                source: "alerts".into(),
                created_ms: alert.at_ms.min(now_ms),
                updated_ms: alert.at_ms.min(now_ms),
                due_ms: None,
                status: InboxStatus::Open,
                snoozed_until_ms: None,
                resolution: None,
            }
        })
        .collect()
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

    #[test]
    fn attention_projection_maps_overdue_to_review_and_due_to_notify() {
        let overdue = AttentionCandidate {
            kind: AttentionKind::CommitmentOverdue,
            id: "c1".into(),
            goal_id: "g1".into(),
            title: "send the report".into(),
            due_ms: Some(10),
            urgency: 100,
            why_now: "overdue".into(),
        };
        let due = AttentionCandidate {
            kind: AttentionKind::GoalDue,
            id: "g1".into(),
            goal_id: "g1".into(),
            title: "ship".into(),
            due_ms: Some(50),
            urgency: 80,
            why_now: "due soon".into(),
        };
        let items = project_attention(&[overdue, due], 20);
        assert_eq!(items[0].kind, InboxKind::Review);
        assert_eq!(items[0].item_id, "attention:c1");
        assert!(items[0].source.contains("CommitmentOverdue"));
        assert_eq!(items[1].kind, InboxKind::Notify);
        assert_eq!(items[1].item_id, "attention:g1");
    }

    #[test]
    fn alert_projection_maps_failures_to_review() {
        let failed = AlertRecord {
            alert_id: "a1".into(),
            at_ms: 10,
            kind: "schedule_run_failed".into(),
            provider_id: None,
            model: None,
            observed: 1,
            threshold: 1,
            message: "run failed".into(),
        };
        let health = AlertRecord {
            alert_id: "a2".into(),
            at_ms: 11,
            kind: "provider_health".into(),
            provider_id: Some("p".into()),
            model: Some("m".into()),
            observed: 2,
            threshold: 2,
            message: "provider unhealthy".into(),
        };
        let items = project_alerts(&[failed, health], 20);
        assert_eq!(items[0].kind, InboxKind::Review);
        assert_eq!(items[0].item_id, "alert:a1");
        assert_eq!(items[1].kind, InboxKind::Notify);
        assert_eq!(items[1].item_id, "alert:a2");
    }

    #[test]
    fn handoff_projection_filters_recipient_and_terminal_items() {
        let open = Handoff {
            id: "h1".into(),
            from: "alice".into(),
            to: "bob".into(),
            scope: "release".into(),
            summary: "prepared the release candidate".into(),
            open_items: vec![],
            evidence: vec!["journal:run-1".into()],
            created_ms: 10,
            status: HandoffStatus::Open,
            accepted_ms: None,
            accepted_by: None,
            cancelled_ms: None,
            cancelled_by: None,
            cancelled_reason: None,
        };
        let mut accepted = open.clone();
        accepted.id = "h2".into();
        accepted.status = HandoffStatus::Accepted;
        accepted.accepted_ms = Some(11);
        accepted.accepted_by = Some("bob".into());
        let mut other = open.clone();
        other.id = "h3".into();
        other.to = "carol".into();
        let mut long = open.clone();
        long.id = "h4".into();
        long.summary = "x".repeat(600);

        let items = project_handoffs(&[&open, &accepted, &other, &long], "bob", 20);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].item_id, "handoff:h1");
        assert_eq!(items[0].kind, InboxKind::Review);
        assert_eq!(items[0].source, "handoffs");
        assert!(items[0].why_now.contains("release"));
        assert_eq!(items[1].title.chars().count(), 500);
    }

    #[test]
    fn sync_dedupes_and_reports_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inbox.jsonl");
        let mut store = InboxStore::open(&path).unwrap();
        store.add(item("i1", InboxKind::Notify, None)).unwrap();
        let report = store
            .sync(
                vec![
                    item("i1", InboxKind::Notify, None),
                    item("i2", InboxKind::Review, None),
                ],
                false,
            )
            .unwrap();
        assert_eq!(report.scanned, 2);
        assert_eq!(report.added, 1);
        assert_eq!(report.skipped, 1);
        assert!(report.dry_run);
        assert!(store.get("i2").is_none());

        let applied = store
            .sync(
                vec![
                    item("i1", InboxKind::Notify, None),
                    item("i2", InboxKind::Review, None),
                ],
                true,
            )
            .unwrap();
        assert_eq!(applied.added, 1);
        assert_eq!(applied.skipped, 1);
        assert!(!applied.dry_run);
        assert!(store.get("i2").is_some());
    }

    #[test]
    fn inline_actions_parse_and_fail_closed() {
        assert_eq!(
            parse_action("resolve i1 approved").unwrap(),
            InboxAction::Resolve {
                item_id: "i1".into(),
                status: InboxStatus::Approved,
                note: None
            }
        );
        assert_eq!(
            parse_action("resolve i1 answered looks good").unwrap(),
            InboxAction::Resolve {
                item_id: "i1".into(),
                status: InboxStatus::Answered,
                note: Some("looks good".into())
            }
        );
        assert_eq!(
            parse_action("snooze i1 100").unwrap(),
            InboxAction::Snooze {
                item_id: "i1".into(),
                until_ms: 100
            }
        );
        assert!(parse_action("resolve i1 open").is_err());
        assert!(parse_action("snooze i1").is_err());
        assert!(parse_action("unknown").is_err());
    }

    #[test]
    fn kind_and_status_parse_round_trip() {
        assert_eq!("review".parse::<InboxKind>().unwrap(), InboxKind::Review);
        assert!("robot".parse::<InboxKind>().is_err());
        assert_eq!(
            "approved".parse::<InboxStatus>().unwrap(),
            InboxStatus::Approved
        );
        assert!("pending".parse::<InboxStatus>().is_err());
    }
}
