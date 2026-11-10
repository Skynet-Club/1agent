//! Duplicate-memory consolidation.
//!
//! Dry-run by default. A group is the same scope + content SHA-256 among
//! active records; the newest `valid_from_ms` (then the largest id) is kept and
//! the rest are soft-forgotten. Hard-forgotten and already soft-forgotten
//! records are ignored.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::memory::{ForgetMode, MemoryRecord, MemoryStore};
use crate::memory_service::{MemoryService, MemoryServiceError};

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
