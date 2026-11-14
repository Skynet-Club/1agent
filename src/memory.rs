//! Minimal memory domain: episodic/semantic records, scoped recall, TTL, and
//! soft/hard forget.
//!
//! This PR is intentionally in-memory. Journal payload encryption, persistent
//! indexes and crypto-shredding are the next memory slice.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

pub const MEMORY_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayer {
    Working,
    Episodic,
    Semantic,
    Procedural,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Decision,
    Project,
    Feedback,
    Relationship,
    SkillHint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySourceKind {
    UserMessage,
    ToolObservation,
    Answer,
    Decision,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTaint {
    Trusted,
    UserAsserted,
    Untrusted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Internal,
    Personal,
    Secret,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemoryScope {
    pub bot_id: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub ticket_id: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

impl MemoryScope {
    pub fn bot(bot_id: impl Into<String>) -> Self {
        Self {
            bot_id: bot_id.into(),
            workspace_id: None,
            ticket_id: None,
            tenant_id: None,
        }
    }

    pub fn scope_id(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| format!("bot:{}", self.bot_id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemorySource {
    pub kind: MemorySourceKind,
    pub event_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
}

impl MemorySource {
    pub fn new(kind: MemorySourceKind, event_id: impl Into<String>) -> Self {
        Self {
            kind,
            event_id: event_id.into(),
            run_id: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemoryUtility {
    pub used: u32,
    pub accepted: u32,
    pub rejected: u32,
    pub last_used_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemoryRecord {
    pub memory_id: String,
    pub layer: MemoryLayer,
    pub kind: MemoryKind,
    pub scope: MemoryScope,
    pub content: String,
    pub content_sha256: String,
    pub source: MemorySource,
    /// 0..=100
    pub confidence: u8,
    pub data_class: DataClass,
    pub taint: MemoryTaint,
    pub valid_from_ms: u128,
    #[serde(default)]
    pub valid_to_ms: Option<u128>,
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    pub version: u32,
    #[serde(default)]
    pub utility: MemoryUtility,
    #[serde(default)]
    pub soft_forgotten_ms: Option<u128>,
    #[serde(default)]
    pub hard_forgotten_ms: Option<u128>,
}

impl MemoryRecord {
    pub fn new(
        memory_id: impl Into<String>,
        layer: MemoryLayer,
        kind: MemoryKind,
        scope: MemoryScope,
        content: impl Into<String>,
        source: MemorySource,
    ) -> Self {
        let content = content.into();
        let content_sha256 = sha256_hex(content.as_bytes());
        Self {
            memory_id: memory_id.into(),
            layer,
            kind,
            scope,
            content,
            content_sha256,
            source,
            confidence: 50,
            data_class: DataClass::Internal,
            taint: MemoryTaint::UserAsserted,
            valid_from_ms: 0,
            valid_to_ms: None,
            supersedes: None,
            superseded_by: None,
            version: 1,
            utility: MemoryUtility::default(),
            soft_forgotten_ms: None,
            hard_forgotten_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MemoryEventKind {
    Remembered {
        memory_id: String,
        layer: MemoryLayer,
        kind: MemoryKind,
        scope: Box<MemoryScope>,
        source: MemorySource,
        content_sha256: String,
        confidence: u8,
        data_class: DataClass,
        taint: MemoryTaint,
        valid_from_ms: u128,
        valid_to_ms: Option<u128>,
        supersedes: Option<String>,
        version: u32,
    },
    Superseded {
        memory_id: String,
        superseded_by: String,
    },
    SoftForgotten {
        memory_id: String,
    },
    HardForgotten {
        memory_id: String,
    },
    Restored {
        memory_id: String,
    },
    Used {
        memory_id: String,
        accepted: bool,
    },
}

impl MemoryEventKind {
    pub fn memory_id(&self) -> &str {
        match self {
            Self::Remembered { memory_id, .. }
            | Self::Superseded { memory_id, .. }
            | Self::SoftForgotten { memory_id }
            | Self::HardForgotten { memory_id }
            | Self::Restored { memory_id }
            | Self::Used { memory_id, .. } => memory_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemoryEvent {
    pub schema_version: u32,
    pub seq: u64,
    pub event_id: String,
    pub timestamp_unix_ms: u128,
    pub kind: MemoryEventKind,
    pub previous_sha256: Option<String>,
    pub event_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForgetMode {
    Soft,
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryHit {
    pub record: MemoryRecord,
    pub score: i32,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryStore {
    records: BTreeMap<String, MemoryRecord>,
    events: Vec<MemoryEvent>,
    next_seq: u64,
    last_sha256: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MemoryError {
    #[error("memory id must not be empty")]
    EmptyMemoryId,
    #[error("memory source event must not be empty")]
    EmptySourceEvent,
    #[error("memory content must not be empty")]
    EmptyContent,
    #[error("working memory is not persisted")]
    WorkingMemoryNotPersisted,
    #[error("secret memory requires encrypted payload support")]
    SecretRequiresEncryption,
    #[error("confidence must be 0..=100")]
    InvalidConfidence,
    #[error("valid_to must be greater than valid_from")]
    InvalidValidityWindow,
    #[error("memory {0} already exists")]
    AlreadyExists(String),
    #[error("memory {0} not found")]
    UnknownMemory(String),
    #[error("superseded memory {0} not found")]
    UnknownSuperseded(String),
    #[error("memory {0} is hard-forgotten")]
    HardForgotten(String),
    #[error("memory event sequence mismatch: expected {expected}, got {actual}")]
    EventSequenceMismatch { expected: u64, actual: u64 },
    #[error("memory event hash mismatch at sequence {0}")]
    EventHashMismatch(u64),
    #[error("memory previous hash mismatch at sequence {0}")]
    PreviousHashMismatch(u64),
    #[error("memory event schema version {0} is unsupported")]
    UnsupportedEventSchema(u32),
    #[error("memory {0} content hash mismatch")]
    ContentHashMismatch(String),
    #[error("memory {0} cannot supersede a record in another scope")]
    SupersedeScopeMismatch(String),
}

impl MemoryStore {
    pub fn remember(&mut self, record: MemoryRecord) -> Result<(), MemoryError> {
        let valid_from_ms = record.valid_from_ms;
        self.remember_at(record, valid_from_ms)
    }

    pub fn remember_at(
        &mut self,
        mut record: MemoryRecord,
        now_ms: u128,
    ) -> Result<(), MemoryError> {
        if record.memory_id.trim().is_empty() {
            return Err(MemoryError::EmptyMemoryId);
        }
        if record.source.event_id.trim().is_empty() {
            return Err(MemoryError::EmptySourceEvent);
        }
        if record.content.trim().is_empty() {
            return Err(MemoryError::EmptyContent);
        }
        if record.layer == MemoryLayer::Working {
            return Err(MemoryError::WorkingMemoryNotPersisted);
        }
        if record.data_class == DataClass::Secret {
            return Err(MemoryError::SecretRequiresEncryption);
        }
        if record.confidence > 100 {
            return Err(MemoryError::InvalidConfidence);
        }
        if record
            .valid_to_ms
            .is_some_and(|valid_to| valid_to <= record.valid_from_ms)
        {
            return Err(MemoryError::InvalidValidityWindow);
        }
        if self.records.contains_key(&record.memory_id) {
            return Err(MemoryError::AlreadyExists(record.memory_id));
        }
        if let Some(old_id) = record.supersedes.clone() {
            let old = self
                .records
                .get_mut(&old_id)
                .ok_or_else(|| MemoryError::UnknownSuperseded(old_id.clone()))?;
            if old.hard_forgotten_ms.is_some() {
                return Err(MemoryError::HardForgotten(old_id));
            }
            if old.scope != record.scope {
                return Err(MemoryError::SupersedeScopeMismatch(old_id));
            }
            old.superseded_by = Some(record.memory_id.clone());
            self.append_event(
                MemoryEventKind::Superseded {
                    memory_id: old_id,
                    superseded_by: record.memory_id.clone(),
                },
                now_ms,
            )?;
        }
        let content_sha256 = sha256_hex(record.content.as_bytes());
        record.content_sha256 = content_sha256.clone();
        self.records
            .insert(record.memory_id.clone(), record.clone());
        self.append_event(
            MemoryEventKind::Remembered {
                memory_id: record.memory_id,
                layer: record.layer,
                kind: record.kind,
                scope: Box::new(record.scope),
                source: record.source,
                content_sha256,
                confidence: record.confidence,
                data_class: record.data_class,
                taint: record.taint,
                valid_from_ms: record.valid_from_ms,
                valid_to_ms: record.valid_to_ms,
                supersedes: record.supersedes,
                version: record.version,
            },
            now_ms,
        )?;
        Ok(())
    }

    pub fn get(&self, memory_id: &str) -> Option<&MemoryRecord> {
        self.records.get(memory_id)
    }

    pub fn records(&self) -> impl Iterator<Item = &MemoryRecord> {
        self.records.values()
    }

    pub fn record_ids(&self) -> Vec<String> {
        self.records.keys().cloned().collect()
    }

    pub fn hydrate_content(&mut self, memory_id: &str, content: String) -> Result<(), MemoryError> {
        let record = self
            .records
            .get_mut(memory_id)
            .ok_or_else(|| MemoryError::UnknownMemory(memory_id.to_string()))?;
        if record.hard_forgotten_ms.is_some() {
            return Err(MemoryError::HardForgotten(memory_id.to_string()));
        }
        if sha256_hex(content.as_bytes()) != record.content_sha256 {
            return Err(MemoryError::ContentHashMismatch(memory_id.to_string()));
        }
        record.content = content;
        Ok(())
    }

    pub fn from_events(events: &[MemoryEvent]) -> Result<Self, MemoryError> {
        Self::verify_events(events)?;
        let mut store = Self {
            records: BTreeMap::new(),
            events: events.to_vec(),
            next_seq: events.last().map(|event| event.seq).unwrap_or(0),
            last_sha256: events.last().map(|event| event.event_sha256.clone()),
        };
        for event in events {
            match &event.kind {
                MemoryEventKind::Remembered {
                    memory_id,
                    layer,
                    kind,
                    scope,
                    source,
                    content_sha256,
                    confidence,
                    data_class,
                    taint,
                    valid_from_ms,
                    valid_to_ms,
                    supersedes,
                    version,
                } => {
                    if store.records.contains_key(memory_id) {
                        return Err(MemoryError::AlreadyExists(memory_id.clone()));
                    }
                    store.records.insert(
                        memory_id.clone(),
                        MemoryRecord {
                            memory_id: memory_id.clone(),
                            layer: *layer,
                            kind: *kind,
                            scope: (**scope).clone(),
                            content: String::new(),
                            content_sha256: content_sha256.clone(),
                            source: source.clone(),
                            confidence: *confidence,
                            data_class: *data_class,
                            taint: *taint,
                            valid_from_ms: *valid_from_ms,
                            valid_to_ms: *valid_to_ms,
                            supersedes: supersedes.clone(),
                            superseded_by: None,
                            version: *version,
                            utility: MemoryUtility::default(),
                            soft_forgotten_ms: None,
                            hard_forgotten_ms: None,
                        },
                    );
                }
                MemoryEventKind::Superseded {
                    memory_id,
                    superseded_by,
                } => {
                    let record = store
                        .records
                        .get_mut(memory_id)
                        .ok_or_else(|| MemoryError::UnknownMemory(memory_id.clone()))?;
                    record.superseded_by = Some(superseded_by.clone());
                }
                MemoryEventKind::SoftForgotten { memory_id } => {
                    let record = store
                        .records
                        .get_mut(memory_id)
                        .ok_or_else(|| MemoryError::UnknownMemory(memory_id.clone()))?;
                    record.soft_forgotten_ms = Some(event.timestamp_unix_ms);
                }
                MemoryEventKind::HardForgotten { memory_id } => {
                    let record = store
                        .records
                        .get_mut(memory_id)
                        .ok_or_else(|| MemoryError::UnknownMemory(memory_id.clone()))?;
                    record.soft_forgotten_ms = Some(event.timestamp_unix_ms);
                    record.hard_forgotten_ms = Some(event.timestamp_unix_ms);
                    record.content.clear();
                }
                MemoryEventKind::Restored { memory_id } => {
                    let record = store
                        .records
                        .get_mut(memory_id)
                        .ok_or_else(|| MemoryError::UnknownMemory(memory_id.clone()))?;
                    record.soft_forgotten_ms = None;
                }
                MemoryEventKind::Used {
                    memory_id,
                    accepted,
                } => {
                    let record = store
                        .records
                        .get_mut(memory_id)
                        .ok_or_else(|| MemoryError::UnknownMemory(memory_id.clone()))?;
                    record.utility.used = record.utility.used.saturating_add(1);
                    if *accepted {
                        record.utility.accepted = record.utility.accepted.saturating_add(1);
                    } else {
                        record.utility.rejected = record.utility.rejected.saturating_add(1);
                    }
                    record.utility.last_used_ms = Some(event.timestamp_unix_ms);
                }
            }
        }
        Ok(store)
    }

    pub fn recall(
        &self,
        query: &str,
        scope: &MemoryScope,
        now_ms: u128,
        limit: usize,
    ) -> Vec<MemoryHit> {
        if limit == 0 {
            return Vec::new();
        }
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|term| term.to_lowercase())
            .filter(|term| !term.is_empty())
            .collect();
        let mut hits = Vec::new();
        for record in self.records.values() {
            if !is_active(record, now_ms) || !scope_matches(scope, &record.scope) {
                continue;
            }
            let lower = record.content.to_lowercase();
            let matches = terms.iter().filter(|term| lower.contains(*term)).count() as i32;
            if !terms.is_empty() && matches == 0 {
                continue;
            }
            let taint_penalty = match record.taint {
                MemoryTaint::Trusted => 0,
                MemoryTaint::UserAsserted => 5,
                MemoryTaint::Untrusted => 25,
            };
            let utility_bonus = i32::try_from(record.utility.accepted)
                .unwrap_or(i32::MAX / 2)
                .saturating_mul(2)
                .saturating_sub(i32::try_from(record.utility.rejected).unwrap_or(0));
            let score =
                matches * 100 + i32::from(record.confidence) - taint_penalty + utility_bonus;
            hits.push(MemoryHit {
                record: record.clone(),
                score,
            });
        }
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.record.valid_from_ms.cmp(&left.record.valid_from_ms))
                .then_with(|| left.record.memory_id.cmp(&right.record.memory_id))
        });
        hits.truncate(limit);
        hits
    }

    pub fn forget(
        &mut self,
        memory_id: &str,
        mode: ForgetMode,
        now_ms: u128,
    ) -> Result<(), MemoryError> {
        {
            let record = self
                .records
                .get_mut(memory_id)
                .ok_or_else(|| MemoryError::UnknownMemory(memory_id.to_string()))?;
            if record.hard_forgotten_ms.is_some() {
                return match mode {
                    ForgetMode::Hard => Ok(()),
                    ForgetMode::Soft => Err(MemoryError::HardForgotten(memory_id.to_string())),
                };
            }
            if record.soft_forgotten_ms.is_some() && mode == ForgetMode::Soft {
                return Ok(());
            }
            match mode {
                ForgetMode::Soft => {
                    record.soft_forgotten_ms = Some(now_ms);
                }
                ForgetMode::Hard => {
                    record.soft_forgotten_ms = Some(now_ms);
                    record.hard_forgotten_ms = Some(now_ms);
                    record.content.clear();
                }
            }
        }
        self.append_event(
            match mode {
                ForgetMode::Soft => MemoryEventKind::SoftForgotten {
                    memory_id: memory_id.to_string(),
                },
                ForgetMode::Hard => MemoryEventKind::HardForgotten {
                    memory_id: memory_id.to_string(),
                },
            },
            now_ms,
        )
    }

    pub fn restore_soft(&mut self, memory_id: &str) -> Result<(), MemoryError> {
        self.restore_soft_at(memory_id, 0)
    }

    pub fn restore_soft_at(&mut self, memory_id: &str, now_ms: u128) -> Result<(), MemoryError> {
        {
            let record = self
                .records
                .get_mut(memory_id)
                .ok_or_else(|| MemoryError::UnknownMemory(memory_id.to_string()))?;
            if record.hard_forgotten_ms.is_some() {
                return Err(MemoryError::HardForgotten(memory_id.to_string()));
            }
            record.soft_forgotten_ms = None;
        }
        self.append_event(
            MemoryEventKind::Restored {
                memory_id: memory_id.to_string(),
            },
            now_ms,
        )
    }

    pub fn record_use(
        &mut self,
        memory_id: &str,
        accepted: bool,
        now_ms: u128,
    ) -> Result<(), MemoryError> {
        {
            let record = self
                .records
                .get_mut(memory_id)
                .ok_or_else(|| MemoryError::UnknownMemory(memory_id.to_string()))?;
            if record.hard_forgotten_ms.is_some() {
                return Err(MemoryError::HardForgotten(memory_id.to_string()));
            }
            record.utility.used = record.utility.used.saturating_add(1);
            if accepted {
                record.utility.accepted = record.utility.accepted.saturating_add(1);
            } else {
                record.utility.rejected = record.utility.rejected.saturating_add(1);
            }
            record.utility.last_used_ms = Some(now_ms);
        }
        self.append_event(
            MemoryEventKind::Used {
                memory_id: memory_id.to_string(),
                accepted,
            },
            now_ms,
        )
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn events(&self) -> &[MemoryEvent] {
        &self.events
    }

    pub fn verify_events(events: &[MemoryEvent]) -> Result<(), MemoryError> {
        let mut previous: Option<String> = None;
        for (expected, event) in (1u64..).zip(events) {
            if event.schema_version != MEMORY_EVENT_SCHEMA_VERSION {
                return Err(MemoryError::UnsupportedEventSchema(event.schema_version));
            }
            if event.seq != expected {
                return Err(MemoryError::EventSequenceMismatch {
                    expected,
                    actual: event.seq,
                });
            }
            if event.previous_sha256.as_deref() != previous.as_deref() {
                return Err(MemoryError::PreviousHashMismatch(event.seq));
            }
            if event.event_sha256 != hash_memory_event(event)? {
                return Err(MemoryError::EventHashMismatch(event.seq));
            }
            previous = Some(event.event_sha256.clone());
        }
        Ok(())
    }

    fn append_event(&mut self, kind: MemoryEventKind, now_ms: u128) -> Result<(), MemoryError> {
        let seq = self.next_seq + 1;
        let mut event = MemoryEvent {
            schema_version: MEMORY_EVENT_SCHEMA_VERSION,
            seq,
            event_id: Uuid::new_v4().to_string(),
            timestamp_unix_ms: now_ms,
            kind,
            previous_sha256: self.last_sha256.clone(),
            event_sha256: String::new(),
        };
        event.event_sha256 = hash_memory_event(&event)?;
        self.next_seq = seq;
        self.last_sha256 = Some(event.event_sha256.clone());
        self.events.push(event);
        Ok(())
    }
}

#[derive(Serialize)]
struct MemoryEventHashPayload<'a> {
    schema_version: u32,
    seq: u64,
    event_id: &'a str,
    timestamp_unix_ms: u128,
    kind: &'a MemoryEventKind,
    previous_sha256: &'a Option<String>,
}

fn hash_memory_event(event: &MemoryEvent) -> Result<String, MemoryError> {
    let payload = MemoryEventHashPayload {
        schema_version: event.schema_version,
        seq: event.seq,
        event_id: &event.event_id,
        timestamp_unix_ms: event.timestamp_unix_ms,
        kind: &event.kind,
        previous_sha256: &event.previous_sha256,
    };
    let bytes =
        serde_json::to_vec(&payload).map_err(|_| MemoryError::EventHashMismatch(event.seq))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn is_active(record: &MemoryRecord, now_ms: u128) -> bool {
    if record.soft_forgotten_ms.is_some()
        || record.hard_forgotten_ms.is_some()
        || record.superseded_by.is_some()
        || record.valid_from_ms > now_ms
    {
        return false;
    }
    record.valid_to_ms.is_none_or(|valid_to| valid_to > now_ms)
}

fn scope_matches(query: &MemoryScope, record: &MemoryScope) -> bool {
    query.bot_id == record.bot_id
        && query.workspace_id == record.workspace_id
        && query.ticket_id == record.ticket_id
        && query.tenant_id == record.tenant_id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, content: &str) -> MemoryRecord {
        MemoryRecord::new(
            id,
            MemoryLayer::Semantic,
            MemoryKind::Preference,
            MemoryScope::bot("bot-1"),
            content,
            MemorySource::new(MemorySourceKind::UserMessage, format!("event-{id}")),
        )
    }

    #[test]
    fn recall_is_scoped_and_ranks_keyword_matches() {
        let mut store = MemoryStore::default();
        store
            .remember(record("mem-1", "prefers pnpm over npm"))
            .unwrap();
        let mut other = record("mem-2", "prefers pnpm elsewhere");
        other.scope.bot_id = "bot-2".into();
        store.remember(other).unwrap();
        let hits = store.recall("pnpm", &MemoryScope::bot("bot-1"), 10, 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.memory_id, "mem-1");
        assert!(hits[0].score > 0);

        let mut tenant = record("mem-tenant", "prefers pnpm in tenant");
        tenant.scope.tenant_id = Some("tenant-a".into());
        store.remember(tenant).unwrap();
        let hits = store.recall("pnpm", &MemoryScope::bot("bot-1"), 10, 5);
        assert_eq!(hits.len(), 1, "None tenant must not match Some(tenant)");
    }

    #[test]
    fn ttl_and_not_yet_valid_records_are_not_recalled() {
        let mut store = MemoryStore::default();
        let mut expired = record("mem-expired", "old project name");
        expired.valid_to_ms = Some(10);
        store.remember(expired).unwrap();
        let mut future = record("mem-future", "future project name");
        future.valid_from_ms = 100;
        store.remember(future).unwrap();
        assert!(
            store
                .recall("project", &MemoryScope::bot("bot-1"), 50, 10)
                .is_empty()
        );
    }

    #[test]
    fn soft_forget_can_be_restored_but_hard_forget_cannot() {
        let mut store = MemoryStore::default();
        store.remember(record("mem-1", "remember me")).unwrap();
        store.forget("mem-1", ForgetMode::Soft, 10).unwrap();
        assert!(
            store
                .recall("remember", &MemoryScope::bot("bot-1"), 11, 10)
                .is_empty()
        );
        store.restore_soft("mem-1").unwrap();
        assert_eq!(
            store
                .recall("remember", &MemoryScope::bot("bot-1"), 12, 10)
                .len(),
            1
        );
        store.forget("mem-1", ForgetMode::Hard, 13).unwrap();
        assert!(store.get("mem-1").unwrap().content.is_empty());
        assert_eq!(
            store.restore_soft("mem-1").unwrap_err(),
            MemoryError::HardForgotten("mem-1".into())
        );
    }

    #[test]
    fn superseded_records_are_hidden() {
        let mut store = MemoryStore::default();
        store.remember(record("mem-old", "uses npm")).unwrap();
        let mut new = record("mem-new", "uses pnpm");
        new.supersedes = Some("mem-old".into());
        store.remember(new).unwrap();
        let hits = store.recall("uses", &MemoryScope::bot("bot-1"), 10, 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.memory_id, "mem-new");
        assert_eq!(
            store.get("mem-old").unwrap().superseded_by.as_deref(),
            Some("mem-new")
        );
    }

    #[test]
    fn untrusted_memory_is_penalized_and_secret_is_rejected() {
        let mut store = MemoryStore::default();
        let mut trusted = record("mem-trusted", "same fact");
        trusted.taint = MemoryTaint::Trusted;
        let mut untrusted = record("mem-untrusted", "same fact");
        untrusted.taint = MemoryTaint::Untrusted;
        store.remember(trusted).unwrap();
        store.remember(untrusted).unwrap();
        let hits = store.recall("fact", &MemoryScope::bot("bot-1"), 10, 10);
        assert_eq!(hits[0].record.memory_id, "mem-trusted");

        let mut secret = record("mem-secret", "token");
        secret.data_class = DataClass::Secret;
        assert_eq!(
            store.remember(secret).unwrap_err(),
            MemoryError::SecretRequiresEncryption
        );
    }

    #[test]
    fn event_log_is_metadata_only_and_verifiable() {
        let mut store = MemoryStore::default();
        store
            .remember_at(record("mem-1", "private body text"), 10)
            .unwrap();
        store.forget("mem-1", ForgetMode::Hard, 11).unwrap();
        assert_eq!(store.events().len(), 2);
        let encoded = serde_json::to_string(store.events()).unwrap();
        assert!(!encoded.contains("private body text"));
        MemoryStore::verify_events(store.events()).unwrap();

        let mut tampered = store.events().to_vec();
        tampered[0].kind = MemoryEventKind::HardForgotten {
            memory_id: "mem-1".into(),
        };
        assert_eq!(
            MemoryStore::verify_events(&tampered).unwrap_err(),
            MemoryError::EventHashMismatch(1)
        );
    }

    #[test]
    fn usage_updates_utility_and_invalid_records_fail_closed() {
        let mut store = MemoryStore::default();
        store.remember(record("mem-1", "useful fact")).unwrap();
        store.record_use("mem-1", true, 20).unwrap();
        store.record_use("mem-1", false, 21).unwrap();
        let record = store.get("mem-1").cloned().unwrap();
        assert_eq!(record.utility.used, 2);
        assert_eq!(record.utility.accepted, 1);
        assert_eq!(record.utility.rejected, 1);
        assert_eq!(record.utility.last_used_ms, Some(21));

        let mut working = record.clone();
        working.memory_id = "mem-working".into();
        working.layer = MemoryLayer::Working;
        assert_eq!(
            store.remember(working).unwrap_err(),
            MemoryError::WorkingMemoryNotPersisted
        );
        let mut no_source = record.clone();
        no_source.memory_id = "mem-no-source".into();
        no_source.source.event_id.clear();
        assert_eq!(
            store.remember(no_source).unwrap_err(),
            MemoryError::EmptySourceEvent
        );
        let mut bad_confidence = record.clone();
        bad_confidence.memory_id = "mem-bad-confidence".into();
        bad_confidence.confidence = 101;
        assert_eq!(
            store.remember(bad_confidence).unwrap_err(),
            MemoryError::InvalidConfidence
        );
    }
}
