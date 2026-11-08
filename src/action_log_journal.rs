//! Append-only JSONL journal for the security action log.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::action_log::{ActionLog, ActionLogEntry, ActionLogError};

#[derive(Debug, Error)]
pub enum ActionLogJournalError {
    #[error("action log io: {0}")]
    Io(#[from] std::io::Error),
    #[error("action log json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("action log verify: {0}")]
    Verify(#[from] ActionLogError),
    #[error("action log sequence is not contiguous")]
    NonContiguousSequence,
    #[error(
        "action log was modified by another writer (expected {expected_bytes} bytes, found {actual_bytes})"
    )]
    ConcurrentWrite {
        expected_bytes: u64,
        actual_bytes: u64,
    },
}

#[derive(Debug, Clone)]
pub struct ActionLogJournal {
    path: PathBuf,
    next_seq: u64,
    persisted_bytes: u64,
    log: ActionLog,
}

impl ActionLogJournal {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ActionLogJournalError> {
        let path = path.into();
        let _lock = crate::jsonl::FileLock::exclusive(&path)?;
        crate::jsonl::repair_torn_tail(&path)?;
        let mut entries = Vec::new();
        if path.exists() {
            let file = File::open(&path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                entries.push(serde_json::from_str::<ActionLogEntry>(&line)?);
            }
        }
        let log = ActionLog::from_entries(entries)?;
        let next_seq = log.entries().last().map(|entry| entry.seq).unwrap_or(0);
        let persisted_bytes = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        drop(_lock);
        Ok(Self {
            path,
            next_seq,
            persisted_bytes,
            log,
        })
    }

    pub fn log(&self) -> &ActionLog {
        &self.log
    }

    pub fn log_mut(&mut self) -> &mut ActionLog {
        &mut self.log
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload(&mut self) -> Result<(), ActionLogJournalError> {
        let path = self.path.clone();
        *self = Self::open(path)?;
        Ok(())
    }

    pub fn persist(&mut self) -> Result<usize, ActionLogJournalError> {
        let new_entries: Vec<&ActionLogEntry> = self
            .log
            .entries()
            .iter()
            .filter(|entry| entry.seq > self.next_seq)
            .collect();
        for (index, entry) in new_entries.iter().enumerate() {
            if entry.seq != self.next_seq + 1 + index as u64 {
                return Err(ActionLogJournalError::NonContiguousSequence);
            }
        }
        if new_entries.is_empty() {
            return Ok(0);
        }
        let count = new_entries.len();
        let mut buffer = String::new();
        for entry in &new_entries {
            buffer.push_str(&serde_json::to_string(entry)?);
            buffer.push('\n');
        }
        self.persisted_bytes =
            crate::jsonl::append_batch_locked(&self.path, self.persisted_bytes, &buffer).map_err(
                |error| match error {
                    crate::jsonl::LockedAppendError::Io(error) => ActionLogJournalError::Io(error),
                    crate::jsonl::LockedAppendError::ConcurrentWrite {
                        expected_bytes,
                        actual_bytes,
                    } => ActionLogJournalError::ConcurrentWrite {
                        expected_bytes,
                        actual_bytes,
                    },
                },
            )?;
        self.next_seq = self.log.entries().last().map(|e| e.seq).unwrap_or(0);
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_log::ActionKind;

    #[test]
    fn action_log_persists_and_replays() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actions.jsonl");
        let mut journal = ActionLogJournal::open(&path).unwrap();
        journal
            .log_mut()
            .record("owner", ActionKind::Approve, "ticket-1", "allowed", "ok", 1)
            .unwrap();
        journal
            .log_mut()
            .record(
                "owner",
                ActionKind::Retry,
                "ticket-1",
                "allowed",
                "retry",
                2,
            )
            .unwrap();
        assert_eq!(journal.persist().unwrap(), 2);
        assert_eq!(journal.persist().unwrap(), 0);

        let reopened = ActionLogJournal::open(&path).unwrap();
        reopened.log().verify().unwrap();
        assert_eq!(reopened.log().len(), 2);
        assert_eq!(
            reopened
                .log()
                .query(Some("owner"), Some(ActionKind::Retry), None, 10)
                .len(),
            1
        );
    }

    #[test]
    fn stale_writer_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actions.jsonl");
        let mut first = ActionLogJournal::open(&path).unwrap();
        let mut second = ActionLogJournal::open(&path).unwrap();
        first
            .log_mut()
            .record("owner", ActionKind::Approve, "ticket-1", "allowed", "ok", 1)
            .unwrap();
        second
            .log_mut()
            .record(
                "owner",
                ActionKind::Retry,
                "ticket-1",
                "allowed",
                "retry",
                1,
            )
            .unwrap();
        assert_eq!(first.persist().unwrap(), 1);
        let error = second.persist().unwrap_err();
        assert!(matches!(
            error,
            ActionLogJournalError::ConcurrentWrite {
                expected_bytes: 0,
                actual_bytes: _
            }
        ));
    }

    #[test]
    fn reload_allows_retry_after_another_writer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actions.jsonl");
        let mut first = ActionLogJournal::open(&path).unwrap();
        let mut second = ActionLogJournal::open(&path).unwrap();
        first
            .log_mut()
            .record("owner", ActionKind::Approve, "ticket-1", "allowed", "ok", 1)
            .unwrap();
        first.persist().unwrap();
        second
            .log_mut()
            .record(
                "owner",
                ActionKind::Retry,
                "ticket-1",
                "allowed",
                "retry",
                1,
            )
            .unwrap();
        assert!(second.persist().is_err());
        second.reload().unwrap();
        second
            .log_mut()
            .record(
                "owner",
                ActionKind::Retry,
                "ticket-1",
                "allowed",
                "retry",
                2,
            )
            .unwrap();
        assert_eq!(second.persist().unwrap(), 1);
        let reopened = ActionLogJournal::open(&path).unwrap();
        assert_eq!(reopened.log().len(), 2);
    }

    #[test]
    fn tampered_action_log_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("actions.jsonl");
        let mut journal = ActionLogJournal::open(&path).unwrap();
        journal
            .log_mut()
            .record("owner", ActionKind::Approve, "ticket-1", "allowed", "ok", 1)
            .unwrap();
        journal.persist().unwrap();
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"owner\"", "\"attacker\"");
        std::fs::write(&path, text).unwrap();
        assert!(ActionLogJournal::open(&path).is_err());
    }
}
