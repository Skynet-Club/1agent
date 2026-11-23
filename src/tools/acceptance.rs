//! `acceptance_gate`: close a task against a contract and return a receipt.
//!
//! This is the tool surface for the acceptance module. It does not execute the
//! task or a domain checker; the caller supplies the contract and independent
//! verification input, and the tool returns an auditable decision.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::acceptance::{AcceptanceContract, ResidualRisk, VerifierKind, close};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::verification::{ArtifactCheck, ArtifactExpectation, CriterionResult, VerificationInput};

pub struct AcceptanceGateTool;

/// One flat request: the contract and the verifier's evidence in a single
/// object. The tool copies criteria and artifacts into `VerificationInput` so
/// the caller cannot accidentally submit mismatched lists.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceArgs {
    task_id: String,
    objective: String,
    acceptance_criteria: Vec<String>,
    #[serde(default)]
    hidden_criteria: Vec<String>,
    #[serde(default)]
    expected_artifacts: Vec<ArtifactExpectation>,
    risk: RiskLevel,
    required_verifier: VerifierKind,
    #[serde(default)]
    residual_risks: Vec<ResidualRisk>,
    executor: String,
    verifier: String,
    results: Vec<CriterionResult>,
    #[serde(default)]
    artifact_checks: Vec<ArtifactCheck>,
}

/// The verifier label is a role marker, not a secret. Models often write
/// `independent:…` or a prose verifier; prefixing it keeps the independence
/// requirement while avoiding a retry over punctuation. Human verification
/// still requires the explicit `human:` prefix.
fn normalize_verifier(kind: VerifierKind, verifier: String) -> String {
    let trimmed = verifier.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("verifier:")
        || trimmed.starts_with("agent:")
        || trimmed.starts_with("human:")
    {
        return trimmed.to_string();
    }
    match kind {
        VerifierKind::Independent => format!("verifier:{trimmed}"),
        VerifierKind::Automated | VerifierKind::Human => trimmed.to_string(),
    }
}

impl AcceptanceArgs {
    fn into_parts(self) -> (AcceptanceContract, VerificationInput) {
        let contract = AcceptanceContract {
            task_id: self.task_id,
            objective: self.objective,
            acceptance_criteria: self.acceptance_criteria,
            hidden_criteria: self.hidden_criteria,
            expected_artifacts: self.expected_artifacts,
            risk: self.risk,
            required_verifier: self.required_verifier,
            residual_risks: self.residual_risks,
        };
        let verification = VerificationInput {
            executor_id: self.executor,
            verifier_id: normalize_verifier(self.required_verifier, self.verifier),
            acceptance_criteria: contract.acceptance_criteria.clone(),
            hidden_criteria: contract.hidden_criteria.clone(),
            criteria_results: self.results,
            expected_artifacts: contract.expected_artifacts.clone(),
            artifact_checks: self.artifact_checks,
        };
        (contract, verification)
    }
}

