//! Static held-out evaluation gate for skills.
//!
//! This is intentionally not a runtime canary. It checks a skill manifest
//! against an operator-supplied held-out policy suite before activation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::skill::{SkillError, SkillManifest};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillEvalError {
    #[error("skill eval id is unsafe")]
    UnsafeId,
    #[error("skill eval suite must have at least one case")]
    EmptySuite,
    #[error("skill eval threshold must be 0..=10000")]
    InvalidThreshold,
    #[error("skill eval case {0} has no markers")]
    EmptyCase(String),
    #[error("skill eval case {0} has an empty marker")]
    EmptyMarker(String),
    #[error("skill eval body is empty")]
    EmptyBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillEvalCase {
    pub case_id: String,
    #[serde(default)]
    pub required_markers: Vec<String>,
    #[serde(default)]
    pub forbidden_markers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillEvalSuite {
    pub suite_id: String,
    /// 0..=10000; the skill activates only when the score is at least this.
    pub min_score_bps: u16,
    pub cases: Vec<SkillEvalCase>,
}

impl SkillEvalSuite {
    pub fn validate(&self) -> Result<(), SkillEvalError> {
        if !safe_id(&self.suite_id) {
            return Err(SkillEvalError::UnsafeId);
        }
        if self.cases.is_empty() {
            return Err(SkillEvalError::EmptySuite);
        }
        if self.min_score_bps > 10_000 {
            return Err(SkillEvalError::InvalidThreshold);
        }
        for case in &self.cases {
            if !safe_id(&case.case_id) {
                return Err(SkillEvalError::UnsafeId);
            }
            if case.required_markers.is_empty() && case.forbidden_markers.is_empty() {
                return Err(SkillEvalError::EmptyCase(case.case_id.clone()));
            }
            if case
                .required_markers
                .iter()
                .chain(case.forbidden_markers.iter())
                .any(|marker| marker.trim().is_empty())
            {
                return Err(SkillEvalError::EmptyMarker(case.case_id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillEvalCaseResult {
    pub case_id: String,
    pub passed: bool,
    pub missing_required: Vec<String>,
    pub forbidden_hits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillEvalReport {
    pub suite_id: String,
    pub skill: String,
    pub version: String,
    pub passed: bool,
    pub score_bps: u16,
    pub cases: Vec<SkillEvalCaseResult>,
}

pub fn evaluate_skill(
    manifest: &SkillManifest,
    suite: &SkillEvalSuite,
) -> Result<SkillEvalReport, SkillEvalError> {
    suite.validate()?;
    if manifest.body.trim().is_empty() {
        return Err(SkillEvalError::EmptyBody);
    }
    let body = manifest.body.to_lowercase();
    let mut cases = Vec::with_capacity(suite.cases.len());
    for case in &suite.cases {
        let missing_required = case
            .required_markers
            .iter()
            .filter(|marker| !body.contains(&marker.to_lowercase()))
            .cloned()
            .collect::<Vec<_>>();
        let forbidden_hits = case
            .forbidden_markers
            .iter()
            .filter(|marker| body.contains(&marker.to_lowercase()))
            .cloned()
            .collect::<Vec<_>>();
        cases.push(SkillEvalCaseResult {
            case_id: case.case_id.clone(),
            passed: missing_required.is_empty() && forbidden_hits.is_empty(),
            missing_required,
            forbidden_hits,
        });
    }
    let passed_cases = cases.iter().filter(|case| case.passed).count();
    let score_bps = ((passed_cases * 10_000) / cases.len()) as u16;
    Ok(SkillEvalReport {
        suite_id: suite.suite_id.clone(),
        skill: manifest.name.clone(),
        version: manifest.version.clone(),
        passed: score_bps >= suite.min_score_bps,
        score_bps,
        cases,
    })
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with('.')
}

pub fn unknown_skill(name: &str, version: &str) -> SkillError {
    SkillError::UnknownVersion(name.into(), version.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillRisk;
    use sha2::{Digest, Sha256};

    fn hash(body: &str) -> String {
        hex::encode(Sha256::digest(body.as_bytes()))
    }

    fn manifest(body: &str) -> SkillManifest {
        SkillManifest {
            name: "release".into(),
            description: "release process".into(),
            version: "1.0.0".into(),
            platforms: vec!["linux".into()],
            scope: "repo".into(),
            requires_tools: Vec::new(),
            risk: SkillRisk::Low,
            body: body.into(),
            content_sha256: hash(body),
        }
    }

    fn suite() -> SkillEvalSuite {
        SkillEvalSuite {
            suite_id: "release-suite".into(),
            min_score_bps: 10_000,
            cases: vec![
                SkillEvalCase {
                    case_id: "has-verify".into(),
                    required_markers: vec!["verify".into()],
                    forbidden_markers: Vec::new(),
                },
                SkillEvalCase {
                    case_id: "no-destructive".into(),
                    required_markers: Vec::new(),
                    forbidden_markers: vec!["rm -rf".into()],
                },
            ],
        }
    }

    #[test]
    fn evaluates_required_and_forbidden_markers() {
        let report = evaluate_skill(&manifest("1. verify\n2. build"), &suite()).unwrap();
        assert!(report.passed);
        assert_eq!(report.score_bps, 10_000);
        let report = evaluate_skill(&manifest("1. verify\n2. rm -rf /"), &suite()).unwrap();
        assert!(!report.passed);
        assert_eq!(report.score_bps, 5_000);
        assert_eq!(report.cases[1].forbidden_hits, vec!["rm -rf"]);
    }

    #[test]
    fn invalid_suites_and_empty_bodies_fail_closed() {
        let mut invalid = suite();
        invalid.cases.clear();
        assert_eq!(
            evaluate_skill(&manifest("verify"), &invalid).unwrap_err(),
            SkillEvalError::EmptySuite
        );
        assert_eq!(
            evaluate_skill(&manifest(""), &suite()).unwrap_err(),
            SkillEvalError::EmptyBody
        );
    }
}
