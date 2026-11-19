//! Typed action tools generated from the 1AO ActionType registry.
//!
//! The first reference action is `code.propose_change`: it writes a reviewable
//! proposal artifact and never modifies the target files or a release target.
//! The tool spec comes from `ontology/actions.json`; the runtime gate still
//! applies the normal permission, risk and approval checks.

use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::ontology::ActionDefinition;

use super::{AgentTool, ToolError, ToolOutcome};

const MAX_SUMMARY_CHARS: usize = 2_000;
const MAX_PATHS: usize = 100;
const MAX_PROPOSAL_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProposalReceipt {
    pub schema: String,
    pub proposal_id: String,
    pub decision: String,
    pub actor: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub proposal_sha256: String,
    pub decided_at_unix_ms: u128,
    pub status: String,
}

/// Record a human decision for a staged proposal. This is deliberately a
/// library/CLI operation, not a model tool: an agent must not approve itself.
pub fn decide_proposal(
    proposal_path: &Path,
    decision: &str,
    actor: &str,
    reason: Option<&str>,
) -> Result<ProposalReceipt, String> {
    if !matches!(decision, "approve" | "reject") {
        return Err("decision must be approve or reject".into());
    }
    let actor = actor.trim();
    if actor.is_empty() || actor.chars().count() > 128 || actor.chars().any(char::is_control) {
        return Err("actor must be a non-empty, control-free string <=128 chars".into());
    }
    let metadata = std::fs::symlink_metadata(proposal_path)
        .map_err(|error| format!("cannot stat proposal: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("proposal must be a regular file".into());
    }
    if metadata.len() > MAX_PROPOSAL_BYTES {
        return Err(format!("proposal exceeds {MAX_PROPOSAL_BYTES} bytes"));
    }
    let bytes =
        std::fs::read(proposal_path).map_err(|error| format!("cannot read proposal: {error}"))?;
    let proposal: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("proposal is not valid JSON: {error}"))?;
    if proposal["schema"] != "1ao.change-proposal/v1" {
        return Err("unsupported proposal schema".into());
    }
    if proposal["status"] != "proposed" {
        return Err("proposal is not in proposed status".into());
    }
    let proposal_id = proposal["proposal_id"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "proposal_id is missing".to_string())?
        .to_string();
    let proposal_sha256 = hex::encode(Sha256::digest(&bytes));
    let approvals_dir = proposal_path
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("approvals"))
        .ok_or_else(|| "proposal path has no approval directory".to_string())?;
    let receipt_path = approvals_dir.join(format!("{proposal_id}.json"));
    if let Ok(existing) = std::fs::read(&receipt_path)
        && let Ok(existing) = serde_json::from_slice::<ProposalReceipt>(&existing)
    {
        if existing.decision == decision && existing.proposal_sha256 == proposal_sha256 {
            return Ok(existing);
        }
        return Err("proposal was already decided".into());
    }
    let receipt = ProposalReceipt {
        schema: "1ao.proposal-receipt/v1".into(),
        proposal_id,
        decision: decision.into(),
        actor: actor.into(),
        reason: reason
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(MAX_SUMMARY_CHARS).collect()),
        proposal_sha256,
        decided_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        status: if decision == "approve" {
            "approved".into()
        } else {
            "rejected".into()
        },
    };
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?;
    crate::jsonl::write_private_atomic(&receipt_path, &bytes, "receipt.json")
        .map_err(|error| format!("cannot write receipt: {error}"))?;
    Ok(receipt)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposeChangeArgs {
    summary: String,
    paths: Vec<String>,
}

pub struct CodeProposeChangeTool {
    action: ActionDefinition,
}

impl CodeProposeChangeTool {
    pub fn new(action: ActionDefinition) -> Self {
        Self { action }
    }
}

#[async_trait]
impl AgentTool for CodeProposeChangeTool {
    fn spec(&self) -> ToolSpec {
        self.action.tool_spec()
    }

