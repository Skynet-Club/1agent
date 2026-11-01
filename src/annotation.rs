use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::safety::contains_disallowed_control;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationLabel {
    Supported,
    Unsupported,
    NeedsEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AnnotationRecord {
    pub case_id: String,
    pub annotator_id: String,
    pub label: AnnotationLabel,
    #[serde(default)]
    pub evidence_sources: Vec<String>,
    #[serde(default)]
    pub note: String,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgreementReport {
    pub annotations: usize,
    pub cases_with_multiple_annotations: usize,
    pub eligible_cases: usize,
    pub observed_agreement_bps: u16,
    pub expected_agreement_bps: u16,
    pub kappa_millis: i32,
    pub enough_data: bool,
}

pub struct AnnotationStore {
    path: PathBuf,
}

impl AnnotationStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, AnnotationError> {
        std::fs::create_dir_all(dir.as_ref())?;
        Ok(Self {
            path: dir.as_ref().join("annotations.jsonl"),
        })
    }

    pub fn add(&mut self, mut record: AnnotationRecord) -> Result<(), AnnotationError> {
        validate_text(&record.case_id)?;
        validate_text(&record.annotator_id)?;
        validate_text(&record.note)?;
        for source in &record.evidence_sources {
            validate_text(source)?;
        }
        let existing = self.records()?;
        if existing.iter().any(|entry| {
            entry.case_id == record.case_id && entry.annotator_id == record.annotator_id
        }) {
            return Err(AnnotationError::Duplicate);
        }
        record.created_unix_ms = now_unix_ms();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &record)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    pub fn records(&self) -> Result<Vec<AnnotationRecord>, AnnotationError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&self.path)?;
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(serde_json::from_str(&line)?);
        }
        Ok(records)
    }

    pub fn agreement(&self) -> Result<AgreementReport, AnnotationError> {
        let records = self.records()?;
        let mut by_case: BTreeMap<String, Vec<AnnotationLabel>> = BTreeMap::new();
        for record in &records {
            by_case
                .entry(record.case_id.clone())
                .or_default()
                .push(record.label);
        }
        let cases_with_multiple_annotations =
            by_case.values().filter(|labels| labels.len() >= 2).count();
        let eligible: Vec<_> = by_case
            .values()
            .filter(|labels| labels.len() >= 2)
            .collect();
        if eligible.is_empty() {
            return Ok(AgreementReport {
                annotations: records.len(),
                cases_with_multiple_annotations,
                eligible_cases: 0,
                observed_agreement_bps: 0,
                expected_agreement_bps: 0,
                kappa_millis: 0,
                enough_data: false,
            });
        }

        let mut pair_agreements = 0usize;
        let mut pair_total = 0usize;
        let mut label_totals: BTreeMap<AnnotationLabel, usize> = BTreeMap::new();
        for labels in &eligible {
            for label in labels.iter() {
                *label_totals.entry(*label).or_default() += 1;
            }
            for left in 0..labels.len() {
                for right in left + 1..labels.len() {
                    pair_total += 1;
                    if labels[left] == labels[right] {
                        pair_agreements += 1;
                    }
                }
            }
        }
        let observed = ratio_bps(pair_agreements, pair_total);
        let total_labels = pair_total * 2;
        let expected_numerator: usize = label_totals.values().map(|count| count * count).sum();
        let expected = ratio_bps(expected_numerator, total_labels * total_labels);
        let kappa = if expected == 10_000 || pair_total == 0 {
            0
        } else {
            (((observed as i64 - expected as i64) * 1_000) / (10_000_i64 - expected as i64)) as i32
        };
        Ok(AgreementReport {
            annotations: records.len(),
            cases_with_multiple_annotations,
            eligible_cases: eligible.len(),
            observed_agreement_bps: observed,
            expected_agreement_bps: expected,
            kappa_millis: kappa,
            enough_data: eligible.len() >= 2,
        })
    }
}

fn ratio_bps(numerator: usize, denominator: usize) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator * 10_000) / denominator).min(10_000) as u16
}

fn validate_text(text: &str) -> Result<(), AnnotationError> {
    if contains_disallowed_control(text) {
        return Err(AnnotationError::UnsafeText);
    }
    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, thiserror::Error)]
pub enum AnnotationError {
    #[error("annotation store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("annotation JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("case and annotator pair already exists")]
    Duplicate,
    #[error("annotation contains unsafe control characters")]
    UnsafeText,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(case: &str, annotator: &str, label: AnnotationLabel) -> AnnotationRecord {
        AnnotationRecord {
            case_id: case.into(),
            annotator_id: annotator.into(),
            label,
            evidence_sources: Vec::new(),
            note: String::new(),
            created_unix_ms: 0,
        }
    }

    #[test]
    fn agreement_is_one_for_identical_labels() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = AnnotationStore::open(dir.path()).unwrap();
        for (case, label) in [
            ("c1", AnnotationLabel::Supported),
            ("c2", AnnotationLabel::Unsupported),
        ] {
            for annotator in ["a1", "a2"] {
                store.add(record(case, annotator, label)).unwrap();
            }
        }
        let report = store.agreement().unwrap();
        assert_eq!(report.observed_agreement_bps, 10_000);
        assert_eq!(report.kappa_millis, 1_000);
        assert!(report.enough_data);
    }

    #[test]
    fn duplicate_case_annotator_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = AnnotationStore::open(dir.path()).unwrap();
        store
            .add(record("c1", "a1", AnnotationLabel::Supported))
            .unwrap();
        assert!(matches!(
            store.add(record("c1", "a1", AnnotationLabel::Unsupported)),
            Err(AnnotationError::Duplicate)
        ));
    }

    #[test]
    fn disagreement_reduces_observed_agreement() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = AnnotationStore::open(dir.path()).unwrap();
        store
            .add(record("c1", "a1", AnnotationLabel::Supported))
            .unwrap();
        store
            .add(record("c1", "a2", AnnotationLabel::Unsupported))
            .unwrap();
        let report = store.agreement().unwrap();
        assert_eq!(report.observed_agreement_bps, 0);
        assert!(!report.enough_data);
    }
}
