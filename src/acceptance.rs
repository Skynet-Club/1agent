//! Completion acceptance: contract, independent verification and receipt.
//!
//! This wraps the existing `verification` module. A verifier is never the
//! executor; every criterion needs a result and evidence; expected artifacts
//! must have matching checks. The receipt is the only thing that can justify a
//! `completed` delivery.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{ImpactLevel, RiskLevel};
use crate::verification::{
    ArtifactExpectation, VerificationError, VerificationInput, VerificationOutcome,
    VerificationReport, verify,
};

#[derive(Debug, Error)]
pub enum AcceptanceError {
    #[error("task id must not be empty")]
    EmptyTaskId,
    #[error("acceptance contract has no criteria")]
    NoCriteria,
    #[error("verification input is missing contract criterion: {0}")]
    MissingContractCriterion(String),
    #[error("verification input is missing hidden criterion: {0}")]
    MissingHiddenCriterion(String),
    #[error("verification artifacts do not match the contract")]
    ArtifactMismatch,
    #[error("verifier id must use the `human:` prefix for a human verifier")]
    HumanVerifierRequired,
    #[error("independent verification requires a `verifier:` or `agent:` verifier id, not `{0}`")]
    IndependentVerifierRequired(String),
    #[error("acceptance contract serialization failed: {0}")]
    Serialization(String),
    #[error(transparent)]
    Verification(#[from] VerificationError),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerifierKind {
    Automated,
    Independent,
    Human,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResidualRisk {
    pub description: String,
    pub impact: ImpactLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AcceptanceContract {
    pub task_id: String,
    pub objective: String,
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub hidden_criteria: Vec<String>,
    #[serde(default)]
    pub expected_artifacts: Vec<ArtifactExpectation>,
    pub risk: RiskLevel,
    pub required_verifier: VerifierKind,
    #[serde(default)]
    pub residual_risks: Vec<ResidualRisk>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceDecision {
    Accepted,
    Conditional,
    Rejected,
    Hacked,
}

impl AcceptanceDecision {
    #[cfg(test)]
    pub fn is_accepted(self) -> bool {
        self == Self::Accepted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompletionReceipt {
    pub task_id: String,
    pub contract_digest: String,
    pub evidence_digest: String,
    pub verifier_id: String,
    pub verifier_kind: VerifierKind,
    pub acceptance_criteria: Vec<String>,
    pub hidden_criteria: Vec<String>,
    pub report: VerificationReport,
    pub decision: AcceptanceDecision,
    pub residual_risks: Vec<ResidualRisk>,
    pub created_ms: u128,
}

/// Close a task against a contract. This does not run the task or a checker;
/// it evaluates the caller-supplied verification input and returns a receipt.
pub fn close(
    contract: &AcceptanceContract,
    input: &VerificationInput,
    now_ms: u128,
) -> Result<CompletionReceipt, AcceptanceError> {
    if contract.task_id.trim().is_empty() {
        return Err(AcceptanceError::EmptyTaskId);
    }
    if contract.acceptance_criteria.is_empty() {
        return Err(AcceptanceError::NoCriteria);
    }
    for criterion in &contract.acceptance_criteria {
        if !input.acceptance_criteria.contains(criterion) {
            return Err(AcceptanceError::MissingContractCriterion(criterion.clone()));
        }
    }
    for criterion in &contract.hidden_criteria {
        if !input.hidden_criteria.contains(criterion) {
            return Err(AcceptanceError::MissingHiddenCriterion(criterion.clone()));
        }
    }
    if contract.expected_artifacts != input.expected_artifacts {
        return Err(AcceptanceError::ArtifactMismatch);
    }
    check_verifier(contract.required_verifier, &input.verifier_id)?;

    let report = verify(input)?;
    let high_residual = contract
        .residual_risks
        .iter()
        .any(|risk| risk.impact == ImpactLevel::High);
    let decision = match report.outcome {
        VerificationOutcome::Rejected => AcceptanceDecision::Rejected,
        VerificationOutcome::Hacked => AcceptanceDecision::Hacked,
        VerificationOutcome::Verified if high_residual => AcceptanceDecision::Conditional,
        VerificationOutcome::Verified => AcceptanceDecision::Accepted,
    };
    let contract_digest = digest_json(contract)?;
    let evidence_digest = digest_json(input)?;
    Ok(CompletionReceipt {
        task_id: contract.task_id.clone(),
        contract_digest,
        evidence_digest,
        verifier_id: report.verifier_id.clone(),
        verifier_kind: contract.required_verifier,
        acceptance_criteria: contract.acceptance_criteria.clone(),
        hidden_criteria: contract.hidden_criteria.clone(),
        report,
        decision,
        residual_risks: contract.residual_risks.clone(),
        created_ms: now_ms,
    })
}

fn check_verifier(kind: VerifierKind, verifier_id: &str) -> Result<(), AcceptanceError> {
    match kind {
        VerifierKind::Automated => Ok(()),
        VerifierKind::Human if verifier_id.starts_with("human:") => Ok(()),
        VerifierKind::Human => Err(AcceptanceError::HumanVerifierRequired),
        VerifierKind::Independent
            if verifier_id.starts_with("verifier:") || verifier_id.starts_with("agent:") =>
        {
            Ok(())
        }
        VerifierKind::Independent => Err(AcceptanceError::IndependentVerifierRequired(
            verifier_id.to_string(),
        )),
    }
}

fn digest_json(value: &impl Serialize) -> Result<String, AcceptanceError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| AcceptanceError::Serialization(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RiskLevel;
    use crate::verification::{ArtifactCheck, CriterionResult};

    fn contract(required_verifier: VerifierKind) -> AcceptanceContract {
        AcceptanceContract {
            task_id: "task-1".into(),
            objective: "deliver a verified artifact".into(),
            acceptance_criteria: vec!["artifact exists".into(), "content matches".into()],
            hidden_criteria: vec!["no hidden failure".into()],
            expected_artifacts: vec![ArtifactExpectation {
                path: "out.html".into(),
                sha256: Some("abc".into()),
            }],
            risk: RiskLevel::Medium,
            required_verifier,
            residual_risks: Vec::new(),
        }
    }

    fn input(verifier: &str) -> VerificationInput {
        VerificationInput {
            executor_id: "model:worker".into(),
            verifier_id: verifier.into(),
            acceptance_criteria: vec!["artifact exists".into(), "content matches".into()],
            hidden_criteria: vec!["no hidden failure".into()],
            criteria_results: vec![
                CriterionResult {
                    criterion: "artifact exists".into(),
                    passed: true,
                    evidence: vec!["write receipt".into()],
                },
                CriterionResult {
                    criterion: "content matches".into(),
                    passed: true,
                    evidence: vec!["sha256 matches".into()],
                },
                CriterionResult {
                    criterion: "no hidden failure".into(),
                    passed: true,
                    evidence: vec!["hidden check".into()],
                },
            ],
            expected_artifacts: vec![ArtifactExpectation {
                path: "out.html".into(),
                sha256: Some("abc".into()),
            }],
            artifact_checks: vec![ArtifactCheck {
                path: "out.html".into(),
                observed_sha256: "abc".into(),
                passed: true,
            }],
        }
    }

    #[test]
    fn independent_verification_accepts() {
        let receipt = close(
            &contract(VerifierKind::Independent),
            &input("verifier:audit"),
            1,
        )
        .expect("receipt");
        assert_eq!(receipt.decision, AcceptanceDecision::Accepted);
        assert!(receipt.decision.is_accepted());
        assert_eq!(receipt.report.outcome, VerificationOutcome::Verified);
        assert!(!receipt.contract_digest.is_empty());
    }

    #[test]
    fn self_verification_fails() {
        let error = close(
            &contract(VerifierKind::Independent),
            &input("model:worker"),
            1,
        )
        .expect_err("self verification");
        assert!(matches!(
            error,
            AcceptanceError::IndependentVerifierRequired(_)
        ));
    }

    #[test]
    fn human_verifier_requires_human_prefix() {
        let error = close(&contract(VerifierKind::Human), &input("verifier:audit"), 1)
            .expect_err("human verifier");
        assert!(matches!(error, AcceptanceError::HumanVerifierRequired));
    }

    #[test]
    fn high_residual_risk_is_conditional() {
        let mut contract = contract(VerifierKind::Independent);
        contract.residual_risks.push(ResidualRisk {
            description: "not tested on the target browser".into(),
            impact: ImpactLevel::High,
        });
        let receipt = close(&contract, &input("verifier:audit"), 1).expect("receipt");
        assert_eq!(receipt.decision, AcceptanceDecision::Conditional);
    }

    #[test]
    fn hidden_failure_is_hacked() {
        let mut input = input("verifier:audit");
        input.criteria_results[2].passed = false;
        let receipt = close(&contract(VerifierKind::Independent), &input, 1).expect("receipt");
        assert_eq!(receipt.decision, AcceptanceDecision::Hacked);
        assert_eq!(receipt.report.outcome, VerificationOutcome::Hacked);
    }

    #[test]
    fn missing_evidence_is_rejected() {
        let mut input = input("verifier:audit");
        input.criteria_results[0].evidence.clear();
        let error =
            close(&contract(VerifierKind::Independent), &input, 1).expect_err("missing evidence");
        assert!(matches!(error, AcceptanceError::Verification(_)));
    }

    #[test]
    fn contract_and_input_must_match() {
        let mut input = input("verifier:audit");
        input.acceptance_criteria.pop();
        let error =
            close(&contract(VerifierKind::Independent), &input, 1).expect_err("criterion mismatch");
        assert!(matches!(
            error,
            AcceptanceError::MissingContractCriterion(_)
        ));
    }
}
