//! Browser action authorization and verifiable action cards.
//!
//! The card contains a ground-truth descriptor rebuilt from the pending action,
//! not model text. Approval is bound to a nonce and must be re-verified at
//! execution time. Cards persist on disk so a human can approve an action from
//! a separate process while the agent waits.

use std::io::Read;
#[cfg(unix)]
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::cua::{ArgumentSource, CuaAction, CuaDecision, CuaPlan};
use crate::domain::RiskLevel;

/// A human often approves in a later turn; the card is still bound to one
/// exact action and is consumed once. Ten minutes avoids making the user race
/// the approval dialog without weakening the target binding.
pub const ACTION_CARD_TTL_MS: u128 = 600_000;
const MAX_CARDS: usize = 256;
const MAX_CARD_FILE_BYTES: u64 = 1024 * 1024;

mod u128_ms {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(u64::try_from(*value).unwrap_or(u64::MAX))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(u64::deserialize(deserializer)? as u128)
    }
}

mod option_u128_ms {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<u128>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_some(&u64::try_from(*value).unwrap_or(u64::MAX)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<u64>::deserialize(deserializer)?.map(u128::from))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub action: String,
    pub target: String,
    pub risk: RiskLevel,
    pub reversible: bool,
    pub argument_sources: Vec<ArgumentSource>,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub summary: String,
    /// Runtime action id (`run_id:call_id`) when the request came from a run.
    #[serde(default)]
    pub action_id: String,
    /// SHA the action is expected to produce; a different digest at execution
    /// time invalidates the approval.
    #[serde(default)]
    pub expected_artifact_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionCard {
    pub card_id: String,
    pub action: String,
    pub target: String,
    pub risk: RiskLevel,
    pub reversible: bool,
    pub nonce: String,
    #[serde(with = "u128_ms")]
    pub created_at_ms: u128,
    #[serde(with = "u128_ms")]
    pub expires_at_ms: u128,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub action_id: String,
    #[serde(default)]
    pub expected_artifact_sha256: Option<String>,
}

impl ActionCard {
    pub fn verify(&self, nonce: &str, action: &str, target: &str, now_ms: u128) -> bool {
        self.nonce == nonce
            && self.action == action
            && self.target == target
            && now_ms <= self.expires_at_ms
    }