#[async_trait]
impl AgentTool for AcceptanceGateTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "acceptance_gate".into(),
            // The detailed acceptance rules live in AGENT_PROMPT_ARTIFACTS;
            // keep this spec shape-only so the schema does not repeat them.
            description: "Close a task against an acceptance contract and return a receipt. Failed hidden criterion -> hacked; high residual risk -> conditional; only `accepted` justifies completed.".into(),
            parameters: json!({
                "type": "object",
                "required": [
                    "task_id", "objective", "acceptance_criteria", "risk",
                    "required_verifier", "executor", "verifier", "results"
                ],
                "properties": {
                    "task_id": {"type": "string"},
                    "objective": {"type": "string"},
                    "acceptance_criteria": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"type": "string"}
                    },
                    "hidden_criteria": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "expected_artifacts": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["path"],
                            "properties": {
                                "path": {"type": "string"},
                                "sha256": {"type": "string"}
                            }
                        }
                    },
                    "risk": {
                        "type": "string",
                        "enum": ["low", "medium", "high"]
                    },
                    "required_verifier": {
                        "type": "string",
                        "enum": ["automated", "independent", "human"]
                    },
                    "residual_risks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["description", "impact"],
                            "properties": {
                                "description": {"type": "string"},
                                "impact": {"type": "string", "enum": ["low", "medium", "high"]}
                            }
                        }
                    },
                    "executor": {"type": "string"},
                    "verifier": {"type": "string"},
                    "results": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "required": ["criterion", "passed", "evidence"],
                            "properties": {
                                "criterion": {"type": "string"},
                                "passed": {"type": "boolean"},
                                "evidence": {
                                    "type": "array",
                                    "items": {"type": "string"}
                                }
                            }
                        }
                    },
                    "artifact_checks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["path", "observed_sha256", "passed"],
                            "properties": {
                                "path": {"type": "string"},
                                "observed_sha256": {"type": "string"},
                                "passed": {"type": "boolean"}
                            }
                        }
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let args: AcceptanceArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let (contract, verification) = args.into_parts();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let receipt = close(&contract, &verification, now_ms)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        let decision = format!("{:?}", receipt.decision).to_lowercase();
        let content = serde_json::to_string(&receipt)
            .map_err(|error| ToolError::Failed(format!("cannot serialize receipt: {error}")))?;
        let mut metadata = BTreeMap::new();
        metadata.insert("decision".into(), decision.clone());
        metadata.insert("verifier_id".into(), receipt.verifier_id.clone());
        metadata.insert(
            "criteria".into(),
            receipt.acceptance_criteria.len().to_string(),
        );
        metadata.insert("contract_digest".into(), receipt.contract_digest.clone());
        Ok(ToolOutcome {
            summary: format!(
                "acceptance: {decision} · {} criteria · verifier {}",
                receipt.acceptance_criteria.len(),
                receipt.verifier_id
            ),
            content,
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_is_shape_only_and_compact() {
        let spec = AcceptanceGateTool.spec();
        let tokens = crate::context::estimate_tokens(&spec.description)
            + crate::context::estimate_tokens(&spec.parameters.to_string());
        assert!(tokens < 400, "acceptance schema is {tokens} tokens");
        assert_eq!(
            spec.parameters["required"].as_array().map(Vec::len),
            Some(8)
        );
        assert_eq!(spec.parameters["properties"]["results"]["minItems"], 1);
        assert!(spec.parameters["properties"]["task_id"]["description"].is_null());
    }

    #[tokio::test]
    async fn tool_returns_an_accepted_receipt() {
        let arguments = json!({
            "task_id": "task-1",
            "objective": "deliver",
            "acceptance_criteria": ["artifact exists"],
            "hidden_criteria": [],
            "expected_artifacts": [],
            "risk": "medium",
            "required_verifier": "independent",
            "residual_risks": [],
            "executor": "model:worker",
            "verifier": "verifier:audit",
            "results": [{
                "criterion": "artifact exists",
                "passed": true,
                "evidence": ["write receipt"]
            }],
            "artifact_checks": []
        });
        let outcome = AcceptanceGateTool
            .execute(
                arguments,
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .expect("receipt");
        assert!(outcome.summary.contains("accepted"));
        let receipt: Value = serde_json::from_str(&outcome.content).expect("json");
        assert_eq!(receipt["decision"], "accepted");
        assert_eq!(receipt["verifier_id"], "verifier:audit");
    }

    #[tokio::test]
    async fn independent_verifier_label_is_normalized() {
        let arguments = json!({
            "task_id": "task-1",
            "objective": "deliver",
            "acceptance_criteria": ["artifact exists"],
            "risk": "medium",
            "required_verifier": "independent",
            "executor": "model:worker",
            "verifier": "independent:artifact-hash+shell-check",
            "results": [{
                "criterion": "artifact exists",
                "passed": true,
                "evidence": ["sha256sum matches"]
            }]
        });
        let outcome = AcceptanceGateTool
            .execute(
                arguments,
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .expect("receipt");
        let receipt: Value = serde_json::from_str(&outcome.content).expect("json");
        assert_eq!(receipt["decision"], "accepted");
        assert_eq!(
            receipt["verifier_id"],
            "verifier:independent:artifact-hash+shell-check"
        );
    }

    #[test]
    fn advertised_schema_accepts_an_omitted_artifact_digest() {
        let arguments = json!({
            "task_id": "task-2",
            "objective": "deliver a page",
            "acceptance_criteria": ["page renders"],
            "risk": "medium",
            "required_verifier": "independent",
            "executor": "model:worker",
            "verifier": "verifier:audit",
            "expected_artifacts": [{"path": "index.html"}],
            "results": [{
                "criterion": "page renders",
                "passed": true,
                "evidence": ["HTTP 200"]
            }],
            "artifact_checks": [{
                "path": "index.html",
                "observed_sha256": "abc",
                "passed": true
            }]
        });
        crate::tools::validate_arguments(&AcceptanceGateTool.spec().parameters, &arguments)
            .expect("optional artifact digest");
    }

    #[tokio::test]
    async fn flat_request_carries_artifacts_into_verification() {
        let arguments = json!({
            "task_id": "task-2",
            "objective": "deliver a page",
            "acceptance_criteria": ["page renders"],
            "risk": "medium",
            "required_verifier": "independent",
            "executor": "model:worker",
            "verifier": "verifier:audit",
            "expected_artifacts": [{"path": "index.html", "sha256": "abc"}],
            "results": [{
                "criterion": "page renders",
                "passed": true,
                "evidence": ["HTTP 200 and 390/768/1440 screenshots"]
            }],
            "artifact_checks": [{
                "path": "index.html",
                "observed_sha256": "abc",
                "passed": true
            }]
        });
        let outcome = AcceptanceGateTool
            .execute(
                arguments,
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .expect("receipt");
        let receipt: Value = serde_json::from_str(&outcome.content).expect("json");
        assert_eq!(receipt["decision"], "accepted");
        assert_eq!(receipt["report"]["public_passed"], true);
    }
}
