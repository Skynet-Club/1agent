//! Reproducible lockfile for active skill versions.
//!
//! The skill event store is the source of truth; the lockfile pins the active
//! name/version/content hash set so an operator can diff two machines or fail
//! a deploy when an active skill changed. It is not a signature.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::skill::{SkillRisk, SkillStatus, SkillStore};

pub const SKILL_LOCK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillLockEntry {
    pub name: String,
    pub version: String,
    pub content_sha256: String,
    pub risk: SkillRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillLock {
    pub schema_version: u32,
    pub skills: Vec<SkillLockEntry>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillLockError {
    #[error("unsupported skill lock schema {0}")]
    UnsupportedSchema(u32),
    #[error("skill lock contains a duplicate name {0}")]
    DuplicateName(String),
    #[error("skill lock entry {name} has an invalid content hash")]
    InvalidHash { name: String },
    #[error("active skill {name}@{version} is missing from the lock")]
    Missing { name: String, version: String },
    #[error("skill lock pins {name}@{version} but {actual} is active")]
    VersionMismatch {
        name: String,
        version: String,
        actual: String,
    },
    #[error("skill lock hash for {name}@{version} does not match the store")]
    HashMismatch { name: String, version: String },
    #[error("skill {name}@{version} is locked but not active")]
    NotActive { name: String, version: String },
}

impl SkillLock {
    pub fn from_store(store: &SkillStore) -> Self {
        let mut skills = store
            .list()
            .into_iter()
            .filter(|summary| summary.status == SkillStatus::Active)
            .filter_map(|summary| {
                store.active(&summary.name).map(|manifest| SkillLockEntry {
                    name: manifest.name.clone(),
                    version: manifest.version.clone(),
                    content_sha256: manifest.content_sha256.clone(),
                    risk: manifest.risk,
                })
            })
            .collect::<Vec<_>>();
        skills.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.version.cmp(&right.version))
        });
        Self {
            schema_version: SKILL_LOCK_SCHEMA_VERSION,
            skills,
        }
    }

    pub fn validate(&self) -> Result<(), SkillLockError> {
        if self.schema_version != SKILL_LOCK_SCHEMA_VERSION {
            return Err(SkillLockError::UnsupportedSchema(self.schema_version));
        }
        let mut seen = std::collections::BTreeSet::new();
        for entry in &self.skills {
            if !seen.insert(entry.name.as_str()) {
                return Err(SkillLockError::DuplicateName(entry.name.clone()));
            }
            if entry.content_sha256.len() != 64
                || !entry
                    .content_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(SkillLockError::InvalidHash {
                    name: entry.name.clone(),
                });
            }
        }
        Ok(())
    }

    /// Exact two-way match against the active set. An extra active skill is a
    /// failure, not a warning, so a deploy cannot silently add an unpinned
    /// skill.
    pub fn verify(&self, store: &SkillStore) -> Result<(), SkillLockError> {
        self.validate()?;
        for entry in &self.skills {
            let Some(manifest) = store.active(&entry.name) else {
                return Err(SkillLockError::NotActive {
                    name: entry.name.clone(),
                    version: entry.version.clone(),
                });
            };
            if manifest.version != entry.version {
                return Err(SkillLockError::VersionMismatch {
                    name: entry.name.clone(),
                    version: entry.version.clone(),
                    actual: manifest.version.clone(),
                });
            }
            if manifest.content_sha256 != entry.content_sha256 {
                return Err(SkillLockError::HashMismatch {
                    name: entry.name.clone(),
                    version: entry.version.clone(),
                });
            }
        }
        let actual = Self::from_store(store);
        for entry in &actual.skills {
            if !self.skills.iter().any(|locked| locked.name == entry.name) {
                return Err(SkillLockError::Missing {
                    name: entry.name.clone(),
                    version: entry.version.clone(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillManifest;

    fn manifest(name: &str, version: &str, body: &str) -> SkillManifest {
        SkillManifest {
            name: name.into(),
            description: "test skill".into(),
            version: version.into(),
            platforms: vec![],
            scope: "global".into(),
            requires_tools: vec![],
            risk: SkillRisk::Low,
            body: body.into(),
            content_sha256: String::new(),
        }
    }

    #[test]
    fn lock_pins_active_versions_and_detects_drift() {
        let mut store = SkillStore::default();
        store
            .register(manifest("alpha", "1.0.0", "alpha body"), 1)
            .unwrap();
        store
            .register(manifest("beta", "1.0.0", "beta body"), 2)
            .unwrap();
        store.activate("alpha", "1.0.0", 3).unwrap();
        let lock = SkillLock::from_store(&store);
        assert_eq!(lock.skills.len(), 1);
        assert_eq!(lock.skills[0].name, "alpha");
        lock.verify(&store).unwrap();

        store.activate("beta", "1.0.0", 4).unwrap();
        assert_eq!(
            lock.verify(&store).unwrap_err(),
            SkillLockError::Missing {
                name: "beta".into(),
                version: "1.0.0".into()
            }
        );

        store
            .register(manifest("alpha", "1.1.0", "new body"), 5)
            .unwrap();
        store.activate("alpha", "1.1.0", 6).unwrap();
        assert!(matches!(
            lock.verify(&store).unwrap_err(),
            SkillLockError::VersionMismatch { .. }
        ));
    }

    #[test]
    fn lock_rejects_duplicates_bad_hashes_and_inactive_entries() {
        let mut lock = SkillLock {
            schema_version: SKILL_LOCK_SCHEMA_VERSION,
            skills: vec![SkillLockEntry {
                name: "alpha".into(),
                version: "1.0.0".into(),
                content_sha256: "0".repeat(64),
                risk: SkillRisk::Low,
            }],
        };
        lock.validate().unwrap();
        lock.skills.push(lock.skills[0].clone());
        assert!(matches!(
            lock.validate().unwrap_err(),
            SkillLockError::DuplicateName(_)
        ));
        lock.skills.pop();
        lock.skills[0].content_sha256 = "xyz".into();
        assert!(matches!(
            lock.validate().unwrap_err(),
            SkillLockError::InvalidHash { .. }
        ));
        let store = SkillStore::default();
        let lock = SkillLock {
            schema_version: SKILL_LOCK_SCHEMA_VERSION,
            skills: vec![SkillLockEntry {
                name: "alpha".into(),
                version: "1.0.0".into(),
                content_sha256: "0".repeat(64),
                risk: SkillRisk::Low,
            }],
        };
        assert!(matches!(
            lock.verify(&store).unwrap_err(),
            SkillLockError::NotActive { .. }
        ));
    }
}
