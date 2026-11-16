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
            verifier_id: self.verifier,
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
            description: "Close a task against an acceptance contract and return a CompletionReceipt. Call once per deliverable and then answer from the receipt; do not re-run the gate on its own receipt. Send one flat object: contract fields plus the independent verifier's per-criterion results and artifact checks. Put the exact criterion text in each result. A passed result needs non-empty evidence. The verifier must differ from the executor; `independent` needs a `verifier:` or `agent:` id, `human` needs `human:`. Read-only: it does not run the task or a checker. Only an `accepted` receipt justifies `completed`.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "task_id", "objective", "acceptance_criteria", "risk",
                    "required_verifier", "executor", "verifier", "results"
                ],
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "Stable id for this task or deliverable."
                    },
                    "objective": {
                        "type": "string",
                        "description": "What the task must achieve."
                    },
                    "acceptance_criteria": {
                        "type": "array",
                        "minItems": 1,
                        "items": {"type": "string"},
                        "description": "Public criteria. Each text must appear exactly once in `results`."
                    },
                    "hidden_criteria": {
                        "type": "array",
                        "items": {"type": "string"},
                        "default": [],
                        "description": "Checks the executor did not see. Any failure makes the decision `hacked`."
                    },
                    "expected_artifacts": {
                        "type": "array",
                        "default": [],
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["path"],
                            "properties": {
                                "path": {"type": "string"},
                                "sha256": {
                                    "type": "string",
                                    "description": "Expected SHA-256; omit this field to require only one passing check."
                                }
                            }
                        }
                    },
                    "risk": {
                        "type": "string",
                        "enum": ["low", "medium", "high"]
                    },
                    "required_verifier": {
                        "type": "string",
                        "enum": ["automated", "independent", "human"],
                        "description": "`independent` requires `verifier:` or `agent:`; `human` requires `human:`."
                    },
                    "residual_risks": {
                        "type": "array",
                        "default": [],
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["description", "impact"],
                            "properties": {
                                "description": {"type": "string"},
                                "impact": {"type": "string", "enum": ["low", "medium", "high"]}
                            }
                        },
                        "description": "Known open risks. Any `high` impact turns `accepted` into `conditional`."
                    },
                    "executor": {
                        "type": "string",
                        "description": "Identity that did the work, e.g. `model:worker`."
                    },
                    "verifier": {
                        "type": "string",
                        "description": "Independent verifier identity; must differ from `executor`."
                    },
                    "results": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["criterion", "passed", "evidence"],
                            "properties": {
                                "criterion": {
                                    "type": "string",
                                    "description": "Exact text of one acceptance or hidden criterion."
                                },
                                "passed": {"type": "boolean"},
                                "evidence": {
                                    "type": "array",
                                    "items": {"type": "string"},
                                    "description": "Required non-empty for a pass."
                                }
                            }
                        }
                    },
                    "artifact_checks": {
                        "type": "array",
                        "default": [],
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["path", "observed_sha256", "passed"],
                            "properties": {
                                "path": {"type": "string"},
                                "observed_sha256": {"type": "string"},
                                "passed": {"type": "boolean"}
                            }
                        },
                        "description": "Exactly one check per expected artifact."
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
