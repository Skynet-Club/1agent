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
    pub action_version: String,
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
    let action_version = proposal["action_version"]
        .as_str()
        .unwrap_or_default()
        .to_string();
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
        action_version,
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
            "action_version": self.action.version,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalIdArgs {
    proposal_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareReleaseArgs {
    proposal_id: String,
    #[serde(default)]
    version: Option<String>,
}

pub struct CodeRunChecksTool {
    action: ActionDefinition,
}

impl CodeRunChecksTool {
    pub fn new(action: ActionDefinition) -> Self {
        Self { action }
    }
}

#[async_trait]
impl AgentTool for CodeRunChecksTool {
    fn spec(&self) -> ToolSpec {
        self.action.tool_spec()
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn reversible(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_write {
            return Err(ToolError::PermissionDenied(
                "run_checks requires write permission for its check report".into(),
            ));
        }
        let args: ProposalIdArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        validate_safe_id(&args.proposal_id)?;
        let root = workspace_root(permissions);
        let proposal_path = root
            .join(".1a/proposals")
            .join(format!("{}.json", args.proposal_id));
        let (proposal, _proposal_bytes, proposal_sha256) = read_proposal(&proposal_path)?;
        let paths = proposal["paths"]
            .as_array()
            .ok_or_else(|| ToolError::Failed("proposal has no paths array".into()))?;
        let canonical_root = std::fs::canonicalize(&root).unwrap_or(root.clone());
        let mut files = Vec::new();
        let mut passed = true;
        for value in paths {
            let raw = value
                .as_str()
                .ok_or_else(|| ToolError::Failed("proposal path is not a string".into()))?;
            validate_relative_path(raw)?;
            let full = root.join(raw);
            let canonical = std::fs::canonicalize(&full);
            let canonical = match canonical {
                Ok(path) if path.starts_with(&canonical_root) && path.is_file() => path,
                _ => {
                    passed = false;
                    files.push(json!({"path": raw, "exists": false, "sha256": null, "bytes": 0}));
                    continue;
                }
            };
            let metadata = std::fs::metadata(&canonical)
                .map_err(|error| ToolError::Failed(format!("cannot stat {raw}: {error}")))?;
            if metadata.len() > 16 * 1024 * 1024 {
                passed = false;
                files.push(json!({"path": raw, "exists": true, "sha256": null, "bytes": metadata.len(), "error": "file exceeds 16 MiB"}));
                continue;
            }
            let bytes = std::fs::read(&canonical)
                .map_err(|error| ToolError::Failed(format!("cannot read {raw}: {error}")))?;
            files.push(json!({
                "path": raw,
                "exists": true,
                "bytes": bytes.len(),
                "sha256": hex::encode(Sha256::digest(&bytes))
            }));
        }
        let report = json!({
            "schema": "1ao.check-report/v1",
            "proposal_id": args.proposal_id,
            "proposal_sha256": proposal_sha256,
            "action_version": self.action.version,
            "status": if passed { "passed" } else { "failed" },
            "files": files,
            "checked_unix_ms": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        });
        let bytes = serde_json::to_vec_pretty(&report)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let path = root
            .join(".1a/checks")
            .join(format!("{}.json", args.proposal_id));
        crate::jsonl::write_private_atomic(&path, &bytes, "check-report.json")
            .map_err(|error| ToolError::Failed(format!("cannot write check report: {error}")))?;
        Ok(ToolOutcome {
            summary: format!(
                "checks {} for proposal {}",
                if passed { "passed" } else { "failed" },
                args.proposal_id
            ),
            content: serde_json::to_string(&report)
                .map_err(|error| ToolError::Failed(error.to_string()))?,
            metadata: std::collections::BTreeMap::from([
                ("action_type".into(), "code.run_checks".into()),
                ("proposal_id".into(), args.proposal_id),
                ("path".into(), path.display().to_string()),
                ("sha256".into(), sha256),
                ("bytes".into(), bytes.len().to_string()),
                (
                    "status".into(),
                    if passed { "passed" } else { "failed" }.into(),
                ),
                ("risk".into(), self.action.risk.clone()),
                ("reversible".into(), self.action.reversible.to_string()),
                ("proposal_sha256".into(), proposal_sha256),
            ]),
        })
    }
}

pub struct CodePrepareReleaseTool {
    action: ActionDefinition,
}

impl CodePrepareReleaseTool {
    pub fn new(action: ActionDefinition) -> Self {
        Self { action }
    }
}

#[async_trait]
impl AgentTool for CodePrepareReleaseTool {
    fn spec(&self) -> ToolSpec {
        self.action.tool_spec()
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn reversible(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_write {
            return Err(ToolError::PermissionDenied(
                "prepare_release requires write permission for its manifest".into(),
            ));
        }
        let args: PrepareReleaseArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        validate_safe_id(&args.proposal_id)?;
        let version = args.version.unwrap_or_else(|| "0.0.0-local".into());
        if version.is_empty()
            || version.len() > 64
            || !version
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
        {
            return Err(ToolError::InvalidArguments("invalid version".into()));
        }
        let root = workspace_root(permissions);
        let proposal_path = root
            .join(".1a/proposals")
            .join(format!("{}.json", args.proposal_id));
        let (proposal, _proposal_bytes, proposal_sha256) = read_proposal(&proposal_path)?;
        let approval_path = root
            .join(".1a/approvals")
            .join(format!("{}.json", args.proposal_id));
        let approval_bytes = read_json_file(&approval_path, "approval receipt")?;
        let approval: ProposalReceipt = serde_json::from_slice(&approval_bytes)
            .map_err(|error| ToolError::Failed(format!("invalid approval receipt: {error}")))?;
        if approval.status != "approved" || approval.proposal_sha256 != proposal_sha256 {
            return Err(ToolError::Failed(
                "approval receipt is not approved for this proposal".into(),
            ));
        }
        if approval.proposal_id != args.proposal_id {
            return Err(ToolError::Failed(
                "approval receipt proposal id does not match".into(),
            ));
        }
        let proposal_version = proposal["action_version"].as_str().unwrap_or_default();
        if !approval.action_version.is_empty()
            && !proposal_version.is_empty()
            && approval.action_version != proposal_version
        {
            return Err(ToolError::Failed(
                "approval receipt action version does not match".into(),
            ));
        }
        let check_path = root
            .join(".1a/checks")
            .join(format!("{}.json", args.proposal_id));
        let check_bytes = read_json_file(&check_path, "check report")?;
        let check_report: Value = serde_json::from_slice(&check_bytes)
            .map_err(|error| ToolError::Failed(format!("invalid check report: {error}")))?;
        if check_report["status"] != "passed" || check_report["proposal_sha256"] != proposal_sha256
        {
            return Err(ToolError::Failed(
                "check report did not pass for this proposal".into(),
            ));
        }
        let release_dir = root.join(".1a/releases").join(&args.proposal_id);
        let manifest_path = release_dir.join("release.json");
        if let Ok(existing) = std::fs::read(&manifest_path)
            && let Ok(existing) = serde_json::from_slice::<Value>(&existing)
            && existing["proposal_sha256"] == proposal_sha256
            && existing["approval_sha256"] == hex::encode(Sha256::digest(&approval_bytes))
            && existing["check_report_sha256"] == hex::encode(Sha256::digest(&check_bytes))
        {
            let sha256 = hex::encode(Sha256::digest(std::fs::read(&manifest_path).unwrap()));
            return Ok(release_outcome(
                self.action.risk.clone(),
                self.action.reversible,
                &args.proposal_id,
                &version,
                &manifest_path,
                &sha256,
                &proposal_sha256,
            ));
        }
        let manifest = json!({
            "schema": "1ao.release-candidate/v1",
            "proposal_id": args.proposal_id,
            "version": version,
            "status": "prepared",
            "proposal_sha256": proposal_sha256,
            "action_version": self.action.version,
            "approval_sha256": hex::encode(Sha256::digest(&approval_bytes)),
            "check_report_sha256": hex::encode(Sha256::digest(&check_bytes)),
            "files": check_report["files"].clone(),
            "created_unix_ms": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
        });
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        let sha256 = hex::encode(Sha256::digest(&bytes));
        crate::jsonl::write_private_atomic(&manifest_path, &bytes, "release.json").map_err(
            |error| ToolError::Failed(format!("cannot write release manifest: {error}")),
        )?;
        let sums = check_report["files"]
            .as_array()
            .map(|files| {
                files
                    .iter()
                    .filter_map(|file| {
                        Some(format!(
                            "{}  {}",
                            file["sha256"].as_str()?,
                            file["path"].as_str()?
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let sums_path = release_dir.join("SHA256SUMS");
        crate::jsonl::write_private_atomic(
            &sums_path,
            format!("{sums}\n").as_bytes(),
            "SHA256SUMS",
        )
        .map_err(|error| ToolError::Failed(format!("cannot write checksums: {error}")))?;
        Ok(release_outcome(
            self.action.risk.clone(),
            self.action.reversible,
            &args.proposal_id,
            &version,
            &manifest_path,
            &sha256,
            &proposal_sha256,
        ))
    }
}

fn release_outcome(
    risk: String,
    reversible: bool,
    proposal_id: &str,
    version: &str,
    path: &Path,
    sha256: &str,
    proposal_sha256: &str,
) -> ToolOutcome {
    ToolOutcome {
        summary: format!("prepared local release candidate {version} for {proposal_id}"),
        content: json!({
            "proposal_id": proposal_id,
            "version": version,
            "status": "prepared",
            "path": path.display().to_string(),
            "sha256": sha256,
        })
        .to_string(),
        metadata: std::collections::BTreeMap::from([
            ("action_type".into(), "code.prepare_release".into()),
            ("proposal_id".into(), proposal_id.to_string()),
            ("path".into(), path.display().to_string()),
            ("sha256".into(), sha256.to_string()),
            ("status".into(), "prepared".into()),
            ("risk".into(), risk),
            ("reversible".into(), reversible.to_string()),
            ("proposal_sha256".into(), proposal_sha256.to_string()),
            ("version".into(), version.to_string()),
        ]),
    }
}

fn workspace_root(permissions: &PermissionSet) -> PathBuf {
    permissions
        .read_roots
        .first()
        .cloned()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn validate_safe_id(value: &str) -> Result<(), ToolError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ToolError::InvalidArguments("invalid proposal_id".into()));
    }
    Ok(())
}

fn read_json_file(path: &Path, label: &str) -> Result<Vec<u8>, ToolError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ToolError::Failed(format!("{label} is missing: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ToolError::Failed(format!("{label} must be a regular file")));
    }
    if metadata.len() > MAX_PROPOSAL_BYTES {
        return Err(ToolError::Failed(format!("{label} is too large")));
    }
    std::fs::read(path).map_err(|error| ToolError::Failed(format!("cannot read {label}: {error}")))
}

fn read_proposal(path: &Path) -> Result<(Value, Vec<u8>, String), ToolError> {
    let bytes = read_json_file(path, "proposal")?;
    let proposal: Value = serde_json::from_slice(&bytes)
        .map_err(|error| ToolError::Failed(format!("invalid proposal: {error}")))?;
    if proposal["schema"] != "1ao.change-proposal/v1" || proposal["status"] != "proposed" {
        return Err(ToolError::Failed(
            "proposal schema/status is not a proposed change".into(),
        ));
    }
    let sha256 = hex::encode(Sha256::digest(&bytes));
    Ok((proposal, bytes, sha256))
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

    fn action_by_id(id: &str) -> ActionDefinition {
        crate::ontology::try_builtin_action_registry()
            .unwrap()
            .actions
            .into_iter()
            .find(|action| action.id == id)
            .unwrap()
    }

    fn action() -> ActionDefinition {
        action_by_id("code.propose_change")
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
        assert_eq!(proposal["action_version"], "1");
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
                "action_version": "1",
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
        assert_eq!(receipt.action_version, "1");
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
    async fn checks_and_release_follow_the_human_approval_chain() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn answer() -> u8 { 42 }",
        )
        .unwrap();
        let permissions = permissions(dir.path());

        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({"summary": "test release", "paths": ["src/lib.rs"]}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        let proposal_path = PathBuf::from(&proposal.metadata["path"]);

        let checks = CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(checks.metadata["status"], "passed");
        assert!(
            dir.path()
                .join(".1a/checks")
                .join(format!("{proposal_id}.json"))
                .exists()
        );

        let release_tool = CodePrepareReleaseTool::new(action_by_id("code.prepare_release"));
        let missing = release_tool
            .execute(
                json!({"proposal_id": proposal_id, "version": "1.0.0"}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("approval"));

        let receipt = decide_proposal(&proposal_path, "approve", "alice", Some("ship it")).unwrap();
        assert_eq!(receipt.status, "approved");
        let release = release_tool
            .execute(
                json!({"proposal_id": proposal_id, "version": "1.0.0"}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(release.metadata["status"], "prepared");
        let manifest_path = PathBuf::from(&release.metadata["path"]);
        assert!(manifest_path.exists());
        assert!(manifest_path.parent().unwrap().join("SHA256SUMS").exists());
        let manifest: Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        assert_eq!(manifest["status"], "prepared");
        assert_eq!(manifest["version"], "1.0.0");
        assert_eq!(manifest["action_version"], "1");
        assert_eq!(manifest["proposal_sha256"], receipt.proposal_sha256);
        assert_eq!(manifest["files"][0]["path"], "src/lib.rs");
    }

    #[tokio::test]
    async fn checks_fail_for_a_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({"summary": "missing", "paths": ["src/missing.rs"]}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let checks = CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal.metadata["proposal_id"]}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(checks.metadata["status"], "failed");
    }

    #[tokio::test]
    async fn checks_and_release_reject_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let mut permissions = permissions(dir.path());
        permissions.allow_write = false;
        let checks = CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": "p1"}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(checks, ToolError::PermissionDenied(_)));
        let release = CodePrepareReleaseTool::new(action_by_id("code.prepare_release"))
            .execute(
                json!({"proposal_id": "p1"}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(release, ToolError::PermissionDenied(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn checks_fail_when_a_symlink_escapes_the_workspace() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.txt");
        std::fs::write(&victim, "outside").unwrap();
        symlink(&victim, dir.path().join("link.txt")).unwrap();
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({"summary": "escape", "paths": ["link.txt"]}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let checks = CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal.metadata["proposal_id"]}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(checks.metadata["status"], "failed");
    }

    #[tokio::test]
    async fn release_rejects_a_stale_approval_after_the_proposal_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "fn old() {}").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({"summary": "old", "paths": ["src/lib.rs"]}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        let proposal_path = PathBuf::from(&proposal.metadata["path"]);
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(&proposal_path, "approve", "alice", None).unwrap();
        std::fs::write(&proposal_path, b"{}").unwrap();
        let error = CodePrepareReleaseTool::new(action_by_id("code.prepare_release"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("proposal") || error.to_string().contains("approval"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn release_rejects_a_tampered_check_report() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "fn ok() {}").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({"summary": "ok", "paths": ["src/lib.rs"]}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        let proposal_path = PathBuf::from(&proposal.metadata["path"]);
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(&proposal_path, "approve", "alice", None).unwrap();
        let check_path = dir
            .path()
            .join(".1a/checks")
            .join(format!("{proposal_id}.json"));
        let mut report: Value =
            serde_json::from_slice(&std::fs::read(&check_path).unwrap()).unwrap();
        report["status"] = json!("failed");
        std::fs::write(&check_path, serde_json::to_vec(&report).unwrap()).unwrap();
        let error = CodePrepareReleaseTool::new(action_by_id("code.prepare_release"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("check report"), "{error}");
    }

    #[tokio::test]
    async fn corrupt_proposal_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".1a/proposals/p1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not json").unwrap();
        let error = CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": "p1"}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid proposal"), "{error}");
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