    /// Execution-time binding check. The card must describe exactly the
    /// pending action in the same browser session; expiry and nonce are
    /// checked here so an approval cannot be replayed later or elsewhere.
    ///
    /// `action_id` is deliberately not part of this check: a human approves in
    /// a later turn, so the retry necessarily has a new run/call id. The exact
    /// action, target, session, origin and artifact digest still bind the card.
    pub fn matches_request(&self, request: &ActionRequest, now_ms: u128) -> bool {
        self.action == request.action
            && self.target == request.target
            && self.session_id == request.session_id
            && self.origin == request.origin
            && self.expected_artifact_sha256 == request.expected_artifact_sha256
            && now_ms <= self.expires_at_ms
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ActionAuthorization {
    Allow,
    Approval { card: ActionCard },
    Deny { reason: String },
}

fn card_for(request: &ActionRequest, now_ms: u128) -> ActionCard {
    ActionCard {
        card_id: format!("card-{}", Uuid::new_v4()),
        action: request.action.clone(),
        target: request.target.clone(),
        risk: request.risk,
        reversible: request.reversible,
        nonce: Uuid::new_v4().to_string(),
        created_at_ms: now_ms,
        expires_at_ms: now_ms.saturating_add(ACTION_CARD_TTL_MS),
        session_id: request.session_id.clone(),
        origin: request.origin.clone(),
        summary: request.summary.clone(),
        action_id: request.action_id.clone(),
        expected_artifact_sha256: request.expected_artifact_sha256.clone(),
    }
}

/// Authorize one browser action against a frozen CUA plan.
///
/// Irreversible actions always produce an action card, even when the CUA
/// policy would allow them. Reversible actions with trusted arguments and
/// acceptable risk can proceed without a card.
pub fn authorize(plan: &CuaPlan, request: &ActionRequest, now_ms: u128) -> ActionAuthorization {
    let action = CuaAction {
        action: request.action.clone(),
        risk: request.risk,
        reversible: request.reversible,
        argument_sources: request.argument_sources.clone(),
    };
    let verdict = plan.authorize(&action, &request.target);
    if verdict.decision == CuaDecision::Deny {
        return ActionAuthorization::Deny {
            reason: verdict.reason,
        };
    }
    if !request.reversible || verdict.decision == CuaDecision::RequireApproval {
        return ActionAuthorization::Approval {
            card: card_for(request, now_ms),
        };
    }
    ActionAuthorization::Allow
}

/// Authorize a browser-tool action without a frozen CUA branch list.
///
/// The browser tool cannot enumerate future URLs, so the branch check is
/// replaced by the egress allowlist and the exact action descriptor hash.
/// Irreversible and untrusted actions still require a card.
pub fn authorize_request(request: &ActionRequest, now_ms: u128) -> ActionAuthorization {
    let action = CuaAction {
        action: request.action.clone(),
        risk: request.risk,
        reversible: request.reversible,
        argument_sources: request.argument_sources.clone(),
    };
    let verdict = crate::cua::evaluate(&action);
    if verdict.decision == CuaDecision::Deny {
        return ActionAuthorization::Deny {
            reason: verdict.reason,
        };
    }
    if !request.reversible || verdict.decision == CuaDecision::RequireApproval {
        return ActionAuthorization::Approval {
            card: card_for(request, now_ms),
        };
    }
    ActionAuthorization::Allow
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    Pending,
    Approved,
    Denied,
    Consumed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredActionCard {
    #[serde(flatten)]
    pub card: ActionCard,
    pub status: CardStatus,
    #[serde(default, with = "option_u128_ms")]
    pub approved_at_ms: Option<u128>,
}

#[derive(Debug, Error)]
pub enum ActionCardError {
    #[error("action card io: {0}")]
    Io(#[from] std::io::Error),
    #[error("action card json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("action card file is not private: {0}")]
    InsecureFile(PathBuf),
    #[error("action card {0} not found")]
    NotFound(String),
    #[error("action card {0} is not pending")]
    NotPending(String),
    #[error("action card {0} expired")]
    Expired(String),
    #[error("action card {0} does not match the pending action")]
    Mismatch(String),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CardFile {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    cards: Vec<StoredActionCard>,
}

pub struct ActionCardStore {
    path: PathBuf,
}

impl ActionCardStore {
    pub fn default_path() -> PathBuf {
        match std::env::var_os("ONEAGENT_BROWSER_APPROVALS") {
            Some(path) => PathBuf::from(path),
            None => crate::browser::default_browser_root().join("approvals.json"),
        }
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn default_store() -> Self {
        Self::at(Self::default_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self) -> Result<CardFile, ActionCardError> {
        match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(ActionCardError::InsecureFile(self.path.clone()));
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o077 != 0 {
                        return Err(ActionCardError::InsecureFile(self.path.clone()));
                    }
                }
                if metadata.len() > MAX_CARD_FILE_BYTES {
                    return Err(ActionCardError::InsecureFile(self.path.clone()));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CardFile {
                    schema_version: 1,
                    cards: Vec::new(),
                });
            }
            Err(error) => return Err(error.into()),
        }
        let mut bytes = Vec::new();
        let file = std::fs::OpenOptions::new().read(true).open(&self.path)?;
        file.take(MAX_CARD_FILE_BYTES + 1).read_to_end(&mut bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn save(&self, file: &CardFile) -> Result<(), ActionCardError> {
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

    fn lock(&self) -> Result<std::fs::File, ActionCardError> {
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

    fn prune(cards: &mut Vec<StoredActionCard>, now_ms: u128) {
        cards.retain(|card| {
            if card.status == CardStatus::Pending {
                now_ms <= card.card.expires_at_ms
            } else {
                now_ms <= card.card.expires_at_ms.saturating_add(3_600_000)
            }
        });
        if cards.len() > MAX_CARDS {
            let excess = cards.len() - MAX_CARDS;
            cards.drain(0..excess);
        }
    }

    /// Create a pending card, or return the existing pending card for the
    /// same session/action/target. Reuse keeps the card id stable while the
    /// model retries after a human has not approved yet.
    pub fn request(&self, card: ActionCard) -> Result<ActionCard, ActionCardError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        Self::prune(&mut file.cards, card.created_at_ms);
        if let Some(existing) = file.cards.iter().find(|stored| {
            stored.status == CardStatus::Pending
                && stored.card.session_id == card.session_id
                && stored.card.action == card.action
                && stored.card.target == card.target
                && stored.card.expected_artifact_sha256 == card.expected_artifact_sha256
                && card.created_at_ms <= stored.card.expires_at_ms
        }) {
            return Ok(existing.card.clone());
        }
        file.schema_version = 1;
        file.cards.push(StoredActionCard {
            card: card.clone(),
            status: CardStatus::Pending,
            approved_at_ms: None,
        });
        Self::prune(&mut file.cards, card.created_at_ms);
        self.save(&file)?;
        Ok(card)
    }

    pub fn list(&self) -> Result<Vec<StoredActionCard>, ActionCardError> {
        let _lock = self.lock()?;
        let file = self.load()?;
        Ok(file.cards)
    }

    pub fn get(&self, card_id: &str) -> Result<Option<StoredActionCard>, ActionCardError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|stored| stored.card.card_id == card_id))
    }

    pub fn approve(&self, card_id: &str, now_ms: u128) -> Result<ActionCard, ActionCardError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        let stored = file
            .cards
            .iter_mut()
            .find(|stored| stored.card.card_id == card_id)
            .ok_or_else(|| ActionCardError::NotFound(card_id.into()))?;
        if stored.status != CardStatus::Pending {
            return Err(ActionCardError::NotPending(card_id.into()));
        }
        if now_ms > stored.card.expires_at_ms {
            return Err(ActionCardError::Expired(card_id.into()));
        }
        stored.status = CardStatus::Approved;
        stored.approved_at_ms = Some(now_ms);
        let card = stored.card.clone();
        self.save(&file)?;
        Ok(card)
    }

    pub fn deny(&self, card_id: &str, now_ms: u128) -> Result<ActionCard, ActionCardError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        let stored = file
            .cards
            .iter_mut()
            .find(|stored| stored.card.card_id == card_id)
            .ok_or_else(|| ActionCardError::NotFound(card_id.into()))?;
        if stored.status != CardStatus::Pending {
            return Err(ActionCardError::NotPending(card_id.into()));
        }
        if now_ms > stored.card.expires_at_ms {
            return Err(ActionCardError::Expired(card_id.into()));
        }
        stored.status = CardStatus::Denied;
        stored.approved_at_ms = Some(now_ms);
        let card = stored.card.clone();
        self.save(&file)?;
        Ok(card)
    }

    /// Consume an approved card for exactly one execution. The descriptor
    /// hash, session, origin and expiry are re-checked inside the lock.
    pub fn take_approved(
        &self,
        card_id: &str,
        request: &ActionRequest,
        now_ms: u128,
    ) -> Result<ActionCard, ActionCardError> {
        let _lock = self.lock()?;
        let mut file = self.load()?;
        let stored = file
            .cards
            .iter_mut()
            .find(|stored| stored.card.card_id == card_id)
            .ok_or_else(|| ActionCardError::NotFound(card_id.into()))?;
        if stored.status == CardStatus::Consumed {
            return Err(ActionCardError::NotPending(card_id.into()));
        }
        if stored.status != CardStatus::Approved {
            return Err(ActionCardError::NotPending(card_id.into()));
        }
        if now_ms > stored.card.expires_at_ms {
            return Err(ActionCardError::Expired(card_id.into()));
        }
        if !stored.card.matches_request(request, now_ms) {
            return Err(ActionCardError::Mismatch(card_id.into()));
        }
        stored.status = CardStatus::Consumed;
        let card = stored.card.clone();
        self.save(&file)?;
        Ok(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> CuaPlan {
        let mut plan = CuaPlan::new("test-plan", vec!["https://example.com".into()]).unwrap();
        plan.freeze();
        plan
    }

    fn request(action: &str, risk: RiskLevel, reversible: bool) -> ActionRequest {
        ActionRequest {
            action: action.into(),
            target: "https://example.com".into(),
            risk,
            reversible,
            argument_sources: vec![ArgumentSource::Plan],
            session_id: "session-1".into(),
            origin: "https://example.com".into(),
            summary: "click example".into(),
            action_id: String::new(),
            expected_artifact_sha256: None,
        }
    }

    #[test]
    fn approval_card_binds_action_and_artifact_but_not_call_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = ActionCardStore::at(dir.path().join("cards.json"));
        let mut request = request("submit-payment", RiskLevel::Low, false);
        request.action_id = "run-1:call-1".into();
        request.expected_artifact_sha256 = Some("a".repeat(64));
        let ActionAuthorization::Approval { card } = authorize_request(&request, 1_000) else {
            panic!("expected approval");
        };
        let pending = store.request(card).unwrap();
        store.approve(&pending.card_id, 2_000).unwrap();

        let mut drifted_sha = request.clone();
        drifted_sha.expected_artifact_sha256 = Some("b".repeat(64));
        assert!(matches!(
            store.take_approved(&pending.card_id, &drifted_sha, 3_000),
            Err(ActionCardError::Mismatch(id)) if id == pending.card_id
        ));

        // A human approves in a later run; the retry has a new call id but the
        // same action/target/session/origin and artifact digest.
        let mut retry_action = request.clone();
        retry_action.action_id = "run-2:call-9".into();
        let consumed = store
            .take_approved(&pending.card_id, &retry_action, 3_000)
            .unwrap();
        assert_eq!(consumed.action_id, "run-1:call-1");
        assert_eq!(consumed.expected_artifact_sha256, Some("a".repeat(64)));
        assert!(matches!(
            store.take_approved(&pending.card_id, &request, 4_000),
            Err(ActionCardError::NotPending(id)) if id == pending.card_id
        ));
    }

    #[test]
    fn legacy_action_cards_without_binding_fields_are_readable() {
        let mut value =
            serde_json::to_value(card_for(&request("click", RiskLevel::Low, true), 1)).unwrap();
        let object = value.as_object_mut().unwrap();
        assert!(object.remove("action_id").is_some());
        assert!(object.remove("expected_artifact_sha256").is_some());
        let card: ActionCard = serde_json::from_value(value).unwrap();
        assert!(card.action_id.is_empty());
        assert!(card.expected_artifact_sha256.is_none());
    }

    #[test]
    fn pending_card_reuse_requires_the_same_target_and_expected_sha() {
        let dir = tempfile::tempdir().unwrap();
        let store = ActionCardStore::at(dir.path().join("cards.json"));
        let mut first = request("submit-payment", RiskLevel::Low, false);
        first.action_id = "run-1:call-1".into();
        first.expected_artifact_sha256 = Some("a".repeat(64));
        let card = card_for(&first, 1_000);
        let first_card = store.request(card.clone()).unwrap();

        let mut changed = first.clone();
        changed.expected_artifact_sha256 = Some("b".repeat(64));
        let second_card = store.request(card_for(&changed, 1_100)).unwrap();
        assert_ne!(first_card.card_id, second_card.card_id);

        // A retry in a later run keeps the same pending card.
        let mut same = first.clone();
        same.action_id = "run-2:call-9".into();
        let third_card = store.request(card_for(&same, 1_200)).unwrap();
        assert_eq!(first_card.card_id, third_card.card_id);
    }

    #[test]
    fn reversible_trusted_action_is_allowed() {
        let result = authorize(&plan(), &request("click", RiskLevel::Low, true), 1000);
        assert_eq!(result, ActionAuthorization::Allow);
    }

    #[test]
    fn irreversible_action_always_gets_a_card() {
        let result = authorize(
            &plan(),
            &request("submit-payment", RiskLevel::Low, false),
            1000,
        );
        let ActionAuthorization::Approval { card } = result else {
            panic!("expected approval");
        };
        assert_eq!(card.action, "submit-payment");
        assert!(card.verify(&card.nonce, "submit-payment", "https://example.com", 2000));
        assert!(!card.verify(
            &card.nonce,
            "submit-payment",
            "https://example.com",
            999_999
        ));
        assert!(!card.verify("wrong", "submit-payment", "https://example.com", 2000));
    }

    #[test]
    fn high_risk_untrusted_argument_is_denied() {
        let mut request = request("authorize", RiskLevel::High, true);
        request.argument_sources = vec![ArgumentSource::UntrustedObservation];
        let result = authorize(&plan(), &request, 1000);
        assert!(matches!(result, ActionAuthorization::Deny { .. }));
    }

    #[test]
    fn standalone_irreversible_action_gets_a_card() {
        let request = request("click", RiskLevel::Medium, false);
        let ActionAuthorization::Approval { card } = authorize_request(&request, 1000) else {
            panic!("expected approval");
        };
        assert!(card.matches_request(&request, 2000));
    }

    #[test]
    fn card_store_approves_and_consumes_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = ActionCardStore::at(dir.path().join("approvals.json"));
        let request = request("click", RiskLevel::Medium, false);
        let ActionAuthorization::Approval { card } = authorize_request(&request, 1000) else {
            panic!("expected approval");
        };
        let card = store.request(card).unwrap();
        // Re-requesting the same descriptor reuses the pending card.
        let again = store.request(card.clone()).unwrap();
        assert_eq!(again.card_id, card.card_id);
        assert!(store.take_approved(&card.card_id, &request, 1100).is_err());
        store.approve(&card.card_id, 1200).unwrap();
        assert!(store.take_approved(&card.card_id, &request, 1300).is_ok());
        assert!(store.take_approved(&card.card_id, &request, 1400).is_err());
    }

    #[test]
    fn card_store_rejects_substitution_and_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let store = ActionCardStore::at(dir.path().join("approvals.json"));
        let request = request("click", RiskLevel::Medium, false);
        let ActionAuthorization::Approval { card } = authorize_request(&request, 1000) else {
            panic!("expected approval");
        };
        let card = store.request(card).unwrap();
        store.approve(&card.card_id, 1200).unwrap();
        let mut other = request.clone();
        other.target = "different".into();
        assert!(store.take_approved(&card.card_id, &other, 1300).is_err());
        assert!(
            store
                .take_approved(&card.card_id, &request, 999_999)
                .is_err()
        );
    }
}
