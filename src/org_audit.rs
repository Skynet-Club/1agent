//! Append-only organization audit index.
//!
//! Entries are hash-chained: each entry commits to the previous hash and its
//! own fields. Verification detects edits, reordering and dropped middle
//! entries inside an existing log. It is a local index, not a remote
//! transparency log.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Error)]
pub enum OrgAuditError {
    #[error("organization audit I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("organization audit JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("organization audit entry is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OrgAuditEntry {
    pub seq: u64,
    pub at_ms: u128,
    pub actor: String,
    pub action: String,
    pub scope: String,
    pub decision: String,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OrgAuditVerify {
    pub valid: bool,
    pub entries: usize,
    pub first_invalid_seq: Option<u64>,
    pub reason: String,
}

#[derive(Debug)]
pub struct OrgAuditStore {
    path: PathBuf,
    entries: Vec<OrgAuditEntry>,
}

impl OrgAuditStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, OrgAuditError> {
        let path = path.into();
        let mut entries = Vec::new();
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let complete = if content.ends_with('\n') {
                    content.as_str()
                } else {
                    content.rsplit_once('\n').map_or("", |(head, _)| head)
                };
                for (index, line) in complete.lines().enumerate() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let entry: OrgAuditEntry = serde_json::from_str(line).map_err(|error| {
                        OrgAuditError::Invalid(format!("line {}: {error}", index + 1))
                    })?;
                    validate_entry(&entry)?;
                    entries.push(entry);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self { path, entries })
    }

    pub fn append(
        &mut self,
        at_ms: u128,
        actor: &str,
        action: &str,
        scope: &str,
        decision: &str,
    ) -> Result<&OrgAuditEntry, OrgAuditError> {
        validate_token(actor, "actor")?;
        validate_token(action, "action")?;
        if decision != "allow" && decision != "deny" {
            return Err(OrgAuditError::Invalid(
                "decision must be allow or deny".into(),
            ));
        }
        if scope.chars().any(char::is_control) {
            return Err(OrgAuditError::Invalid(
                "scope contains a control character".into(),
            ));
        }
        let seq = self.entries.last().map_or(0, |entry| entry.seq + 1);
        let prev_hash = self
            .entries
            .last()
            .map_or_else(|| GENESIS_HASH.into(), |entry| entry.hash.clone());
        let hash = entry_hash(seq, at_ms, actor, action, scope, decision, &prev_hash)?;
        let entry = OrgAuditEntry {
            seq,
            at_ms,
            actor: actor.into(),
            action: action.into(),
            scope: scope.into(),
            decision: decision.into(),
            prev_hash,
            hash,
        };
        let line = serde_json::to_vec(&entry)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&line)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        self.entries.push(entry);
        Ok(self.entries.last().expect("entry was just pushed"))
    }

    pub fn verify(&self) -> OrgAuditVerify {
        let mut previous = GENESIS_HASH.to_string();
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.seq != index as u64 {
                return invalid(entry.seq, "sequence is not contiguous", self.entries.len());
            }
            if entry.prev_hash != previous {
                return invalid(
                    entry.seq,
                    "previous hash does not match",
                    self.entries.len(),
                );
            }
            let Ok(hash) = entry_hash(
                entry.seq,
                entry.at_ms,
                &entry.actor,
                &entry.action,
                &entry.scope,
                &entry.decision,
                &entry.prev_hash,
            ) else {
                return invalid(entry.seq, "entry cannot be hashed", self.entries.len());
            };
            if entry.hash != hash {
                return invalid(entry.seq, "entry hash does not match", self.entries.len());
            }
            previous = entry.hash.clone();
        }
        OrgAuditVerify {
            valid: true,
            entries: self.entries.len(),
            first_invalid_seq: None,
            reason: "hash chain is valid".into(),
        }
    }

    pub fn entries(&self) -> &[OrgAuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn invalid(seq: u64, reason: &str, entries: usize) -> OrgAuditVerify {
    OrgAuditVerify {
        valid: false,
        entries,
        first_invalid_seq: Some(seq),
        reason: reason.into(),
    }
}

fn validate_entry(entry: &OrgAuditEntry) -> Result<(), OrgAuditError> {
    validate_token(&entry.actor, "actor")?;
    validate_token(&entry.action, "action")?;
    if entry.decision != "allow" && entry.decision != "deny" {
        return Err(OrgAuditError::Invalid(
            "decision must be allow or deny".into(),
        ));
    }
    if entry.prev_hash.len() != 64 || entry.hash.len() != 64 {
        return Err(OrgAuditError::Invalid(
            "entry hashes must be 32-byte hex".into(),
        ));
    }
    Ok(())
}

fn validate_token(value: &str, field: &str) -> Result<(), OrgAuditError> {
    if value.trim().is_empty() {
        return Err(OrgAuditError::Invalid(format!("{field} is empty")));
    }
    if value.chars().any(char::is_control) {
        return Err(OrgAuditError::Invalid(format!(
            "{field} contains a control character"
        )));
    }
    Ok(())
}

#[derive(Serialize)]
struct EntryBody<'a> {
    seq: u64,
    at_ms: u128,
    actor: &'a str,
    action: &'a str,
    scope: &'a str,
    decision: &'a str,
    prev_hash: &'a str,
}

