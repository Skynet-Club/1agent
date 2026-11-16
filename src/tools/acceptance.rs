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
use crate::acceptance::{AcceptanceContract, close};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::verification::VerificationInput;

pub struct AcceptanceGateTool;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptanceArgs {
    contract: AcceptanceContract,
    verification: VerificationInput,
}

#[async_trait]
impl AgentTool for AcceptanceGateTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "acceptance_gate".into(),
            description: "Close a task against an acceptance contract and return a CompletionReceipt. The contract defines criteria, hidden criteria, artifacts, risk, required verifier and residual risks; the verification input supplies the executor, independent verifier, per-criterion results with evidence and artifact checks. Read-only: it does not run the task or a checker. A delivery with an unaccepted receipt must not be reported as completed.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["contract", "verification"],
                "properties": {
                    "contract": {"type": "object"},
                    "verification": {"type": "object"}
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
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let receipt = close(&args.contract, &args.verification, now_ms)
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
            "contract": {
                "task_id": "task-1",
                "objective": "deliver",
                "acceptance_criteria": ["artifact exists"],
                "hidden_criteria": [],
                "expected_artifacts": [],
                "risk": "medium",
                "required_verifier": "independent",
                "residual_risks": []
            },
            "verification": {
                "executor_id": "model:worker",
                "verifier_id": "verifier:audit",
                "acceptance_criteria": ["artifact exists"],
                "hidden_criteria": [],
                "criteria_results": [{
                    "criterion": "artifact exists",
                    "passed": true,
                    "evidence": ["write receipt"]
                }],
                "expected_artifacts": [],
                "artifact_checks": []
            }
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
}
