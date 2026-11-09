//! Append-only journal for skill registry events.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::skill::{SkillError, SkillEvent, SkillStore};

#[derive(Debug, Error)]
pub enum SkillJournalError {
    #[error("skill journal io: {0}")]
    Io(#[from] std::io::Error),
    #[error("skill journal json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("skill journal verify: {0}")]
    Verify(#[from] SkillError),
    #[error("skill journal event sequence is not contiguous")]
    NonContiguousSequence,
    #[error("skill journal was opened read-only")]
    ReadOnly,
    #[error(
        "skill journal was modified by another writer (expected {expected_bytes} bytes, found {actual_bytes})"
    )]
    ConcurrentWrite {
        expected_bytes: u64,
        actual_bytes: u64,
    },
}

#[derive(Debug, Clone)]
pub struct SkillJournalStore {
    path: PathBuf,
    next_seq: u64,
    persisted_bytes: u64,
    writable: bool,
    events: Vec<SkillEvent>,
}

impl SkillJournalStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SkillJournalError> {
        Self::open_with(path.into(), true)
    }

    pub fn open_read_only(path: impl Into<PathBuf>) -> Result<Self, SkillJournalError> {
        Self::open_with(path.into(), false)
    }

    fn open_with(path: PathBuf, repair: bool) -> Result<Self, SkillJournalError> {
        let _lock = if repair {
            Some(crate::jsonl::FileLock::exclusive(&path)?)
        } else {
            None
        };
        if repair {
            crate::jsonl::repair_torn_tail(&path)?;
        }
        let mut events = Vec::new();
        if path.exists() {
            let file = File::open(&path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                events.push(serde_json::from_str::<SkillEvent>(&line)?);
            }
        }
        SkillStore::verify_events(&events)?;
        let next_seq = events.last().map(|event| event.seq).unwrap_or(0);
        let persisted_bytes = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        drop(_lock);
        Ok(Self {
            path,
            next_seq,
            persisted_bytes,
            writable: repair,
            events,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn events(&self) -> &[SkillEvent] {
        &self.events
    }

    pub fn persist(&mut self, store: &SkillStore) -> Result<usize, SkillJournalError> {
        if !self.writable {
            return Err(SkillJournalError::ReadOnly);
        }
        let new_events: Vec<&SkillEvent> = store
            .events()
            .iter()
            .filter(|event| event.seq > self.next_seq)
            .collect();
        for (index, event) in new_events.iter().enumerate() {
            let expected = self.next_seq + 1 + index as u64;
            if event.seq != expected {
                return Err(SkillJournalError::NonContiguousSequence);
            }
        }
        if new_events.is_empty() {
            return Ok(0);
        }
        let count = new_events.len();
        let mut buffer = String::new();
        for event in &new_events {
            buffer.push_str(&serde_json::to_string(event)?);
            buffer.push('\n');
        }
        self.persisted_bytes =
            crate::jsonl::append_batch_locked(&self.path, self.persisted_bytes, &buffer).map_err(
                |error| match error {
                    crate::jsonl::LockedAppendError::Io(error) => SkillJournalError::Io(error),
                    crate::jsonl::LockedAppendError::ConcurrentWrite {
                        expected_bytes,
                        actual_bytes,
                    } => SkillJournalError::ConcurrentWrite {
                        expected_bytes,
                        actual_bytes,
                    },
                },
            )?;
        for event in new_events {
            self.next_seq = event.seq;
            self.events.push(event.clone());
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{SkillManifest, SkillRisk};

    fn manifest(version: &str) -> SkillManifest {
        SkillManifest {
            name: "release".into(),
            description: "release process".into(),
            version: version.into(),
            platforms: vec!["linux".into()],
            scope: "repo:1-agent".into(),
            requires_tools: vec!["shell".into()],
            risk: SkillRisk::Medium,
            body: "## Procedure\n1. verify".into(),
            content_sha256: String::new(),
        }
    }

    #[test]
    fn journal_persists_and_replays_registry_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        store.activate("release", "1.0.0", 11).unwrap();
        let mut journal = SkillJournalStore::open(&path).unwrap();
        assert_eq!(journal.persist(&store).unwrap(), 2);
        assert_eq!(journal.persist(&store).unwrap(), 0);

        let reopened = SkillJournalStore::open(&path).unwrap();
        let replayed = SkillStore::replay(reopened.events()).unwrap();
        assert_eq!(replayed.active("release").unwrap().version, "1.0.0");
        assert_eq!(replayed.events().len(), 2);
    }

    #[test]
    fn read_only_journal_cannot_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut journal = SkillJournalStore::open_read_only(&path).unwrap();
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        assert!(matches!(
            journal.persist(&store).unwrap_err(),
            SkillJournalError::ReadOnly
        ));
    }

    #[test]
    fn stale_writer_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut first = SkillJournalStore::open(&path).unwrap();
        let mut second = SkillJournalStore::open(&path).unwrap();
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        assert_eq!(first.persist(&store).unwrap(), 1);
        let error = second.persist(&store).unwrap_err();
        assert!(matches!(
            error,
            SkillJournalError::ConcurrentWrite {
                expected_bytes: 0,
                actual_bytes: _
            }
        ));
    }

    #[test]
    fn tampered_skill_journal_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skill-journal.jsonl");
        let mut store = SkillStore::default();
        store.register(manifest("1.0.0"), 10).unwrap();
        let mut journal = SkillJournalStore::open(&path).unwrap();
        journal.persist(&store).unwrap();
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"release\"", "\"tampered\"");
        std::fs::write(&path, text).unwrap();
        assert!(SkillJournalStore::open(&path).is_err());
    }
}
