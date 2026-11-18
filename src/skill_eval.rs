//! Static held-out evaluation gate for skills.
//!
//! This is intentionally not a runtime canary. It checks a skill manifest
//! against an operator-supplied held-out policy suite before activation.

use std::future::Future;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::skill::SkillManifest;

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
    #[error("skill runtime eval case {0} has an empty prompt")]
    EmptyPrompt(String),
    #[error("skill runtime eval failed: {0}")]
    Runtime(String),
    #[error("skill eval split must be 1..=9999 basis points")]
    InvalidSplit,
    #[error("skill eval split needs at least two cases and a non-empty test side")]
    EmptySplit,
    #[error("blind comparator case id {0} is invalid")]
    InvalidBlindCase(String),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillRuntimeCase {
    pub case_id: String,
    pub prompt: String,
    #[serde(default)]
    pub required_markers: Vec<String>,
    #[serde(default)]
    pub forbidden_markers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillRuntimeSuite {
    pub suite_id: String,
    /// 0..=10000; the skill is accepted only when the score is at least this.
    pub min_score_bps: u16,
    pub cases: Vec<SkillRuntimeCase>,
}

impl SkillRuntimeSuite {
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
            if case.prompt.trim().is_empty() {
                return Err(SkillEvalError::EmptyPrompt(case.case_id.clone()));
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
pub struct SkillRuntimeCaseResult {
    pub case_id: String,
    pub passed: bool,
    pub output_sha256: String,
    pub output_chars: usize,
    pub missing_required: Vec<String>,
    pub forbidden_hits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillRuntimeReport {
    pub suite_id: String,
    pub skill: String,
    pub version: String,
    pub passed: bool,
    pub score_bps: u16,
    pub cases: Vec<SkillRuntimeCaseResult>,
}

/// Execute every case through a model (or another runner), then score the
/// output with held-out required/forbidden markers. The raw output is hashed
/// and not stored in the report.
pub async fn evaluate_skill_runtime<F, Fut>(
    manifest: &SkillManifest,
    suite: &SkillRuntimeSuite,
    mut runner: F,
) -> Result<SkillRuntimeReport, SkillEvalError>
where
    F: FnMut(String, String) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    suite.validate()?;
    evaluate_skill_runtime_subset(
        manifest,
        &suite.suite_id,
        suite.min_score_bps,
        &suite.cases,
        &mut runner,
    )
    .await
}

/// Evaluate only the supplied cases. Used by held-out split evaluation.
pub async fn evaluate_skill_runtime_subset<F, Fut>(
    manifest: &SkillManifest,
    suite_id: &str,
    min_score_bps: u16,
    eval_cases: &[SkillRuntimeCase],
    runner: &mut F,
) -> Result<SkillRuntimeReport, SkillEvalError>
where
    F: FnMut(String, String) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    if manifest.body.trim().is_empty() {
        return Err(SkillEvalError::EmptyBody);
    }
    if eval_cases.is_empty() {
        return Err(SkillEvalError::EmptySuite);
    }
    let mut cases = Vec::with_capacity(eval_cases.len());
    for case in eval_cases {
        let output = runner(manifest.body.clone(), case.prompt.clone()).await;
        let (passed, output_sha256, output_chars, missing_required, forbidden_hits) = match output {
            Ok(output) => {
                let lower = output.to_lowercase();
                let missing = case
                    .required_markers
                    .iter()
                    .filter(|marker| !lower.contains(&marker.to_lowercase()))
                    .cloned()
                    .collect::<Vec<_>>();
                let forbidden = case
                    .forbidden_markers
                    .iter()
                    .filter(|marker| lower.contains(&marker.to_lowercase()))
                    .cloned()
                    .collect::<Vec<_>>();
                (
                    missing.is_empty() && forbidden.is_empty(),
                    hex::encode(Sha256::digest(output.as_bytes())),
                    output.chars().count(),
                    missing,
                    forbidden,
                )
            }
            Err(error) => (
                false,
                String::new(),
                0,
                case.required_markers.clone(),
                vec![format!("<runtime error: {error}>")],
            ),
        };
        cases.push(SkillRuntimeCaseResult {
            case_id: case.case_id.clone(),
            passed,
            output_sha256,
            output_chars,
            missing_required,
            forbidden_hits,
        });
    }
    let passed_cases = cases.iter().filter(|case| case.passed).count();
    let score_bps = ((passed_cases * 10_000) / cases.len()) as u16;
    Ok(SkillRuntimeReport {
        suite_id: suite_id.into(),
        skill: manifest.name.clone(),
        version: manifest.version.clone(),
        passed: score_bps >= min_score_bps,
        score_bps,
        cases,
    })
}

/// Deterministic held-out split: cases are ordered by a seeded SHA-256 of
/// their id and the first `test_bps` of them form the test side. Both sides
/// are non-empty, so tiny suites fail closed instead of producing a
/// meaningless 100% score.
#[cfg(test)]
pub fn split_runtime_cases(
    cases: &[SkillRuntimeCase],
    seed: &str,
    test_bps: u16,
) -> Result<(Vec<SkillRuntimeCase>, Vec<SkillRuntimeCase>), SkillEvalError> {
    if cases.len() < 2 {
        return Err(SkillEvalError::EmptySplit);
    }
    if !(1..=9_999).contains(&test_bps) {
        return Err(SkillEvalError::InvalidSplit);
    }
    let mut ordered = (0..cases.len()).collect::<Vec<_>>();
    ordered.sort_by_key(|index| {
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(b"\0");
        hasher.update(cases[*index].case_id.as_bytes());
        hasher.finalize()
    });
    let test_count = ((ordered.len() as u64 * u64::from(test_bps)) / 10_000)
        .clamp(1, ordered.len() as u64 - 1) as usize;
    let mut test = Vec::with_capacity(test_count);
    let mut train = Vec::with_capacity(ordered.len() - test_count);
    for (position, index) in ordered.into_iter().enumerate() {
        if position < test_count {
            test.push(cases[index].clone());
        } else {
            train.push(cases[index].clone());
        }
    }
    Ok((train, test))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlindSide {
    A,
    B,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlindCase {
    pub case_id: String,
    pub baseline_output: String,
    pub candidate_output: String,
    #[serde(default)]
    pub required_markers: Vec<String>,
    #[serde(default)]
    pub forbidden_markers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlindSuite {
    pub suite_id: String,
    /// Seed fixes the A/B arm assignment; the same suite reproduces the same
    /// blind comparison.
    #[serde(default)]
    pub seed: String,
    pub cases: Vec<BlindCase>,
}

impl BlindSuite {
    pub fn compare(&self) -> Result<BlindReport, SkillEvalError> {
        compare_blind(&self.suite_id, &self.seed, &self.cases)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlindCaseResult {
    pub case_id: String,
    pub candidate_side: BlindSide,
    pub candidate_passed: bool,
    pub baseline_passed: bool,
    pub outcome: String,
    pub candidate_output_sha256: String,
    pub baseline_output_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlindReport {
    pub suite_id: String,
    pub cases: Vec<BlindCaseResult>,
    pub candidate_passes: usize,
    pub baseline_passes: usize,
    pub candidate_wins: usize,
    pub baseline_wins: usize,
    pub ties: usize,
    pub baseline_bps: u16,
    pub candidate_bps: u16,
    pub delta_bps: i32,
    /// Conservative 95% lower bound of (candidate - baseline) using Wilson
    /// intervals; positive means the lift survives the interval check.
    pub delta_lower_95_bps: i32,
    pub delta_upper_95_bps: i32,
    pub significant: bool,
}

fn marker_passes(output: &str, required: &[String], forbidden: &[String]) -> bool {
    let lower = output.to_lowercase();
    required
        .iter()
        .all(|marker| lower.contains(&marker.to_lowercase()))
        && !forbidden
            .iter()
            .any(|marker| lower.contains(&marker.to_lowercase()))
}

fn output_hash(output: &str) -> String {
    hex::encode(Sha256::digest(output.as_bytes()))
}

/// Blind A/B comparison. Each case is assigned to A/B by a seeded hash, so
/// the marker evaluator never sees which arm is the candidate; raw outputs
/// are hashed, never stored in the report.
pub fn compare_blind(
    suite_id: &str,
    seed: &str,
    cases: &[BlindCase],
) -> Result<BlindReport, SkillEvalError> {
    if !safe_id(suite_id) {
        return Err(SkillEvalError::UnsafeId);
    }
    if cases.is_empty() {
        return Err(SkillEvalError::EmptySuite);
    }
    let mut results = Vec::with_capacity(cases.len());
    let mut candidate_passes = 0;
    let mut baseline_passes = 0;
    let mut candidate_wins = 0;
    let mut baseline_wins = 0;
    let mut ties = 0;
    for case in cases {
        if !safe_id(&case.case_id) {
            return Err(SkillEvalError::InvalidBlindCase(case.case_id.clone()));
        }
        let mut hasher = Sha256::new();
        hasher.update(seed.as_bytes());
        hasher.update(b"\0");
        hasher.update(suite_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(case.case_id.as_bytes());
        let digest = hasher.finalize();
        let candidate_is_a = digest[0] & 1 == 0;
        let candidate_side = if candidate_is_a {
            BlindSide::A
        } else {
            BlindSide::B
        };
        let (arm_a, arm_b) = if candidate_is_a {
            (&case.candidate_output, &case.baseline_output)
        } else {
            (&case.baseline_output, &case.candidate_output)
        };
        let a_passed = marker_passes(arm_a, &case.required_markers, &case.forbidden_markers);
        let b_passed = marker_passes(arm_b, &case.required_markers, &case.forbidden_markers);
        let candidate_passed = if candidate_is_a { a_passed } else { b_passed };
        let baseline_passed = if candidate_is_a { b_passed } else { a_passed };
        let outcome = match (candidate_passed, baseline_passed) {
            (true, true) | (false, false) => {
                ties += 1;
                "tie"
            }
            (true, false) => {
                candidate_wins += 1;
                "candidate"
            }
            (false, true) => {
                baseline_wins += 1;
                "baseline"
            }
        };
        candidate_passes += usize::from(candidate_passed);
        baseline_passes += usize::from(baseline_passed);
        results.push(BlindCaseResult {
            case_id: case.case_id.clone(),
            candidate_side,
            candidate_passed,
            baseline_passed,
            outcome: outcome.into(),
            candidate_output_sha256: output_hash(&case.candidate_output),
            baseline_output_sha256: output_hash(&case.baseline_output),
        });
    }
    let total = results.len();
    let baseline_bps = ((baseline_passes * 10_000) / total) as u16;
    let candidate_bps = ((candidate_passes * 10_000) / total) as u16;
    let (base_low, base_high) = wilson_bps(baseline_passes, total);
    let (cand_low, cand_high) = wilson_bps(candidate_passes, total);
    let delta_lower = cand_low as i32 - base_high as i32;
    let delta_upper = cand_high as i32 - base_low as i32;
    Ok(BlindReport {
        suite_id: suite_id.into(),
        candidate_passes,
        baseline_passes,
        candidate_wins,
        baseline_wins,
        ties,
        cases: results,
        baseline_bps,
        candidate_bps,
        delta_bps: candidate_bps as i32 - baseline_bps as i32,
        delta_lower_95_bps: delta_lower,
        delta_upper_95_bps: delta_upper,
        significant: delta_lower > 0,
    })
}

/// 95% Wilson score interval in basis points.
fn wilson_bps(successes: usize, total: usize) -> (u16, u16) {
    if total == 0 {
        return (0, 10_000);
    }
    let n = total as f64;
    let p = successes as f64 / n;
    let z = 1.96_f64;
    let denominator = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denominator;
    let margin = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    (
        ((center - margin).clamp(0.0, 1.0) * 10_000.0).round() as u16,
        ((center + margin).clamp(0.0, 1.0) * 10_000.0).round() as u16,
    )
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with('.')
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

    #[tokio::test]
    async fn runtime_eval_scores_held_out_outputs_without_storing_them() {
        let suite = SkillRuntimeSuite {
            suite_id: "runtime".into(),
            min_score_bps: 10_000,
            cases: vec![SkillRuntimeCase {
                case_id: "verify".into(),
                prompt: "release the package".into(),
                required_markers: vec!["verify".into()],
                forbidden_markers: vec!["rm -rf".into()],
            }],
        };
        let report =
            evaluate_skill_runtime(&manifest("1. verify"), &suite, |_system, _user| async {
                Ok::<_, String>("1. verify\n2. build".into())
            })
            .await
            .unwrap();
        assert!(report.passed);
        assert_eq!(report.cases[0].output_chars, 18);
        assert_eq!(report.cases[0].output_sha256.len(), 64);

        let report =
            evaluate_skill_runtime(&manifest("1. verify"), &suite, |_system, _user| async {
                Ok::<_, String>("rm -rf /".into())
            })
            .await
            .unwrap();
        assert!(!report.passed);
        assert_eq!(report.cases[0].forbidden_hits, vec!["rm -rf"]);
    }

    #[test]
    fn held_out_split_is_deterministic_and_both_sides_non_empty() {
        let cases = (0..10)
            .map(|index| SkillRuntimeCase {
                case_id: format!("case-{index}"),
                prompt: "p".into(),
                required_markers: vec!["ok".into()],
                forbidden_markers: vec![],
            })
            .collect::<Vec<_>>();
        let (train, test) = split_runtime_cases(&cases, "seed-1", 3_000).unwrap();
        assert_eq!(train.len(), 7);
        assert_eq!(test.len(), 3);
        let (_, again) = split_runtime_cases(&cases, "seed-1", 3_000).unwrap();
        assert_eq!(
            test.iter().map(|case| &case.case_id).collect::<Vec<_>>(),
            again.iter().map(|case| &case.case_id).collect::<Vec<_>>()
        );
        assert!(split_runtime_cases(&cases, "seed-1", 0).is_err());
        assert!(split_runtime_cases(&cases[..1], "seed-1", 5_000).is_err());
    }

    #[test]
    fn blind_comparator_hides_arms_and_reports_a_significant_lift() {
        let cases = (0..20)
            .map(|index| BlindCase {
                case_id: format!("case-{index}"),
                baseline_output: "wrong".into(),
                candidate_output: "verify build".into(),
                required_markers: vec!["verify".into()],
                forbidden_markers: vec![],
            })
            .collect::<Vec<_>>();
        let report = compare_blind("blind-suite", "seed", &cases).unwrap();
        assert_eq!(report.candidate_wins, 20);
        assert_eq!(report.baseline_wins, 0);
        assert_eq!(report.ties, 0);
        assert_eq!(report.candidate_bps, 10_000);
        assert_eq!(report.baseline_bps, 0);
        assert!(report.significant, "{report:?}");
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.candidate_side == BlindSide::A)
        );
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.candidate_side == BlindSide::B)
        );
        assert!(report.cases[0].candidate_output_sha256.len() == 64);

        let tie = compare_blind(
            "blind-suite",
            "seed",
            &[BlindCase {
                case_id: "both".into(),
                baseline_output: "verify".into(),
                candidate_output: "verify".into(),
                required_markers: vec!["verify".into()],
                forbidden_markers: vec![],
            }],
        )
        .unwrap();
        assert_eq!(tie.ties, 1);
        assert!(!tie.significant);
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
