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

impl ApprovalEnvelope {
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
    #[error("artifact bytes changed after approval")]
    ArtifactChanged,
    #[error("approval MAC is invalid")]
    InvalidMac,
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
}
