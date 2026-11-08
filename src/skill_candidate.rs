//! Skill candidates and demonstration capture.
//!
//! Demonstrations are stored as hashes and metadata only; raw user input and
//! model output never enter the candidate store.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::skill::{SkillError, SkillManifest};
use crate::skill_eval::{SkillEvalError, SkillEvalReport, SkillEvalSuite, evaluate_skill};
use crate::skill_journal::SkillJournalError;
use crate::skill_service::{SkillService, SkillServiceError};

const MAX_DEMONSTRATIONS: usize = 32;

#[derive(Debug, Error)]
pub enum SkillCandidateError {
    #[error("skill candidate id is unsafe")]
    UnsafeId,
    #[error("skill candidate {0} not found")]
    UnknownCandidate(String),
    #[error("skill candidate {0} has too many demonstrations")]
    TooManyDemonstrations(String),
    #[error("skill candidate {0} must be evaluated before activation")]
    NotEvaluated(String),
    #[error("skill candidate {0} did not pass evaluation")]
    EvalFailed(String),
    #[error("skill candidate already activated")]
    AlreadyActivated,
    #[error("skill candidate eval: {0}")]
    Eval(#[from] SkillEvalError),
    #[error("skill candidate service: {0}")]
    Service(#[from] SkillServiceError),
    #[error("skill candidate journal: {0}")]
    Journal(#[from] SkillJournalError),
    #[error("skill candidate skill: {0}")]
    Skill(#[from] SkillError),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Proposed,
    Evaluated,
    Activated,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Demonstration {
    pub demonstration_id: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub input_chars: u64,
    pub output_chars: u64,
    pub accepted: bool,
    pub captured_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillCandidate {
    pub candidate_id: String,
    pub manifest: SkillManifest,
    pub demonstrations: Vec<Demonstration>,
    pub eval: Option<SkillEvalReport>,
    pub status: CandidateStatus,
}

#[derive(Debug, Default)]
pub struct SkillCandidateStore {
    candidates: BTreeMap<String, SkillCandidate>,
}

impl SkillCandidateStore {
    pub fn propose(&mut self, manifest: SkillManifest, now_ms: u128) -> SkillCandidate {
        let candidate = SkillCandidate {
            candidate_id: format!("candidate-{}", Uuid::new_v4()),
            manifest,
            demonstrations: Vec::new(),
            eval: None,
            status: CandidateStatus::Proposed,
        };
        let _ = now_ms;
        self.candidates
            .insert(candidate.candidate_id.clone(), candidate.clone());
        candidate
    }

    pub fn capture(
        &mut self,
        candidate_id: &str,
        input: &str,
        output: &str,
        accepted: bool,
        now_ms: u128,
    ) -> Result<Demonstration, SkillCandidateError> {
        let candidate = self
            .candidates
            .get_mut(candidate_id)
            .ok_or_else(|| SkillCandidateError::UnknownCandidate(candidate_id.into()))?;
        if candidate.demonstrations.len() >= MAX_DEMONSTRATIONS {
            return Err(SkillCandidateError::TooManyDemonstrations(
                candidate_id.into(),
            ));
        }
        let demonstration = Demonstration {
            demonstration_id: format!("demo-{}", Uuid::new_v4()),
            input_sha256: hash(input),
            output_sha256: hash(output),
            input_chars: input.chars().count() as u64,
            output_chars: output.chars().count() as u64,
            accepted,
            captured_ms: now_ms,
        };
        candidate.demonstrations.push(demonstration.clone());
        Ok(demonstration)
    }

    pub fn evaluate(
        &mut self,
        candidate_id: &str,
        suite: &SkillEvalSuite,
    ) -> Result<SkillEvalReport, SkillCandidateError> {
        let candidate = self
            .candidates
            .get_mut(candidate_id)
            .ok_or_else(|| SkillCandidateError::UnknownCandidate(candidate_id.into()))?;
        let report = evaluate_skill(&candidate.manifest, suite)?;
        candidate.status = if report.passed {
            CandidateStatus::Evaluated
        } else {
            CandidateStatus::Rejected
        };
        candidate.eval = Some(report.clone());
        Ok(report)
    }

    pub fn activate(
        &mut self,
        candidate_id: &str,
        skills: &mut SkillService,
        now_ms: u128,
    ) -> Result<SkillEvalReport, SkillCandidateError> {
        let candidate = self
            .candidates
            .get_mut(candidate_id)
            .ok_or_else(|| SkillCandidateError::UnknownCandidate(candidate_id.into()))?;
        if candidate.status == CandidateStatus::Activated {
            return Err(SkillCandidateError::AlreadyActivated);
        }
        if candidate.status == CandidateStatus::Rejected {
            return Err(SkillCandidateError::EvalFailed(candidate_id.into()));
        }
        if candidate.status != CandidateStatus::Evaluated {
            return Err(SkillCandidateError::NotEvaluated(candidate_id.into()));
        }
        let report = candidate
            .eval
            .clone()
            .ok_or_else(|| SkillCandidateError::NotEvaluated(candidate_id.into()))?;
        if !report.passed {
            return Err(SkillCandidateError::EvalFailed(candidate_id.into()));
        }
        skills.register(candidate.manifest.clone(), now_ms)?;
        skills.activate(
            &candidate.manifest.name,
            &candidate.manifest.version,
            now_ms,
        )?;
        candidate.status = CandidateStatus::Activated;
        Ok(report)
    }

    pub fn get(&self, candidate_id: &str) -> Option<&SkillCandidate> {
        self.candidates.get(candidate_id)
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::SkillRisk;
    use crate::skill_eval::SkillEvalCase;

    fn manifest() -> SkillManifest {
        SkillManifest {
            name: "release".into(),
            description: "release process".into(),
            version: "1.0.0".into(),
            platforms: vec!["linux".into()],
            scope: "repo".into(),
            requires_tools: Vec::new(),
            risk: SkillRisk::Low,
            body: "1. verify\n2. build".into(),
            content_sha256: String::new(),
        }
    }

    fn suite() -> SkillEvalSuite {
        SkillEvalSuite {
            suite_id: "release-suite".into(),
            min_score_bps: 10_000,
            cases: vec![SkillEvalCase {
                case_id: "verify".into(),
                required_markers: vec!["verify".into()],
                forbidden_markers: vec!["rm -rf".into()],
            }],
        }
    }

    #[test]
    fn captures_only_hashes_and_activation_requires_evaluation() {
        let dir = tempfile::tempdir().unwrap();
        let mut skills = SkillService::open(dir.path().join("skills.jsonl")).unwrap();
        let mut candidates = SkillCandidateStore::default();
        let candidate = candidates.propose(manifest(), 10);
        let demo = candidates
            .capture(
                &candidate.candidate_id,
                "secret input",
                "secret output",
                true,
                11,
            )
            .unwrap();
        assert!(!format!("{demo:?}").contains("secret"));
        assert_eq!(demo.input_chars, 12);
        assert!(matches!(
            candidates
                .activate(&candidate.candidate_id, &mut skills, 12)
                .unwrap_err(),
            SkillCandidateError::NotEvaluated(_)
        ));
        let report = candidates
            .evaluate(&candidate.candidate_id, &suite())
            .unwrap();
        assert!(report.passed);
        candidates
            .activate(&candidate.candidate_id, &mut skills, 13)
            .unwrap();
        assert_eq!(skills.active("release").unwrap().version, "1.0.0");
    }

    #[test]
    fn failed_evaluation_cannot_activate_and_capture_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut skills = SkillService::open(dir.path().join("skills.jsonl")).unwrap();
        let mut candidates = SkillCandidateStore::default();
        let mut bad = manifest();
        bad.body = "rm -rf /".into();
        let candidate = candidates.propose(bad, 10);
        let report = candidates
            .evaluate(&candidate.candidate_id, &suite())
            .unwrap();
        assert!(!report.passed);
        assert!(matches!(
            candidates
                .activate(&candidate.candidate_id, &mut skills, 11)
                .unwrap_err(),
            SkillCandidateError::EvalFailed(_)
        ));
        for index in 0..MAX_DEMONSTRATIONS {
            candidates
                .capture(&candidate.candidate_id, "in", "out", true, index as u128)
                .unwrap();
        }
        assert!(matches!(
            candidates
                .capture(&candidate.candidate_id, "in", "out", true, 100)
                .unwrap_err(),
            SkillCandidateError::TooManyDemonstrations(_)
        ));
    }
}
