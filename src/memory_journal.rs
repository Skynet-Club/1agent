//! Append-only metadata journal for memory events.
//!
//! The journal contains no memory body text: remember events carry a content
//! hash and metadata only. Payload encryption/persistence is a later slice.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::memory::{MemoryError, MemoryEvent, MemoryStore};

#[derive(Debug, Error)]
pub enum MemoryJournalError {
    #[error("memory journal io: {0}")]
    Io(#[from] std::io::Error),
    #[error("memory journal json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("memory journal verify: {0}")]
    Verify(#[from] MemoryError),
    #[error("memory journal event sequence is not contiguous")]
    NonContiguousSequence,
}

#[derive(Debug, Clone)]
pub struct MemoryJournalStore {
    path: PathBuf,
    next_seq: u64,
    events: Vec<MemoryEvent>,
}

impl MemoryJournalStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, MemoryJournalError> {
        Self::open_with(path.into(), true)
    }

    pub fn open_read_only(path: impl Into<PathBuf>) -> Result<Self, MemoryJournalError> {
        Self::open_with(path.into(), false)
    }

    fn open_with(path: PathBuf, repair: bool) -> Result<Self, MemoryJournalError> {
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
                events.push(serde_json::from_str::<MemoryEvent>(&line)?);
            }
        }
        MemoryStore::verify_events(&events)?;
        let next_seq = events.last().map(|event| event.seq).unwrap_or(0);
        Ok(Self {
            path,
            next_seq,
            events,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn events(&self) -> &[MemoryEvent] {
        &self.events
    }

    pub fn persist(&mut self, store: &MemoryStore) -> Result<usize, MemoryJournalError> {
        let new_events: Vec<&MemoryEvent> = store
            .events()
            .iter()
            .filter(|event| event.seq > self.next_seq)
            .collect();
        for (index, event) in new_events.iter().enumerate() {
            let expected = self.next_seq + 1 + index as u64;
            if event.seq != expected {
                return Err(MemoryJournalError::NonContiguousSequence);
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
        crate::jsonl::append_batch(&self.path, &buffer)?;
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
    use crate::memory::{
        MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemorySource, MemorySourceKind,
        MemoryTaint,
    };

    fn memory() -> MemoryRecord {
        let mut record = MemoryRecord::new(
            "mem-1",
            MemoryLayer::Semantic,
            MemoryKind::Preference,
            MemoryScope::bot("bot-1"),
            "private preference body",
            MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
        );
        record.taint = MemoryTaint::Trusted;
        record
    }

    #[test]
    fn metadata_journal_persists_without_body_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-journal.jsonl");
        let mut store = MemoryStore::default();
        store.remember_at(memory(), 10).unwrap();
        store
            .forget("mem-1", crate::memory::ForgetMode::Hard, 11)
            .unwrap();
        let mut journal = MemoryJournalStore::open(&path).unwrap();
        assert_eq!(journal.persist(&store).unwrap(), 2);
        assert_eq!(journal.persist(&store).unwrap(), 0);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("private preference body"));

        let reopened = MemoryJournalStore::open(&path).unwrap();
        assert_eq!(reopened.events().len(), 2);
        MemoryStore::verify_events(reopened.events()).unwrap();
    }

    #[test]
    fn tampered_metadata_journal_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-journal.jsonl");
        let mut store = MemoryStore::default();
        store.remember_at(memory(), 10).unwrap();
        let mut journal = MemoryJournalStore::open(&path).unwrap();
        journal.persist(&store).unwrap();
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"mem-1\"", "\"mem-tampered\"");
        std::fs::write(&path, text).unwrap();
        assert!(MemoryJournalStore::open(&path).is_err());
    }
}
