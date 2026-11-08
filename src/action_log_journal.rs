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
}

#[derive(Debug, Clone)]
pub struct ActionLogJournal {
    path: PathBuf,
    next_seq: u64,
    log: ActionLog,
}

impl ActionLogJournal {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, ActionLogJournalError> {
        let path = path.into();
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
        Ok(Self {
            path,
            next_seq,
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
        crate::jsonl::append_batch(&self.path, &buffer)?;
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
