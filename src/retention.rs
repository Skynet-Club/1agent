//! Run-journal retention.
//!
//! Dry-run by default. `apply` deletes only top-level `.jsonl` run journals
//! older than the cutoff and appends a receipt; the receipt itself is never a
//! candidate. Memory/audit stores have their own retention boundaries and are
//! not touched here.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const RECEIPT_NAME: &str = "retention-receipt.jsonl";
const MAX_DAYS: u64 = 3650;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionCandidate {
    pub path: PathBuf,
    pub modified_ms: u128,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionReport {
    pub older_than_days: u64,
    pub cutoff_ms: u128,
    pub scanned: usize,
    pub matched: usize,
    pub deleted: Vec<RetentionCandidate>,
    pub dry_run: bool,
    pub receipt_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum RetentionError {
    #[error("retention io: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid retention policy: {0}")]
    Invalid(String),
}

fn validate_days(days: u64) -> Result<(), RetentionError> {
    if days == 0 || days > MAX_DAYS {
        return Err(RetentionError::Invalid(format!(
            "older_than_days must be 1..={MAX_DAYS}"
        )));
    }
    Ok(())
}

fn modified_ms(path: &Path) -> u128 {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub fn scan(
    journal_dir: &Path,
    older_than_days: u64,
    now_ms: u128,
) -> Result<(u128, Vec<RetentionCandidate>), RetentionError> {
    validate_days(older_than_days)?;
    let cutoff_ms = now_ms.saturating_sub(u128::from(older_than_days) * 86_400_000);
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(journal_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.metadata()?.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if path.file_name().and_then(|value| value.to_str()) == Some(RECEIPT_NAME) {
            continue;
        }
        let bytes = entry.metadata()?.len();
        let modified = modified_ms(&path);
        if modified <= cutoff_ms {
            candidates.push(RetentionCandidate {
                path,
                modified_ms: modified,
                bytes,
            });
        }
    }
    candidates.sort_by(|left, right| left.path.cmp(&right.path));
    Ok((cutoff_ms, candidates))
}

pub fn apply(
    journal_dir: &Path,
    older_than_days: u64,
    now_ms: u128,
    receipt_path: &Path,
) -> Result<RetentionReport, RetentionError> {
    let (cutoff_ms, candidates) = scan(journal_dir, older_than_days, now_ms)?;
    let mut deleted = Vec::new();
    for candidate in &candidates {
        std::fs::remove_file(&candidate.path)?;
        deleted.push(candidate.clone());
    }
    if let Some(parent) = receipt_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let receipt = serde_json::json!({
        "timestamp_unix_ms": now_ms,
        "older_than_days": older_than_days,
        "cutoff_ms": cutoff_ms,
        "deleted": deleted
    });
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(receipt_path)?;
    use std::io::Write;
    writeln!(file, "{receipt}")?;
    Ok(RetentionReport {
        older_than_days,
        cutoff_ms,
        scanned: candidates.len(),
        matched: candidates.len(),
        deleted,
        dry_run: false,
        receipt_path: receipt_path.to_path_buf(),
    })
}

pub fn dry_run(
    journal_dir: &Path,
    older_than_days: u64,
    now_ms: u128,
    receipt_path: &Path,
) -> Result<RetentionReport, RetentionError> {
    let (cutoff_ms, candidates) = scan(journal_dir, older_than_days, now_ms)?;
    Ok(RetentionReport {
        older_than_days,
        cutoff_ms,
        scanned: candidates.len(),
        matched: candidates.len(),
        deleted: candidates,
        dry_run: true,
        receipt_path: receipt_path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs::{File, FileTimes};
    use std::time::{Duration, SystemTime};

    use super::*;

    fn age(path: &Path, days: u64) {
        let file = File::options().write(true).open(path).unwrap();
        let modified = SystemTime::now() - Duration::from_secs(days * 86_400);
        file.set_times(FileTimes::new().set_modified(modified))
            .unwrap();
    }

    #[test]
    fn dry_run_matches_without_deleting_and_apply_writes_a_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.jsonl");
        let new = dir.path().join("new.jsonl");
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        age(&old, 40);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let receipt = dir.path().join(RECEIPT_NAME);
        let report = dry_run(dir.path(), 30, now, &receipt).unwrap();
        assert_eq!(report.matched, 1);
        assert!(old.exists());
        assert!(!receipt.exists());

        let report = apply(dir.path(), 30, now, &receipt).unwrap();
        assert_eq!(report.deleted.len(), 1);
        assert!(!old.exists());
        assert!(new.exists());
        assert!(receipt.exists());
        let receipt_text = std::fs::read_to_string(&receipt).unwrap();
        assert!(receipt_text.contains("old.jsonl"));
    }

    #[test]
    fn invalid_days_are_rejected() {
        assert!(validate_days(0).is_err());
        assert!(validate_days(MAX_DAYS + 1).is_err());
        assert!(validate_days(30).is_ok());
    }
}