    fn risk(&self) -> RiskLevel {
        match self.action.risk.as_str() {
            "high" => RiskLevel::High,
            "medium" => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn reversible(&self) -> bool {
        self.action.reversible
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_write {
            return Err(ToolError::PermissionDenied(
                "propose_change requires write permission for its proposal artifact".into(),
            ));
        }
        let args: ProposeChangeArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        let summary = args.summary.trim();
        if summary.is_empty() || summary.chars().count() > MAX_SUMMARY_CHARS {
            return Err(ToolError::InvalidArguments(format!(
                "summary must be 1..={MAX_SUMMARY_CHARS} characters"
            )));
        }
        if args.paths.is_empty() || args.paths.len() > MAX_PATHS {
            return Err(ToolError::InvalidArguments(format!(
                "paths must contain 1..={MAX_PATHS} entries"
            )));
        }
        for path in &args.paths {
            validate_relative_path(path)?;
        }

        let root = permissions
            .read_roots
            .first()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let proposal_id = uuid::Uuid::new_v4().to_string();
        let path = root
            .join(".1a")
            .join("proposals")
            .join(format!("{proposal_id}.json"));
        let created_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let run_id = super::RUN_CONTEXT
            .try_with(|context| context.run_id.clone())
            .ok();
        let proposal = json!({
            "schema": "1ao.change-proposal/v1",
            "proposal_id": proposal_id,
            "action_type": "code.propose_change",
            "status": "proposed",
            "summary": summary,
            "paths": args.paths,
            "run_id": run_id,
            "created_unix_ms": created_unix_ms,
        });
        let bytes = serde_json::to_vec_pretty(&proposal)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        crate::jsonl::write_private_atomic(&path, &bytes, "proposal.json")
            .map_err(|error| ToolError::Failed(format!("cannot write proposal: {error}")))?;
        Ok(ToolOutcome {
            summary: format!("proposed change {proposal_id}"),
            content: serde_json::to_string(&proposal)
                .map_err(|error| ToolError::Failed(error.to_string()))?,
            metadata: std::collections::BTreeMap::from([
                ("action_type".into(), "code.propose_change".into()),
                ("proposal_id".into(), proposal_id),
                ("path".into(), path.display().to_string()),
                ("sha256".into(), sha256),
                ("bytes".into(), bytes.len().to_string()),
                ("risk".into(), self.action.risk.clone()),
                ("reversible".into(), self.action.reversible.to_string()),
                ("status".into(), "proposed".into()),
            ]),
        })
    }
}

fn validate_relative_path(value: &str) -> Result<(), ToolError> {
    if value.trim().is_empty() {
        return Err(ToolError::InvalidArguments(
            "paths must not be empty".into(),
        ));
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(ToolError::InvalidArguments(format!(
            "path `{value}` must stay inside the workspace"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn permissions(root: &Path) -> PermissionSet {
        PermissionSet {
            read_roots: vec![root.to_path_buf()],
            allow_network: false,
            allow_write: true,
        }
    }

    fn action() -> ActionDefinition {
        crate::ontology::try_builtin_action_registry()
            .unwrap()
            .actions
            .into_iter()
            .find(|action| action.id == "code.propose_change")
            .unwrap()
    }

    #[tokio::test]
    async fn propose_change_writes_a_private_proposal() {
        let dir = tempfile::tempdir().unwrap();
        let tool = CodeProposeChangeTool::new(action());
        let outcome = tool
            .execute(
                json!({"summary": "tighten the parser", "paths": ["src/parser.rs"]}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome.metadata["action_type"], "code.propose_change");
        let path = PathBuf::from(&outcome.metadata["path"]);
        assert!(path.starts_with(dir.path().join(".1a/proposals")));
        let bytes = std::fs::read(&path).unwrap();
        let proposal: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(proposal["status"], "proposed");
        assert_eq!(proposal["paths"][0], "src/parser.rs");
        assert_eq!(
            outcome.metadata["sha256"],
            hex::encode(Sha256::digest(&bytes))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn propose_change_rejects_absolute_and_parent_paths() {
        let dir = tempfile::tempdir().unwrap();
        let tool = CodeProposeChangeTool::new(action());
        for path in ["/etc/passwd", "../outside"] {
            let error = tool
                .execute(
                    json!({"summary": "x", "paths": [path]}),
                    &permissions(dir.path()),
                    &tokio_util::sync::CancellationToken::new(),
                )
                .await
                .unwrap_err();
            assert!(error.to_string().contains("inside the workspace"));
        }
    }

    fn proposal_file(dir: &Path) -> PathBuf {
        let path = dir.join(".1a/proposals/p1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "schema": "1ao.change-proposal/v1",
                "proposal_id": "p1",
                "status": "proposed",
                "summary": "test",
                "paths": ["src/lib.rs"]
            }))
            .unwrap(),
        )
        .unwrap();
        path
    }

    #[test]
    fn human_can_approve_a_proposal_once() {
        let dir = tempfile::tempdir().unwrap();
        let proposal = proposal_file(dir.path());
        let receipt = decide_proposal(&proposal, "approve", "alice", Some("looks good")).unwrap();
        assert_eq!(receipt.status, "approved");
        assert_eq!(receipt.actor, "alice");
        assert_eq!(receipt.proposal_sha256.len(), 64);
        let path = dir.path().join(".1a/approvals/p1.json");
        assert!(path.exists());
        let again = decide_proposal(&proposal, "approve", "alice", None).unwrap();
        assert_eq!(again, receipt, "identical approval is idempotent");
        let error = decide_proposal(&proposal, "reject", "alice", None).unwrap_err();
        assert!(error.contains("already decided"));
    }

    #[test]
    fn human_can_reject_and_actor_is_validated() {
        let dir = tempfile::tempdir().unwrap();
        let proposal = proposal_file(dir.path());
        let receipt = decide_proposal(&proposal, "reject", "bob", Some("not needed")).unwrap();
        assert_eq!(receipt.status, "rejected");
        assert!(decide_proposal(&proposal, "approve", "", None).is_err());
        assert!(decide_proposal(&proposal, "maybe", "bob", None).is_err());
    }

    #[tokio::test]
    async fn propose_change_requires_write_permission() {
        let dir = tempfile::tempdir().unwrap();
        let tool = CodeProposeChangeTool::new(action());
        let mut permissions = permissions(dir.path());
        permissions.allow_write = false;
        let error = tool
            .execute(
                json!({"summary": "x", "paths": ["src/lib.rs"]}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }
}
