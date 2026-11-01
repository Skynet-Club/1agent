use sha2::{Digest, Sha256};

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::domain::{AnswerStatus, ClaimStatus, StopReason, TaskRequest, ToolStatus, Usage};
use crate::experience::{ExperienceCandidate, ExperienceVerificationInput};
use crate::journal::{audit_events, read_events};
use crate::provider::DeterministicProvider;
use crate::runtime::{AgentRuntime, ProviderCandidate, local_manifest};
use crate::tools::default_registry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalSuite {
    pub name: String,
    #[serde(default = "default_suite_version")]
    pub suite_version: String,
    #[serde(default)]
    pub kind: EvalSuiteKind,
    #[serde(default)]
    pub execution_status: EvalExecutionStatus,
    #[serde(default)]
    pub provider_requirements: Vec<String>,
    #[serde(default)]
    pub reviewer: Option<String>,
    #[serde(default)]
    pub calibration_required: bool,
    #[serde(default)]
    pub min_decision_completeness_bps: Option<u16>,
    pub cases: Vec<EvalCase>,
}

fn default_suite_version() -> String {
    "1".into()
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvalSuiteKind {
    #[default]
    Unspecified,
    ContractFixture,
    ModelQuality,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvalExecutionStatus {
    #[default]
    Unspecified,
    Template,
    Draft,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    pub id: String,
    pub objective: String,
    #[serde(default)]
    pub setup_files: Vec<SetupFile>,
    #[serde(default)]
    pub experience_scope: Option<String>,
    #[serde(default)]
    pub rubric: Option<EvalRubric>,
    pub expect: EvalExpectation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalRubric {
    #[serde(default)]
    pub required_conclusion_terms: Vec<String>,
    #[serde(default)]
    pub forbidden_conclusion_terms: Vec<String>,
    #[serde(default)]
    pub required_sources: Vec<String>,
    #[serde(default)]
    pub required_claim_statuses: Vec<ClaimStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SetupFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalExpectation {
    pub stop_reason: StopReason,
    pub answer_status: AnswerStatus,
    #[serde(default)]
    pub min_evidence: usize,
    #[serde(default)]
    pub min_claims: usize,
    #[serde(default)]
    pub min_unknowns: usize,
    #[serde(default)]
    pub min_flip_conditions: usize,
    #[serde(default)]
    pub min_high_impact_unknowns: usize,
    #[serde(default)]
    pub min_high_impact_assumptions: usize,
    #[serde(default)]
    pub min_unresolved_conflicts: usize,
    #[serde(default)]
    pub expect_checkpoint: bool,
    #[serde(default)]
    pub tool_status: Option<ToolStatus>,
    #[serde(default)]
    pub tool_error_code: Option<String>,
    #[serde(default = "default_true")]
    pub audit_ok: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvalCaseResult {
    pub id: String,
    pub passed: bool,
    pub hard_gate_failed: bool,
    pub stop_reason: Option<StopReason>,
    pub answer_status: Option<AnswerStatus>,
    pub evidence_count: usize,
    pub claim_count: usize,
    pub unknown_count: usize,
    pub low_impact_unknown_count: usize,
    pub medium_impact_unknown_count: usize,
    pub high_impact_unknown_count: usize,
    pub low_impact_assumption_count: usize,
    pub medium_impact_assumption_count: usize,
    pub high_impact_assumption_count: usize,
    pub conflict_count: usize,
    pub unresolved_conflict_count: usize,
    pub stale_evidence_count: usize,
    pub flip_condition_count: usize,
    pub decision_completeness: DecisionCompletenessRollup,
    pub rubric_checks: usize,
    pub rubric_passed: usize,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub duration_ms: u64,
    pub usage: Usage,
    pub audit_ok: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionCompletenessRollup {
    pub conditional_answers: usize,
    pub conditional_with_bound_flip: usize,
    pub bound_flip_conditions: usize,
    pub unbound_flip_conditions: usize,
    pub high_impact_unknowns_without_reduction: usize,
    pub completed_with_high_impact_assumptions: usize,
    pub ok: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionCompletenessAggregate {
    pub cases: usize,
    pub ok_cases: usize,
    pub completion_rate_bps: u16,
    pub conditional_answers: usize,
    pub conditional_with_bound_flip: usize,
    pub bound_flip_conditions: usize,
    pub unbound_flip_conditions: usize,
    pub high_impact_unknowns_without_reduction: usize,
    pub completed_with_high_impact_assumptions: usize,
}

pub const DECISION_COMPLETENESS_POLICY_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionCompletenessPolicy {
    pub version: u8,
    pub floor_source: DecisionCompletenessFloorSource,
}

impl Default for DecisionCompletenessPolicy {
    fn default() -> Self {
        Self {
            version: 0,
            floor_source: DecisionCompletenessFloorSource::Legacy,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionCompletenessFloorSource {
    Legacy,
    Default,
    Explicit,
}

impl DecisionCompletenessFloorSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Default => "default",
            Self::Explicit => "explicit",
        }
    }
}

impl DecisionCompletenessPolicy {
    pub fn validate(&self) -> Result<(), EvalError> {
        match (self.version, self.floor_source) {
            (0, DecisionCompletenessFloorSource::Legacy)
            | (
                1,
                DecisionCompletenessFloorSource::Default
                | DecisionCompletenessFloorSource::Explicit,
            ) => Ok(()),
            (0 | 1, _) => Err(EvalError::InvalidDecisionCompletenessPolicyCombination),
            (version, _) => Err(EvalError::UnsupportedDecisionCompletenessPolicyVersion(
                version,
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvalReport {
    pub suite: String,
    pub kind: EvalSuiteKind,
    pub execution_status: EvalExecutionStatus,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub hard_gate_failures: usize,
    pub rubric_checks: usize,
    pub rubric_passed: usize,
    pub decision_completeness: DecisionCompletenessAggregate,
    pub min_decision_completeness_bps: u16,
    pub decision_completeness_floor_passed: bool,
    #[serde(default)]
    pub decision_completeness_policy: DecisionCompletenessPolicy,
    #[serde(default)]
    pub content_sha256: Option<String>,
    pub cases: Vec<EvalCaseResult>,
}

impl EvalReport {
    pub fn validate_integrity(&self) -> Result<(), EvalError> {
        self.decision_completeness_policy.validate()?;
        if self.min_decision_completeness_bps > 10_000 {
            return Err(EvalError::InvalidDecisionCompletenessFloor);
        }
        if self.total != self.cases.len() {
            return Err(EvalError::InvalidReportIntegrity(
                "case count does not match total",
            ));
        }
        let passed = self.cases.iter().filter(|case| case.passed).count();
        if self.passed != passed {
            return Err(EvalError::InvalidReportIntegrity(
                "passed count does not match cases",
            ));
        }
        let failed = self.cases.iter().filter(|case| !case.passed).count();
        if self.failed != failed || self.passed.checked_add(self.failed) != Some(self.total) {
            return Err(EvalError::InvalidReportIntegrity(
                "failed count does not match cases",
            ));
        }
        let hard_gate_failures = self
            .cases
            .iter()
            .filter(|case| case.hard_gate_failed)
            .count();
        if self.hard_gate_failures != hard_gate_failures {
            return Err(EvalError::InvalidReportIntegrity(
                "hard-gate count does not match cases",
            ));
        }
        let rubric_checks: usize = self.cases.iter().map(|case| case.rubric_checks).sum();
        let rubric_passed: usize = self.cases.iter().map(|case| case.rubric_passed).sum();
        if self.rubric_checks != rubric_checks
            || self.rubric_passed != rubric_passed
            || self.rubric_passed > self.rubric_checks
        {
            return Err(EvalError::InvalidReportIntegrity(
                "rubric aggregates do not match cases",
            ));
        }
        if self.decision_completeness != aggregate_decision_completeness(&self.cases) {
            return Err(EvalError::InvalidReportIntegrity(
                "decision-completeness aggregates do not match cases",
            ));
        }
        let floor_passed =
            self.decision_completeness.completion_rate_bps >= self.min_decision_completeness_bps;
        if self.decision_completeness_floor_passed != floor_passed {
            return Err(EvalError::InvalidReportIntegrity(
                "decision-completeness floor status is inconsistent",
            ));
        }
        if let Some(expected) = &self.content_sha256
            && report_content_sha256(self)? != *expected
        {
            return Err(EvalError::InvalidReportIntegrity(
                "report content digest does not match payload",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalReportSummary {
    #[serde(default)]
    pub schema_version: u8,
    pub suite: String,
    pub kind: EvalSuiteKind,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub hard_gate_failures: usize,
    pub decision_completeness_cases: usize,
    pub decision_completeness_ok_cases: usize,
    pub decision_completeness_rate_bps: u16,
    pub min_decision_completeness_bps: u16,
    pub decision_completeness_floor_passed: bool,
    pub decision_completeness_policy: DecisionCompletenessPolicy,
    #[serde(default)]
    pub report_content_sha256: Option<String>,
    #[serde(default)]
    pub verification: EvalReportVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalReportVerification {
    pub integrity_checked: bool,
    pub content_binding: ContentBindingStatus,
    pub signature_present: bool,
    pub authenticity: AuthenticityStatus,
}

impl Default for EvalReportVerification {
    fn default() -> Self {
        Self {
            integrity_checked: false,
            content_binding: ContentBindingStatus::Unbound,
            signature_present: false,
            authenticity: AuthenticityStatus::Unauthenticated,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentBindingStatus {
    Bound,
    Unbound,
}

impl ContentBindingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Unbound => "unbound",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticityStatus {
    Unauthenticated,
    Verified,
}

impl AuthenticityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::Verified => "verified",
        }
    }
}

pub const REPORT_SUMMARY_SCHEMA_VERSION: u8 = 4;

impl From<&EvalReport> for EvalReportSummary {
    fn from(report: &EvalReport) -> Self {
        Self {
            schema_version: REPORT_SUMMARY_SCHEMA_VERSION,
            suite: report.suite.clone(),
            kind: report.kind,
            total: report.total,
            passed: report.passed,
            failed: report.failed,
            hard_gate_failures: report.hard_gate_failures,
            decision_completeness_cases: report.decision_completeness.cases,
            decision_completeness_ok_cases: report.decision_completeness.ok_cases,
            decision_completeness_rate_bps: report.decision_completeness.completion_rate_bps,
            min_decision_completeness_bps: report.min_decision_completeness_bps,
            decision_completeness_floor_passed: report.decision_completeness_floor_passed,
            decision_completeness_policy: report.decision_completeness_policy.clone(),
            report_content_sha256: report.content_sha256.clone(),
            verification: EvalReportVerification {
                integrity_checked: true,
                content_binding: if report.content_sha256.is_some() {
                    ContentBindingStatus::Bound
                } else {
                    ContentBindingStatus::Unbound
                },
                signature_present: false,
                authenticity: AuthenticityStatus::Unauthenticated,
            },
        }
    }
}

impl EvalReportSummary {
    pub fn validate(&self) -> Result<(), EvalError> {
        match self.schema_version {
            0..=REPORT_SUMMARY_SCHEMA_VERSION => {}
            version => return Err(EvalError::UnsupportedReportSummaryVersion(version)),
        }
        self.decision_completeness_policy.validate()?;
        if self.passed.checked_add(self.failed) != Some(self.total) {
            return Err(EvalError::InvalidReportSummary(
                "passed and failed counts do not sum to total",
            ));
        }
        if self.hard_gate_failures > self.failed {
            return Err(EvalError::InvalidReportSummary(
                "hard-gate failures exceed failed cases",
            ));
        }
        if self.decision_completeness_cases != self.total {
            return Err(EvalError::InvalidReportSummary(
                "decision-completeness case count does not match total",
            ));
        }
        if self.decision_completeness_ok_cases > self.decision_completeness_cases {
            return Err(EvalError::InvalidReportSummary(
                "decision-completeness passes exceed cases",
            ));
        }
        if self.min_decision_completeness_bps > 10_000
            || self.decision_completeness_rate_bps > 10_000
        {
            return Err(EvalError::InvalidReportSummary(
                "basis-point values exceed 10000",
            ));
        }
        let expected_rate = if self.decision_completeness_cases == 0 {
            0
        } else {
            ((self.decision_completeness_ok_cases as u128 * 10_000)
                / self.decision_completeness_cases as u128)
                .min(10_000) as u16
        };
        if self.decision_completeness_rate_bps != expected_rate {
            return Err(EvalError::InvalidReportSummary(
                "decision-completeness rate does not match counts",
            ));
        }
        let floor_passed =
            self.decision_completeness_rate_bps >= self.min_decision_completeness_bps;
        if self.decision_completeness_floor_passed != floor_passed {
            return Err(EvalError::InvalidReportSummary(
                "decision-completeness floor status is inconsistent",
            ));
        }
        if self.schema_version == REPORT_SUMMARY_SCHEMA_VERSION
            && !self.verification.integrity_checked
        {
            return Err(EvalError::InvalidReportSummary(
                "current summary must record integrity checking",
            ));
        }
        let digest_present = self.report_content_sha256.is_some();
        if (self.verification.content_binding == ContentBindingStatus::Bound) != digest_present {
            return Err(EvalError::InvalidReportSummary(
                "content-binding status does not match report digest",
            ));
        }
        if self.verification.authenticity == AuthenticityStatus::Verified
            && !self.verification.signature_present
        {
            return Err(EvalError::InvalidReportSummary(
                "verified authenticity requires a signature",
            ));
        }
        if self
            .report_content_sha256
            .as_deref()
            .is_some_and(|digest| !is_sha256_hex(digest))
        {
            return Err(EvalError::InvalidReportSummary(
                "report content digest must be a lowercase SHA-256",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvalReportTrustRequirements {
    pub require_content_binding: bool,
    pub require_authenticity: bool,
}

impl EvalReportTrustRequirements {
    pub fn validate(
        self,
        report: &EvalReport,
        authenticity_verified: bool,
    ) -> Result<(), EvalError> {
        report.validate_integrity()?;
        if self.require_content_binding && report.content_sha256.is_none() {
            return Err(EvalError::ReportContentBindingRequired);
        }
        if self.require_authenticity && !authenticity_verified {
            return Err(EvalError::ReportAuthenticityRequired);
        }
        Ok(())
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn report_content_sha256(report: &EvalReport) -> Result<String, EvalError> {
    let mut unhashed = report.clone();
    unhashed.content_sha256 = None;
    let bytes = serde_json::to_vec(&unhashed)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExperienceEvaluationReport {
    pub verification: ExperienceVerificationInput,
    pub baseline: EvalReport,
    pub learned: EvalReport,
}

pub async fn evaluate_experience_suite(
    suite_path: impl AsRef<Path>,
    candidate: ExperienceCandidate,
    providers: Vec<ProviderCandidate>,
    allow_network: bool,
    training_group: String,
    evaluation_group: String,
) -> Result<ExperienceEvaluationReport, EvalError> {
    if training_group == evaluation_group {
        return Err(EvalError::InvalidGrouping);
    }
    let bytes = std::fs::read(suite_path.as_ref())?;
    let suite: EvalSuite = serde_json::from_slice(&bytes)?;
    if suite.kind == EvalSuiteKind::ContractFixture {
        return Err(EvalError::FixtureCannotProveModelQuality);
    }
    ensure_runnable_model_quality(&suite)?;
    if suite.cases.is_empty() && suite.execution_status != EvalExecutionStatus::Template {
        return Err(EvalError::EmptySuite);
    }
    if suite.cases.iter().any(|case| {
        case.experience_scope
            .as_deref()
            .unwrap_or_default()
            .is_empty()
    }) {
        return Err(EvalError::MissingExperienceScope);
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let suite_sha256 = hex::encode(hasher.finalize());
    let case_ids = suite
        .cases
        .iter()
        .map(|case| case.id.clone())
        .collect::<Vec<_>>();
    let baseline =
        run_suite_with_learning(suite.clone(), providers.clone(), allow_network, Vec::new())
            .await?;
    let learned = run_suite_with_learning(
        suite.clone(),
        providers,
        allow_network,
        vec![candidate.clone()],
    )
    .await?;
    let baseline_success_bps = success_bps(&baseline);
    let learned_success_bps = success_bps(&learned);
    let baseline_unsupported_bps = unsupported_certainty_bps(&baseline);
    let learned_unsupported_bps = unsupported_certainty_bps(&learned);
    let baseline_duration: u64 = baseline.cases.iter().map(|case| case.duration_ms).sum();
    let learned_duration: u64 = learned.cases.iter().map(|case| case.duration_ms).sum();
    let cost_delta_bps = if baseline_duration == 0 {
        if learned_duration == 0 { 0 } else { 10_000 }
    } else {
        (((learned_duration as i128 - baseline_duration as i128) * 10_000)
            / baseline_duration as i128)
            .clamp(i32::MIN as i128, i32::MAX as i128) as i32
    };
    let verification = ExperienceVerificationInput {
        suite_id: suite.name,
        suite_version: suite.suite_version,
        case_ids,
        suite_sha256,
        training_group,
        evaluation_group,
        baseline_success_bps,
        learned_success_bps,
        baseline_unsupported_bps,
        learned_unsupported_bps,
        cost_delta_bps,
        evidence: format!(
            "baseline {}/{} passed; learned {}/{} passed; hard gates {} -> {}",
            baseline.passed,
            baseline.total,
            learned.passed,
            learned.total,
            baseline.hard_gate_failures,
            learned.hard_gate_failures
        ),
    };
    Ok(ExperienceEvaluationReport {
        verification,
        baseline,
        learned,
    })
}

fn success_bps(report: &EvalReport) -> u16 {
    ((report.passed * 10_000) / report.total) as u16
}

fn unsupported_certainty_bps(report: &EvalReport) -> u16 {
    let count = report
        .cases
        .iter()
        .filter(|case| case.answer_status == Some(AnswerStatus::Completed) && case.claim_count == 0)
        .count();
    ((count * 10_000) / report.total) as u16
}

pub fn load_suite(path: impl AsRef<Path>) -> Result<EvalSuite, EvalError> {
    let content = std::fs::read_to_string(path.as_ref())?;
    let suite: EvalSuite = serde_json::from_str(&content)?;
    if suite.cases.is_empty() && suite.execution_status != EvalExecutionStatus::Template {
        return Err(EvalError::EmptySuite);
    }
    Ok(suite)
}

pub fn load_eval_report(path: impl AsRef<Path>) -> Result<EvalReport, EvalError> {
    let content = std::fs::read_to_string(path.as_ref())?;
    let report: EvalReport = serde_json::from_str(&content)?;
    report.validate_integrity()?;
    Ok(report)
}

pub async fn run_suite(suite: EvalSuite) -> Result<EvalReport, EvalError> {
    run_suite_with_providers(
        suite,
        vec![ProviderCandidate {
            provider: std::sync::Arc::new(DeterministicProvider::new()),
            manifest: local_manifest(),
        }],
        false,
    )
    .await
}

pub async fn run_suite_with_providers(
    suite: EvalSuite,
    providers: Vec<ProviderCandidate>,
    allow_network: bool,
) -> Result<EvalReport, EvalError> {
    run_suite_with_learning(suite, providers, allow_network, Vec::new()).await
}

pub async fn run_suite_with_learning(
    suite: EvalSuite,
    providers: Vec<ProviderCandidate>,
    allow_network: bool,
    shadow_experience: Vec<ExperienceCandidate>,
) -> Result<EvalReport, EvalError> {
    if providers.is_empty() {
        return Err(EvalError::NoProviders);
    }
    validate_decision_completeness_floor(&suite)?;
    if suite.execution_status == EvalExecutionStatus::Template
        || suite.execution_status == EvalExecutionStatus::Draft
    {
        return Err(EvalError::SuiteNotRunnable);
    }
    if suite.kind == EvalSuiteKind::ModelQuality {
        ensure_runnable_model_quality(&suite)?;
    }
    let min_decision_completeness_bps = effective_decision_completeness_floor(&suite);
    let decision_completeness_policy = decision_completeness_policy(&suite);
    let mut results = Vec::new();
    for case in suite.cases {
        results.push(
            run_case(
                case,
                providers.clone(),
                allow_network,
                shadow_experience.clone(),
            )
            .await,
        );
    }
    let passed = results.iter().filter(|result| result.passed).count();
    let hard_gate_failures = results
        .iter()
        .filter(|result| result.hard_gate_failed)
        .count();
    let rubric_checks = results.iter().map(|result| result.rubric_checks).sum();
    let rubric_passed = results.iter().map(|result| result.rubric_passed).sum();
    let decision_completeness = aggregate_decision_completeness(&results);
    let decision_completeness_floor_passed =
        decision_completeness.completion_rate_bps >= min_decision_completeness_bps;
    let mut report = EvalReport {
        suite: suite.name,
        kind: suite.kind,
        execution_status: suite.execution_status,
        total: results.len(),
        passed,
        failed: results.len() - passed,
        hard_gate_failures,
        rubric_checks,
        rubric_passed,
        decision_completeness,
        min_decision_completeness_bps,
        decision_completeness_floor_passed,
        decision_completeness_policy,
        content_sha256: None,
        cases: results,
    };
    report.content_sha256 = Some(report_content_sha256(&report)?);
    Ok(report)
}

async fn run_case(
    case: EvalCase,
    providers: Vec<ProviderCandidate>,
    allow_network: bool,
    shadow_experience: Vec<ExperienceCandidate>,
) -> EvalCaseResult {
    let workspace = match tempfile::tempdir() {
        Ok(workspace) => workspace,
        Err(error) => {
            return failed_case(case.id, format!("cannot create workspace: {error}"));
        }
    };
    if let Err(error) = materialize_files(workspace.path(), &case.setup_files) {
        return failed_case(case.id, error);
    }
    let objective = case
        .objective
        .replace("${workspace}", &workspace.path().to_string_lossy());
    let mut request = TaskRequest::from_objective(objective, workspace.path().to_path_buf());
    request.permissions.allow_network = allow_network;
    request.experience_scope = case.experience_scope.clone();
    let mut runtime = AgentRuntime::new(
        providers,
        default_registry(),
        workspace.path().join(".journal"),
    );
    if !shadow_experience.is_empty() {
        runtime = runtime.with_shadow_experience_candidates(shadow_experience);
    }
    let started = std::time::Instant::now();
    let result = match runtime.run(request, CancellationToken::new()).await {
        Ok(result) => result,
        Err(error) => return failed_case(case.id, error.to_string()),
    };
    let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let events = match read_events(&result.journal_path) {
        Ok(events) => events,
        Err(error) => return failed_case(case.id, error.to_string()),
    };
    let audit = audit_events(&events);
    let mut issues = audit.issues.clone();
    let mut provider_id = None;
    let mut model = None;
    let mut usage = Usage::default();
    let mut checkpoint_present = false;
    let mut observations = Vec::new();
    for event in &events {
        match &event.event {
            crate::journal::JournalEventKind::RouteSelected {
                provider_id: selected_provider,
                model: selected_model,
                ..
            } => {
                provider_id = Some(selected_provider.clone());
                model = Some(selected_model.clone());
            }
            crate::journal::JournalEventKind::ModelResponded { response } => {
                usage.input_tokens += response.usage.input_tokens;
                usage.output_tokens += response.usage.output_tokens;
                usage.cost_micros += response.usage.cost_micros;
            }
            crate::journal::JournalEventKind::Checkpoint { .. } => {
                checkpoint_present = true;
            }
            crate::journal::JournalEventKind::ToolObserved { observation } => {
                observations.push(observation);
            }
            _ => {}
        }
    }
    if result.stop_reason != case.expect.stop_reason {
        issues.push(format!(
            "expected stop reason {:?}, got {:?}",
            case.expect.stop_reason, result.stop_reason
        ));
    }
    if result.answer.status != case.expect.answer_status {
        issues.push(format!(
            "expected answer status {:?}, got {:?}",
            case.expect.answer_status, result.answer.status
        ));
    }
    if result.answer.evidence.len() < case.expect.min_evidence {
        issues.push(format!(
            "expected at least {} evidence records, got {}",
            case.expect.min_evidence,
            result.answer.evidence.len()
        ));
    }
    if result.answer.claims.len() < case.expect.min_claims {
        issues.push(format!(
            "expected at least {} claims, got {}",
            case.expect.min_claims,
            result.answer.claims.len()
        ));
    }
    if result.answer.unknowns.len() < case.expect.min_unknowns {
        issues.push(format!(
            "expected at least {} unknowns, got {}",
            case.expect.min_unknowns,
            result.answer.unknowns.len()
        ));
    }
    if result.answer.flip_conditions.len() < case.expect.min_flip_conditions {
        issues.push(format!(
            "expected at least {} flip conditions, got {}",
            case.expect.min_flip_conditions,
            result.answer.flip_conditions.len()
        ));
    }
    let high_impact_unknowns = result
        .answer
        .unknowns
        .iter()
        .filter(|unknown| unknown.impact == crate::domain::ImpactLevel::High)
        .count();
    if high_impact_unknowns < case.expect.min_high_impact_unknowns {
        issues.push(format!(
            "expected at least {} high-impact unknowns, got {}",
            case.expect.min_high_impact_unknowns, high_impact_unknowns
        ));
    }
    let high_impact_assumptions = result
        .answer
        .assumptions
        .iter()
        .filter(|assumption| assumption.impact == crate::domain::ImpactLevel::High)
        .count();
    if high_impact_assumptions < case.expect.min_high_impact_assumptions {
        issues.push(format!(
            "expected at least {} high-impact assumptions, got {}",
            case.expect.min_high_impact_assumptions, high_impact_assumptions
        ));
    }
    let unresolved_conflicts = result
        .answer
        .conflicts
        .iter()
        .filter(|conflict| conflict.status == crate::domain::ConflictStatus::Unresolved)
        .count();
    if unresolved_conflicts < case.expect.min_unresolved_conflicts {
        issues.push(format!(
            "expected at least {} unresolved conflicts, got {}",
            case.expect.min_unresolved_conflicts, unresolved_conflicts
        ));
    }
    if audit.ok != case.expect.audit_ok {
        issues.push(format!(
            "expected audit_ok={}, got {}",
            case.expect.audit_ok, audit.ok
        ));
    }
    if checkpoint_present != case.expect.expect_checkpoint {
        issues.push(format!(
            "expected checkpoint={}, got {}",
            case.expect.expect_checkpoint, checkpoint_present
        ));
    }
    if let Some(expected_status) = case.expect.tool_status {
        let actual = observations.last().map(|observation| observation.status);
        if actual != Some(expected_status) {
            issues.push(format!(
                "expected last tool status {:?}, got {:?}",
                expected_status, actual
            ));
        }
    }
    if let Some(expected_code) = &case.expect.tool_error_code {
        let actual = observations
            .last()
            .and_then(|observation| observation.error_code.as_deref());
        if actual != Some(expected_code.as_str()) {
            issues.push(format!(
                "expected tool error code {expected_code}, got {actual:?}"
            ));
        }
    }
    let (rubric_passed, rubric_checks, rubric_issues) =
        score_rubric(case.rubric.as_ref(), &result.answer);
    issues.extend(rubric_issues);
    let decision_completeness = summarize_decision_completeness(&result.answer);
    if !decision_completeness.ok {
        issues.push("decision completeness failed".into());
    }
    let hard_gate_failed = (!audit.ok || !decision_completeness.ok) && case.expect.audit_ok;
    EvalCaseResult {
        id: case.id,
        passed: issues.is_empty(),
        hard_gate_failed,
        stop_reason: Some(result.stop_reason),
        answer_status: Some(result.answer.status),
        evidence_count: result.answer.evidence.len(),
        claim_count: result.answer.claims.len(),
        unknown_count: result.answer.unknowns.len(),
        low_impact_unknown_count: result
            .answer
            .unknowns
            .iter()
            .filter(|unknown| unknown.impact == crate::domain::ImpactLevel::Low)
            .count(),
        medium_impact_unknown_count: result
            .answer
            .unknowns
            .iter()
            .filter(|unknown| unknown.impact == crate::domain::ImpactLevel::Medium)
            .count(),
        high_impact_unknown_count: high_impact_unknowns,
        low_impact_assumption_count: result
            .answer
            .assumptions
            .iter()
            .filter(|assumption| assumption.impact == crate::domain::ImpactLevel::Low)
            .count(),
        medium_impact_assumption_count: result
            .answer
            .assumptions
            .iter()
            .filter(|assumption| assumption.impact == crate::domain::ImpactLevel::Medium)
            .count(),
        high_impact_assumption_count: high_impact_assumptions,
        conflict_count: result.answer.conflicts.len(),
        unresolved_conflict_count: unresolved_conflicts,
        stale_evidence_count: result
            .answer
            .evidence
            .iter()
            .filter(|evidence| {
                evidence.expires_unix_ms.is_some_and(|expires| {
                    expires
                        <= std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis()
                })
            })
            .count(),
        flip_condition_count: result.answer.flip_conditions.len(),
        decision_completeness,
        rubric_checks,
        rubric_passed,
        provider_id,
        model,
        duration_ms,
        usage,
        audit_ok: audit.ok,
        issues,
    }
}

fn materialize_files(workspace: &Path, files: &[SetupFile]) -> Result<(), String> {
    for file in files {
        let relative = PathBuf::from(&file.path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(format!("unsafe setup path '{}'", file.path));
        }
        let target = workspace.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create setup directory: {error}"))?;
        }
        std::fs::write(target, &file.content)
            .map_err(|error| format!("cannot write setup file: {error}"))?;
    }
    Ok(())
}

fn failed_case(id: String, issue: String) -> EvalCaseResult {
    EvalCaseResult {
        id,
        passed: false,
        hard_gate_failed: true,
        stop_reason: None,
        answer_status: None,
        evidence_count: 0,
        claim_count: 0,
        unknown_count: 0,
        low_impact_unknown_count: 0,
        medium_impact_unknown_count: 0,
        high_impact_unknown_count: 0,
        low_impact_assumption_count: 0,
        medium_impact_assumption_count: 0,
        high_impact_assumption_count: 0,
        conflict_count: 0,
        unresolved_conflict_count: 0,
        stale_evidence_count: 0,
        flip_condition_count: 0,
        decision_completeness: DecisionCompletenessRollup::default(),
        rubric_checks: 0,
        rubric_passed: 0,
        provider_id: None,
        model: None,
        duration_ms: 0,
        usage: Usage::default(),
        audit_ok: false,
        issues: vec![issue],
    }
}

fn summarize_decision_completeness(
    answer: &crate::domain::AgentAnswer,
) -> DecisionCompletenessRollup {
    let conditional_answers = usize::from(answer.status == AnswerStatus::Conditional);
    let bound_flip_conditions = answer
        .flip_conditions
        .iter()
        .filter(|flip| !flip.target_ids.is_empty())
        .count();
    let unbound_flip_conditions = answer.flip_conditions.len() - bound_flip_conditions;
    let high_impact_unknowns_without_reduction = answer
        .unknowns
        .iter()
        .filter(|unknown| {
            unknown.impact == crate::domain::ImpactLevel::High
                && unknown
                    .reduction_method
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
        })
        .count();
    let completed_with_high_impact_assumptions = usize::from(
        answer.status == AnswerStatus::Completed
            && answer
                .assumptions
                .iter()
                .any(|assumption| assumption.impact == crate::domain::ImpactLevel::High),
    );
    let conditional_with_bound_flip =
        usize::from(conditional_answers == 1 && bound_flip_conditions > 0);
    let ok = (conditional_answers == 0 || conditional_with_bound_flip == 1)
        && unbound_flip_conditions == 0
        && high_impact_unknowns_without_reduction == 0
        && completed_with_high_impact_assumptions == 0;
    DecisionCompletenessRollup {
        conditional_answers,
        conditional_with_bound_flip,
        bound_flip_conditions,
        unbound_flip_conditions,
        high_impact_unknowns_without_reduction,
        completed_with_high_impact_assumptions,
        ok,
    }
}

fn aggregate_decision_completeness(results: &[EvalCaseResult]) -> DecisionCompletenessAggregate {
    let cases = results.len();
    let ok_cases = results
        .iter()
        .filter(|result| result.decision_completeness.ok)
        .count();
    let completion_rate_bps = (ok_cases * 10_000)
        .checked_div(cases)
        .unwrap_or(0)
        .min(10_000) as u16;
    DecisionCompletenessAggregate {
        cases,
        ok_cases,
        completion_rate_bps,
        conditional_answers: results
            .iter()
            .map(|result| result.decision_completeness.conditional_answers)
            .sum(),
        conditional_with_bound_flip: results
            .iter()
            .map(|result| result.decision_completeness.conditional_with_bound_flip)
            .sum(),
        bound_flip_conditions: results
            .iter()
            .map(|result| result.decision_completeness.bound_flip_conditions)
            .sum(),
        unbound_flip_conditions: results
            .iter()
            .map(|result| result.decision_completeness.unbound_flip_conditions)
            .sum(),
        high_impact_unknowns_without_reduction: results
            .iter()
            .map(|result| {
                result
                    .decision_completeness
                    .high_impact_unknowns_without_reduction
            })
            .sum(),
        completed_with_high_impact_assumptions: results
            .iter()
            .map(|result| {
                result
                    .decision_completeness
                    .completed_with_high_impact_assumptions
            })
            .sum(),
    }
}

fn score_rubric(
    rubric: Option<&EvalRubric>,
    answer: &crate::domain::AgentAnswer,
) -> (usize, usize, Vec<String>) {
    let Some(rubric) = rubric else {
        return (0, 0, Vec::new());
    };
    let conclusion = answer.conclusion.to_lowercase();
    let mut passed = 0;
    let mut checks = 0;
    let mut issues = Vec::new();
    for term in &rubric.required_conclusion_terms {
        checks += 1;
        if conclusion.contains(&term.to_lowercase()) {
            passed += 1;
        } else {
            issues.push(format!("rubric: conclusion must contain '{term}'"));
        }
    }
    for term in &rubric.forbidden_conclusion_terms {
        checks += 1;
        if conclusion.contains(&term.to_lowercase()) {
            issues.push(format!("rubric: conclusion must not contain '{term}'"));
        } else {
            passed += 1;
        }
    }
    for source in &rubric.required_sources {
        checks += 1;
        if answer
            .evidence
            .iter()
            .any(|evidence| &evidence.source == source)
        {
            passed += 1;
        } else {
            issues.push(format!("rubric: evidence source '{source}' is missing"));
        }
    }
    for status in &rubric.required_claim_statuses {
        checks += 1;
        if answer.claims.iter().any(|claim| claim.status == *status) {
            passed += 1;
        } else {
            issues.push(format!("rubric: claim status {status:?} is missing"));
        }
    }
    (passed, checks, issues)
}

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("cannot read evaluation suite: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid evaluation suite JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("evaluation suite has no cases")]
    EmptySuite,
    #[error("evaluation requires at least one provider")]
    NoProviders,
    #[error("training and evaluation groups must differ")]
    InvalidGrouping,
    #[error("every experience evaluation case requires a non-empty experience_scope")]
    MissingExperienceScope,
    #[error("contract fixture suites cannot be promoted as model-quality evidence")]
    FixtureCannotProveModelQuality,
    #[error("evaluation suite is a template or draft and cannot run")]
    SuiteNotRunnable,
    #[error("model-quality suite is missing requirements, reviewer or ready status")]
    ModelQualityMetadataMissing,
    #[error("decision-completeness floor must be between 0 and 10000 basis points")]
    InvalidDecisionCompletenessFloor,
    #[error("decision-completeness policy version and floor source are inconsistent")]
    InvalidDecisionCompletenessPolicyCombination,
    #[error("unsupported decision-completeness policy version: {0}")]
    UnsupportedDecisionCompletenessPolicyVersion(u8),
    #[error("evaluation report integrity violation: {0}")]
    InvalidReportIntegrity(&'static str),
    #[error("unsupported evaluation report summary version: {0}")]
    UnsupportedReportSummaryVersion(u8),
    #[error("evaluation report summary integrity violation: {0}")]
    InvalidReportSummary(&'static str),
    #[error("report content binding is required but the report is unbound")]
    ReportContentBindingRequired,
    #[error("report authenticity is required but no verified report signature was provided")]
    ReportAuthenticityRequired,
}

fn effective_decision_completeness_floor(suite: &EvalSuite) -> u16 {
    suite
        .min_decision_completeness_bps
        .unwrap_or(if suite.kind == EvalSuiteKind::ModelQuality {
            10_000
        } else {
            0
        })
}

fn decision_completeness_policy(suite: &EvalSuite) -> DecisionCompletenessPolicy {
    DecisionCompletenessPolicy {
        version: DECISION_COMPLETENESS_POLICY_VERSION,
        floor_source: if suite.min_decision_completeness_bps.is_some() {
            DecisionCompletenessFloorSource::Explicit
        } else {
            DecisionCompletenessFloorSource::Default
        },
    }
}

fn validate_decision_completeness_floor(suite: &EvalSuite) -> Result<(), EvalError> {
    if suite
        .min_decision_completeness_bps
        .is_some_and(|floor| floor > 10_000)
    {
        return Err(EvalError::InvalidDecisionCompletenessFloor);
    }
    Ok(())
}

fn ensure_runnable_model_quality(suite: &EvalSuite) -> Result<(), EvalError> {
    if suite.kind == EvalSuiteKind::ModelQuality
        && (suite.execution_status != EvalExecutionStatus::Ready
            || suite.provider_requirements.is_empty()
            || suite
                .reviewer
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            || !suite.calibration_required)
    {
        return Err(EvalError::ModelQualityMetadataMissing);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    use crate::domain::{AgentAnswer, Claim, ClaimStatus, MessageRole, ModelResponse, ModelTurn};
    use crate::experience::{ExperienceStatus, ExperienceVerificationInput};
    use crate::provider::{ModelProvider, ProviderError};
    use crate::router::PrivacyClass;
    use crate::runtime::ProviderCandidate;

    struct FixtureReasoningProvider;

    impl FixtureReasoningProvider {
        fn latest_observation(
            messages: &[crate::domain::ModelMessage],
        ) -> Option<crate::domain::ToolObservation> {
            messages.iter().rev().find_map(|message| {
                (message.role == MessageRole::Tool)
                    .then(|| serde_json::from_str(&message.content).ok())
                    .flatten()
            })
        }

        fn read_target(messages: &[crate::domain::ModelMessage]) -> Option<String> {
            let text = messages
                .iter()
                .rev()
                .find(|message| message.role == MessageRole::User)
                .map(|message| message.content.as_str())?;
            let start = text.find("@read-file:\"")? + "@read-file:\"".len();
            let end = text[start..].find('"')? + start;
            Some(text[start..end].to_string())
        }
    }

    #[async_trait]
    impl ModelProvider for FixtureReasoningProvider {
        fn id(&self) -> &str {
            "fixture-reasoning"
        }

        fn model(&self) -> &str {
            "fixture-v1"
        }

        async fn complete(
            &self,
            request: crate::domain::ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            if request
                .messages
                .iter()
                .any(|message| message.content.contains("observe-marker"))
                && Self::latest_observation(&request.messages).is_none()
            {
                let path = Self::read_target(&request.messages).ok_or_else(|| ProviderError {
                    code: crate::provider::ProviderErrorCode::InvalidResponse,
                    message: "missing read target".into(),
                    retryable: false,
                })?;
                return Ok(ModelResponse {
                    provider_id: self.id().into(),
                    model: self.model().into(),
                    turn: ModelTurn::ToolCalls {
                        calls: vec![crate::domain::ToolCall {
                            id: "fixture-call-1".into(),
                            name: "read_file".into(),
                            arguments: serde_json::json!({"path": path}),
                        }],
                    },
                    usage: crate::domain::Usage::default(),
                });
            }

            let answer = if let Some(observation) = Self::latest_observation(&request.messages) {
                AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Completed,
                    conclusion: format!("verified after observation: {}", observation.summary),
                    claims: vec![Claim {
                        id: "claim-observed".into(),
                        text: observation.summary.clone(),
                        status: ClaimStatus::Verified,
                        sources: vec!["evidence.txt".into()],
                        scope: Some("fixture observation".into()),
                    }],
                    evidence: vec![crate::domain::Evidence {
                        source: "evidence.txt".into(),
                        statement: observation.summary,
                        observed_unix_ms: None,
                        expires_unix_ms: None,
                        claim_ids: vec!["claim-observed".into()],
                    }],
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                }
            } else if request
                .messages
                .iter()
                .any(|message| message.content.contains("conflict-marker"))
            {
                AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Conditional,
                    conclusion: "sources conflict; result is conditional".into(),
                    claims: vec![
                        Claim {
                            id: "claim-a".into(),
                            text: "source A reports one value".into(),
                            status: ClaimStatus::Verified,
                            sources: vec!["source-a".into()],
                            scope: Some("fixture conflict".into()),
                        },
                        Claim {
                            id: "claim-b".into(),
                            text: "source B reports another value".into(),
                            status: ClaimStatus::Verified,
                            sources: vec!["source-b".into()],
                            scope: Some("fixture conflict".into()),
                        },
                    ],
                    evidence: vec![
                        crate::domain::Evidence {
                            source: "source-a".into(),
                            statement: "source A reports one value".into(),
                            observed_unix_ms: None,
                            expires_unix_ms: None,
                            claim_ids: vec!["claim-a".into()],
                        },
                        crate::domain::Evidence {
                            source: "source-b".into(),
                            statement: "source B reports another value".into(),
                            observed_unix_ms: None,
                            expires_unix_ms: None,
                            claim_ids: vec!["claim-b".into()],
                        },
                    ],
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: vec![crate::domain::UnknownItem {
                        id: "current-source".into(),
                        description: "which source is current is unknown".into(),
                        impact: crate::domain::ImpactLevel::High,
                        why_it_matters: Some("it flips the conclusion".into()),
                        reduction_method: Some("obtain timestamps".into()),
                    }],
                    conflicts: vec![crate::domain::ConflictRecord {
                        id: "conflict-1".into(),
                        claim_ids: ["claim-a".into(), "claim-b".into()],
                        status: crate::domain::ConflictStatus::Unresolved,
                        resolution: None,
                        resolution_sources: Vec::new(),
                        resolution_claim_ids: Vec::new(),
                    }],
                    flip_conditions: vec![crate::domain::FlipCondition {
                        if_observed: "source B is newer".into(),
                        then: "prefer source B".into(),
                        target_ids: vec!["current-source".into()],
                    }],
                    action_receipts: Vec::new(),
                }
            } else {
                AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Conditional,
                    conclusion: "decision is conditional on cost".into(),
                    claims: vec![Claim {
                        id: "claim-flip".into(),
                        text: "option A is preferred under current assumptions".into(),
                        status: ClaimStatus::ModelPrior,
                        sources: Vec::new(),
                        scope: Some("fixture decision".into()),
                    }],
                    evidence: Vec::new(),
                    assumptions: vec!["current cost estimate holds".into()],
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: vec![crate::domain::FlipCondition {
                        if_observed: "option B becomes cheaper".into(),
                        then: "switch to option B".into(),
                        target_ids: vec!["claim-flip".into()],
                    }],
                    action_receipts: Vec::new(),
                }
            };
            Ok(ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn: ModelTurn::Final { answer },
                usage: crate::domain::Usage::default(),
            })
        }
    }

    #[tokio::test]
    async fn deterministic_suite_reports_success_and_hard_gates() {
        let report = run_suite(EvalSuite {
            name: "test".into(),
            suite_version: "1".into(),
            kind: EvalSuiteKind::ContractFixture,
            execution_status: EvalExecutionStatus::Unspecified,
            provider_requirements: Vec::new(),
            reviewer: None,
            calibration_required: false,
            min_decision_completeness_bps: None,
            cases: vec![EvalCase {
                id: "read".into(),
                objective: "Research @read-file:\"${workspace}/evidence.txt\"".into(),
                setup_files: vec![SetupFile {
                    path: "evidence.txt".into(),
                    content: "verified evidence".into(),
                }],
                experience_scope: None,
                rubric: None,
                expect: EvalExpectation {
                    stop_reason: StopReason::Completed,
                    answer_status: AnswerStatus::Completed,
                    min_evidence: 1,
                    min_claims: 1,
                    min_unknowns: 0,
                    min_flip_conditions: 0,
                    min_high_impact_unknowns: 0,
                    min_high_impact_assumptions: 0,
                    min_unresolved_conflicts: 0,
                    expect_checkpoint: true,
                    tool_status: Some(ToolStatus::Ok),
                    tool_error_code: None,
                    audit_ok: true,
                },
            }],
        })
        .await
        .unwrap();
        assert_eq!(report.passed, 1);
        assert_eq!(report.hard_gate_failures, 0);
        report.validate_integrity().unwrap();
        assert_eq!(
            report.decision_completeness_policy,
            DecisionCompletenessPolicy {
                version: DECISION_COMPLETENESS_POLICY_VERSION,
                floor_source: DecisionCompletenessFloorSource::Default,
            }
        );

        let mut legacy_report = serde_json::to_value(&report).unwrap();
        let _ = legacy_report
            .as_object_mut()
            .unwrap()
            .remove("decision_completeness_policy");
        let _ = legacy_report
            .as_object_mut()
            .unwrap()
            .remove("content_sha256");
        let decoded: EvalReport = serde_json::from_value(legacy_report).unwrap();
        assert_eq!(
            decoded.decision_completeness_policy,
            DecisionCompletenessPolicy::default()
        );
        assert!(decoded.content_sha256.is_none());
        decoded.validate_integrity().unwrap();
    }

    #[test]
    fn report_integrity_rejects_tampered_aggregates() {
        let report = EvalReport {
            suite: "integrity".into(),
            kind: EvalSuiteKind::ContractFixture,
            execution_status: EvalExecutionStatus::Unspecified,
            total: 1,
            passed: 1,
            failed: 0,
            hard_gate_failures: 0,
            rubric_checks: 1,
            rubric_passed: 1,
            decision_completeness: DecisionCompletenessAggregate {
                cases: 1,
                ok_cases: 1,
                completion_rate_bps: 10_000,
                ..DecisionCompletenessAggregate::default()
            },
            min_decision_completeness_bps: 0,
            decision_completeness_floor_passed: true,
            decision_completeness_policy: DecisionCompletenessPolicy {
                version: DECISION_COMPLETENESS_POLICY_VERSION,
                floor_source: DecisionCompletenessFloorSource::Default,
            },
            content_sha256: None,
            cases: vec![EvalCaseResult {
                id: "passed".into(),
                passed: true,
                hard_gate_failed: false,
                stop_reason: None,
                answer_status: None,
                evidence_count: 0,
                claim_count: 0,
                unknown_count: 0,
                low_impact_unknown_count: 0,
                medium_impact_unknown_count: 0,
                high_impact_unknown_count: 0,
                low_impact_assumption_count: 0,
                medium_impact_assumption_count: 0,
                high_impact_assumption_count: 0,
                conflict_count: 0,
                unresolved_conflict_count: 0,
                stale_evidence_count: 0,
                flip_condition_count: 0,
                decision_completeness: DecisionCompletenessRollup {
                    ok: true,
                    ..DecisionCompletenessRollup::default()
                },
                rubric_checks: 1,
                rubric_passed: 1,
                provider_id: None,
                model: None,
                duration_ms: 0,
                usage: Usage::default(),
                audit_ok: true,
                issues: Vec::new(),
            }],
        };
        report.validate_integrity().unwrap();

        let mut invalid = report.clone();
        invalid.total = 2;
        assert!(matches!(
            invalid.validate_integrity(),
            Err(EvalError::InvalidReportIntegrity(_))
        ));

        let mut invalid = report.clone();
        invalid.decision_completeness.ok_cases = 0;
        assert!(matches!(
            invalid.validate_integrity(),
            Err(EvalError::InvalidReportIntegrity(
                "decision-completeness aggregates do not match cases"
            ))
        ));

        let mut invalid = report.clone();
        invalid.decision_completeness_floor_passed = false;
        assert!(matches!(
            invalid.validate_integrity(),
            Err(EvalError::InvalidReportIntegrity(
                "decision-completeness floor status is inconsistent"
            ))
        ));

        EvalReportTrustRequirements::default()
            .validate(&report, false)
            .unwrap();
        assert!(matches!(
            EvalReportTrustRequirements {
                require_content_binding: true,
                require_authenticity: false,
            }
            .validate(&report, false),
            Err(EvalError::ReportContentBindingRequired)
        ));

        let mut report = report;
        report.content_sha256 = Some(report_content_sha256(&report).unwrap());
        report.validate_integrity().unwrap();
        EvalReportTrustRequirements {
            require_content_binding: true,
            require_authenticity: false,
        }
        .validate(&report, false)
        .unwrap();
        assert!(matches!(
            EvalReportTrustRequirements {
                require_content_binding: false,
                require_authenticity: true,
            }
            .validate(&report, false),
            Err(EvalError::ReportAuthenticityRequired)
        ));
        EvalReportTrustRequirements {
            require_content_binding: true,
            require_authenticity: true,
        }
        .validate(&report, true)
        .unwrap();
        let summary = EvalReportSummary::from(&report);
        summary.validate().unwrap();
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["schema_version"], REPORT_SUMMARY_SCHEMA_VERSION);
        assert_eq!(json["total"], 1);
        assert!(json.get("cases").is_none());
        assert!(json.get("id").is_none());
        assert_eq!(
            summary.verification,
            EvalReportVerification {
                integrity_checked: true,
                content_binding: ContentBindingStatus::Bound,
                signature_present: false,
                authenticity: AuthenticityStatus::Unauthenticated,
            }
        );

        let mut legacy = json.clone();
        legacy.as_object_mut().unwrap().remove("schema_version");
        legacy
            .as_object_mut()
            .unwrap()
            .remove("report_content_sha256");
        legacy.as_object_mut().unwrap().remove("verification");
        let legacy: EvalReportSummary = serde_json::from_value(legacy).unwrap();
        legacy.validate().unwrap();
        assert!(legacy.report_content_sha256.is_none());
        assert!(!legacy.verification.integrity_checked);

        let mut no_verification = json.clone();
        no_verification
            .as_object_mut()
            .unwrap()
            .remove("verification");
        let no_verification: EvalReportSummary = serde_json::from_value(no_verification).unwrap();
        assert!(matches!(
            no_verification.validate(),
            Err(EvalError::InvalidReportSummary(
                "current summary must record integrity checking"
            ))
        ));

        let mut invalid = summary.clone();
        invalid.schema_version = REPORT_SUMMARY_SCHEMA_VERSION + 1;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::UnsupportedReportSummaryVersion(5))
        ));

        let mut invalid = summary.clone();
        invalid.decision_completeness_rate_bps = 10_001;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::InvalidReportSummary(
                "basis-point values exceed 10000"
            ))
        ));

        let mut invalid = summary.clone();
        invalid.verification.integrity_checked = false;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::InvalidReportSummary(
                "current summary must record integrity checking"
            ))
        ));

        let mut invalid = summary.clone();
        invalid.verification.content_binding = ContentBindingStatus::Unbound;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::InvalidReportSummary(
                "content-binding status does not match report digest"
            ))
        ));

        let mut invalid = summary.clone();
        invalid.verification.authenticity = AuthenticityStatus::Verified;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::InvalidReportSummary(
                "verified authenticity requires a signature"
            ))
        ));

        let mut invalid = summary;
        invalid.passed = 0;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::InvalidReportSummary(
                "passed and failed counts do not sum to total"
            ))
        ));

        let mut invalid = EvalReportSummary::from(&report);
        invalid.passed = usize::MAX;
        invalid.failed = 1;
        invalid.total = 0;
        assert!(matches!(
            invalid.validate(),
            Err(EvalError::InvalidReportSummary(
                "passed and failed counts do not sum to total"
            ))
        ));

        let mut invalid = report;
        invalid.cases[0].duration_ms = 1;
        assert!(matches!(
            invalid.validate_integrity(),
            Err(EvalError::InvalidReportIntegrity(
                "report content digest does not match payload"
            ))
        ));
    }

    #[tokio::test]
    async fn scripted_fixture_covers_conflict_flip_and_external_correction() {
        let suite = EvalSuite {
            name: "fixture-reasoning-v1".into(),
            suite_version: "1".into(),
            kind: EvalSuiteKind::ContractFixture,
            execution_status: EvalExecutionStatus::Unspecified,
            provider_requirements: Vec::new(),
            reviewer: None,
            calibration_required: false,
            min_decision_completeness_bps: None,
            cases: vec![
                EvalCase {
                    id: "source-conflict".into(),
                    objective: "Research conflict-marker sources".into(),
                    setup_files: Vec::new(),
                    experience_scope: None,
                    rubric: Some(EvalRubric {
                        required_conclusion_terms: vec!["conflict".into()],
                        forbidden_conclusion_terms: Vec::new(),
                        required_sources: vec!["source-a".into(), "source-b".into()],
                        required_claim_statuses: vec![ClaimStatus::Verified],
                    }),
                    expect: EvalExpectation {
                        stop_reason: StopReason::Completed,
                        answer_status: AnswerStatus::Conditional,
                        min_evidence: 2,
                        min_claims: 1,
                        min_unknowns: 1,
                        min_flip_conditions: 1,
                        min_high_impact_unknowns: 1,
                        min_high_impact_assumptions: 0,
                        min_unresolved_conflicts: 1,
                        expect_checkpoint: false,
                        tool_status: None,
                        tool_error_code: None,
                        audit_ok: true,
                    },
                },
                EvalCase {
                    id: "decision-flip".into(),
                    objective: "Choose flip-marker option".into(),
                    setup_files: Vec::new(),
                    experience_scope: None,
                    rubric: None,
                    expect: EvalExpectation {
                        stop_reason: StopReason::Completed,
                        answer_status: AnswerStatus::Conditional,
                        min_evidence: 0,
                        min_claims: 1,
                        min_unknowns: 0,
                        min_flip_conditions: 1,
                        min_high_impact_unknowns: 0,
                        min_high_impact_assumptions: 0,
                        min_unresolved_conflicts: 0,
                        expect_checkpoint: false,
                        tool_status: None,
                        tool_error_code: None,
                        audit_ok: true,
                    },
                },
                EvalCase {
                    id: "external-correction".into(),
                    objective: "Research observe-marker @read-file:\"${workspace}/evidence.txt\""
                        .into(),
                    setup_files: vec![SetupFile {
                        path: "evidence.txt".into(),
                        content: "observed evidence".into(),
                    }],
                    experience_scope: None,
                    rubric: Some(EvalRubric {
                        required_conclusion_terms: vec!["verified after observation".into()],
                        forbidden_conclusion_terms: Vec::new(),
                        required_sources: vec!["evidence.txt".into()],
                        required_claim_statuses: vec![ClaimStatus::Verified],
                    }),
                    expect: EvalExpectation {
                        stop_reason: StopReason::Completed,
                        answer_status: AnswerStatus::Completed,
                        min_evidence: 1,
                        min_claims: 1,
                        min_unknowns: 0,
                        min_flip_conditions: 0,
                        min_high_impact_unknowns: 0,
                        min_high_impact_assumptions: 0,
                        min_unresolved_conflicts: 0,
                        expect_checkpoint: true,
                        tool_status: Some(ToolStatus::Ok),
                        tool_error_code: None,
                        audit_ok: true,
                    },
                },
            ],
        };
        let provider = ProviderCandidate {
            provider: std::sync::Arc::new(FixtureReasoningProvider),
            manifest: crate::router::ProviderManifest {
                provider_id: "fixture-reasoning".into(),
                model: "fixture-v1".into(),
                privacy: PrivacyClass::Local,
                supported_modes: vec![
                    crate::domain::CognitiveMode::EvidenceResearch,
                    crate::domain::CognitiveMode::DecisionAnalysis,
                ],
                supports_tools: true,
                supports_structured_output: true,
                max_risk: crate::domain::RiskLevel::High,
                quality_score: 1,
                input_cost_micros_per_k: 0,
                output_cost_micros_per_k: 0,
                p95_latency_ms: 0,
            },
        };
        let report = run_suite_with_providers(suite, vec![provider], false)
            .await
            .unwrap();
        assert_eq!(report.passed, 3, "{:?}", report.cases);
        assert_eq!(report.hard_gate_failures, 0);
    }

    #[test]
    fn decision_completeness_rollup_tracks_each_failure_mode() {
        let mut answer = AgentAnswer {
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Conditional,
            conclusion: "conditional".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: vec![crate::domain::UnknownItem {
                id: "risk".into(),
                description: "critical unknown".into(),
                impact: crate::domain::ImpactLevel::High,
                why_it_matters: None,
                reduction_method: None,
            }],
            conflicts: Vec::new(),
            flip_conditions: vec![crate::domain::FlipCondition {
                if_observed: "new evidence".into(),
                then: "change decision".into(),
                target_ids: Vec::new(),
            }],
            action_receipts: Vec::new(),
        };
        let summary = summarize_decision_completeness(&answer);
        assert!(!summary.ok);
        assert_eq!(summary.conditional_answers, 1);
        assert_eq!(summary.conditional_with_bound_flip, 0);
        assert_eq!(summary.unbound_flip_conditions, 1);
        assert_eq!(summary.high_impact_unknowns_without_reduction, 1);

        answer.unknowns[0].reduction_method = Some("inspect the source".into());
        answer.flip_conditions[0].target_ids = vec!["risk".into()];
        let summary = summarize_decision_completeness(&answer);
        assert!(summary.ok);
        assert_eq!(summary.bound_flip_conditions, 1);

        answer.status = AnswerStatus::Completed;
        answer.assumptions = vec![crate::domain::AssumptionItem {
            description: "cost remains stable".into(),
            impact: crate::domain::ImpactLevel::High,
            verification_method: Some("refresh the quote".into()),
        }];
        let summary = summarize_decision_completeness(&answer);
        assert!(!summary.ok);
        assert_eq!(summary.completed_with_high_impact_assumptions, 1);

        let mut passed_case = failed_case("passed-decision".into(), "fixture".into());
        passed_case.decision_completeness = DecisionCompletenessRollup {
            ok: true,
            ..DecisionCompletenessRollup::default()
        };
        let failed_case = failed_case("failed-decision".into(), "fixture".into());
        let aggregate = aggregate_decision_completeness(&[passed_case, failed_case]);
        assert_eq!(aggregate.cases, 2);
        assert_eq!(aggregate.ok_cases, 1);
        assert_eq!(aggregate.completion_rate_bps, 5_000);
    }

    #[test]
    fn setup_files_cannot_escape_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let error = materialize_files(
            workspace.path(),
            &[SetupFile {
                path: "../escape.txt".into(),
                content: "bad".into(),
            }],
        )
        .unwrap_err();
        assert!(error.contains("unsafe setup path"));
    }

    #[test]
    fn decision_completeness_floor_is_bounded_and_kind_aware() {
        let mut suite = EvalSuite {
            name: "floor".into(),
            suite_version: "1".into(),
            kind: EvalSuiteKind::ContractFixture,
            execution_status: EvalExecutionStatus::Unspecified,
            provider_requirements: Vec::new(),
            reviewer: None,
            calibration_required: false,
            min_decision_completeness_bps: None,
            cases: Vec::new(),
        };
        assert_eq!(effective_decision_completeness_floor(&suite), 0);
        assert_eq!(
            decision_completeness_policy(&suite),
            DecisionCompletenessPolicy {
                version: DECISION_COMPLETENESS_POLICY_VERSION,
                floor_source: DecisionCompletenessFloorSource::Default,
            }
        );
        suite.kind = EvalSuiteKind::ModelQuality;
        assert_eq!(effective_decision_completeness_floor(&suite), 10_000);
        suite.min_decision_completeness_bps = Some(9_000);
        assert_eq!(effective_decision_completeness_floor(&suite), 9_000);
        assert_eq!(
            decision_completeness_policy(&suite).floor_source,
            DecisionCompletenessFloorSource::Explicit
        );
        validate_decision_completeness_floor(&suite).unwrap();
        suite.min_decision_completeness_bps = Some(10_001);
        assert!(matches!(
            validate_decision_completeness_floor(&suite),
            Err(EvalError::InvalidDecisionCompletenessFloor)
        ));
    }

    #[test]
    fn report_policy_metadata_rejects_unknown_floor_source() {
        let invalid = serde_json::json!({
            "version": 1,
            "floor_source": "environment"
        });
        assert!(serde_json::from_value::<DecisionCompletenessPolicy>(invalid).is_err());
    }

    #[test]
    fn report_policy_validation_accepts_known_versions_and_fails_closed() {
        for policy in [
            DecisionCompletenessPolicy {
                version: 0,
                floor_source: DecisionCompletenessFloorSource::Legacy,
            },
            DecisionCompletenessPolicy {
                version: 1,
                floor_source: DecisionCompletenessFloorSource::Default,
            },
            DecisionCompletenessPolicy {
                version: 1,
                floor_source: DecisionCompletenessFloorSource::Explicit,
            },
        ] {
            policy.validate().unwrap();
        }

        for policy in [
            DecisionCompletenessPolicy {
                version: 0,
                floor_source: DecisionCompletenessFloorSource::Default,
            },
            DecisionCompletenessPolicy {
                version: 1,
                floor_source: DecisionCompletenessFloorSource::Legacy,
            },
        ] {
            assert!(matches!(
                policy.validate(),
                Err(EvalError::InvalidDecisionCompletenessPolicyCombination)
            ));
        }
        assert!(matches!(
            DecisionCompletenessPolicy {
                version: 2,
                floor_source: DecisionCompletenessFloorSource::Default,
            }
            .validate(),
            Err(EvalError::UnsupportedDecisionCompletenessPolicyVersion(2))
        ));
    }

    #[tokio::test]
    async fn model_quality_template_loads_but_cannot_run() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("template.json");
        std::fs::write(&path, include_str!("../evals/model-quality-template.json")).unwrap();
        let suite = load_suite(&path).unwrap();
        assert_eq!(suite.kind, EvalSuiteKind::ModelQuality);
        assert_eq!(suite.execution_status, EvalExecutionStatus::Template);
        assert!(matches!(
            run_suite(suite).await,
            Err(EvalError::SuiteNotRunnable)
        ));
    }

    struct GuidanceAwareProvider;

    #[async_trait]
    impl ModelProvider for GuidanceAwareProvider {
        fn id(&self) -> &str {
            "guidance-aware"
        }

        fn model(&self) -> &str {
            "fixture-v1"
        }

        async fn complete(
            &self,
            request: crate::domain::ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            let guided = request.messages.iter().any(|message| {
                message.role == MessageRole::System
                    && message.content.contains("Repository guidance test")
            });
            let answer = if guided {
                AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Completed,
                    conclusion: "guided answer".into(),
                    claims: vec![Claim {
                        id: "claim-1".into(),
                        text: "guided claim".into(),
                        status: ClaimStatus::ModelPrior,
                        sources: Vec::new(),
                        scope: Some("test".into()),
                    }],
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                }
            } else {
                AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::NeedsEvidence,
                    conclusion: "baseline answer".into(),
                    claims: Vec::new(),
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                }
            };
            Ok(ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn: ModelTurn::Final { answer },
                usage: crate::domain::Usage::default(),
            })
        }
    }

    #[tokio::test]
    async fn experience_evaluation_generates_measured_heldout_verification() {
        let workspace = tempfile::tempdir().unwrap();
        let suite_path = workspace.path().join("heldout.json");
        std::fs::write(
            &suite_path,
            serde_json::to_vec_pretty(&EvalSuite {
                name: "heldout-test".into(),
                suite_version: "1".into(),
                kind: EvalSuiteKind::Unspecified,
                execution_status: EvalExecutionStatus::Unspecified,
                provider_requirements: Vec::new(),
                reviewer: None,
                calibration_required: false,
                min_decision_completeness_bps: None,
                cases: vec![EvalCase {
                    id: "heldout-1".into(),
                    objective: "Choose the repository option".into(),
                    setup_files: Vec::new(),
                    experience_scope: Some("repository decisions".into()),
                    rubric: None,
                    expect: EvalExpectation {
                        stop_reason: StopReason::Completed,
                        answer_status: AnswerStatus::Completed,
                        min_evidence: 0,
                        min_claims: 1,
                        min_unknowns: 0,
                        min_flip_conditions: 0,
                        min_high_impact_unknowns: 0,
                        min_high_impact_assumptions: 0,
                        min_unresolved_conflicts: 0,
                        expect_checkpoint: false,
                        tool_status: None,
                        tool_error_code: None,
                        audit_ok: true,
                    },
                }],
            })
            .unwrap(),
        )
        .unwrap();
        let candidate = ExperienceCandidate {
            id: uuid::Uuid::new_v4(),
            version: 1,
            kind: crate::experience::ExperienceKind::Skill,
            title: "Repository guidance test".into(),
            content: "Use the guided repository workflow.".into(),
            scope: "repository decisions".into(),
            source_run_id: uuid::Uuid::new_v4(),
            evidence_event_seqs: vec![1],
            evidence_sha256: String::new(),
            sensitive: false,
            applicable_modes: vec![crate::domain::CognitiveMode::DecisionAnalysis],
            expires_at_unix_ms: None,
            supersedes: None,
            status: ExperienceStatus::Reviewed,
            verification: None,
            created_unix_ms: 0,
            updated_unix_ms: 0,
        };
        let fixture_path = workspace.path().join("fixture.json");
        let mut fixture_suite: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&suite_path).unwrap()).unwrap();
        fixture_suite["kind"] = serde_json::json!("contract_fixture");
        std::fs::write(
            &fixture_path,
            serde_json::to_vec_pretty(&fixture_suite).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            evaluate_experience_suite(
                &fixture_path,
                candidate.clone(),
                Vec::new(),
                false,
                "A".into(),
                "B".into(),
            )
            .await,
            Err(EvalError::FixtureCannotProveModelQuality)
        ));
        let provider = ProviderCandidate {
            provider: std::sync::Arc::new(GuidanceAwareProvider),
            manifest: crate::router::ProviderManifest {
                provider_id: "guidance-aware".into(),
                model: "fixture-v1".into(),
                privacy: PrivacyClass::Local,
                supported_modes: vec![crate::domain::CognitiveMode::DecisionAnalysis],
                supports_tools: true,
                supports_structured_output: true,
                max_risk: crate::domain::RiskLevel::High,
                quality_score: 1,
                input_cost_micros_per_k: 0,
                output_cost_micros_per_k: 0,
                p95_latency_ms: 0,
            },
        };
        let report = evaluate_experience_suite(
            &suite_path,
            candidate,
            vec![provider],
            false,
            "A".into(),
            "B".into(),
        )
        .await
        .unwrap();
        assert_eq!(report.baseline.passed, 0);
        assert_eq!(report.learned.passed, 1);
        assert_eq!(report.verification.learned_success_bps, 10_000);
        assert_eq!(report.verification.case_ids, vec!["heldout-1"]);
        assert_eq!(report.verification.suite_sha256.len(), 64);
        let ExperienceVerificationInput { .. } = report.verification;
    }
}