#[allow(clippy::too_many_arguments)]
fn entry_hash(
    seq: u64,
    at_ms: u128,
    actor: &str,
    action: &str,
    scope: &str,
    decision: &str,
    prev_hash: &str,
) -> Result<String, OrgAuditError> {
    let body = EntryBody {
        seq,
        at_ms,
        actor,
        action,
        scope,
        decision,
        prev_hash,
    };
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&body)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "1agent-org-audit-{}-{name}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn appends_a_valid_chain_and_survives_reopen() {
        let path = temp_store("chain");
        let mut store = OrgAuditStore::open(&path).unwrap();
        store
            .append(1, "alice", "approve", "release", "allow")
            .unwrap();
        store
            .append(2, "bob", "write", "workspace", "deny")
            .unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.verify().valid);
        drop(store);

        let reopened = OrgAuditStore::open(&path).unwrap();
        assert_eq!(reopened.len(), 2);
        assert_eq!(reopened.entries()[1].prev_hash, reopened.entries()[0].hash);
        assert!(reopened.verify().valid);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn tampering_and_reordering_are_detected() {
        let path = temp_store("tamper");
        let mut store = OrgAuditStore::open(&path).unwrap();
        store
            .append(1, "alice", "approve", "release", "allow")
            .unwrap();
        store
            .append(2, "bob", "write", "workspace", "deny")
            .unwrap();
        drop(store);

        let raw = std::fs::read_to_string(&path).unwrap();
        let mut lines: Vec<&str> = raw.lines().collect();
        lines.swap(0, 1);
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
        let store = OrgAuditStore::open(&path).unwrap();
        assert!(!store.verify().valid);

        let tampered = raw.replacen("\"actor\":\"alice\"", "\"actor\":\"mallory\"", 1);
        std::fs::write(&path, tampered).unwrap();
        let store = OrgAuditStore::open(&path).unwrap();
        let report = store.verify();
        assert!(!report.valid);
        assert_eq!(report.first_invalid_seq, Some(0));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn empty_and_partial_logs_are_handled() {
        let path = temp_store("empty");
        let store = OrgAuditStore::open(&path).unwrap();
        assert!(store.is_empty());
        assert!(store.verify().valid);

        std::fs::write(&path, b"{\"seq\":0").unwrap();
        let store = OrgAuditStore::open(&path).unwrap();
        assert!(store.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn invalid_entries_are_rejected() {
        let mut store = OrgAuditStore::open(temp_store("invalid")).unwrap();
        assert!(store.append(1, "", "approve", "release", "allow").is_err());
        assert!(store.append(1, "alice", "", "release", "allow").is_err());
        assert!(
            store
                .append(1, "alice", "approve", "release", "maybe")
                .is_err()
        );
    }
}
