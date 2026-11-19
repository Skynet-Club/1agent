//! Duplicate-memory consolidation.
//!
//! Dry-run by default. A group is the same scope + content SHA-256 among
//! active records; the newest `valid_from_ms` (then the largest id) is kept and
//! the rest are soft-forgotten. Hard-forgotten and already soft-forgotten
//! records are ignored.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::memory::{ForgetMode, MemoryError, MemoryKind, MemoryRecord, MemoryStore};
use crate::memory_service::{MemoryService, MemoryServiceError};

const STALE_AFTER_MS: u128 = 90 * 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub scope_id: String,
    pub content_sha256: String,
    pub keep: String,
    pub forget: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationReport {
    pub groups: Vec<DuplicateGroup>,
    pub forgotten: usize,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContradictionGroup {
    pub scope_id: String,
    pub key: String,
    pub keep: String,
    pub supersede: Vec<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaleRecord {
    pub memory_id: String,
    pub reason: String,
    pub age_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationPlan {
    pub duplicates: Vec<DuplicateGroup>,
    pub near_duplicates: Vec<DuplicateGroup>,
    pub contradictions: Vec<ContradictionGroup>,
    pub stale: Vec<StaleRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationReportV2 {
    pub plan: ConsolidationPlan,
    pub forgotten: usize,
    pub superseded: usize,
    pub dry_run: bool,
}

pub fn plan(store: &MemoryStore) -> Vec<DuplicateGroup> {
    let mut groups = BTreeMap::<(String, String), Vec<&MemoryRecord>>::new();
    for record in store.records() {
        if record.hard_forgotten_ms.is_some()
            || record.soft_forgotten_ms.is_some()
            || record.content_sha256.is_empty()
        {
            continue;
        }
        groups
            .entry((record.scope.scope_id(), record.content_sha256.clone()))
            .or_default()
            .push(record);
    }
    let mut groups = groups
        .into_iter()
        .filter_map(|((scope_id, content_sha256), mut records)| {
            if records.len() < 2 {
                return None;
            }
            records.sort_by(|left, right| {
                left.valid_from_ms
                    .cmp(&right.valid_from_ms)
                    .then_with(|| left.memory_id.cmp(&right.memory_id))
            });
            let keep = records.last()?.memory_id.clone();
            let forget = records
                .iter()
                .filter(|record| record.memory_id != keep)
                .map(|record| record.memory_id.clone())
                .collect();
            Some(DuplicateGroup {
                scope_id,
                content_sha256,
                keep,
                forget,
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.scope_id.cmp(&right.scope_id));
    groups
}

pub fn apply(
    service: &mut MemoryService,
    groups: &[DuplicateGroup],
    now_ms: u128,
) -> Result<ConsolidationReport, MemoryServiceError> {
    let mut forgotten = 0;
    for group in groups {
        for memory_id in &group.forget {
            let Some(record) = service.store().get(memory_id).cloned() else {
                continue;
            };
            service.forget(memory_id, &record.scope, ForgetMode::Soft, now_ms)?;
            forgotten += 1;
        }
    }
    Ok(ConsolidationReport {
        groups: groups.to_vec(),
        forgotten,
        dry_run: false,
    })
}

pub fn plan_v2(store: &MemoryStore, now_ms: u128) -> ConsolidationPlan {
    let duplicates = plan(store);
    let duplicate_forget = duplicates
        .iter()
        .flat_map(|group| group.forget.iter().cloned())
        .collect::<BTreeSet<_>>();

    let mut near_groups: BTreeMap<(String, String, String), Vec<&MemoryRecord>> = BTreeMap::new();
    for record in store.records().filter(|record| active(record, now_ms)) {
        near_groups
            .entry((
                record.scope.scope_id(),
                format!("{:?}", record.kind),
                normalized_content(&record.content),
            ))
            .or_default()
            .push(record);
    }
    let mut near_duplicates = Vec::new();
    for ((scope_id, _, _), mut records) in near_groups {
        if records.len() < 2
            || records
                .iter()
                .all(|record| record.content_sha256 == records[0].content_sha256)
        {
            continue;
        }
        sort_records(&mut records);
        let keep = records.last().expect("group is non-empty");
        let forget = records
            .iter()
            .filter(|record| {
                record.memory_id != keep.memory_id && !duplicate_forget.contains(&record.memory_id)
            })
            .map(|record| record.memory_id.clone())
            .collect::<Vec<_>>();
        if !forget.is_empty() {
            near_duplicates.push(DuplicateGroup {
                scope_id,
                content_sha256: keep.content_sha256.clone(),
                keep: keep.memory_id.clone(),
                forget,
            });
        }
    }

    let mut preference_groups: BTreeMap<(String, String), Vec<&MemoryRecord>> = BTreeMap::new();
    for record in store.records().filter(|record| {
        active(record, now_ms)
            && record.kind == MemoryKind::Preference
            && !duplicate_forget.contains(&record.memory_id)
    }) {
        if let Some((key, _)) = preference_pair(record) {
            preference_groups
                .entry((record.scope.scope_id(), key))
                .or_default()
                .push(record);
        }
    }
    let mut contradictions = Vec::new();
    for ((scope_id, key), mut records) in preference_groups {
        let values = records
            .iter()
            .filter_map(|record| preference_pair(record).map(|(_, value)| value))
            .collect::<BTreeSet<_>>();
        if values.len() < 2 {
            continue;
        }
        sort_records(&mut records);
        let keep = records.last().expect("group is non-empty");
        let supersede = records
            .iter()
            .filter(|record| record.memory_id != keep.memory_id)
            .map(|record| record.memory_id.clone())
            .collect::<Vec<_>>();
        if !supersede.is_empty() {
            contradictions.push(ContradictionGroup {
                scope_id,
                key,
                keep: keep.memory_id.clone(),
                supersede,
                values: values.into_iter().collect(),
            });
        }
    }

    let mut stale = Vec::new();
    for record in store.records() {
        if record.soft_forgotten_ms.is_some()
            || record.hard_forgotten_ms.is_some()
            || record.superseded_by.is_some()
        {
            continue;
        }
        let age_ms = now_ms.saturating_sub(record.valid_from_ms);
        let reason = if record
            .valid_to_ms
            .is_some_and(|valid_to| valid_to <= now_ms)
        {
            Some("expired")
        } else if age_ms > STALE_AFTER_MS && record.utility.used == 0 {
            Some("unused for 90 days")
        } else {
            None
        };
        if let Some(reason) = reason {
            stale.push(StaleRecord {
                memory_id: record.memory_id.clone(),
                reason: reason.into(),
                age_ms,
            });
        }
    }
    stale.sort_by(|left, right| left.memory_id.cmp(&right.memory_id));

    ConsolidationPlan {
        duplicates,
        near_duplicates,
        contradictions,
        stale,
    }
}

pub fn apply_v2(
    service: &mut MemoryService,
    plan: &ConsolidationPlan,
    now_ms: u128,
) -> Result<ConsolidationReportV2, MemoryServiceError> {
    let mut forgotten = 0;
    let mut forgotten_ids = BTreeSet::new();
    for group in plan.duplicates.iter().chain(plan.near_duplicates.iter()) {
        for memory_id in &group.forget {
            let Some(record) = service.store().get(memory_id).cloned() else {
                continue;
            };
            if record.soft_forgotten_ms.is_some() {
                continue;
            }
            service.forget(memory_id, &record.scope, ForgetMode::Soft, now_ms)?;
            forgotten += 1;
            forgotten_ids.insert(memory_id.clone());
        }
    }
    let mut superseded = 0;
    for group in &plan.contradictions {
        for memory_id in &group.supersede {
            if forgotten_ids.contains(memory_id) {
                continue;
            }
            let Some(record) = service.store().get(memory_id).cloned() else {
                continue;
            };
            match service.supersede(memory_id, &group.keep, &record.scope, now_ms) {
                Ok(()) => superseded += 1,
                Err(MemoryServiceError::Memory(MemoryError::AlreadySuperseded(_))) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(ConsolidationReportV2 {
        plan: plan.clone(),
        forgotten,
        superseded,
        dry_run: false,
    })
}

fn active(record: &MemoryRecord, now_ms: u128) -> bool {
    record.soft_forgotten_ms.is_none()
        && record.hard_forgotten_ms.is_none()
        && record.superseded_by.is_none()
        && record.valid_from_ms <= now_ms
        && record.valid_to_ms.is_none_or(|valid_to| valid_to > now_ms)
}

fn sort_records(records: &mut Vec<&MemoryRecord>) {
    records.sort_by(|left, right| {
        left.valid_from_ms
            .cmp(&right.valid_from_ms)
            .then_with(|| left.confidence.cmp(&right.confidence))
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
}

fn normalized_content(content: &str) -> String {
    content
        .chars()
        .filter(|character| character.is_alphanumeric() || matches!(character, '=' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn preference_pair(record: &MemoryRecord) -> Option<(String, String)> {
    if record.kind != MemoryKind::Preference {
        return None;
    }
    let (key, value) = record.content.split_once('=')?;
    let key = key.trim().to_lowercase();
    let value = value.trim().to_lowercase();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    Some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{
        DataClass, MemoryKind, MemoryLayer, MemoryScope, MemorySource, MemorySourceKind,
        MemoryTaint,
    };

    fn record(id: &str, content: &str, valid_from_ms: u128) -> MemoryRecord {
        let mut record = MemoryRecord::new(
            id,
            MemoryLayer::Semantic,
            MemoryKind::Preference,
            MemoryScope::bot("bot-1"),
            content,
            MemorySource::new(MemorySourceKind::UserMessage, id),
        );
        record.valid_from_ms = valid_from_ms;
        record.taint = MemoryTaint::Trusted;
        record.data_class = DataClass::Personal;
        record
    }

    #[test]
    fn near_duplicates_keep_newest_and_apply_soft_forgets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-journal.jsonl");
        let payloads = dir.path().join("payloads");
        let key = dir.path().join("memory.key");
        let mut service = MemoryService::open(path, payloads, &key).unwrap();
        service
            .remember_at(record("mem-1", "editor=pnpm", 10), 10)
            .unwrap();
        service
            .remember_at(record("mem-2", "Editor = PNPM", 20), 20)
            .unwrap();
        let plan = plan_v2(service.store(), 30);
        assert_eq!(plan.near_duplicates.len(), 1);
        assert_eq!(plan.near_duplicates[0].keep, "mem-2");
        assert_eq!(plan.near_duplicates[0].forget, vec!["mem-1"]);

        let report = apply_v2(&mut service, &plan, 30).unwrap();
        assert_eq!(report.forgotten, 1);
        assert!(
            service
                .store()
                .get("mem-1")
                .unwrap()
                .soft_forgotten_ms
                .is_some()
        );
        assert!(
            service
                .store()
                .get("mem-2")
                .unwrap()
                .soft_forgotten_ms
                .is_none()
        );
    }

    #[test]
    fn contradicting_preferences_supersede_the_older_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-journal.jsonl");
        let payloads = dir.path().join("payloads");
        let key = dir.path().join("memory.key");
        let mut service = MemoryService::open(path, payloads, &key).unwrap();
        service
            .remember_at(record("mem-1", "editor=vim", 10), 10)
            .unwrap();
        service
            .remember_at(record("mem-2", "editor=vscode", 20), 20)
            .unwrap();
        let plan = plan_v2(service.store(), 30);
        assert_eq!(plan.contradictions.len(), 1);
        assert_eq!(plan.contradictions[0].keep, "mem-2");
        assert_eq!(plan.contradictions[0].supersede, vec!["mem-1"]);
        assert_eq!(plan.contradictions[0].values, vec!["vim", "vscode"]);

        let report = apply_v2(&mut service, &plan, 30).unwrap();
        assert_eq!(report.superseded, 1);
        assert_eq!(
            service
                .store()
                .get("mem-1")
                .unwrap()
                .superseded_by
                .as_deref(),
            Some("mem-2")
        );
        let model = service.build_user_model("local-user", &MemoryScope::bot("bot-1"));
        assert_eq!(model.preference("editor").unwrap().value, "vscode");
        let hits = service.recall("editor", &MemoryScope::bot("bot-1"), 30, 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.memory_id, "mem-2");
    }

    #[test]
    fn stale_records_are_reported_not_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-journal.jsonl");
        let payloads = dir.path().join("payloads");
        let key = dir.path().join("memory.key");
        let mut service = MemoryService::open(path, payloads, &key).unwrap();
        service
            .remember_at(record("mem-1", "editor=pnpm", 0), 0)
            .unwrap();
        let now = STALE_AFTER_MS + 1;
        let plan = plan_v2(service.store(), now);
        assert_eq!(plan.stale.len(), 1);
        assert_eq!(plan.stale[0].memory_id, "mem-1");
        assert_eq!(plan.stale[0].reason, "unused for 90 days");
        apply_v2(&mut service, &plan, now).unwrap();
        assert!(
            service
                .store()
                .get("mem-1")
                .unwrap()
                .soft_forgotten_ms
                .is_none()
        );
    }

    #[test]
    fn duplicates_keep_the_newest_and_apply_soft_forgets_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-journal.jsonl");
        let payloads = dir.path().join("payloads");
        let key = dir.path().join("memory.key");
        let mut service = MemoryService::open(path.clone(), payloads.clone(), &key).unwrap();
        service
            .remember_at(record("mem-1", "same content", 10), 10)
            .unwrap();
        service
            .remember_at(record("mem-2", "same content", 20), 20)
            .unwrap();
        let groups = plan(service.store());
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].keep, "mem-2");
        assert_eq!(groups[0].forget, vec!["mem-1"]);

        let report = apply(&mut service, &groups, 30).unwrap();
        assert_eq!(report.forgotten, 1);
        assert!(
            service
                .store()
                .get("mem-1")
                .unwrap()
                .soft_forgotten_ms
                .is_some()
        );
        assert!(
            service
                .store()
                .get("mem-2")
                .unwrap()
                .soft_forgotten_ms
                .is_none()
        );
    }
}
