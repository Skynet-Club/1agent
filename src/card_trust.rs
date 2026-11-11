//! Trust anchors for signed Agent Cards.
//!
//! The store is a local file of trusted Ed25519 public keys. Verification
//! requires the card's embedded key to be trusted and not revoked, then
//! delegates the signature check to `agent_card::verify_card`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::agent_card::{self, AgentCard, AgentCardError};

#[derive(Debug, Error)]
pub enum CardTrustError {
    #[error("card trust store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("card trust store JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("public key must be 32-byte hex")]
    InvalidKey,
    #[error("public key is revoked")]
    Revoked,
    #[error("public key is not trusted")]
    UnknownKey,
    #[error("agent card rejected: {0}")]
    Card(#[from] AgentCardError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustedKey {
    pub public_key: String,
    #[serde(default)]
    pub note: String,
    pub added_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CardTrustStore {
    pub keys: Vec<TrustedKey>,
}

impl CardTrustStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, CardTrustError> {
        match std::fs::read(path.into()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn add(
        &mut self,
        path: &std::path::Path,
        public_key: &str,
        note: &str,
        now_ms: u128,
    ) -> Result<&TrustedKey, CardTrustError> {
        let public_key = normalize_key(public_key)?;
        if let Some(existing) = self
            .keys
            .iter()
            .position(|key| key.public_key == public_key)
        {
            if self.keys[existing].revoked_at_ms.is_some() {
                return Err(CardTrustError::Revoked);
            }
            return Ok(&self.keys[existing]);
        }
        self.keys.push(TrustedKey {
            public_key,
            note: note.into(),
            added_at_ms: now_ms,
            revoked_at_ms: None,
            revoked_reason: None,
        });
        self.save(path)?;
        Ok(self.keys.last().expect("key was just pushed"))
    }

    pub fn revoke(
        &mut self,
        path: &std::path::Path,
        public_key: &str,
        reason: &str,
        now_ms: u128,
    ) -> Result<&TrustedKey, CardTrustError> {
        let public_key = normalize_key(public_key)?;
        let Some(key) = self
            .keys
            .iter_mut()
            .find(|key| key.public_key == public_key)
        else {
            return Err(CardTrustError::UnknownKey);
        };
        if key.revoked_at_ms.is_some() {
            return Err(CardTrustError::Revoked);
        }
        key.revoked_at_ms = Some(now_ms);
        key.revoked_reason = if reason.is_empty() {
            None
        } else {
            Some(reason.into())
        };
        self.save(path)?;
        Ok(self
            .keys
            .iter()
            .find(|key| key.public_key == public_key)
            .expect("key exists"))
    }

    pub fn verify(&self, card: &AgentCard) -> Result<(), CardTrustError> {
        let Some(signature) = &card.signature else {
            return Err(CardTrustError::Card(AgentCardError::Unsigned));
        };
        let public_key = normalize_key(&signature.public_key).map_err(|_| {
            CardTrustError::Card(AgentCardError::InvalidSignature(
                "public key is not 32-byte hex".into(),
            ))
        })?;
        let Some(trusted) = self.keys.iter().find(|key| key.public_key == public_key) else {
            return Err(CardTrustError::UnknownKey);
        };
        if trusted.revoked_at_ms.is_some() {
            return Err(CardTrustError::Revoked);
        }
        agent_card::verify_card(card, &public_key)?;
        Ok(())
    }

    fn save(&self, path: &std::path::Path) -> Result<(), CardTrustError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        let bytes = serde_json::to_vec_pretty(self)?;
        {
            use std::io::Write;
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}

fn normalize_key(value: &str) -> Result<String, CardTrustError> {
    let bytes = hex::decode(value.trim()).map_err(|_| CardTrustError::InvalidKey)?;
    if bytes.len() != 32 {
        return Err(CardTrustError::InvalidKey);
    }
    Ok(hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_card::{build_card, generate_signing_key, sign_card};

    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "1agent-card-trust-{}-{name}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn trusted_signed_cards_verify_and_survive_reopen() {
        let path = temp_store("trusted");
        let (private, public) = generate_signing_key().unwrap();
        let mut card = build_card("0.97.0");
        sign_card(&mut card, &private).unwrap();

        let mut store = CardTrustStore::open(&path).unwrap();
        store.add(&path, &public, "release", 1).unwrap();
        store.verify(&card).unwrap();
        drop(store);

        let reopened = CardTrustStore::open(&path).unwrap();
        reopened.verify(&card).unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unknown_revoked_and_tampered_cards_are_rejected() {
        let path = temp_store("reject");
        let (private, public) = generate_signing_key().unwrap();
        let (_, other) = generate_signing_key().unwrap();
        let mut card = build_card("0.97.0");
        sign_card(&mut card, &private).unwrap();

        let mut store = CardTrustStore::open(&path).unwrap();
        store.verify(&card).unwrap_err();
        store.add(&path, &other, "", 1).unwrap();
        assert!(matches!(
            store.verify(&card),
            Err(CardTrustError::UnknownKey)
        ));

        store.add(&path, &public, "", 1).unwrap();
        store.verify(&card).unwrap();

        let mut tampered = card.clone();
        tampered.version = "9.9.9".into();
        assert!(matches!(
            store.verify(&tampered),
            Err(CardTrustError::Card(_))
        ));

        store.revoke(&path, &public, "retired", 2).unwrap();
        assert!(matches!(store.verify(&card), Err(CardTrustError::Revoked)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn invalid_keys_and_unknown_revocations_are_rejected() {
        let path = temp_store("invalid");
        let mut store = CardTrustStore::open(&path).unwrap();
        assert!(matches!(
            store.add(&path, "not-hex", "", 1),
            Err(CardTrustError::InvalidKey)
        ));
        assert!(matches!(
            store.revoke(&path, &"00".repeat(32), "", 1),
            Err(CardTrustError::UnknownKey)
        ));
    }

    #[test]
    fn duplicate_add_is_idempotent_until_revoked() {
        let path = temp_store("duplicate");
        let (_, public) = generate_signing_key().unwrap();
        let mut store = CardTrustStore::open(&path).unwrap();
        store.add(&path, &public, "first", 1).unwrap();
        let same = store.add(&path, &public, "second", 2).unwrap();
        assert_eq!(same.note, "first");
        assert_eq!(store.keys.len(), 1);
        store.revoke(&path, &public, "gone", 3).unwrap();
        assert!(matches!(
            store.add(&path, &public, "", 4),
            Err(CardTrustError::Revoked)
        ));
        std::fs::remove_file(&path).ok();
    }
}
