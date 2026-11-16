//! Pending human approvals for gated tool calls.
//!
//! Authority/Policy remains the only allow/deny source. When policy returns
//! `require_approval`, the runtime creates a card bound to the run, tool and
//! canonical arguments digest. A human approves or denies it out of band; a
//! retry with the same run/tool/arguments consumes the card exactly once.
//! Changed arguments, a different run, expiry, denial or reuse require a new
//! card.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::RiskLevel;

pub const TOOL_APPROVAL_SCHEMA: u32 = 1;
pub const TOOL_APPROVAL_TTL_MS: u128 = 15 * 60 * 1000;
pub const TOOL_APPROVAL_ENV: &str = "ONEAGENT_TOOL_APPROVALS";
const MAX_CARDS: usize = 256;
const MAX_CARD_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalStatus {
    Pending,
    Approved,
    Denied,
    Consumed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ToolApprovalCard {
    pub card_id: String,
    pub action_id: String,
    pub run_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub tool: String,
    pub arguments_sha256: String,
    pub risk: RiskLevel,
    pub reversible: bool,
    pub created_ms: u128,
    pub expires_ms: u128,
    pub status: ToolApprovalStatus,
    #[serde(default)]
    pub decided_ms: Option<u128>,
}

impl ToolApprovalCard {
    pub fn pending(
        action_id: impl Into<String>,
        run_id: impl Into<String>,
        session_id: Option<String>,
        tool: impl Into<String>,
        arguments_sha256: impl Into<String>,
        risk: RiskLevel,
        reversible: bool,
        now_ms: u128,
    ) -> Self {
        Self {
            card_id: format!("tool-card-{}", Uuid::new_v4()),
            action_id: action_id.into(),
            run_id: run_id.into(),
            session_id,
            tool: tool.into(),
            arguments_sha256: arguments_sha256.into(),
            risk,
            reversible,
            created_ms: now_ms,
            expires_ms: now_ms.saturating_add(TOOL_APPROVAL_TTL_MS),
            status: ToolApprovalStatus::Pending,
            decided_ms: None,
        }
    }

    fn matches_request(&self, run_id: &str, tool: &str, arguments_sha256: &str) -> bool {
        self.run_id == run_id && self.tool == tool && self.arguments_sha256 == arguments_sha256
    }
}

#[derive(Debug, Error)]
pub enum ToolApprovalError {
    #[error("tool approval io: {0}")]
    Io(#[from] std::io::Error),
    #[error("tool approval json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tool approval file is not private: {0}")]
    InsecureFile(PathBuf),
    #[error("tool approval card {0} not found")]
    NotFound(String),
    #[error("tool approval card {0} is not pending")]
    NotPending(String),
    #[error("tool approval card {0} expired")]
    Expired(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CardFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    cards: Vec<ToolApprovalCard>,
}

#[derive(Debug, Clone)]
pub struct ToolApprovalStore {
    path: PathBuf,
}

impl ToolApprovalStore {
    pub fn default_path() -> PathBuf {
        match std::env::var_os(TOOL_APPROVAL_ENV) {
            Some(path) => PathBuf::from(path),
            None => std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".cache/1agent-tmp/tool-approvals.json"),
        }
    }

    pub fn default_store() -> Self {
        Self::at(Self::default_path())
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self) -> Result<CardFile, ToolApprovalError> {
        match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink()
                    || !metadata.is_file()
                    || metadata.len() > MAX_CARD_FILE_BYTES
                {
                    return Err(ToolApprovalError::InsecureFile(self.path.clone()));
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o077 != 0 {
                        return Err(ToolApprovalError::InsecureFile(self.path.clone()));
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CardFile {
                    schema_version: TOOL_APPROVAL_SCHEMA,
                    cards: Vec::new(),
                });
            }
            Err(error) => return Err(error.into()),
        }
        let bytes = std::fs::read(&self.path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn save(&self, file: &CardFile) -> Result<(), ToolApprovalError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let bytes = serde_json::to_vec_pretty(file)?;
        let temporary = self
            .path
            .with_extension(format!("tmp-{}", std::process::id()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&temporary)?;
            out.write_all(&bytes)?;
            out.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&temporary, &bytes)?;
        }
        std::fs::rename(&temporary, &self.path)?;
        Ok(())
    }

    fn lock(&self) -> Result<std::fs::File, ToolApprovalError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lock_path = self.path.with_extension("lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(lock_path)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(file)
    }

    fn prune(cards: &mut Vec<ToolApprovalCard>, now_ms: u128) {
        cards.retain(|card| match card.status {
            ToolApprovalStatus::Pending => now_ms <= card.expires_ms,
            ToolApprovalStatus::Consumed | ToolApprovalStatus::Denied => {
                now_ms <= card.expires_ms.saturating_add(3_600_000)
            }
            ToolApprovalStatus::Approved => now_ms <= card.expires_ms,
        });
        if cards.len() > MAX_CARDS {
            let excess = cards.len() - MAX_CARDS;
            cards.drain(0..excess);
        }
    }

    /// Create a pending card, or return the existing pending card for the
    /// same run/tool/arguments.
    pub fn request(&self, card: ToolApprovalCard) -> Result<ToolApprovalCard, ToolApprovalError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        Self::prune(&mut file.cards, card.created_ms);
        if let Some(existing) = file.cards.iter().find(|stored| {
            stored.status == ToolApprovalStatus::Pending
                && stored.matches_request(&card.run_id, &card.tool, &card.arguments_sha256)
                && card.created_ms <= stored.expires_ms
        }) {
            return Ok(existing.clone());
        }
        file.schema_version = TOOL_APPROVAL_SCHEMA;
        file.cards.push(card.clone());
        Self::prune(&mut file.cards, card.created_ms);
        self.save(&file)?;
        Ok(card)
    }

    pub fn list(&self) -> Result<Vec<ToolApprovalCard>, ToolApprovalError> {
        let _lock = self.lock()?;
        Ok(self.load()?.cards)
    }

    pub fn get(&self, card_id: &str) -> Result<Option<ToolApprovalCard>, ToolApprovalError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|card| card.card_id == card_id))
    }

    pub fn approve(
        &self,
        card_id: &str,
        now_ms: u128,
    ) -> Result<ToolApprovalCard, ToolApprovalError> {
        self.decide(card_id, ToolApprovalStatus::Approved, now_ms)
    }

    pub fn deny(&self, card_id: &str, now_ms: u128) -> Result<ToolApprovalCard, ToolApprovalError> {
        self.decide(card_id, ToolApprovalStatus::Denied, now_ms)
    }

    fn decide(
        &self,
        card_id: &str,
        status: ToolApprovalStatus,
        now_ms: u128,
    ) -> Result<ToolApprovalCard, ToolApprovalError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        let card = file
            .cards
            .iter_mut()
            .find(|card| card.card_id == card_id)
            .ok_or_else(|| ToolApprovalError::NotFound(card_id.into()))?;
        if card.status != ToolApprovalStatus::Pending {
            return Err(ToolApprovalError::NotPending(card_id.into()));
        }
        if now_ms > card.expires_ms {
            return Err(ToolApprovalError::Expired(card_id.into()));
        }
        card.status = status;
        card.decided_ms = Some(now_ms);
        let card = card.clone();
        self.save(&file)?;
        Ok(card)
    }

    /// Consume an approved card for exactly one execution. Matching is by
    /// run/tool/arguments digest; the action id is recorded but not matched
    /// because a retry is a new tool call.
    pub fn take_matching(
        &self,
        run_id: &str,
        tool: &str,
        arguments_sha256: &str,
        now_ms: u128,
    ) -> Result<ToolApprovalCard, ToolApprovalError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        let card = file
            .cards
            .iter_mut()
            .filter(|card| card.status == ToolApprovalStatus::Approved)
            .find(|card| card.matches_request(run_id, tool, arguments_sha256))
            .ok_or_else(|| ToolApprovalError::NotFound(tool.into()))?;
        if now_ms > card.expires_ms {
            return Err(ToolApprovalError::Expired(card.card_id.clone()));
        }
        card.status = ToolApprovalStatus::Consumed;
        let card = card.clone();
        self.save(&file)?;
        Ok(card)
    }
}

