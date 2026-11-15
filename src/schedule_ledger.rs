//! Durable scheduled-run claims.
//!
//! The run identity is `(schedule_id, scheduled_at_ms)`. A claim is appended
//! and fsynced before the child process starts, so a crash after the claim
//! cannot silently repeat the same slot. The default policy is at-most-once:
//! a claimed or running record found on restart is marked `interrupted` and is
//! not automatically re-run.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::jsonl::FileLock;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Claimed,
    Running,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub schema_version: u32,
    pub schedule_id: String,
    pub scheduled_at_ms: u128,
    pub run_id: String,
    pub status: RunStatus,
    pub updated_at_ms: u128,
    #[serde(default)]
    pub journal: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimOutcome {
    Claimed(RunRecord),
    AlreadyClaimed(RunRecord),
}

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("schedule ledger io: {0}")]
    Io(#[from] std::io::Error),
    #[error("schedule ledger json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid schedule ledger record: {0}")]
    Invalid(String),
}

pub struct RunLedger {
    path: PathBuf,
}

impl RunLedger {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Claim a slot. The claim line is synced before this returns.
    pub fn claim(
        &self,
        schedule_id: &str,
        scheduled_at_ms: u128,
        run_id: &str,
        now_ms: u128,
    ) -> Result<ClaimOutcome, LedgerError> {
        validate_token(schedule_id, "schedule_id")?;
        validate_token(run_id, "run_id")?;
        let _lock = FileLock::exclusive(&self.path)?;
        let latest = self.read_latest_unlocked()?;
        if let Some(existing) = latest.get(&(schedule_id.to_string(), scheduled_at_ms)) {
            return Ok(ClaimOutcome::AlreadyClaimed(existing.clone()));
        }
        let record = RunRecord {
            schema_version: SCHEMA_VERSION,
            schedule_id: schedule_id.into(),
            scheduled_at_ms,
            run_id: run_id.into(),
            status: RunStatus::Claimed,
            updated_at_ms: now_ms,
            journal: None,
            detail: None,
        };
        self.append_unlocked(&record)?;
        Ok(ClaimOutcome::Claimed(record))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mark(
        &self,
        schedule_id: &str,
        scheduled_at_ms: u128,
        run_id: &str,
        status: RunStatus,
        now_ms: u128,
        journal: Option<&str>,
        detail: Option<&str>,
    ) -> Result<RunRecord, LedgerError> {
        validate_token(schedule_id, "schedule_id")?;
        validate_token(run_id, "run_id")?;
        let _lock = FileLock::exclusive(&self.path)?;
        let record = RunRecord {
            schema_version: SCHEMA_VERSION,
            schedule_id: schedule_id.into(),
            scheduled_at_ms,
            run_id: run_id.into(),
            status,
            updated_at_ms: now_ms,
            journal: journal.map(str::to_string),
            detail: detail.map(str::to_string),
        };
        self.append_unlocked(&record)?;
        Ok(record)
    }

    /// Mark every claimed/running slot as interrupted. Returns the records
    /// that were changed. A second call is a no-op.
    pub fn reconcile(&self, now_ms: u128) -> Result<Vec<RunRecord>, LedgerError> {
        let _lock = FileLock::exclusive(&self.path)?;
        let latest = self.read_latest_unlocked()?;
        let mut interrupted = Vec::new();
        for record in latest.values() {
            if matches!(record.status, RunStatus::Claimed | RunStatus::Running) {
                let updated = RunRecord {
                    status: RunStatus::Interrupted,
                    updated_at_ms: now_ms,
                    detail: Some("daemon restarted before completion".into()),
                    ..record.clone()
                };
                self.append_unlocked(&updated)?;
                interrupted.push(updated);
            }
        }
        Ok(interrupted)
    }

    pub fn latest(&self) -> Result<BTreeMap<(String, u128), RunRecord>, LedgerError> {
        let _lock = FileLock::exclusive(&self.path)?;
        self.read_latest_unlocked()
    }

    fn read_latest_unlocked(&self) -> Result<BTreeMap<(String, u128), RunRecord>, LedgerError> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BTreeMap::new());
            }
            Err(error) => return Err(error.into()),
        };
        let complete = if content.ends_with('\n') {
            content.as_str()
        } else {
            content.rsplit_once('\n').map_or("", |(head, _)| head)
        };
        let mut latest = BTreeMap::new();
        for (index, line) in complete.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let record: RunRecord = serde_json::from_str(line)
                .map_err(|error| LedgerError::Invalid(format!("line {}: {error}", index + 1)))?;
            if record.schema_version != SCHEMA_VERSION {
                return Err(LedgerError::Invalid(format!(
                    "line {}: unsupported schema version {}",
                    index + 1,
                    record.schema_version
                )));
            }
            validate_token(&record.schedule_id, "schedule_id")?;
            validate_token(&record.run_id, "run_id")?;
            latest.insert((record.schedule_id.clone(), record.scheduled_at_ms), record);
        }
        Ok(latest)
    }

    fn append_unlocked(&self, record: &RunRecord) -> Result<(), LedgerError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_vec(record)?;
        file.write_all(&line)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(())
    }
}

