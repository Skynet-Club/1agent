//! Minimal skill registry: SKILL.md index, immutable versions, scan, activate,
//! disable and rollback. Execution and evaluation are later slices.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const SKILL_EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    Draft,
    Active,
    Disabled,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub platforms: Vec<String>,
    pub scope: String,
    pub requires_tools: Vec<String>,
    pub risk: SkillRisk,
    pub body: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillRecord {
    pub manifest: SkillManifest,
    pub status: SkillStatus,
    pub created_ms: u128,
    #[serde(default)]
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub version: String,
    pub status: SkillStatus,
    pub risk: SkillRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum SkillEventKind {
    Registered { manifest: SkillManifest },
    Activated { name: String, version: String },
    Disabled { name: String, version: String },
}

impl SkillEventKind {
    pub fn name(&self) -> &str {
        match self {
            Self::Registered { manifest } => &manifest.name,
            Self::Activated { name, .. } | Self::Disabled { name, .. } => name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            Self::Registered { manifest } => &manifest.version,
            Self::Activated { version, .. } | Self::Disabled { version, .. } => version,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillEvent {
    pub schema_version: u32,
    pub seq: u64,
    pub event_id: String,
    pub timestamp_unix_ms: u128,
    pub kind: SkillEventKind,
    pub previous_sha256: Option<String>,
    pub event_sha256: String,
}

#[derive(Debug, Clone, Default)]
pub struct SkillStore {
    records: BTreeMap<(String, String), SkillRecord>,
    active: BTreeMap<String, String>,
    events: Vec<SkillEvent>,
    next_seq: u64,
    last_sha256: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillError {
    #[error("skill name is unsafe")]
    UnsafeName,
    #[error("skill description must be 1..=60 chars")]
    InvalidDescription,
    #[error("skill version is unsafe")]
    UnsafeVersion,
    #[error("skill body must not be empty")]
    EmptyBody,
    #[error("skill {0} version {1} already exists")]
    AlreadyExists(String, String),
    #[error("skill {0} version {1} not found")]
    UnknownVersion(String, String),
    #[error("skill {0} has no active version")]
    NoActiveVersion(String),
    #[error("skill scan rejected: {0}")]
    ScanRejected(String),
    #[error("skill frontmatter is invalid: {0}")]
    InvalidFrontmatter(String),
    #[error("unsupported skill event schema {0}")]
    UnsupportedEventSchema(u32),
    #[error("skill event sequence mismatch: expected {expected}, got {actual}")]
    EventSequenceMismatch { expected: u64, actual: u64 },
    #[error("skill event {0} previous hash mismatch")]
    PreviousHashMismatch(u64),
    #[error("skill event {0} hash mismatch")]
    EventHashMismatch(u64),
}

impl SkillManifest {
    pub fn from_markdown(source: &str) -> Result<Self, SkillError> {
        let mut lines = source.lines();
        if lines.next() != Some("---") {
            return Err(SkillError::InvalidFrontmatter(
                "missing opening frontmatter".into(),
            ));
        }
        let mut frontmatter = BTreeMap::<String, String>::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line.trim() == "---" {
                closed = true;
                break;
            }
            let Some((key, value)) = line.split_once(':') else {
                return Err(SkillError::InvalidFrontmatter(format!(
                    "expected key: value, got {line}"
                )));
            };
            frontmatter.insert(key.trim().to_string(), value.trim().to_string());
        }
        if !closed {
            return Err(SkillError::InvalidFrontmatter(
                "missing closing frontmatter".into(),
            ));
        }
        let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        if body.is_empty() {
            return Err(SkillError::EmptyBody);
        }
        let content_sha256 = sha256_hex(body.as_bytes());
        let name = required(&frontmatter, "name")?;
        let description = required(&frontmatter, "description")?;
        let version = required(&frontmatter, "version")?;
        let risk = match frontmatter.get("risk").map(String::as_str) {
            Some("low") | None => SkillRisk::Low,
            Some("medium") => SkillRisk::Medium,
            Some("high") => SkillRisk::High,
            Some(other) => {
                return Err(SkillError::InvalidFrontmatter(format!(
                    "unknown risk {other}"
                )));
            }
        };
        let manifest = Self {
            name,
            description,
            version,
            platforms: parse_list(frontmatter.get("platforms")),
            scope: frontmatter
                .get("scope")
                .cloned()
                .unwrap_or_else(|| "global".into()),
            requires_tools: parse_list(frontmatter.get("requires_tools")),
            risk,
            body,
            content_sha256,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), SkillError> {
        if !safe_token(&self.name) {
            return Err(SkillError::UnsafeName);
        }
        let description_len = self.description.chars().count();
        if description_len == 0 || description_len > 60 {
            return Err(SkillError::InvalidDescription);
        }
        if !safe_token(&self.version)
            || !self
                .version
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        {
            return Err(SkillError::UnsafeVersion);
        }
        if self.body.trim().is_empty() {
            return Err(SkillError::EmptyBody);
        }
        Ok(())
    }
}

impl SkillStore {
    pub fn register(
        &mut self,
        mut manifest: SkillManifest,
        now_ms: u128,
    ) -> Result<(), SkillError> {
        manifest.validate()?;
        manifest.content_sha256 = sha256_hex(manifest.body.as_bytes());
        scan_manifest(&manifest)?;
        let key = (manifest.name.clone(), manifest.version.clone());
        if self.records.contains_key(&key) {
            return Err(SkillError::AlreadyExists(
                manifest.name.clone(),
                manifest.version.clone(),
            ));
        }
        self.append_event(SkillEventKind::Registered { manifest }, now_ms)
    }

    pub fn activate(&mut self, name: &str, version: &str, now_ms: u128) -> Result<(), SkillError> {
        let key = (name.to_string(), version.to_string());
        if !self.records.contains_key(&key) {
            return Err(SkillError::UnknownVersion(
                name.to_string(),
                version.to_string(),
            ));
        }
        self.append_event(
            SkillEventKind::Activated {
                name: name.to_string(),
                version: version.to_string(),
            },
            now_ms,
        )
    }

    pub fn disable(&mut self, name: &str) -> Result<(), SkillError> {
        self.disable_at(name, 0)
    }

    pub fn disable_at(&mut self, name: &str, now_ms: u128) -> Result<(), SkillError> {
        let Some(version) = self.active.get(name).cloned() else {
            return Err(SkillError::NoActiveVersion(name.to_string()));
        };
        self.append_event(
            SkillEventKind::Disabled {
                name: name.to_string(),
                version,
            },
            now_ms,
        )
    }

    pub fn rollback(&mut self, name: &str, version: &str, now_ms: u128) -> Result<(), SkillError> {
        self.activate(name, version, now_ms)
    }

    pub fn active(&self, name: &str) -> Option<&SkillManifest> {
        let version = self.active.get(name)?;
        self.records
            .get(&(name.to_string(), version.clone()))
            .map(|record| &record.manifest)
    }

    pub fn view(&self, name: &str, version: &str) -> Option<&SkillManifest> {
        self.records
            .get(&(name.to_string(), version.to_string()))
            .map(|record| &record.manifest)
    }

    pub fn list(&self) -> Vec<SkillSummary> {
        let mut summaries = Vec::new();
        for record in self.records.values() {
            summaries.push(SkillSummary {
                name: record.manifest.name.clone(),
                description: record.manifest.description.clone(),
                version: record.manifest.version.clone(),
                status: record.status,
                risk: record.manifest.risk,
            });
        }
        summaries.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.version.cmp(&right.version))
        });
        summaries
    }

    pub fn events(&self) -> &[SkillEvent] {
        &self.events
    }

    pub fn replay(events: &[SkillEvent]) -> Result<Self, SkillError> {
        Self::verify_events(events)?;
        let mut store = Self::default();
        for event in events {
            store.apply_event(event)?;
        }
        store.next_seq = events.len() as u64;
        store.last_sha256 = events.last().map(|event| event.event_sha256.clone());
        store.events = events.to_vec();
        Ok(store)
    }

    pub fn verify_events(events: &[SkillEvent]) -> Result<(), SkillError> {
        let mut previous: Option<String> = None;
        for (expected, event) in (1u64..).zip(events) {
            if event.schema_version != SKILL_EVENT_SCHEMA_VERSION {
                return Err(SkillError::UnsupportedEventSchema(event.schema_version));
            }
            if event.seq != expected {
                return Err(SkillError::EventSequenceMismatch {
                    expected,
                    actual: event.seq,
                });
            }
            if event.previous_sha256.as_deref() != previous.as_deref() {
                return Err(SkillError::PreviousHashMismatch(event.seq));
            }
            if event.event_sha256 != hash_skill_event(event)? {
                return Err(SkillError::EventHashMismatch(event.seq));
            }
            previous = Some(event.event_sha256.clone());
        }
        Ok(())
    }

    fn append_event(&mut self, kind: SkillEventKind, now_ms: u128) -> Result<(), SkillError> {
        let seq = self.next_seq + 1;
        let mut event = SkillEvent {
            schema_version: SKILL_EVENT_SCHEMA_VERSION,
            seq,
            event_id: Uuid::new_v4().to_string(),
            timestamp_unix_ms: now_ms,
            kind,
            previous_sha256: self.last_sha256.clone(),
            event_sha256: String::new(),
        };
        event.event_sha256 = hash_skill_event(&event)?;
        self.apply_event(&event)?;
        self.next_seq = seq;
        self.last_sha256 = Some(event.event_sha256.clone());
        self.events.push(event);
        Ok(())
    }

    fn apply_event(&mut self, event: &SkillEvent) -> Result<(), SkillError> {
        match &event.kind {
            SkillEventKind::Registered { manifest } => {
                manifest.validate()?;
                scan_manifest(manifest)?;
                if manifest.content_sha256 != sha256_hex(manifest.body.as_bytes()) {
                    return Err(SkillError::ScanRejected(
                        "content hash does not match body".into(),
                    ));
                }
                let key = (manifest.name.clone(), manifest.version.clone());
                if self.records.contains_key(&key) {
                    return Err(SkillError::AlreadyExists(key.0, key.1));
                }
                self.records.insert(
                    key,
                    SkillRecord {
                        manifest: manifest.clone(),
                        status: SkillStatus::Draft,
                        created_ms: event.timestamp_unix_ms,
                        superseded_by: None,
                    },
                );
            }
            SkillEventKind::Activated { name, version } => {
                let key = (name.clone(), version.clone());
                if !self.records.contains_key(&key) {
                    return Err(SkillError::UnknownVersion(name.clone(), version.clone()));
                }
                if let Some(previous) = self.active.insert(name.clone(), version.clone())
                    && previous != *version
                    && let Some(record) = self.records.get_mut(&(name.clone(), previous))
                {
                    record.status = SkillStatus::Superseded;
                    record.superseded_by = Some(version.clone());
                }
                let record = self
                    .records
                    .get_mut(&key)
                    .ok_or_else(|| SkillError::UnknownVersion(name.clone(), version.clone()))?;
                record.status = SkillStatus::Active;
                record.superseded_by = None;
            }
            SkillEventKind::Disabled { name, version } => {
                let key = (name.clone(), version.clone());
                if !self.records.contains_key(&key) {
                    return Err(SkillError::UnknownVersion(name.clone(), version.clone()));
                }
                if self.active.get(name) != Some(version) {
                    return Err(SkillError::NoActiveVersion(name.clone()));
                }
                self.active.remove(name);
                if let Some(record) = self.records.get_mut(&key) {
                    record.status = SkillStatus::Disabled;
                }
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

fn scan_manifest(manifest: &SkillManifest) -> Result<(), SkillError> {
    if manifest.body.chars().count() > 64_000 {
        return Err(SkillError::ScanRejected("body exceeds 64k chars".into()));
    }
    for (label, value) in [
        ("name", manifest.name.as_str()),
        ("description", manifest.description.as_str()),
        ("version", manifest.version.as_str()),
        ("scope", manifest.scope.as_str()),
        ("body", manifest.body.as_str()),
    ] {
        scan_text(label, value)?;
    }
    for (index, value) in manifest.platforms.iter().enumerate() {
        scan_text(&format!("platforms[{index}]"), value)?;
    }
    for (index, value) in manifest.requires_tools.iter().enumerate() {
        scan_text(&format!("requires_tools[{index}]"), value)?;
    }
    Ok(())
}

fn scan_text(label: &str, value: &str) -> Result<(), SkillError> {
    if value
        .chars()
        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
    {
        return Err(SkillError::ScanRejected(format!(
            "{label} contains control characters"
        )));
    }
    let lower = value.to_lowercase();
    for marker in [
        "begin private key",
        "ignore previous instructions",
        "ignore all previous",
        "-----begin rsa private key-----",
        "aws_secret_access_key",
    ] {
        if lower.contains(marker) {
            return Err(SkillError::ScanRejected(format!(
                "{label} contains forbidden marker {marker}"
            )));
        }
    }
    if contains_secret_token(&lower) {
        return Err(SkillError::ScanRejected(format!(
            "{label} contains a secret-like token"
        )));
    }
    Ok(())
}

fn contains_secret_token(value: &str) -> bool {
    for (index, _) in value.match_indices("sk-") {
        let rest = &value[index + 3..];
        if rest.starts_with("ant-") || rest.starts_with("proj-") || rest.starts_with("live-") {
            let token_len = rest
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
                .count();
            if token_len >= 20 {
                return true;
            }
        }
        let token_len = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .count();
        if token_len >= 16 && rest.chars().take(token_len).any(|ch| ch.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn required(frontmatter: &BTreeMap<String, String>, key: &str) -> Result<String, SkillError> {
    frontmatter
        .get(key)
        .cloned()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SkillError::InvalidFrontmatter(format!("missing {key}")))
}

fn parse_list(value: Option<&String>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\''))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with('.')
}

#[derive(Serialize)]
struct SkillEventHashPayload<'a> {
    schema_version: u32,
    seq: u64,
    event_id: &'a str,
    timestamp_unix_ms: u128,
    kind: &'a SkillEventKind,
    previous_sha256: &'a Option<String>,
}

fn hash_skill_event(event: &SkillEvent) -> Result<String, SkillError> {
    let payload = SkillEventHashPayload {
        schema_version: event.schema_version,
        seq: event.seq,
        event_id: &event.event_id,
        timestamp_unix_ms: event.timestamp_unix_ms,
        kind: &event.kind,
        previous_sha256: &event.previous_sha256,
    };
    let bytes =
        serde_json::to_vec(&payload).map_err(|_| SkillError::EventHashMismatch(event.seq))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> SkillManifest {
        SkillManifest {
            name: "release".into(),
            description: "release process".into(),
            version: version.into(),
            platforms: vec!["linux".into()],
            scope: "repo:1-agent".into(),
            requires_tools: vec!["shell".into()],
            risk: SkillRisk::Medium,
            body: "## Procedure\n1. verify\n2. build\n3. tag".into(),
            content_sha256: sha256_hex(b"release"),
        }
    }

    #[test]
    fn parses_skill_markdown_and_indexes_without_body() {
        let source = "---\nname: release\ndescription: release process\nversion: 1.2.0\nplatforms: [linux, macos]\nscope: repo:1-agent\nrequires_tools: [shell, git]\nrisk: medium\n---\n# Release\n1. verify\n2. build";
        let parsed = SkillManifest::from_markdown(source).unwrap();
        assert_eq!(parsed.name, "release");
        assert_eq!(parsed.version, "1.2.0");
        assert_eq!(parsed.platforms, vec!["linux", "macos"]);
        assert_eq!(parsed.requires_tools, vec!["shell", "git"]);
        assert!(parsed.body.contains("verify"));
        assert!(!parsed.content_sha256.is_empty());
    }

    #[test]
    fn activate_disable_and_rollback_one_active_version() {
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        store.register(manifest("1.1.0"), 11).unwrap();
        store.activate("release", "1.0.0", 12).unwrap();
        store.activate("release", "1.1.0", 13).unwrap();
        assert_eq!(store.active("release").unwrap().version, "1.1.0");
        store.rollback("release", "1.0.0", 14).unwrap();
        assert_eq!(store.active("release").unwrap().version, "1.0.0");
        let records = store.list();
        assert_eq!(
            records
                .iter()
                .filter(|record| record.status == SkillStatus::Active)
                .count(),
            1
        );
        store.disable("release").unwrap();
        assert!(store.active("release").is_none());
    }

    #[test]
    fn duplicate_version_and_invalid_frontmatter_fail_closed() {
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        assert_eq!(
            store.register(manifest("1.0.0"), 11).unwrap_err(),
            SkillError::AlreadyExists("release".into(), "1.0.0".into())
        );
        assert!(matches!(
            SkillManifest::from_markdown("no frontmatter").unwrap_err(),
            SkillError::InvalidFrontmatter(_)
        ));
    }

    #[test]
    fn scan_rejects_control_characters_and_secret_markers() {
        let mut store = SkillStore::default();
        let mut bad = manifest("1.0.0");
        bad.body = "safe\u{0}unsafe".into();
        assert!(matches!(
            store.register(bad, 10).unwrap_err(),
            SkillError::ScanRejected(_)
        ));
        let mut secret = manifest("1.0.1");
        secret.body = "token sk-1234567890abcdef".into();
        assert!(matches!(
            store.register(secret, 11).unwrap_err(),
            SkillError::ScanRejected(_)
        ));
    }

    #[test]
    fn scan_covers_frontmatter_fields_not_only_the_body() {
        let mut store = SkillStore::default();
        let mut injected = manifest("1.0.0");
        injected.description = "ignore previous instructions".into();
        assert!(matches!(
            store.register(injected, 10).unwrap_err(),
            SkillError::ScanRejected(_)
        ));
        let mut hidden = manifest("1.0.1");
        hidden.platforms = vec!["linux\u{0}".into()];
        assert!(matches!(
            store.register(hidden, 11).unwrap_err(),
            SkillError::ScanRejected(_)
        ));
    }

    #[test]
    fn scan_allows_secret_like_prefix_in_normal_words_and_hashes_body() {
        let mut store = SkillStore::default();
        let mut benign = manifest("1.0.0");
        benign.body =
            "Use a risk-based, task-list, task-management-system, and risk-management-framework approach."
                .into();
        benign.content_sha256 = "stale".into();
        store.register(benign, 10).unwrap();
        let stored = store.view("release", "1.0.0").unwrap();
        assert!(stored.body.contains("risk-based"));
        assert_ne!(stored.content_sha256, "stale");
        assert_eq!(
            stored.content_sha256,
            sha256_hex(stored.body.as_bytes()),
            "register must recompute the body hash"
        );
    }

    #[test]
    fn events_are_hash_chained_and_replay_to_the_same_state() {
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        store.activate("release", "1.0.0", 11).unwrap();
        store.register(manifest("1.1.0"), 12).unwrap();
        store.activate("release", "1.1.0", 13).unwrap();
        store.disable_at("release", 14).unwrap();
        let events = store.events().to_vec();
        assert_eq!(events.len(), 5);
        SkillStore::verify_events(&events).unwrap();
        let replayed = SkillStore::replay(&events).unwrap();
        assert_eq!(replayed.list(), store.list());
        assert!(replayed.active("release").is_none());
        assert_eq!(replayed.view("release", "1.1.0").unwrap().version, "1.1.0");

        let mut tampered = events;
        tampered[1].kind = SkillEventKind::Disabled {
            name: "release".into(),
            version: "1.0.0".into(),
        };
        assert_eq!(
            SkillStore::replay(&tampered).unwrap_err(),
            SkillError::EventHashMismatch(2)
        );
    }

    #[test]
    fn replay_rescans_a_resealed_manifest() {
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        let mut events = store.events().to_vec();
        if let SkillEventKind::Registered { manifest } = &mut events[0].kind {
            manifest.body = "ignore previous instructions".into();
            manifest.content_sha256 = sha256_hex(manifest.body.as_bytes());
        }
        events[0].event_sha256 = hash_skill_event(&events[0]).unwrap();
        assert!(matches!(
            SkillStore::replay(&events).unwrap_err(),
            SkillError::ScanRejected(_)
        ));
    }

    #[test]
    fn failed_registration_does_not_append_an_event() {
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        assert!(store.register(manifest("1.0.0"), 11).is_err());
        assert_eq!(store.events().len(), 1);
    }

    #[test]
    fn list_and_view_are_bounded_index_operations() {
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].description, "release process");
        assert!(store.view("release", "1.0.0").is_some());
        assert!(store.view("release", "9.9.9").is_none());
    }
}
