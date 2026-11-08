//! Append-only local alert log for operator-visible health events.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AlertError {
    #[error("alert log io: {0}")]
    Io(#[from] std::io::Error),
    #[error("alert log json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("alert log record is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AlertRecord {
    pub alert_id: String,
    pub at_ms: u128,
    pub kind: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub observed: u64,
    pub threshold: u64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AlertLog {
    path: PathBuf,
}

impl AlertLog {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AlertError> {
        let path = path.into();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            crate::jsonl::ensure_parent_private(&path)?;
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, record: &AlertRecord) -> Result<(), AlertError> {
        validate(record)?;
        let line = serde_json::to_string(record)?;
        crate::jsonl::append_batch(&self.path, &format!("{line}\n"))?;
        Ok(())
    }

    pub fn read(&self) -> Result<Vec<AlertRecord>, AlertError> {
        let mut records = Vec::new();
        if !self.path.exists() {
            return Ok(records);
        }
        let file = std::fs::File::open(&self.path)?;
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: AlertRecord = serde_json::from_str(&line)?;
            validate(&record)?;
            records.push(record);
        }
        Ok(records)
    }
}

pub fn provider_health_alert(
    provider_id: &str,
    model: &str,
    at_ms: u128,
    failures: u32,
    threshold: u32,
) -> AlertRecord {
    AlertRecord {
        alert_id: Uuid::new_v4().to_string(),
        at_ms,
        kind: "provider_health".into(),
        provider_id: Some(provider_id.into()),
        model: Some(model.into()),
        observed: u64::from(failures),
        threshold: u64::from(threshold),
        message: format!(
            "provider {provider_id}/{model} reached {failures} consecutive failures (threshold {threshold})"
        ),
    }
}

fn validate(record: &AlertRecord) -> Result<(), AlertError> {
    if record.alert_id.trim().is_empty()
        || record.kind.trim().is_empty()
        || record.message.trim().is_empty()
        || record.message.chars().count() > 2_000
    {
        return Err(AlertError::Invalid("empty or oversized alert field".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_log_appends_and_reads_valid_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alerts.jsonl");
        let log = AlertLog::open(&path).unwrap();
        log.append(&provider_health_alert("p", "m", 10, 2, 2))
            .unwrap();
        log.append(&provider_health_alert("p", "m", 20, 3, 2))
            .unwrap();
        let records = log.read().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].observed, 2);
        assert_eq!(records[1].at_ms, 20);
    }

    #[test]
    fn invalid_and_malformed_alerts_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alerts.jsonl");
        let log = AlertLog::open(&path).unwrap();
        let mut record = provider_health_alert("p", "m", 10, 2, 2);
        record.message.clear();
        assert!(matches!(log.append(&record), Err(AlertError::Invalid(_))));

        std::fs::write(&path, b"{\"alert_id\":\"x\"}\n").unwrap();
        assert!(matches!(log.read(), Err(AlertError::Json(_))));
    }
}
