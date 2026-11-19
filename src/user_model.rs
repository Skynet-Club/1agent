//! Derived user model over scoped memory records.
//!
//! This is a view, not a second source of truth: every preference/boundary
//! keeps the memory id it came from, and the model can be rebuilt from journal
//! replay.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::autonomy::QuietHours;
use crate::memory::{MemoryKind, MemoryRecord, MemoryScope};

/// How much the partner may do without asking, per domain.
///
/// This is a preference, not an authorization grant: Authority/Policy still
/// decides whether an action is allowed.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    Observe,
    #[default]
    Suggest,
    Act,
    Auto,
}

impl AutonomyLevel {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "observe" => Some(Self::Observe),
            "suggest" => Some(Self::Suggest),
            "act" => Some(Self::Act),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Suggest => "suggest",
            Self::Act => "act",
            Self::Auto => "auto",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::Observe => 0,
            Self::Suggest => 1,
            Self::Act => 2,
            Self::Auto => 3,
        }
    }
}

/// How the partner speaks. Execution stays terse; advice keeps the reasoning.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMode {
    #[default]
    Operator,
    Advisor,
    Companion,
}

impl VoiceMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "operator" => Some(Self::Operator),
            "advisor" => Some(Self::Advisor),
            "companion" => Some(Self::Companion),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::Advisor => "advisor",
            Self::Companion => "companion",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AutonomyEntry {
    pub level: AutonomyLevel,
    pub source_memory_id: String,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VoiceEntry {
    pub mode: VoiceMode,
    pub source_memory_id: String,
    pub updated_ms: u128,
}

/// What the partner may remember, for how long, and whether it may leave the
/// machine. `retention_ms = None` means the memory's own TTL decides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuietHoursEntry {
    pub hours: QuietHours,
    pub source_memory_id: String,
    pub updated_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConsentRule {
    pub memory_kind: MemoryKind,
    pub allow: bool,
    pub retention_ms: Option<u128>,
    pub source_memory_id: String,
    pub updated_ms: u128,
}

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
pub struct UserModelSnapshot {
    pub schema: String,
    pub generated_ms: u128,
    pub model: UserModel,
    pub stale_preferences: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UserModel {
    pub user_id: String,
    pub scope: MemoryScope,
    pub preferences: BTreeMap<String, Preference>,
    pub boundaries: Vec<Boundary>,
    /// Domain -> autonomy preference. Domains are free-form (`workspace`,
    /// `email`, `browser`, ...). Unset domains default to `suggest`.
    #[serde(default)]
    pub autonomy: BTreeMap<String, AutonomyEntry>,
    /// Unset voice defaults to `operator`.
    #[serde(default)]
    pub voice: Option<VoiceEntry>,
    /// Quiet hours are a preference; they do not grant or deny authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet_hours: Option<QuietHoursEntry>,
    /// Latest consent rule per memory kind.
    #[serde(default)]
    pub consents: Vec<ConsentRule>,
    pub updated_ms: u128,
}

impl UserModel {
    pub fn new(user_id: impl Into<String>, scope: MemoryScope) -> Self {
        Self {
            user_id: user_id.into(),
            scope,
            preferences: BTreeMap::new(),
            boundaries: Vec::new(),
            autonomy: BTreeMap::new(),
            voice: None,
            quiet_hours: None,
            consents: Vec::new(),
            updated_ms: 0,
        }
    }

    pub fn apply(&mut self, record: &MemoryRecord) {
        if !self.scope.matches(&record.scope)
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
                if let Some(domain) = key.strip_prefix("autonomy.")
                    && !domain.trim().is_empty()
                    && let Some(level) = AutonomyLevel::parse(value)
                {
                    self.set_autonomy(
                        domain.trim(),
                        level,
                        &record.memory_id,
                        record.valid_from_ms,
                    );
                }
                if matches!(key, "voice" | "voice_mode")
                    && let Some(mode) = VoiceMode::parse(value)
                {
                    self.set_voice(mode, &record.memory_id, record.valid_from_ms);
                }
                if key == "quiet_hours"
                    && let Ok(hours) = QuietHours::parse(value)
                {
                    self.set_quiet_hours(hours, &record.memory_id, record.valid_from_ms);
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
                if let Some((kind, allow, retention_ms)) = parse_consent(&record.content) {
                    self.set_consent(
                        kind,
                        allow,
                        retention_ms,
                        &record.memory_id,
                        record.valid_from_ms,
                    );
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

    /// Safe default: a new domain starts at `suggest`, never `auto`.
    pub fn autonomy_for(&self, domain: &str) -> AutonomyLevel {
        self.autonomy
            .get(domain)
            .map(|entry| entry.level)
            .unwrap_or_default()
    }

    pub fn voice_mode(&self) -> VoiceMode {
        self.voice
            .as_ref()
            .map(|entry| entry.mode)
            .unwrap_or_default()
    }

    pub fn quiet_hours(&self) -> Option<QuietHours> {
        self.quiet_hours.as_ref().map(|entry| entry.hours)
    }

    pub fn consent_for(&self, kind: MemoryKind) -> Option<&ConsentRule> {
        self.consents
            .iter()
            .filter(|rule| rule.memory_kind == kind)
            .max_by_key(|rule| rule.updated_ms)
    }

    /// Default is allow: consent rules opt out, they do not opt in.
    #[cfg(test)]
    pub fn can_remember(&self, kind: MemoryKind) -> bool {
        self.consent_for(kind).is_none_or(|rule| rule.allow)
    }

    pub fn retention_ms(&self, kind: MemoryKind) -> Option<u128> {
        self.consent_for(kind).and_then(|rule| rule.retention_ms)
    }

    pub fn snapshot(&self, now_ms: u128, max_age_ms: u128) -> UserModelSnapshot {
        let stale_preferences = self
            .preferences
            .iter()
            .filter(|(_, preference)| now_ms.saturating_sub(preference.updated_ms) > max_age_ms)
            .map(|(key, _)| key.clone())
            .collect();
        UserModelSnapshot {
            schema: "1agent.user-model-snapshot/v1".into(),
            generated_ms: now_ms,
            model: self.clone(),
            stale_preferences,
        }
    }

    fn set_autonomy(
        &mut self,
        domain: &str,
        level: AutonomyLevel,
        source_memory_id: &str,
        updated_ms: u128,
    ) {
        let replace = self
            .autonomy
            .get(domain)
            .is_none_or(|existing| existing.updated_ms <= updated_ms);
        if replace {
            self.autonomy.insert(
                domain.to_string(),
                AutonomyEntry {
                    level,
                    source_memory_id: source_memory_id.to_string(),
                    updated_ms,
                },
            );
            self.updated_ms = self.updated_ms.max(updated_ms);
        }
    }

    fn set_voice(&mut self, mode: VoiceMode, source_memory_id: &str, updated_ms: u128) {
        let replace = self
            .voice
            .as_ref()
            .is_none_or(|existing| existing.updated_ms <= updated_ms);
        if replace {
            self.voice = Some(VoiceEntry {
                mode,
                source_memory_id: source_memory_id.to_string(),
                updated_ms,
            });
            self.updated_ms = self.updated_ms.max(updated_ms);
        }
    }

    fn set_quiet_hours(&mut self, hours: QuietHours, source_memory_id: &str, updated_ms: u128) {
        let replace = self
            .quiet_hours
            .as_ref()
            .is_none_or(|existing| existing.updated_ms <= updated_ms);
        if replace {
            self.quiet_hours = Some(QuietHoursEntry {
                hours,
                source_memory_id: source_memory_id.to_string(),
                updated_ms,
            });
            self.updated_ms = self.updated_ms.max(updated_ms);
        }
    }

    fn set_consent(
        &mut self,
        kind: MemoryKind,
        allow: bool,
        retention_ms: Option<u128>,
        source_memory_id: &str,
        updated_ms: u128,
    ) {
        let existing = self
            .consents
            .iter()
            .position(|rule| rule.memory_kind == kind);
        if let Some(index) = existing
            && self.consents[index].updated_ms > updated_ms
        {
            return;
        }
        let rule = ConsentRule {
            memory_kind: kind,
            allow,
            retention_ms,
            source_memory_id: source_memory_id.to_string(),
            updated_ms,
        };
        match existing {
            Some(index) => self.consents[index] = rule,
            None => self.consents.push(rule),
        }
        self.updated_ms = self.updated_ms.max(updated_ms);
    }
}

/// Parse `consent:<kind>=allow|deny[@<retention_ms>]`.
fn parse_consent(content: &str) -> Option<(MemoryKind, bool, Option<u128>)> {
    let rest = content.strip_prefix("consent:")?;
    let (kind, rest) = rest.split_once('=')?;
    let kind = parse_memory_kind(kind)?;
    let mut parts = rest.trim().split('@');
    let allow = match parts.next()?.trim().to_ascii_lowercase().as_str() {
        "allow" => true,
        "deny" => false,
        _ => return None,
    };
    let retention_ms = parts.next().and_then(|value| value.trim().parse().ok());
    Some((kind, allow, retention_ms))
}

fn parse_memory_kind(value: &str) -> Option<MemoryKind> {
    match value.trim().to_ascii_lowercase().as_str() {
        "fact" => Some(MemoryKind::Fact),
        "preference" => Some(MemoryKind::Preference),
        "decision" => Some(MemoryKind::Decision),
        "project" => Some(MemoryKind::Project),
        "feedback" => Some(MemoryKind::Feedback),
        "relationship" => Some(MemoryKind::Relationship),
        "skill_hint" | "skill-hint" => Some(MemoryKind::SkillHint),
        "entity" => Some(MemoryKind::Entity),
        "process" => Some(MemoryKind::Process),
        _ => None,
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
    fn snapshot_flags_stale_preferences_and_keeps_values() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record("m1", MemoryKind::Preference, "editor=pnpm", 10));
        let fresh = model.snapshot(50, 100);
        assert!(fresh.stale_preferences.is_empty());
        assert_eq!(fresh.model.preference("editor").unwrap().value, "pnpm");
        let stale = model.snapshot(200, 100);
        assert_eq!(stale.stale_preferences, vec!["editor"]);
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

    #[test]
    fn autonomy_voice_and_consent_are_parsed() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record(
            "m1",
            MemoryKind::Preference,
            "autonomy.workspace=act",
            10,
        ));
        model.apply(&record(
            "m2",
            MemoryKind::Preference,
            "voice_mode=advisor",
            11,
        ));
        model.apply(&record(
            "m3",
            MemoryKind::Feedback,
            "consent:preference=deny@2592000000",
            12,
        ));
        model.apply(&record(
            "m4",
            MemoryKind::Preference,
            "quiet_hours=22:00-07:00",
            13,
        ));
        model.apply(&record(
            "m5",
            MemoryKind::Preference,
            "quiet_hours=not-a-window",
            14,
        ));

        assert_eq!(model.autonomy_for("workspace"), AutonomyLevel::Act);
        assert_eq!(model.autonomy_for("email"), AutonomyLevel::Suggest);
        assert_eq!(model.voice_mode(), VoiceMode::Advisor);
        let quiet = model.quiet_hours().unwrap();
        assert_eq!(quiet.start_minute, 22 * 60);
        assert_eq!(quiet.end_minute, 7 * 60);
        assert!(!model.can_remember(MemoryKind::Preference));
        assert_eq!(
            model.retention_ms(MemoryKind::Preference),
            Some(2_592_000_000)
        );
        assert!(model.can_remember(MemoryKind::Fact));
    }

    #[test]
    fn newer_autonomy_wins_and_older_is_ignored() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record(
            "m1",
            MemoryKind::Preference,
            "autonomy.workspace=act",
            20,
        ));
        model.apply(&record(
            "m2",
            MemoryKind::Preference,
            "autonomy.workspace=observe",
            10,
        ));
        assert_eq!(model.autonomy_for("workspace"), AutonomyLevel::Act);
        model.apply(&record(
            "m3",
            MemoryKind::Preference,
            "autonomy.workspace=observe",
            30,
        ));
        assert_eq!(model.autonomy_for("workspace"), AutonomyLevel::Observe);
    }

    #[test]
    fn forgotten_autonomy_voice_and_consent_are_ignored() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        let mut autonomy = record("m1", MemoryKind::Preference, "autonomy.workspace=auto", 10);
        autonomy.soft_forgotten_ms = Some(11);
        model.apply(&autonomy);
        let mut voice = record("m2", MemoryKind::Preference, "voice_mode=companion", 12);
        voice.hard_forgotten_ms = Some(13);
        model.apply(&voice);
        let mut consent = record("m3", MemoryKind::Feedback, "consent:fact=deny", 14);
        consent.superseded_by = Some("m4".into());
        model.apply(&consent);

        assert_eq!(model.autonomy_for("workspace"), AutonomyLevel::Suggest);
        assert_eq!(model.voice_mode(), VoiceMode::Operator);
        assert!(model.can_remember(MemoryKind::Fact));
    }

    #[test]
    fn invalid_values_are_ignored() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record(
            "m1",
            MemoryKind::Preference,
            "autonomy.workspace=maybe",
            10,
        ));
        model.apply(&record(
            "m2",
            MemoryKind::Preference,
            "voice_mode=robot",
            11,
        ));
        model.apply(&record(
            "m3",
            MemoryKind::Feedback,
            "consent:unknown=allow",
            12,
        ));
        assert_eq!(model.autonomy_for("workspace"), AutonomyLevel::Suggest);
        assert_eq!(model.voice_mode(), VoiceMode::Operator);
        assert!(model.consents.is_empty());
    }

    #[test]
    fn serialization_roundtrip_keeps_relationship_fields() {
        let mut model = UserModel::new("user-1", MemoryScope::bot("bot-1"));
        model.apply(&record(
            "m1",
            MemoryKind::Preference,
            "autonomy.email=observe",
            10,
        ));
        model.apply(&record(
            "m2",
            MemoryKind::Preference,
            "voice_mode=companion",
            11,
        ));
        model.apply(&record(
            "m3",
            MemoryKind::Feedback,
            "consent:fact=allow@1000",
            12,
        ));
        let encoded = serde_json::to_string(&model).unwrap();
        let decoded: UserModel = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, model);
    }
}
