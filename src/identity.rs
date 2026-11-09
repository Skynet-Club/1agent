//! External subject -> actor identity bindings.
//!
//! Authority profiles bind actors to permissions; this directory binds an
//! authenticated external subject (token subject, certificate subject, OIDC
//! `sub`) to the actor id used by the authority layer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error("identity subject is unsafe")]
    UnsafeSubject,
    #[error("identity actor is unsafe")]
    UnsafeActor,
    #[error("identity binding already exists for {0}")]
    AlreadyBound(String),
    #[error("identity subject {0} is not bound")]
    UnknownSubject(String),
    #[error("identity binding for {0} is expired")]
    Expired(String),
    #[error("identity binding for {0} is revoked")]
    Revoked(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IdentityBinding {
    pub subject: String,
    pub actor_id: String,
    pub issuer: String,
    pub issued_ms: u128,
    pub expires_ms: u128,
    #[serde(default)]
    pub revoked_ms: Option<u128>,
}

#[derive(Debug, Default)]
pub struct IdentityDirectory {
    bindings: BTreeMap<String, IdentityBinding>,
}

impl IdentityDirectory {
    pub fn bind(&mut self, binding: IdentityBinding) -> Result<(), IdentityError> {
        if !safe_id(&binding.subject) {
            return Err(IdentityError::UnsafeSubject);
        }
        if !safe_id(&binding.actor_id) {
            return Err(IdentityError::UnsafeActor);
        }
        if binding.expires_ms <= binding.issued_ms {
            return Err(IdentityError::Expired(binding.subject));
        }
        if self.bindings.contains_key(&binding.subject) {
            return Err(IdentityError::AlreadyBound(binding.subject));
        }
        self.bindings.insert(binding.subject.clone(), binding);
        Ok(())
    }

    pub fn resolve(&self, subject: &str, now_ms: u128) -> Result<&IdentityBinding, IdentityError> {
        let binding = self
            .bindings
            .get(subject)
            .ok_or_else(|| IdentityError::UnknownSubject(subject.into()))?;
        if binding.revoked_ms.is_some_and(|revoked| revoked <= now_ms) {
            return Err(IdentityError::Revoked(subject.into()));
        }
        if binding.expires_ms <= now_ms {
            return Err(IdentityError::Expired(subject.into()));
        }
        Ok(binding)
    }

    pub fn revoke(&mut self, subject: &str, now_ms: u128) -> Result<(), IdentityError> {
        let binding = self
            .bindings
            .get_mut(subject)
            .ok_or_else(|| IdentityError::UnknownSubject(subject.into()))?;
        binding.revoked_ms = Some(now_ms);
        Ok(())
    }

    pub fn get(&self, subject: &str) -> Option<&IdentityBinding> {
        self.bindings.get(subject)
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'@')
        })
        && !value.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(subject: &str, actor: &str, expires_ms: u128) -> IdentityBinding {
        IdentityBinding {
            subject: subject.into(),
            actor_id: actor.into(),
            issuer: "test-idp".into(),
            issued_ms: 1,
            expires_ms,
            revoked_ms: None,
        }
    }

    #[test]
    fn resolves_live_binding_and_rejects_expiry_revocation() {
        let mut directory = IdentityDirectory::default();
        directory
            .bind(binding("oidc:alice", "owner-1", 100))
            .unwrap();
        assert_eq!(
            directory.resolve("oidc:alice", 10).unwrap().actor_id,
            "owner-1"
        );
        assert_eq!(
            directory.resolve("oidc:alice", 100).unwrap_err(),
            IdentityError::Expired("oidc:alice".into())
        );
        directory.revoke("oidc:alice", 20).unwrap();
        assert_eq!(
            directory.resolve("oidc:alice", 30).unwrap_err(),
            IdentityError::Revoked("oidc:alice".into())
        );
    }

    #[test]
    fn duplicate_and_unsafe_bindings_fail_closed() {
        let mut directory = IdentityDirectory::default();
        directory
            .bind(binding("subject-1", "actor-1", 100))
            .unwrap();
        assert_eq!(
            directory
                .bind(binding("subject-1", "actor-2", 100))
                .unwrap_err(),
            IdentityError::AlreadyBound("subject-1".into())
        );
        assert_eq!(
            directory
                .bind(binding("bad/subject", "actor-1", 100))
                .unwrap_err(),
            IdentityError::UnsafeSubject
        );
        assert_eq!(
            directory
                .bind(binding("subject-2", "bad/actor", 100))
                .unwrap_err(),
            IdentityError::UnsafeActor
        );
    }
}
