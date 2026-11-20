//! Idempotency ledger for completed subagent spawns.
//!
//! A tool call id is the natural retry key: the runtime may replay the same
//! call after a provider retry. The ledger binds `(parent_run_id, call_id)` to
//! one completed `ToolOutcome`, and an exclusive per-call lock stops two
//! concurrent attempts from starting two children.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::ToolOutcome;
use crate::idempotency::IdempotencyStore;

const MAX_KEY_CHARS: usize = 512;

#[derive(Debug)]
pub struct SpawnLedger {
    store_path: PathBuf,
    key: String,
    _lock: File,
}

impl SpawnLedger {
    pub fn new(cache_root: &Path, run_id: &str, call_id: &str) -> Result<Self, String> {
        let key = format!("{run_id}:{call_id}");
        if key.chars().count() > MAX_KEY_CHARS
            || key.is_empty()
            || key.chars().any(char::is_control)
        {
            return Err(
                "spawn idempotency key is empty, too long or contains control characters".into(),
            );
        }
        let directory = cache_root.join("spawn-idempotency");
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
        let digest = hex::encode(Sha256::digest(key.as_bytes()));
        let lock_path = directory.join(format!("{digest}.lock"));
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| format!("cannot open {}: {error}", lock_path.display()))?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => {
                "spawn already in progress for this call id".to_string()
            }
            std::fs::TryLockError::Error(error) => {
                format!("cannot lock {}: {error}", lock_path.display())
            }
        })?;
        Ok(Self {
            store_path: directory.join(format!("{digest}.jsonl")),
            key,
            _lock: lock,
        })
    }

    pub fn replay(&self, fingerprint: &str) -> Result<Option<ToolOutcome>, String> {
        let store = IdempotencyStore::open(&self.store_path).map_err(|error| error.to_string())?;
        let Some(response) = store
            .replay(&self.key, fingerprint)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        serde_json::from_value(response.clone())
            .map(Some)
            .map_err(|error| format!("cached spawn result is invalid: {error}"))
    }

    pub fn record(&self, fingerprint: &str, outcome: &ToolOutcome) -> Result<(), String> {
        let mut store =
            IdempotencyStore::open(&self.store_path).map_err(|error| error.to_string())?;
        let response =
            serde_json::to_value(outcome).map_err(|error| format!("spawn result: {error}"))?;
        store
            .record(&self.key, fingerprint, response)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

/// Hash the request fields that make a spawn unique. A retry with the same call
/// id but different arguments is a conflict, not a cache hit.
pub fn fingerprint(value: &serde_json::Value) -> String {
    hex::encode(Sha256::digest(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn outcome() -> ToolOutcome {
        ToolOutcome {
            summary: "done".into(),
            content: "{\"ok\":true}".into(),
            metadata: BTreeMap::from([("status".to_string(), "completed".to_string())]),
        }
    }

    #[test]
    fn replay_returns_the_recorded_outcome_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = SpawnLedger::new(dir.path(), "run-1", "call-1").unwrap();
        assert!(ledger.replay("fp-1").unwrap().is_none());
        ledger.record("fp-1", &outcome()).unwrap();
        drop(ledger);

        let reopened = SpawnLedger::new(dir.path(), "run-1", "call-1").unwrap();
        let cached = reopened.replay("fp-1").unwrap().unwrap();
        assert_eq!(cached.summary, "done");
        assert_eq!(
            cached.metadata.get("status").map(String::as_str),
            Some("completed")
        );
        assert!(
            reopened.replay("fp-2").is_err(),
            "different fingerprint must conflict"
        );
    }

    #[test]
    fn a_second_in_flight_handle_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let _first = SpawnLedger::new(dir.path(), "run-1", "call-1").unwrap();
        let error = SpawnLedger::new(dir.path(), "run-1", "call-1").unwrap_err();
        assert!(error.contains("already in progress"), "{error}");
    }

    #[test]
    fn fingerprints_are_stable_and_request_sensitive() {
        let first = fingerprint(&serde_json::json!({"task": "a", "steps": 8}));
        let same = fingerprint(&serde_json::json!({"steps": 8, "task": "a"}));
        let different = fingerprint(&serde_json::json!({"task": "b", "steps": 8}));
        assert_eq!(first, same);
        assert_ne!(first, different);
    }
}
