use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationPrediction {
    pub case_id: String,
    pub confidence_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReliabilityBin {
    pub lower_bps: u16,
    pub upper_bps: u16,
    pub samples: usize,
    pub mean_confidence_bps: u16,
    pub observed_success_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SelectiveRiskPoint {
    pub threshold_bps: u16,
    pub coverage_bps: u16,
    pub risk_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationReport {
    pub samples: usize,
    pub kappa_millis: i32,
    pub ece_bps: u16,
    pub brier_micros: u32,
    pub bins: Vec<ReliabilityBin>,
    pub selective_risk: Vec<SelectiveRiskPoint>,
    pub calibration_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoldSetBatch {
    pub batch_id: String,
    pub case_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoldSetManifest {
    pub goldset_version: String,
    pub suite_id: String,
    pub suite_sha256: String,
    pub case_ids: Vec<String>,
    pub owner: String,
    pub reviewer_cohort: Vec<String>,
    pub redaction_policy: String,
    pub frozen_unix_ms: u128,
    pub batches: Vec<GoldSetBatch>,
    pub manifest_sha256: String,
}

pub struct AnnotationStore {
    path: PathBuf,
}

pub fn create_goldset_manifest(
    suite_path: impl AsRef<Path>,
    goldset_version: String,
    owner: String,
    reviewer_cohort: Vec<String>,
    redaction_policy: String,
    batch_size: usize,
) -> Result<GoldSetManifest, AnnotationError> {
    validate_text(&goldset_version)?;
    validate_text(&owner)?;
    validate_text(&redaction_policy)?;
    if reviewer_cohort.is_empty() || batch_size == 0 {
        return Err(AnnotationError::InvalidGoldSet(
            "reviewers and batch size are required".into(),
        ));
    }
    for reviewer in &reviewer_cohort {
        validate_text(reviewer)?;
    }
    let bytes = std::fs::read(suite_path)?;
    let suite: serde_json::Value = serde_json::from_slice(&bytes)?;
    let suite_id = suite
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| AnnotationError::InvalidGoldSet("suite name is missing".into()))?
        .to_string();
    let cases = suite
        .get("cases")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| AnnotationError::InvalidGoldSet("suite cases are missing".into()))?;
    let mut case_ids = Vec::new();
    for case in cases {
        let id = case
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| AnnotationError::InvalidGoldSet("case id is missing".into()))?
            .to_string();
        validate_text(&id)?;
        case_ids.push(id);
    }
    let unique: BTreeSet<_> = case_ids.iter().collect();
    if case_ids.is_empty() || unique.len() != case_ids.len() {
        return Err(AnnotationError::InvalidGoldSet(
            "case ids must be non-empty and unique".into(),
        ));
    }
    let batches: Vec<GoldSetBatch> = case_ids
        .chunks(batch_size)
        .enumerate()
        .map(|(index, chunk)| GoldSetBatch {
            batch_id: format!("batch-{}", index + 1),
            case_ids: chunk.to_vec(),
        })
        .collect();
    let suite_sha256 = hex::encode(Sha256::digest(&bytes));
    let frozen_unix_ms = now_unix_ms();
    let digest_material = serde_json::json!({
        "goldset_version": goldset_version,
        "suite_id": suite_id,
        "suite_sha256": suite_sha256,
        "case_ids": case_ids,
        "owner": owner,
        "reviewer_cohort": reviewer_cohort,
        "redaction_policy": redaction_policy,
        "frozen_unix_ms": frozen_unix_ms,
        "batches": batches,
    });
    let manifest_sha256 = hex::encode(Sha256::digest(serde_json::to_vec(&digest_material)?));
    let manifest = GoldSetManifest {
        goldset_version: digest_material["goldset_version"]
            .as_str()
            .unwrap()
            .to_string(),
        suite_id: digest_material["suite_id"].as_str().unwrap().to_string(),
        suite_sha256: digest_material["suite_sha256"]
            .as_str()
            .unwrap()
            .to_string(),
        case_ids: digest_material["case_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect(),
        owner: digest_material["owner"].as_str().unwrap().to_string(),
        reviewer_cohort: digest_material["reviewer_cohort"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect(),
        redaction_policy: digest_material["redaction_policy"]
            .as_str()
            .unwrap()
            .to_string(),
        frozen_unix_ms,
        batches: digest_material["batches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|batch| GoldSetBatch {
                batch_id: batch["batch_id"].as_str().unwrap().to_string(),
                case_ids: batch["case_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_string())
                    .collect(),
            })
            .collect(),
        manifest_sha256,
    };
    Ok(manifest)
}

pub fn verify_goldset_manifest(
    manifest: &GoldSetManifest,
    suite_path: impl AsRef<Path>,
) -> Result<(), AnnotationError> {
    let digest_material = serde_json::json!({
        "goldset_version": manifest.goldset_version,
        "suite_id": manifest.suite_id,
        "suite_sha256": manifest.suite_sha256,
        "case_ids": manifest.case_ids,
        "owner": manifest.owner,
        "reviewer_cohort": manifest.reviewer_cohort,
        "redaction_policy": manifest.redaction_policy,
        "frozen_unix_ms": manifest.frozen_unix_ms,
        "batches": manifest.batches,
    });
    let actual_manifest_sha = hex::encode(Sha256::digest(serde_json::to_vec(&digest_material)?));
    if actual_manifest_sha != manifest.manifest_sha256 {
        return Err(AnnotationError::GoldSetDrift);
    }
    let bytes = std::fs::read(suite_path)?;
    let actual_suite_sha = hex::encode(Sha256::digest(&bytes));
    if actual_suite_sha != manifest.suite_sha256 {
        return Err(AnnotationError::GoldSetDrift);
    }
    let batch_cases: Vec<_> = manifest
        .batches
        .iter()
        .flat_map(|batch| batch.case_ids.iter().cloned())
        .collect();
    let declared: BTreeSet<_> = manifest.case_ids.iter().collect();
    let batched: BTreeSet<_> = batch_cases.iter().collect();
    if declared != batched || batch_cases.len() != manifest.case_ids.len() {
        return Err(AnnotationError::InvalidGoldSet(
            "batches must cover each case exactly once".into(),
        ));
    }
    Ok(())
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

    pub fn calibrate(
        &self,
        predictions: Vec<CalibrationPrediction>,
        min_kappa_millis: i32,
        min_samples: usize,
    ) -> Result<CalibrationReport, AnnotationError> {
        let agreement = self.agreement()?;
        if !agreement.enough_data
            || agreement.kappa_millis < min_kappa_millis
            || agreement.cases_with_multiple_annotations < min_samples
        {
            return Err(AnnotationError::InsufficientCalibrationEvidence {
                kappa_millis: agreement.kappa_millis,
                eligible_cases: agreement.cases_with_multiple_annotations,
            });
        }
        let records = self.records()?;
        let mut by_case: BTreeMap<String, Vec<AnnotationLabel>> = BTreeMap::new();
        for record in records {
            by_case
                .entry(record.case_id)
                .or_default()
                .push(record.label);
        }
        let mut seen = BTreeSet::new();
        let mut samples = Vec::new();
        for prediction in predictions {
            if prediction.confidence_bps > 10_000 {
                return Err(AnnotationError::InvalidConfidence);
            }
            if !seen.insert(prediction.case_id.clone()) {
                return Err(AnnotationError::DuplicatePrediction);
            }
            let Some(labels) = by_case.get(&prediction.case_id) else {
                continue;
            };
            let Some(success) = consensus_success(labels) else {
                continue;
            };
            samples.push((prediction.confidence_bps, success));
        }
        if samples.len() < min_samples {
            return Err(AnnotationError::InsufficientCalibrationSamples);
        }

        let mut bins = Vec::new();
        let mut ece_numerator = 0u64;
        for lower in (0u16..10_000).step_by(1_000) {
            let upper = lower + 1_000;
            let selected: Vec<_> = samples
                .iter()
                .filter(|(confidence, _)| {
                    if upper == 10_000 {
                        *confidence >= lower && *confidence <= upper
                    } else {
                        *confidence >= lower && *confidence < upper
                    }
                })
                .collect();
            if selected.is_empty() {
                continue;
            }
            let mean_confidence = selected
                .iter()
                .map(|(confidence, _)| u32::from(*confidence))
                .sum::<u32>()
                / selected.len() as u32;
            let success_count = selected.iter().filter(|(_, success)| *success).count();
            let observed_success = ((success_count * 10_000) / selected.len()) as u16;
            ece_numerator += u64::from(mean_confidence.abs_diff(u32::from(observed_success)))
                * selected.len() as u64;
            bins.push(ReliabilityBin {
                lower_bps: lower,
                upper_bps: upper,
                samples: selected.len(),
                mean_confidence_bps: mean_confidence as u16,
                observed_success_bps: observed_success,
            });
        }
        let ece_bps = (ece_numerator / samples.len() as u64) as u16;
        let brier_micros = (samples
            .iter()
            .map(|(confidence, success)| {
                let probability = i64::from(*confidence);
                let outcome = if *success { 10_000 } else { 0 };
                let delta = probability - outcome;
                (delta * delta) as u64
            })
            .sum::<u64>()
            / samples.len() as u64) as u32;
        let mut selective_risk = Vec::new();
        for threshold in [5_000u16, 6_000, 7_000, 8_000, 9_000] {
            let selected: Vec<_> = samples
                .iter()
                .filter(|(confidence, _)| *confidence >= threshold)
                .collect();
            let success_count = selected.iter().filter(|(_, success)| *success).count();
            selective_risk.push(SelectiveRiskPoint {
                threshold_bps: threshold,
                coverage_bps: ratio_bps(selected.len(), samples.len()),
                risk_bps: if selected.is_empty() {
                    0
                } else {
                    ratio_bps(selected.len() - success_count, selected.len())
                },
            });
        }
        Ok(CalibrationReport {
            samples: samples.len(),
            kappa_millis: agreement.kappa_millis,
            ece_bps,
            brier_micros,
            bins,
            selective_risk,
            calibration_allowed: true,
        })
    }
}

fn consensus_success(labels: &[AnnotationLabel]) -> Option<bool> {
    let supported = labels
        .iter()
        .filter(|label| **label == AnnotationLabel::Supported)
        .count();
    let unsupported = labels
        .iter()
        .filter(|label| **label == AnnotationLabel::Unsupported)
        .count();
    if supported > unsupported {
        Some(true)
    } else if unsupported > supported {
        Some(false)
    } else {
        None
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
    #[error(
        "insufficient calibration evidence: kappa={kappa_millis}, eligible_cases={eligible_cases}"
    )]
    InsufficientCalibrationEvidence {
        kappa_millis: i32,
        eligible_cases: usize,
    },
    #[error("insufficient calibration samples")]
    InsufficientCalibrationSamples,
    #[error("confidence must be between 0 and 10000 basis points")]
    InvalidConfidence,
    #[error("duplicate calibration prediction")]
    DuplicatePrediction,
    #[error("suite bytes no longer match the frozen gold-set manifest")]
    GoldSetDrift,
    #[error("invalid gold-set manifest: {0}")]
    InvalidGoldSet(String),
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

    #[test]
    fn calibration_requires_reliable_labels_and_reports_metrics() {
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
        let report = store
            .calibrate(
                vec![
                    CalibrationPrediction {
                        case_id: "c1".into(),
                        confidence_bps: 8_000,
                    },
                    CalibrationPrediction {
                        case_id: "c2".into(),
                        confidence_bps: 2_000,
                    },
                ],
                600,
                2,
            )
            .unwrap();
        assert!(report.calibration_allowed);
        assert_eq!(report.samples, 2);
        assert_eq!(report.ece_bps, 2_000);
        assert_eq!(report.brier_micros, 4_000_000);
    }

    #[test]
    fn calibration_refuses_unreliable_labels() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = AnnotationStore::open(dir.path()).unwrap();
        store
            .add(record("c1", "a1", AnnotationLabel::Supported))
            .unwrap();
        assert!(matches!(
            store.calibrate(
                vec![CalibrationPrediction {
                    case_id: "c1".into(),
                    confidence_bps: 8_000
                }],
                600,
                1
            ),
            Err(AnnotationError::InsufficientCalibrationEvidence { .. })
        ));
    }

    #[test]
    fn goldset_manifest_binds_suite_and_batches() {
        let dir = tempfile::tempdir().unwrap();
        let suite = dir.path().join("suite.json");
        std::fs::write(
            &suite,
            serde_json::to_vec(&serde_json::json!({
                "name": "suite-v1",
                "cases": [{"id":"c1"},{"id":"c2"},{"id":"c3"}]
            }))
            .unwrap(),
        )
        .unwrap();
        let manifest = create_goldset_manifest(
            &suite,
            "1".into(),
            "owner".into(),
            vec!["reviewer-a".into(), "reviewer-b".into()],
            "redact secrets and personal data".into(),
            2,
        )
        .unwrap();
        assert_eq!(manifest.batches.len(), 2);
        verify_goldset_manifest(&manifest, &suite).unwrap();
        std::fs::write(&suite, "{\"name\":\"changed\",\"cases\":[]}").unwrap();
        assert!(matches!(
            verify_goldset_manifest(&manifest, &suite),
            Err(AnnotationError::GoldSetDrift)
        ));
    }
}