/// Stable digest of tool arguments: object keys are sorted recursively.
pub fn canonical_arguments_sha256(arguments: &serde_json::Value) -> String {
    fn canonical(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let sorted = map
                    .iter()
                    .map(|(key, value)| (key.clone(), canonical(value)))
                    .collect::<std::collections::BTreeMap<_, _>>();
                serde_json::Value::Object(sorted.into_iter().collect())
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(canonical).collect())
            }
            other => other.clone(),
        }
    }
    let bytes = serde_json::to_vec(&canonical(arguments)).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(now: u128) -> ToolApprovalCard {
        ToolApprovalCard::pending(
            "run-1:call-1",
            "run-1",
            Some("ses-1".into()),
            "write_file",
            canonical_arguments_sha256(&serde_json::json!({"b": 1, "a": "x"})),
            RiskLevel::Medium,
            false,
            now,
        )
    }

    #[test]
    fn pending_cards_dedupe_and_approval_is_consumed_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = ToolApprovalStore::at(dir.path().join("tool-approvals.json"));
        let first = store.request(card(1_000)).unwrap();
        let second = store.request(card(1_001)).unwrap();
        assert_eq!(first.card_id, second.card_id);

        store.approve(&first.card_id, 2_000).unwrap();
        let args = canonical_arguments_sha256(&serde_json::json!({"a": "x", "b": 1}));
        let consumed = store
            .take_matching("run-1", "write_file", &args, 3_000)
            .unwrap();
        assert_eq!(consumed.card_id, first.card_id);
        assert_eq!(consumed.status, ToolApprovalStatus::Consumed);
        assert!(matches!(
            store.take_matching("run-1", "write_file", &args, 3_001),
            Err(ToolApprovalError::NotFound(_))
        ));
        let stored = store.get(&first.card_id).unwrap().unwrap();
        assert_eq!(stored.status, ToolApprovalStatus::Consumed);
    }

    #[test]
    fn changed_arguments_run_or_denial_require_a_new_card() {
        let dir = tempfile::tempdir().unwrap();
        let store = ToolApprovalStore::at(dir.path().join("tool-approvals.json"));
        let first = store.request(card(1_000)).unwrap();
        store.approve(&first.card_id, 2_000).unwrap();

        let other_args = canonical_arguments_sha256(&serde_json::json!({"a": "y", "b": 1}));
        assert!(matches!(
            store.take_matching("run-1", "write_file", &other_args, 3_000),
            Err(ToolApprovalError::NotFound(_))
        ));
        assert!(matches!(
            store.take_matching(
                "run-2",
                "write_file",
                &canonical_arguments_sha256(&serde_json::json!({"b": 1, "a": "x"})),
                3_000
            ),
            Err(ToolApprovalError::NotFound(_))
        ));
        store
            .take_matching(
                "run-1",
                "write_file",
                &canonical_arguments_sha256(&serde_json::json!({"a": "x", "b": 1})),
                3_050,
            )
            .unwrap();

        let second = store.request(card(3_100)).unwrap();
        store.deny(&second.card_id, 3_200).unwrap();
        assert!(matches!(
            store.take_matching(
                "run-1",
                "write_file",
                &canonical_arguments_sha256(&serde_json::json!({"a": "x", "b": 1})),
                3_300
            ),
            Err(ToolApprovalError::NotFound(_))
        ));
    }

    #[test]
    fn expired_cards_cannot_be_approved_or_consumed() {
        let dir = tempfile::tempdir().unwrap();
        let store = ToolApprovalStore::at(dir.path().join("tool-approvals.json"));
        let card = store.request(card(1_000)).unwrap();
        assert!(matches!(
            store.approve(&card.card_id, card.expires_ms + 1),
            Err(ToolApprovalError::Expired(_))
        ));
    }

    #[test]
    fn canonical_digest_ignores_object_key_order() {
        let left = canonical_arguments_sha256(&serde_json::json!({
            "b": {"y": 2, "x": 1},
            "a": [1, 2]
        }));
        let right = canonical_arguments_sha256(&serde_json::json!({
            "a": [1, 2],
            "b": {"x": 1, "y": 2}
        }));
        assert_eq!(left, right);
    }
}
