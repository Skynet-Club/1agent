use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use ring::hmac;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    GoldSet,
    Experience,
    EvaluationReport,
    ApprovalRegistry,
    Provenance,
    ReleaseManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalEnvelope {
    pub envelope_version: u32,
    pub artifact_kind: ArtifactKind,
    pub artifact_sha256: String,
    pub approver: String,
    pub approved_unix_ms: u128,
    pub purpose: String,
    pub key_id: String,
    pub mac_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKeyStatus {
    Active,
    Retired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalKeyRecord {
    pub key_id: String,
    pub status: ApprovalKeyStatus,
    pub changed_unix_ms: u128,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalKeyRegistry {
    #[serde(default)]
    pub version: u64,
    pub keys: Vec<ApprovalKeyRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegistrySnapshot {
    pub snapshot_version: u32,
    pub registry_version: u64,
    pub registry_sha256: String,
    pub approved_unix_ms: u128,
    pub approver: String,
    pub key_id: String,
    pub mac_sha256: String,
    #[serde(default)]
    pub previous_snapshot_sha256: Option<String>,
    #[serde(default)]
    pub snapshot_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustAnchorManifest {
    pub anchor_version: u32,
    pub first_snapshot_sha256: String,
    pub operator: String,
    pub admin_key_ids: Vec<String>,
    pub created_unix_ms: u128,
}

impl ApprovalKeyRegistry {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ApprovalError> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ApprovalError> {
        std::fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn add_active(&mut self, key_id: String) -> Result<(), ApprovalError> {
        validate_text(&key_id)?;
        if self.keys.iter().any(|record| record.key_id == key_id) {
            return Err(ApprovalError::DuplicateKey);
        }
        self.keys.push(ApprovalKeyRecord {
            key_id,
            status: ApprovalKeyStatus::Active,
            changed_unix_ms: now_unix_ms(),
            reason: String::new(),
        });
        self.version += 1;
        Ok(())
    }

    pub fn transition(
        &mut self,
        key_id: &str,
        status: ApprovalKeyStatus,
        reason: String,
    ) -> Result<(), ApprovalError> {
        let record = self
            .keys
            .iter_mut()
            .find(|record| record.key_id == key_id)
            .ok_or(ApprovalError::UnknownKey)?;
        if record.status == ApprovalKeyStatus::Revoked {
            return Err(ApprovalError::KeyRevoked);
        }
        record.status = status;
        record.changed_unix_ms = now_unix_ms();
        record.reason = reason;
        self.version += 1;
        Ok(())
    }

    pub fn verify_approval(
        &self,
        approval: &ApprovalEnvelope,
        artifact_path: impl AsRef<Path>,
        key: &[u8],
    ) -> Result<(), ApprovalError> {
        let record = self
            .keys
            .iter()
            .find(|record| record.key_id == approval.key_id)
            .ok_or(ApprovalError::UnknownKey)?;
        match record.status {
            ApprovalKeyStatus::Active | ApprovalKeyStatus::Retired => {
                approval.verify(artifact_path, key)
            }
            ApprovalKeyStatus::Revoked => Err(ApprovalError::KeyRevoked),
        }
    }

    pub fn require_active(&self, key_id: &str) -> Result<(), ApprovalError> {
        let record = self
            .keys
            .iter()
            .find(|record| record.key_id == key_id)
            .ok_or(ApprovalError::UnknownKey)?;
        if record.status == ApprovalKeyStatus::Active {
            Ok(())
        } else {
            Err(ApprovalError::KeyNotActive)
        }
    }

    pub fn sign_snapshot(
        &self,
        registry_path: impl AsRef<Path>,
        approver: String,
        key_id: String,
        key: &[u8],
        previous: Option<&RegistrySnapshot>,
    ) -> Result<RegistrySnapshot, ApprovalError> {
        validate_text(&approver)?;
        validate_text(&key_id)?;
        if key.len() < 16 {
            return Err(ApprovalError::WeakKey);
        }
        if let Some(previous) = previous
            && self.version <= previous.registry_version
        {
            return Err(ApprovalError::RegistryVersionMismatch);
        }
        let bytes = std::fs::read(registry_path)?;
        let mut snapshot = RegistrySnapshot {
            snapshot_version: 1,
            registry_version: self.version,
            registry_sha256: hex::encode(Sha256::digest(&bytes)),
            approved_unix_ms: now_unix_ms(),
            approver,
            key_id,
            mac_sha256: String::new(),
            previous_snapshot_sha256: previous.map(|value| value.snapshot_sha256.clone()),
            snapshot_sha256: String::new(),
        };
        snapshot.mac_sha256 = snapshot.compute_mac(key)?;
        snapshot.snapshot_sha256 = snapshot.compute_snapshot_digest()?;
        Ok(snapshot)
    }
}

impl RegistrySnapshot {
    pub fn verify(
        &self,
        registry: &ApprovalKeyRegistry,
        registry_path: impl AsRef<Path>,
        key: &[u8],
        minimum_registry_version: u64,
    ) -> Result<(), ApprovalError> {
        self.verify_integrity(key, minimum_registry_version)?;
        if self.registry_version != registry.version {
            return Err(ApprovalError::RegistryVersionMismatch);
        }
        let bytes = std::fs::read(registry_path)?;
        if hex::encode(Sha256::digest(&bytes)) != self.registry_sha256 {
            return Err(ApprovalError::ArtifactChanged);
        }
        Ok(())
    }

    pub fn verify_integrity(
        &self,
        key: &[u8],
        minimum_registry_version: u64,
    ) -> Result<(), ApprovalError> {
        if self.snapshot_version != 1 {
            return Err(ApprovalError::UnsupportedVersion);
        }
        if self.registry_version < minimum_registry_version {
            return Err(ApprovalError::RegistryRollback);
        }
        if self.compute_snapshot_digest()? != self.snapshot_sha256 {
            return Err(ApprovalError::InvalidSnapshotDigest);
        }
        let key = hmac::Key::new(hmac::HMAC_SHA256, key);
        hmac::verify(
            &key,
            self.mac_material()?.as_bytes(),
            &hex::decode(&self.mac_sha256).map_err(|_| ApprovalError::InvalidMac)?,
        )
        .map_err(|_| ApprovalError::InvalidMac)
    }

    fn compute_mac(&self, key: &[u8]) -> Result<String, ApprovalError> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, key);
        Ok(hex::encode(
            hmac::sign(&key, self.mac_material()?.as_bytes()).as_ref(),
        ))
    }

    fn mac_material(&self) -> Result<String, ApprovalError> {
        Ok(serde_json::to_string(&(
            self.snapshot_version,
            self.registry_version,
            &self.registry_sha256,
            self.approved_unix_ms,
            &self.approver,
            &self.key_id,
            &self.previous_snapshot_sha256,
        ))?)
    }

    fn compute_snapshot_digest(&self) -> Result<String, ApprovalError> {
        Ok(hex::encode(Sha256::digest(serde_json::to_vec(&(
            self.snapshot_version,
            self.registry_version,
            &self.registry_sha256,
            self.approved_unix_ms,
            &self.approver,
            &self.key_id,
            &self.previous_snapshot_sha256,
            &self.mac_sha256,
        ))?)))
    }
}

pub fn verify_snapshot_chain(
    snapshots: &[RegistrySnapshot],
    registry: &ApprovalKeyRegistry,
    registry_path: impl AsRef<Path>,
    key: &[u8],
    minimum_registry_version: u64,
) -> Result<(), ApprovalError> {
    if snapshots.is_empty() {
        return Err(ApprovalError::EmptySnapshotChain);
    }
    let mut previous: Option<&RegistrySnapshot> = None;
    for snapshot in snapshots {
        snapshot.verify_integrity(key, minimum_registry_version)?;
        if let Some(previous) = previous {
            if snapshot.registry_version <= previous.registry_version {
                return Err(ApprovalError::SnapshotChainOrder);
            }
            if snapshot.previous_snapshot_sha256.as_deref()
                != Some(previous.snapshot_sha256.as_str())
            {
                return Err(ApprovalError::SnapshotChainFork);
            }
        } else if snapshot.previous_snapshot_sha256.is_some() {
            return Err(ApprovalError::SnapshotChainFork);
        }
        previous = Some(snapshot);
    }
    snapshots.last().expect("chain is non-empty").verify(
        registry,
        registry_path,
        key,
        minimum_registry_version,
    )?;
    Ok(())
}

pub fn create_trust_anchor(
    first_snapshot: &RegistrySnapshot,
    operator: String,
    admin_key_ids: Vec<String>,
) -> Result<TrustAnchorManifest, ApprovalError> {
    validate_text(&operator)?;
    if admin_key_ids.is_empty() {
        return Err(ApprovalError::InvalidApproval);
    }
    for key_id in &admin_key_ids {
        validate_text(key_id)?;
    }
    if !admin_key_ids.contains(&first_snapshot.key_id) {
        return Err(ApprovalError::AnchorKeyMismatch);
    }
    Ok(TrustAnchorManifest {
        anchor_version: 1,
        first_snapshot_sha256: first_snapshot.snapshot_sha256.clone(),
        operator,
        admin_key_ids,
        created_unix_ms: now_unix_ms(),
    })
}

pub fn verify_snapshot_chain_with_anchor(
    snapshots: &[RegistrySnapshot],
    anchor: &TrustAnchorManifest,
    registry: &ApprovalKeyRegistry,
    registry_path: impl AsRef<Path>,
    key: &[u8],
    minimum_registry_version: u64,
) -> Result<(), ApprovalError> {
    if anchor.anchor_version != 1 || anchor.admin_key_ids.is_empty() {
        return Err(ApprovalError::InvalidApproval);
    }
    let first = snapshots.first().ok_or(ApprovalError::EmptySnapshotChain)?;
    if first.snapshot_sha256 != anchor.first_snapshot_sha256 {
        return Err(ApprovalError::TrustAnchorMismatch);
    }
    if snapshots
        .iter()
        .any(|snapshot| !anchor.admin_key_ids.contains(&snapshot.key_id))
    {
        return Err(ApprovalError::AnchorKeyMismatch);
    }
    verify_snapshot_chain(
        snapshots,
        registry,
        registry_path,
        key,
        minimum_registry_version,
    )
}

impl ApprovalEnvelope {
    pub fn require_kind(&self, expected: ArtifactKind) -> Result<(), ApprovalError> {
        if self.artifact_kind != expected {
            return Err(ApprovalError::UnexpectedArtifactKind);
        }
        Ok(())
    }

    pub fn sign(
        artifact_path: impl AsRef<Path>,
        artifact_kind: ArtifactKind,
        approver: String,
        purpose: String,
        key_id: String,
        key: &[u8],
    ) -> Result<Self, ApprovalError> {
        validate_text(&approver)?;
        validate_text(&purpose)?;
        validate_text(&key_id)?;
        if key.len() < 16 {
            return Err(ApprovalError::WeakKey);
        }
        let bytes = std::fs::read(artifact_path)?;
        let artifact_sha256 = hex::encode(Sha256::digest(&bytes));
        let mut envelope = Self {
            envelope_version: 1,
            artifact_kind,
            artifact_sha256,
            approver,
            approved_unix_ms: now_unix_ms(),
            purpose,
            key_id,
            mac_sha256: String::new(),
        };
        envelope.mac_sha256 = envelope.compute_mac(key)?;
        Ok(envelope)
    }

    pub fn verify(&self, artifact_path: impl AsRef<Path>, key: &[u8]) -> Result<(), ApprovalError> {
        if self.envelope_version != 1 {
            return Err(ApprovalError::UnsupportedVersion);
        }
        if key.len() < 16 {
            return Err(ApprovalError::WeakKey);
        }
        let bytes = std::fs::read(artifact_path)?;
        let artifact_sha256 = hex::encode(Sha256::digest(&bytes));
        if artifact_sha256 != self.artifact_sha256 {
            return Err(ApprovalError::ArtifactChanged);
        }
        let key = hmac::Key::new(hmac::HMAC_SHA256, key);
        hmac::verify(
            &key,
            self.mac_material()?.as_bytes(),
            &hex::decode(&self.mac_sha256).map_err(|_| ApprovalError::InvalidMac)?,
        )
        .map_err(|_| ApprovalError::InvalidMac)
    }

    fn compute_mac(&self, key: &[u8]) -> Result<String, ApprovalError> {
        let key = hmac::Key::new(hmac::HMAC_SHA256, key);
        let tag = hmac::sign(&key, self.mac_material()?.as_bytes());
        Ok(hex::encode(tag.as_ref()))
    }

    fn mac_material(&self) -> Result<String, ApprovalError> {
        Ok(serde_json::to_string(&(
            self.envelope_version,
            self.artifact_kind,
            &self.artifact_sha256,
            &self.approver,
            self.approved_unix_ms,
            &self.purpose,
            &self.key_id,
        ))?)
    }
}

fn validate_text(text: &str) -> Result<(), ApprovalError> {
    if text.trim().is_empty() {
        return Err(ApprovalError::InvalidApproval);
    }
    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("approval JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("approval key must contain at least 16 bytes")]
    WeakKey,
    #[error("unsupported approval envelope version")]
    UnsupportedVersion,
    #[error("approval fields must not be empty")]
    InvalidApproval,
    #[error("approval artifact kind does not match the required use")]
    UnexpectedArtifactKind,
    #[error("artifact bytes changed after approval")]
    ArtifactChanged,
    #[error("approval MAC is invalid")]
    InvalidMac,
    #[error("approval key id already exists")]
    DuplicateKey,
    #[error("approval key id is unknown")]
    UnknownKey,
    #[error("approval key is revoked")]
    KeyRevoked,
    #[error("approval key is not active")]
    KeyNotActive,
    #[error("registry snapshot is below the trusted minimum version")]
    RegistryRollback,
    #[error("registry snapshot version does not match registry")]
    RegistryVersionMismatch,
    #[error("registry snapshot digest is invalid")]
    InvalidSnapshotDigest,
    #[error("registry snapshot chain is empty")]
    EmptySnapshotChain,
    #[error("registry snapshot chain is out of order")]
    SnapshotChainOrder,
    #[error("registry snapshot chain forked")]
    SnapshotChainFork,
    #[error("snapshot chain does not start at the trusted anchor")]
    TrustAnchorMismatch,
    #[error("snapshot key is not authorized by the trust anchor")]
    AnchorKeyMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Vec<u8> {
        b"test-only-approval-key-32-bytes!!".to_vec()
    }

    #[test]
    fn approval_detects_artifact_change_and_bad_mac() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("artifact.json");
        std::fs::write(&artifact, b"{\"a\":1}").unwrap();
        let envelope = ApprovalEnvelope::sign(
            &artifact,
            ArtifactKind::GoldSet,
            "reviewer".into(),
            "calibration".into(),
            "key-1".into(),
            &key(),
        )
        .unwrap();
        envelope.verify(&artifact, &key()).unwrap();

        std::fs::write(&artifact, b"{\"a\":2}").unwrap();
        assert!(matches!(
            envelope.verify(&artifact, &key()),
            Err(ApprovalError::ArtifactChanged)
        ));

        std::fs::write(&artifact, b"{\"a\":1}").unwrap();
        let mut tampered = envelope;
        tampered.approver = "attacker".into();
        assert!(matches!(
            tampered.verify(&artifact, &key()),
            Err(ApprovalError::InvalidMac)
        ));
    }

    #[test]
    fn retired_keys_verify_existing_approvals_but_cannot_sign_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("artifact.json");
        std::fs::write(&artifact, b"{\"a\":1}").unwrap();
        let envelope = ApprovalEnvelope::sign(
            &artifact,
            ArtifactKind::GoldSet,
            "reviewer".into(),
            "calibration".into(),
            "key-1".into(),
            &key(),
        )
        .unwrap();
        let mut registry = ApprovalKeyRegistry::default();
        registry.add_active("key-1".into()).unwrap();
        registry.require_active("key-1").unwrap();
        registry
            .transition("key-1", ApprovalKeyStatus::Retired, "rotated".into())
            .unwrap();
        registry
            .verify_approval(&envelope, &artifact, &key())
            .unwrap();
        assert!(matches!(
            registry.require_active("key-1"),
            Err(ApprovalError::KeyNotActive)
        ));
    }

    #[test]
    fn revoked_keys_cannot_verify_approvals() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("artifact.json");
        std::fs::write(&artifact, b"{\"a\":1}").unwrap();
        let envelope = ApprovalEnvelope::sign(
            &artifact,
            ArtifactKind::GoldSet,
            "reviewer".into(),
            "calibration".into(),
            "key-1".into(),
            &key(),
        )
        .unwrap();
        let mut registry = ApprovalKeyRegistry::default();
        registry.add_active("key-1".into()).unwrap();
        registry
            .transition("key-1", ApprovalKeyStatus::Revoked, "compromised".into())
            .unwrap();
        assert!(matches!(
            registry.verify_approval(&envelope, &artifact, &key()),
            Err(ApprovalError::KeyRevoked)
        ));
    }

    #[test]
    fn signed_registry_snapshot_detects_change_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let mut registry = ApprovalKeyRegistry::default();
        registry.add_active("key-1".into()).unwrap();
        registry.save(&path).unwrap();
        let snapshot = registry
            .sign_snapshot(&path, "admin".into(), "admin-key".into(), &key(), None)
            .unwrap();
        snapshot.verify(&registry, &path, &key(), 1).unwrap();
        assert!(matches!(
            snapshot.verify(&registry, &path, &key(), 2),
            Err(ApprovalError::RegistryRollback)
        ));
        registry.add_active("key-2".into()).unwrap();
        registry.save(&path).unwrap();
        assert!(matches!(
            snapshot.verify(&registry, &path, &key(), 1),
            Err(ApprovalError::ArtifactChanged | ApprovalError::RegistryVersionMismatch)
        ));
    }

    #[test]
    fn snapshot_chain_rejects_fork_and_out_of_order_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let mut registry = ApprovalKeyRegistry::default();
        registry.add_active("key-1".into()).unwrap();
        registry.save(&path).unwrap();
        let first = registry
            .sign_snapshot(&path, "admin".into(), "admin-key".into(), &key(), None)
            .unwrap();
        registry.add_active("key-2".into()).unwrap();
        registry.save(&path).unwrap();
        let second = registry
            .sign_snapshot(
                &path,
                "admin".into(),
                "admin-key".into(),
                &key(),
                Some(&first),
            )
            .unwrap();
        verify_snapshot_chain(
            &[first.clone(), second.clone()],
            &registry,
            &path,
            &key(),
            1,
        )
        .unwrap();
        assert!(matches!(
            verify_snapshot_chain(
                &[second.clone(), first.clone()],
                &registry,
                &path,
                &key(),
                1
            ),
            Err(ApprovalError::SnapshotChainOrder | ApprovalError::SnapshotChainFork)
        ));
        let mut second_fork = second;
        second_fork.previous_snapshot_sha256 = Some("f".repeat(64));
        assert!(matches!(
            verify_snapshot_chain(&[first, second_fork], &registry, &path, &key(), 1),
            Err(ApprovalError::InvalidSnapshotDigest | ApprovalError::SnapshotChainFork)
        ));
    }

    #[test]
    fn trust_anchor_rejects_alternate_history_and_unauthorized_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        let mut registry = ApprovalKeyRegistry::default();
        registry.add_active("key-1".into()).unwrap();
        registry.save(&path).unwrap();
        let first = registry
            .sign_snapshot(&path, "admin".into(), "key-1".into(), &key(), None)
            .unwrap();
        let anchor = create_trust_anchor(
            &first,
            "operator".into(),
            vec!["key-1".into(), "key-2".into()],
        )
        .unwrap();
        verify_snapshot_chain_with_anchor(
            std::slice::from_ref(&first),
            &anchor,
            &registry,
            &path,
            &key(),
            1,
        )
        .unwrap();
        let mut alternate = first.clone();
        alternate.snapshot_sha256 = "a".repeat(64);
        assert!(matches!(
            verify_snapshot_chain_with_anchor(&[alternate], &anchor, &registry, &path, &key(), 1),
            Err(ApprovalError::TrustAnchorMismatch)
        ));
        let mut unauthorized = first;
        unauthorized.key_id = "rogue".into();
        assert!(matches!(
            verify_snapshot_chain_with_anchor(
                &[unauthorized],
                &anchor,
                &registry,
                &path,
                &key(),
                1
            ),
            Err(ApprovalError::AnchorKeyMismatch | ApprovalError::InvalidSnapshotDigest)
        ));
    }
}
