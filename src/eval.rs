use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::domain::{AnswerStatus, StopReason, TaskRequest, ToolStatus, Usage};
use crate::journal::{audit_events, read_events};
use crate::provider::DeterministicProvider;
use crate::runtime::{AgentRuntime, ProviderCandidate, local_manifest};
use crate::tools::default_registry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalSuite {
    pub name: String,
    pub cases: Vec<EvalCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    pub id: String,
    pub objective: String,
    #[serde(default)]
    pub setup_files: Vec<SetupFile>,
    pub expect: EvalExpectation,
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
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub hard_gate_failures: usize,
    pub cases: Vec<EvalCaseResult>,
}

pub fn load_suite(path: impl AsRef<Path>) -> Result<EvalSuite, EvalError> {
    let content = std::fs::read_to_string(path.as_ref())?;
    let suite: EvalSuite = serde_json::from_str(&content)?;
    if suite.cases.is_empty() {
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
    if providers.is_empty() {
        return Err(EvalError::NoProviders);
    }
    let mut results = Vec::new();
    for case in suite.cases {
        results.push(run_case(case, providers.clone(), allow_network).await);
    }
    let passed = results.iter().filter(|result| result.passed).count();
    let hard_gate_failures = results
        .iter()
        .filter(|result| result.hard_gate_failed)
        .count();
    Ok(EvalReport {
        suite: suite.name,
        total: results.len(),
        passed,
        failed: results.len() - passed,
        hard_gate_failures,
        cases: results,
    })
}

async fn run_case(
    case: EvalCase,
    providers: Vec<ProviderCandidate>,
    allow_network: bool,
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
    let runtime = AgentRuntime::new(
        providers,
        default_registry(),
        workspace.path().join(".journal"),
    );
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
    let hard_gate_failed = !audit.ok && case.expect.audit_ok;
    EvalCaseResult {
        id: case.id,
        passed: issues.is_empty(),
        hard_gate_failed,
        stop_reason: Some(result.stop_reason),
        answer_status: Some(result.answer.status),
        evidence_count: result.answer.evidence.len(),
        claim_count: result.answer.claims.len(),
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
        provider_id: None,
        model: None,
        duration_ms: 0,
        usage: Usage::default(),
        audit_ok: false,
        issues: vec![issue],
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deterministic_suite_reports_success_and_hard_gates() {
        let report = run_suite(EvalSuite {
            name: "test".into(),
            cases: vec![EvalCase {
                id: "read".into(),
                objective: "Research @read-file:\"${workspace}/evidence.txt\"".into(),
                setup_files: vec![SetupFile {
                    path: "evidence.txt".into(),
                    content: "verified evidence".into(),
                }],
                expect: EvalExpectation {
                    stop_reason: StopReason::Completed,
                    answer_status: AnswerStatus::Completed,
                    min_evidence: 1,
                    min_claims: 1,
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
}
