//! Append-only store for independent verification reports.
//!
//! This closes the loop between `verification::verify` and the value report:
//! a caller records a report once, and `verified_count` counts only clean
//! `Verified` outcomes in a time window.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::verification::{HackSuiteSummary, VerificationOutcome, VerificationReport, summarize};

#[derive(Debug, Error)]
pub enum VerificationStoreError {
    #[error("verification store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("verification store JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("verification record is invalid: {0}")]
    Invalid(String),
    #[error("verification id already exists: {0}")]
    Duplicate(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerificationRecord {
    pub verification_id: String,
    pub run_id: String,
    pub at_ms: u128,
    pub report: VerificationReport,
}

impl VerificationRecord {
    pub fn validate(&self) -> Result<(), VerificationStoreError> {
        if self.verification_id.trim().is_empty() {
            return Err(VerificationStoreError::Invalid(
                "verification_id is empty".into(),
            ));
        }
        if self.run_id.trim().is_empty() {
            return Err(VerificationStoreError::Invalid("run_id is empty".into()));
        }
        if self.report.executor_id.trim().is_empty() || self.report.verifier_id.trim().is_empty() {
            return Err(VerificationStoreError::Invalid(
                "report executor/verifier is empty".into(),
            ));
        }
        if self.report.executor_id == self.report.verifier_id {
            return Err(VerificationStoreError::Invalid(
                "executor and verifier must differ".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct VerificationStore {
    path: PathBuf,
    entries: Vec<VerificationRecord>,
}

impl VerificationStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, VerificationStoreError> {
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
                    let record: VerificationRecord =
                        serde_json::from_str(line).map_err(|error| {
                            VerificationStoreError::Invalid(format!("line {}: {error}", index + 1))
                        })?;
                    record.validate()?;
                    if entries.iter().any(|existing: &VerificationRecord| {
                        existing.verification_id == record.verification_id
                    }) {
                        return Err(VerificationStoreError::Duplicate(record.verification_id));
                    }
                    entries.push(record);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(Self { path, entries })
    }

    pub fn append(
        &mut self,
        record: VerificationRecord,
    ) -> Result<&VerificationRecord, VerificationStoreError> {
        record.validate()?;
        if self
            .entries
            .iter()
            .any(|existing| existing.verification_id == record.verification_id)
        {
            return Err(VerificationStoreError::Duplicate(record.verification_id));
        }
        let line = serde_json::to_vec(&record)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&line)?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        let id = record.verification_id.clone();
        self.entries.push(record);
        Ok(self
            .entries
            .iter()
            .find(|record| record.verification_id == id)
            .expect("record is present"))
    }

    pub fn records(&self) -> &[VerificationRecord] {
        &self.entries
    }

    pub fn summary(&self) -> HackSuiteSummary {
        let reports = self
            .entries
            .iter()
            .map(|record| record.report.clone())
            .collect::<Vec<_>>();
        summarize(&reports)
    }

    pub fn summary_between(&self, since_ms: u128, until_ms: u128) -> HackSuiteSummary {
        let reports = self
            .entries
            .iter()
            .filter(|record| record.at_ms >= since_ms && record.at_ms <= until_ms)
            .map(|record| record.report.clone())
            .collect::<Vec<_>>();
        summarize(&reports)
    }

    /// Counts only clean `Verified` outcomes inside the inclusive window.
    pub fn verified_count(&self, since_ms: u128, until_ms: u128) -> u32 {
        self.entries
            .iter()
            .filter(|record| {
                record.at_ms >= since_ms
                    && record.at_ms <= until_ms
                    && record.report.outcome == VerificationOutcome::Verified
            })
            .count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::VerificationReport;

    fn temp_store(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "1agent-verification-{}-{name}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn report(outcome: VerificationOutcome, public: bool) -> VerificationReport {
        VerificationReport {
            outcome,
            executor_id: "executor".into(),
            verifier_id: "verifier".into(),
            public_passed: public,
            hidden_passed: outcome == VerificationOutcome::Verified,
            failures: Vec::new(),
        }
    }

    fn record(
        id: &str,
        at_ms: u128,
        outcome: VerificationOutcome,
        public: bool,
    ) -> VerificationRecord {
        VerificationRecord {
            verification_id: id.into(),
            run_id: format!("run-{id}"),
            at_ms,
            report: report(outcome, public),
        }
    }

    #[test]
    fn appends_and_summarizes_verified_and_hacked() {
        let path = temp_store("summary");
        let mut store = VerificationStore::open(&path).unwrap();
        store
            .append(record("v1", 10, VerificationOutcome::Verified, true))
            .unwrap();
        store
            .append(record("v2", 20, VerificationOutcome::Hacked, true))
            .unwrap();
        store
            .append(record("v3", 30, VerificationOutcome::Rejected, false))
            .unwrap();
        let summary = store.summary();
        assert_eq!(summary.verified, 1);
        assert_eq!(summary.hacked, 1);
        assert_eq!(summary.rejected, 1);
        assert_eq!(summary.resolved, 2);
        assert_eq!(summary.hacked_resolved_bps, 5_000);
        assert_eq!(store.verified_count(0, 100), 1);
        assert_eq!(store.verified_count(15, 100), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn duplicate_ids_and_self_verification_are_rejected() {
        let path = temp_store("duplicate");
        let mut store = VerificationStore::open(&path).unwrap();
        store
            .append(record("v1", 10, VerificationOutcome::Verified, true))
            .unwrap();
        assert!(matches!(
            store.append(record("v1", 20, VerificationOutcome::Verified, true)),
            Err(VerificationStoreError::Duplicate(_))
        ));

        let mut self_verified = record("v2", 20, VerificationOutcome::Verified, true);
        self_verified.report.verifier_id = self_verified.report.executor_id.clone();
        assert!(store.append(self_verified).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn records_survive_reopen_and_a_partial_tail_is_ignored() {
        let path = temp_store("reopen");
        let mut store = VerificationStore::open(&path).unwrap();
        store
            .append(record("v1", 10, VerificationOutcome::Verified, true))
            .unwrap();
        drop(store);

        let reopened = VerificationStore::open(&path).unwrap();
        assert_eq!(reopened.records().len(), 1);
        assert_eq!(reopened.verified_count(0, 100), 1);

        std::fs::write(&path, b"{\"verification_id\":\"broken\"").unwrap();
        let partial = VerificationStore::open(&path).unwrap();
        assert!(partial.records().is_empty());
        std::fs::remove_file(&path).ok();
    }
}
