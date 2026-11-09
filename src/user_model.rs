//! Derived user model over scoped memory records.
//!
//! This is a view, not a second source of truth: every preference/boundary
//! keeps the memory id it came from, and the model can be rebuilt from journal
//! replay.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::memory::{MemoryKind, MemoryRecord, MemoryScope};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Preference {
    pub key: String,
    pub value: String,
    pub confidence: u8,
    pub source_memory_id: String,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    pub rule: String,
    pub source_memory_id: String,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UserModel {
    pub user_id: String,
    pub scope: MemoryScope,
    pub preferences: BTreeMap<String, Preference>,
    pub boundaries: Vec<Boundary>,
    pub updated_ms: u128,
}

impl UserModel {
    pub fn new(user_id: impl Into<String>, scope: MemoryScope) -> Self {
        Self {
            user_id: user_id.into(),
            scope,
            preferences: BTreeMap::new(),
            boundaries: Vec::new(),
            updated_ms: 0,
        }
    }

    pub fn apply(&mut self, record: &MemoryRecord) {
        if record.scope != self.scope
            || record.soft_forgotten_ms.is_some()
            || record.hard_forgotten_ms.is_some()
            || record.superseded_by.is_some()
        {
            return;
        }
        match record.kind {
            MemoryKind::Preference => {
                let (key, value) = record
                    .content
                    .split_once('=')
                    .map(|(key, value)| (key.trim(), value.trim()))
                    .filter(|(key, value)| !key.is_empty() && !value.is_empty())
                    .unwrap_or((record.memory_id.as_str(), record.content.trim()));
                let preference = Preference {
                    key: key.to_string(),
                    value: value.to_string(),
                    confidence: record.confidence,
                    source_memory_id: record.memory_id.clone(),
                    updated_ms: record.valid_from_ms,
                };
                let replace = self
                    .preferences
                    .get(key)
                    .is_none_or(|existing| existing.updated_ms <= preference.updated_ms);
                if replace {
                    self.preferences.insert(key.to_string(), preference);
                    self.updated_ms = self.updated_ms.max(record.valid_from_ms);
                }
            }
            MemoryKind::Feedback => {
                if let Some(rule) = record.content.strip_prefix("boundary:") {
                    let rule = rule.trim();
                    if !rule.is_empty() {
                        self.boundaries.push(Boundary {
                            rule: rule.to_string(),
                            source_memory_id: record.memory_id.clone(),
                            updated_ms: record.valid_from_ms,
                        });
                        self.updated_ms = self.updated_ms.max(record.valid_from_ms);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn preference(&self, key: &str) -> Option<&Preference> {
        self.preferences.get(key)
    }

    pub fn has_boundary(&self, rule: &str) -> bool {
        self.boundaries.iter().any(|boundary| boundary.rule == rule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemorySource, MemorySourceKind};

    fn record(id: &str, kind: MemoryKind, content: &str, at: u128) -> MemoryRecord {
        let mut record = MemoryRecord::new(
            id,
            crate::memory::MemoryLayer::Semantic,
            kind,
            MemoryScope::bot("bot-1"),
            content,
            MemorySource::new(MemorySourceKind::UserMessage, format!("event-{id}")),
        );
        record.valid_from_ms = at;
        record
    }

    #[test]
    fn builds_preferences_and_boundaries_with_sources() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record("m1", MemoryKind::Preference, "editor=pnpm", 10));
        model.apply(&record(
            "m2",
            MemoryKind::Feedback,
            "boundary: never touch production",
            11,
        ));
        assert_eq!(model.preference("editor").unwrap().value, "pnpm");
        assert_eq!(model.preference("editor").unwrap().source_memory_id, "m1");
        assert!(model.has_boundary("never touch production"));
        assert_eq!(model.boundaries[0].source_memory_id, "m2");
    }

    #[test]
    fn newer_preference_wins_and_forgotten_records_are_ignored() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record("m1", MemoryKind::Preference, "editor=pnpm", 10));
        model.apply(&record("m2", MemoryKind::Preference, "editor=npm", 20));
        assert_eq!(model.preference("editor").unwrap().value, "npm");
        let mut forgotten = record("m3", MemoryKind::Preference, "editor=yarn", 30);
        forgotten.soft_forgotten_ms = Some(31);
        model.apply(&forgotten);
        assert_eq!(model.preference("editor").unwrap().value, "npm");
    }
}
