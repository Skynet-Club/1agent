//! Durable idempotency records for external writes.
//!
//! Each record binds one idempotency key to the request fingerprint and the
//! response that was produced. Records are appended as JSON lines and synced
//! before they are acknowledged, so a restarted process can replay the same
//! response instead of repeating the external write.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_TOKEN_CHARS: usize = 512;

#[derive(Debug, Error)]
pub enum IdempotencyError {
    #[error("idempotency store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("idempotency record is invalid: {0}")]
    Invalid(String),
    #[error("idempotency key {0} was reused with a different request")]
    Conflict(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IdempotencyRecord {
    pub key: String,
    pub fingerprint: String,
    pub response: serde_json::Value,
}

#[derive(Debug)]
pub struct IdempotencyStore {
    path: PathBuf,
    entries: BTreeMap<String, IdempotencyRecord>,
}

impl IdempotencyStore {
    /// Loads an existing store. A missing file is an empty store.
    /// A final line without a newline is treated as an interrupted append and
    /// is ignored.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, IdempotencyError> {
        let path = path.into();
        let mut entries: BTreeMap<String, IdempotencyRecord> = BTreeMap::new();
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
                    let record: IdempotencyRecord =
                        serde_json::from_str(line).map_err(|error| {
                            IdempotencyError::Invalid(format!("line {}: {error}", index + 1))
                        })?;
                    validate(&record.key, "key")?;
                    validate(&record.fingerprint, "fingerprint")?;
                    if let Some(existing) = entries.get(&record.key)
                        && existing.fingerprint != record.fingerprint
                    {
                        return Err(IdempotencyError::Conflict(record.key));
                    }
                    entries.insert(record.key.clone(), record);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self { path, entries })
    }

    pub fn replay(
        &self,
        key: &str,
        fingerprint: &str,
    ) -> Result<Option<&serde_json::Value>, IdempotencyError> {
        validate(key, "key")?;
        validate(fingerprint, "fingerprint")?;
        let Some(record) = self.entries.get(key) else {
            return Ok(None);
        };
        if record.fingerprint != fingerprint {
            return Err(IdempotencyError::Conflict(key.into()));
        }
        Ok(Some(&record.response))
    }

    /// Appends a record and syncs it before returning. Re-recording the same
    /// key and fingerprint returns the existing record without another append.
    pub fn record(
        &mut self,
        key: &str,
        fingerprint: &str,
        response: serde_json::Value,
    ) -> Result<&IdempotencyRecord, IdempotencyError> {
        validate(key, "key")?;
        validate(fingerprint, "fingerprint")?;
        if let Some(existing) = self.entries.get(key) {
            if existing.fingerprint != fingerprint {
                return Err(IdempotencyError::Conflict(key.into()));
            }
        } else {
            let record = IdempotencyRecord {
                key: key.into(),
                fingerprint: fingerprint.into(),
                response,
            };
            let line = serde_json::to_vec(&record)
                .map_err(|error| IdempotencyError::Invalid(format!("response: {error}")))?;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            file.write_all(&line)?;
            file.write_all(b"\n")?;
            file.sync_data()?;
            self.entries.insert(key.into(), record);
        }
        Ok(self.entries.get(key).expect("record is present"))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn validate(value: &str, field: &str) -> Result<(), IdempotencyError> {
    if value.trim().is_empty() {
        return Err(IdempotencyError::Invalid(format!("{field} is empty")));
    }
    if value.chars().count() > MAX_TOKEN_CHARS {
        return Err(IdempotencyError::Invalid(format!(
            "{field} exceeds {MAX_TOKEN_CHARS} characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(IdempotencyError::Invalid(format!(
            "{field} contains a control character"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "1agent-idem-{}-{name}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn missing_store_is_empty() {
        let store = IdempotencyStore::open(temp_store("missing")).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn records_survive_a_restart() {
        let path = temp_store("restart");
        let mut store = IdempotencyStore::open(&path).unwrap();
        store
            .record("idem-1", "fp-1", serde_json::json!({"ok": true}))
            .unwrap();
        drop(store);

        let reopened = IdempotencyStore::open(&path).unwrap();
        assert_eq!(
            reopened.replay("idem-1", "fp-1").unwrap(),
            Some(&serde_json::json!({"ok": true}))
        );
        assert_eq!(reopened.replay("idem-2", "fp-2").unwrap(), None);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn same_key_and_fingerprint_does_not_append_twice() {
        let path = temp_store("same");
        let mut store = IdempotencyStore::open(&path).unwrap();
        store
            .record("idem-1", "fp-1", serde_json::json!(1))
            .unwrap();
        store
            .record("idem-1", "fp-1", serde_json::json!(1))
            .unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn key_reuse_with_another_fingerprint_conflicts() {
        let path = temp_store("conflict");
        let mut store = IdempotencyStore::open(&path).unwrap();
        store
            .record("idem-1", "fp-1", serde_json::json!(1))
            .unwrap();
        assert!(matches!(
            store.record("idem-1", "fp-2", serde_json::json!(2)),
            Err(IdempotencyError::Conflict(_))
        ));
        assert!(matches!(
            store.replay("idem-1", "fp-2"),
            Err(IdempotencyError::Conflict(_))
        ));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn interrupted_append_is_ignored() {
        let path = temp_store("partial");
        std::fs::write(&path, "{\"key\":\"broken\"").unwrap();
        let store = IdempotencyStore::open(&path).unwrap();
        assert!(store.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn empty_and_control_tokens_are_rejected() {
        let mut store = IdempotencyStore::open(temp_store("tokens")).unwrap();
        assert!(store.record("", "fp", serde_json::json!(1)).is_err());
        assert!(store.record("idem", "fp\nx", serde_json::json!(1)).is_err());
    }
}