fn validate_token(value: &str, field: &str) -> Result<(), LedgerError> {
    if value.trim().is_empty() || value.chars().count() > 256 {
        return Err(LedgerError::Invalid(format!(
            "{field} must be 1..=256 characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(LedgerError::Invalid(format!(
            "{field} contains a control character"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_is_durable_and_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = RunLedger::new(dir.path().join("runs.jsonl"));
        let first = ledger.claim("job-1", 1000, "run-1", 1000).unwrap();
        assert!(matches!(first, ClaimOutcome::Claimed(_)));
        let second = ledger.claim("job-1", 1000, "run-2", 1001).unwrap();
        assert!(matches!(second, ClaimOutcome::AlreadyClaimed(_)));
        let reopened = RunLedger::new(dir.path().join("runs.jsonl"));
        let latest = reopened.latest().unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[&("job-1".into(), 1000)].run_id, "run-1");
    }

    #[test]
    fn reconcile_marks_claimed_and_running_interrupted_once() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = RunLedger::new(dir.path().join("runs.jsonl"));
        ledger.claim("job-1", 1000, "run-1", 1000).unwrap();
        ledger.claim("job-2", 2000, "run-2", 2000).unwrap();
        ledger
            .mark("job-2", 2000, "run-2", RunStatus::Running, 2001, None, None)
            .unwrap();
        let interrupted = ledger.reconcile(3000).unwrap();
        assert_eq!(interrupted.len(), 2);
        assert!(ledger.reconcile(3001).unwrap().is_empty());
        let latest = ledger.latest().unwrap();
        assert_eq!(
            latest[&("job-1".into(), 1000)].status,
            RunStatus::Interrupted
        );
        assert_eq!(
            latest[&("job-2".into(), 2000)].status,
            RunStatus::Interrupted
        );
    }

    #[test]
    fn completed_slots_are_not_interrupted() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = RunLedger::new(dir.path().join("runs.jsonl"));
        ledger.claim("job-1", 1000, "run-1", 1000).unwrap();
        ledger
            .mark(
                "job-1",
                1000,
                "run-1",
                RunStatus::Completed,
                1100,
                Some("journal.jsonl"),
                None,
            )
            .unwrap();
        assert!(ledger.reconcile(2000).unwrap().is_empty());
    }

    #[test]
    fn hundred_claimed_slots_reconcile_once_without_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = RunLedger::new(dir.path().join("runs.jsonl"));
        for slot in 0..100u128 {
            let outcome = ledger
                .claim("job", slot, &format!("run-{slot}"), slot)
                .unwrap();
            assert!(matches!(outcome, ClaimOutcome::Claimed(_)));
        }
        let interrupted = ledger.reconcile(10_000).unwrap();
        assert_eq!(interrupted.len(), 100);
        assert!(ledger.reconcile(10_001).unwrap().is_empty());
        let latest = ledger.latest().unwrap();
        assert_eq!(latest.len(), 100);
        assert!(
            latest
                .values()
                .all(|record| record.status == RunStatus::Interrupted)
        );
    }

    #[test]
    fn torn_tail_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runs.jsonl");
        let ledger = RunLedger::new(&path);
        ledger.claim("job-1", 1000, "run-1", 1000).unwrap();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"schema_version\":1,\"schedule_id\":\"job-2\"")
            .unwrap();
        drop(file);
        let latest = ledger.latest().unwrap();
        assert_eq!(latest.len(), 1);
    }
}
