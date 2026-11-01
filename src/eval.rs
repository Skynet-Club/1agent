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
    pub flip_condition_count: usize,
    pub rubric_checks: usize,
    pub rubric_passed: usize,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub duration_ms: u64,
    pub usage: Usage,
    pub audit_ok: bool,
    pub issues: Vec<String>,
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
    pub cases: Vec<EvalCaseResult>,
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
    if suite.execution_status == EvalExecutionStatus::Template
        || suite.execution_status == EvalExecutionStatus::Draft
    {
        return Err(EvalError::SuiteNotRunnable);
    }
    if suite.kind == EvalSuiteKind::ModelQuality {
        ensure_runnable_model_quality(&suite)?;
    }
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
    Ok(EvalReport {
        suite: suite.name,
        kind: suite.kind,
        execution_status: suite.execution_status,
        total: results.len(),
        passed,
        failed: results.len() - passed,
        hard_gate_failures,
        rubric_checks,
        rubric_passed,
        cases: results,
    })
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
    let hard_gate_failed = !audit.ok && case.expect.audit_ok;
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
        flip_condition_count: result.answer.flip_conditions.len(),
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
        flip_condition_count: 0,
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
                            claim_ids: vec!["claim-a".into()],
                        },
                        crate::domain::Evidence {
                            source: "source-b".into(),
                            statement: "source B reports another value".into(),
                            claim_ids: vec!["claim-b".into()],
                        },
                    ],
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: vec![crate::domain::UnknownItem {
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
                    }],
                    action_receipts: Vec::new(),
                }
            } else {
                AgentAnswer {
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
