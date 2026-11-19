//! Typed action tools generated from the 1AO ActionType registry.
//!
//! Reference actions cover propose -> check -> human decision -> merge/release.
//! Proposals and checks never modify target files; only `code.merge_change`
//! writes them, after an approved receipt and a passing staged check report.
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
const MAX_PROPOSAL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CHANGE_BYTES: usize = 512 * 1024;
const MAX_TARGET_BYTES: u64 = 16 * 1024 * 1024;

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
    let control_root = proposal_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| "proposal path has no workspace root".to_string())?;
    ensure_control_dir(control_root, ".1a/approvals").map_err(|error| error.to_string())?;
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
    #[serde(default)]
    changes: Vec<ChangeInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeInput {
    path: String,
    content: String,
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
        let root = workspace_root(permissions);
        let canonical_root = std::fs::canonicalize(&root).unwrap_or(root.clone());
        let path_set = args
            .paths
            .iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        let mut seen_changes = std::collections::BTreeSet::new();
        let mut change_bytes = 0usize;
        let mut changes = Vec::new();
        for change in &args.changes {
            if !path_set.contains(change.path.as_str()) {
                return Err(ToolError::InvalidArguments(format!(
                    "change path `{}` is not listed in paths",
                    change.path
                )));
            }
            if !seen_changes.insert(change.path.as_str()) {
                return Err(ToolError::InvalidArguments(format!(
                    "duplicate change path `{}`",
                    change.path
                )));
            }
            change_bytes = change_bytes.saturating_add(change.content.len());
            if change_bytes > MAX_CHANGE_BYTES {
                return Err(ToolError::InvalidArguments(format!(
                    "change payload exceeds {MAX_CHANGE_BYTES} bytes"
                )));
            }
            let full = target_path(&root, &canonical_root, &change.path)?;
            let (before_exists, before_sha256) = target_state(&full)?;
            let after_sha256 = hex::encode(Sha256::digest(change.content.as_bytes()));
            changes.push(json!({
                "path": change.path,
                "content": change.content,
                "before_exists": before_exists,
                "before_sha256": before_sha256,
                "after_sha256": after_sha256,
                "bytes": change.content.len(),
            }));
        }
        let payload_sha256 = if args.changes.is_empty() {
            Value::Null
        } else {
            let payload = args
                .changes
                .iter()
                .map(|change| json!({"path": change.path, "content": change.content}))
                .collect::<Vec<_>>();
            Value::String(hex::encode(Sha256::digest(
                serde_json::to_vec(&payload)
                    .map_err(|error| ToolError::Failed(error.to_string()))?,
            )))
        };
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
            "changes": changes,
            "payload_sha256": payload_sha256.clone(),
            "run_id": run_id,
            "created_unix_ms": created_unix_ms,
        });
        let bytes = serde_json::to_vec_pretty(&proposal)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        if bytes.len() as u64 > MAX_PROPOSAL_BYTES {
            return Err(ToolError::InvalidArguments(format!(
                "proposal exceeds {MAX_PROPOSAL_BYTES} bytes"
            )));
        }
        let sha256 = hex::encode(Sha256::digest(&bytes));
        ensure_control_dir(&root, ".1a/proposals")?;
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
                (
                    "payload_sha256".into(),
                    payload_sha256.as_str().unwrap_or_default().to_string(),
                ),
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
        ensure_control_dir(&root, ".1a/proposals")?;
        let proposal_path = root
            .join(".1a/proposals")
            .join(format!("{}.json", args.proposal_id));
        let (proposal, _proposal_bytes, proposal_sha256) = read_proposal(&proposal_path)?;
        let paths = proposal["paths"]
            .as_array()
            .ok_or_else(|| ToolError::Failed("proposal has no paths array".into()))?;
        let changes = proposal["changes"].as_array().cloned().unwrap_or_default();
        let changes_by_path = changes
            .iter()
            .filter_map(|change| {
                change["path"]
                    .as_str()
                    .map(|path| (path.to_string(), change))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let canonical_root = std::fs::canonicalize(&root).unwrap_or(root.clone());
        let mut files = Vec::new();
        let mut matched_changes = std::collections::BTreeSet::new();
        let mut passed = true;
        for value in paths {
            let raw = value
                .as_str()
                .ok_or_else(|| ToolError::Failed("proposal path is not a string".into()))?;
            validate_relative_path(raw)?;
            if let Some(change) = changes_by_path.get(raw) {
                matched_changes.insert(raw.to_string());
                let content = change["content"].as_str().ok_or_else(|| {
                    ToolError::Failed(format!("change `{raw}` has no string content"))
                })?;
                let before_exists = change["before_exists"].as_bool().unwrap_or(false);
                let before_sha256 = change["before_sha256"].as_str();
                let after_sha256 = change["after_sha256"].as_str().unwrap_or_default();
                let actual_after = hex::encode(Sha256::digest(content.as_bytes()));
                let full = target_path(&root, &canonical_root, raw)?;
                let (current_exists, current_sha256) = target_state(&full)?;
                let current_sha256 = current_sha256.as_str();
                let ok = current_exists == before_exists
                    && current_sha256 == before_sha256
                    && actual_after == after_sha256;
                if !ok {
                    passed = false;
                }
                files.push(json!({
                    "path": raw,
                    "exists": current_exists,
                    "bytes": content.len(),
                    "sha256": after_sha256,
                    "before_exists": before_exists,
                    "before_sha256": before_sha256,
                    "after_sha256": after_sha256,
                    "staged": true,
                    "ok": ok,
                }));
                continue;
            }
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
            if metadata.len() > MAX_TARGET_BYTES {
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
        if matched_changes.len() != changes.len() {
            passed = false;
        }
        let report = json!({
            "schema": "1ao.check-report/v1",
            "proposal_id": args.proposal_id,
            "proposal_sha256": proposal_sha256,
            "proposal_action_version": proposal["action_version"].clone(),
            "payload_sha256": proposal["payload_sha256"].clone(),
            "mode": if changes.is_empty() { "workspace" } else { "staged" },
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
        ensure_control_dir(&root, ".1a/checks")?;
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
                (
                    "payload_sha256".into(),
                    proposal["payload_sha256"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                ),
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
        ensure_control_dir(&root, ".1a/proposals")?;
        ensure_control_dir(&root, ".1a/approvals")?;
        ensure_control_dir(&root, ".1a/checks")?;
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
        if !proposal_version.is_empty() && approval.action_version != proposal_version {
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
        let has_changes = proposal["changes"]
            .as_array()
            .is_some_and(|changes| !changes.is_empty());
        if check_report["status"] != "passed"
            || check_report["proposal_sha256"] != proposal_sha256
            || (has_changes
                && (check_report["mode"] != "staged"
                    || check_report["payload_sha256"] != proposal["payload_sha256"]
                    || check_report["proposal_action_version"] != proposal["action_version"]))
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

pub struct CodeMergeChangeTool {
    action: ActionDefinition,
}

impl CodeMergeChangeTool {
    pub fn new(action: ActionDefinition) -> Self {
        Self { action }
    }
}

struct PendingMerge {
    relative: String,
    full: PathBuf,
    content: Vec<u8>,
    before_exists: bool,
    before_sha256: Option<String>,
    before_bytes: Vec<u8>,
    before_mode: u32,
    after_sha256: String,
    already_applied: bool,
}

#[async_trait]
impl AgentTool for CodeMergeChangeTool {
    fn spec(&self) -> ToolSpec {
        self.action.tool_spec()
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn reversible(&self) -> bool {
        self.action.reversible
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
                "merge_change requires write permission".into(),
            ));
        }
        let args: ProposalIdArgs = serde_json::from_value(arguments)
            .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
        validate_safe_id(&args.proposal_id)?;
        let root = workspace_root(permissions);
        let canonical_root = std::fs::canonicalize(&root).unwrap_or(root.clone());
        ensure_control_dir(&root, ".1a/proposals")?;
        ensure_control_dir(&root, ".1a/approvals")?;
        ensure_control_dir(&root, ".1a/checks")?;
        ensure_control_dir(&root, &format!(".1a/merges/{}", args.proposal_id))?;
        let proposal_path = root
            .join(".1a/proposals")
            .join(format!("{}.json", args.proposal_id));
        let (proposal, _proposal_bytes, proposal_sha256) = read_proposal(&proposal_path)?;
        if proposal["status"] != "proposed" {
            return Err(ToolError::Failed(
                "proposal is not in proposed status".into(),
            ));
        }
        let changes = proposal["changes"]
            .as_array()
            .ok_or_else(|| ToolError::Failed("proposal has no changes array".into()))?;
        if changes.is_empty() {
            return Err(ToolError::Failed(
                "proposal has no staged change payload to merge".into(),
            ));
        }
        if changes.len() > MAX_PATHS {
            return Err(ToolError::Failed("proposal has too many changes".into()));
        }
        let proposal_version = proposal["action_version"].as_str().unwrap_or_default();

        let approval_path = root
            .join(".1a/approvals")
            .join(format!("{}.json", args.proposal_id));
        let approval_bytes = read_json_file(&approval_path, "approval receipt")?;
        let approval: ProposalReceipt = serde_json::from_slice(&approval_bytes)
            .map_err(|error| ToolError::Failed(format!("invalid approval receipt: {error}")))?;
        if approval.status != "approved"
            || approval.decision != "approve"
            || approval.proposal_sha256 != proposal_sha256
            || approval.proposal_id != args.proposal_id
        {
            return Err(ToolError::Failed(
                "approval does not match this proposal".into(),
            ));
        }
        if !proposal_version.is_empty() && approval.action_version != proposal_version {
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
        if check_report["status"] != "passed"
            || check_report["proposal_sha256"] != proposal_sha256
            || check_report["mode"] != "staged"
            || check_report["payload_sha256"] != proposal["payload_sha256"]
        {
            return Err(ToolError::Failed(
                "check report does not match the staged proposal".into(),
            ));
        }
        let check_version = check_report["action_version"].as_str().unwrap_or_default();
        if check_version.is_empty()
            || check_report["proposal_action_version"].as_str() != Some(proposal_version)
        {
            return Err(ToolError::Failed(
                "check report does not bind the proposal version".into(),
            ));
        }

        let merge_dir = root.join(".1a/merges").join(&args.proposal_id);
        ensure_control_dir(&root, &format!(".1a/merges/{}/backup", args.proposal_id))?;
        let receipt_path = merge_dir.join("merge.json");
        if receipt_path.exists() {
            let existing_bytes = read_json_file(&receipt_path, "merge receipt")?;
            let existing: Value = serde_json::from_slice(&existing_bytes).map_err(|error| {
                ToolError::Failed(format!("invalid existing merge receipt: {error}"))
            })?;
            if existing["schema"] != "1ao.merge-receipt/v1"
                || existing["proposal_sha256"] != proposal_sha256
                || existing["status"] != "merged"
                || existing["payload_sha256"] != proposal["payload_sha256"]
                || existing["approval_sha256"] != hex::encode(Sha256::digest(&approval_bytes))
                || existing["check_report_sha256"] != hex::encode(Sha256::digest(&check_bytes))
            {
                return Err(ToolError::Failed(
                    "a different or unbound merge receipt already exists for this proposal".into(),
                ));
            }
            let receipt_files = existing["files"]
                .as_array()
                .ok_or_else(|| ToolError::Failed("merge receipt has no files".into()))?
                .iter()
                .filter_map(|file| {
                    Some((
                        file["path"].as_str()?.to_string(),
                        file["after_sha256"].as_str()?.to_string(),
                    ))
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            for change in changes {
                let relative = change["path"]
                    .as_str()
                    .ok_or_else(|| ToolError::Failed("change path is missing".into()))?;
                let expected = change["after_sha256"]
                    .as_str()
                    .ok_or_else(|| ToolError::Failed("change after_sha256 is missing".into()))?;
                let full = target_path(&root, &canonical_root, relative)?;
                let (exists, sha256) = target_state(&full)?;
                if !exists
                    || sha256.as_str() != Some(expected)
                    || receipt_files.get(relative).map(String::as_str) != Some(expected)
                {
                    return Err(ToolError::Failed(format!(
                        "merge receipt exists but target `{relative}` is not at the recorded after hash"
                    )));
                }
            }
            let sha256 = hex::encode(Sha256::digest(&existing_bytes));
            return Ok(merge_outcome(
                self.action.risk.clone(),
                &args.proposal_id,
                &receipt_path,
                &sha256,
                &proposal_sha256,
            ));
        }

        let checked = check_report["files"]
            .as_array()
            .ok_or_else(|| ToolError::Failed("check report has no files".into()))?
            .iter()
            .filter_map(|file| {
                let path = file["path"].as_str()?;
                (file["staged"] == true).then(|| (path.to_string(), file.clone()))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut pending = Vec::with_capacity(changes.len());
        let mut seen = std::collections::BTreeSet::new();
        for change in changes {
            let relative = change["path"]
                .as_str()
                .ok_or_else(|| ToolError::Failed("change path is missing".into()))?
                .to_string();
            if !seen.insert(relative.clone()) {
                return Err(ToolError::Failed("duplicate change path".into()));
            }
            let content = change["content"]
                .as_str()
                .ok_or_else(|| ToolError::Failed("change content is missing".into()))?
                .as_bytes()
                .to_vec();
            let after_sha256 = hex::encode(Sha256::digest(&content));
            if change["after_sha256"].as_str() != Some(after_sha256.as_str())
                || checked
                    .get(&relative)
                    .and_then(|file| file["after_sha256"].as_str())
                    != Some(after_sha256.as_str())
            {
                return Err(ToolError::Failed(format!(
                    "staged payload for `{relative}` does not match the check report"
                )));
            }
            let before_exists = change["before_exists"].as_bool().unwrap_or(false);
            let before_sha256 = change["before_sha256"].as_str().map(str::to_string);
            let full = target_path(&root, &canonical_root, &relative)?;
            let (current_exists, current_sha256) = target_state(&full)?;
            let matches_before = current_exists == before_exists
                && current_sha256.as_str() == before_sha256.as_deref();
            let matches_after =
                current_exists && current_sha256.as_str() == Some(after_sha256.as_str());
            if !matches_before && !matches_after {
                return Err(ToolError::Failed(format!(
                    "target `{relative}` drifted after the proposal"
                )));
            }
            let already_applied = matches_after && !matches_before;
            let (before_bytes, before_mode) = if before_exists {
                let bytes = std::fs::read(&full).map_err(|error| {
                    ToolError::Failed(format!("cannot read target `{relative}`: {error}"))
                })?;
                let mode = target_mode(&full)?;
                (bytes, mode)
            } else {
                (Vec::new(), 0o644)
            };
            pending.push(PendingMerge {
                relative,
                full,
                content,
                before_exists,
                before_sha256,
                before_bytes,
                before_mode,
                after_sha256,
                already_applied,
            });
        }
        let already_count = pending.iter().filter(|file| file.already_applied).count();
        if already_count == pending.len() {
            write_merge_receipt(
                &receipt_path,
                &args.proposal_id,
                &proposal_sha256,
                &proposal,
                &approval_bytes,
                &check_bytes,
                &pending,
            )
            .map_err(ToolError::Failed)?;
            let receipt_bytes = read_json_file(&receipt_path, "merge receipt")?;
            let sha256 = hex::encode(Sha256::digest(&receipt_bytes));
            return Ok(merge_outcome(
                self.action.risk.clone(),
                &args.proposal_id,
                &receipt_path,
                &sha256,
                &proposal_sha256,
            ));
        } else if already_count > 0 {
            return Err(ToolError::Failed(format!(
                "partial merge detected for {}; backups are under {} and manual recovery is required",
                args.proposal_id,
                merge_dir.join("backup").display()
            )));
        }

        let backup_dir = merge_dir.join("backup");
        for file in &pending {
            if !file.before_exists {
                continue;
            }
            let backup_path = backup_dir.join(&file.relative);
            crate::jsonl::write_private_atomic(&backup_path, &file.before_bytes, "merge-backup")
                .map_err(|error| {
                    ToolError::Failed(format!(
                        "cannot write backup for `{}`: {error}",
                        file.relative
                    ))
                })?;
        }

        let mut applied = 0usize;
        let result = (|| -> Result<(), String> {
            for file in &pending {
                let (exists, sha256) = target_state(&file.full)
                    .map_err(|error| format!("cannot recheck `{}`: {error}", file.relative))?;
                if exists != file.before_exists || sha256.as_str() != file.before_sha256.as_deref()
                {
                    return Err(format!("target `{}` changed during merge", file.relative));
                }
                let mode = if file.before_exists {
                    file.before_mode
                } else {
                    0o644
                };
                replace_file_atomic(&file.full, &file.content, mode, &canonical_root)
                    .map_err(|error| format!("cannot write target `{}`: {error}", file.relative))?;
                applied += 1;
                let (exists, sha256) = target_state(&file.full)
                    .map_err(|error| format!("cannot verify `{}`: {error}", file.relative))?;
                if !exists || sha256.as_str() != Some(file.after_sha256.as_str()) {
                    return Err(format!(
                        "post-write verification failed for `{}`",
                        file.relative
                    ));
                }
            }
            write_merge_receipt(
                &receipt_path,
                &args.proposal_id,
                &proposal_sha256,
                &proposal,
                &approval_bytes,
                &check_bytes,
                &pending,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            let rollback_errors = rollback_merge(&pending[..applied], &canonical_root);
            if rollback_errors.is_empty() {
                return Err(ToolError::Failed(format!(
                    "merge failed and was rolled back: {error}"
                )));
            }
            return Err(ToolError::Failed(format!(
                "merge failed; rollback incomplete: {error}; {}",
                rollback_errors.join("; ")
            )));
        }
        let receipt_bytes = read_json_file(&receipt_path, "merge receipt")?;
        let sha256 = hex::encode(Sha256::digest(&receipt_bytes));
        Ok(merge_outcome(
            self.action.risk.clone(),
            &args.proposal_id,
            &receipt_path,
            &sha256,
            &proposal_sha256,
        ))
    }
}

fn write_merge_receipt(
    path: &Path,
    proposal_id: &str,
    proposal_sha256: &str,
    proposal: &Value,
    approval_bytes: &[u8],
    check_bytes: &[u8],
    pending: &[PendingMerge],
) -> Result<Vec<u8>, String> {
    let proposal_version = proposal["action_version"].as_str().unwrap_or_default();
    let receipt = json!({
        "schema": "1ao.merge-receipt/v1",
        "proposal_id": proposal_id,
        "status": "merged",
        "proposal_sha256": proposal_sha256,
        "payload_sha256": proposal["payload_sha256"].clone(),
        "approval_sha256": hex::encode(Sha256::digest(approval_bytes)),
        "check_report_sha256": hex::encode(Sha256::digest(check_bytes)),
        "action_version": proposal_version,
        "files": pending
            .iter()
            .map(|file| json!({
                "path": file.relative,
                "before_exists": file.before_exists,
                "before_sha256": file.before_sha256,
                "after_sha256": file.after_sha256,
            }))
            .collect::<Vec<_>>(),
        "merged_unix_ms": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    });
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("cannot serialize merge receipt: {error}"))?;
    if let Err(error) = crate::jsonl::write_private_atomic(path, &bytes, "merge.json") {
        // A rename can commit before a parent fsync fails. Treat an existing
        // matching receipt as committed; otherwise the caller must roll back.
        if let Ok(existing_bytes) = std::fs::read(path)
            && let Ok(existing) = serde_json::from_slice::<Value>(&existing_bytes)
            && existing["proposal_sha256"] == proposal_sha256
            && existing["status"] == "merged"
        {
            return Ok(existing_bytes);
        }
        return Err(format!("cannot write merge receipt: {error}"));
    }
    Ok(bytes)
}

fn rollback_merge(applied: &[PendingMerge], canonical_root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    for file in applied.iter().rev() {
        if file.before_exists {
            if let Err(error) = replace_file_atomic(
                &file.full,
                &file.before_bytes,
                file.before_mode,
                canonical_root,
            ) {
                errors.push(format!("cannot restore `{}`: {error}", file.relative));
            }
        } else if let Err(error) = std::fs::remove_file(&file.full)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            errors.push(format!("cannot remove `{}`: {error}", file.relative));
        }
    }
    errors
}

fn target_mode(path: &Path) -> Result<u32, ToolError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path).map_err(|error| {
            ToolError::Failed(format!("cannot stat {}: {error}", path.display()))
        })?;
        Ok(metadata.permissions().mode() & 0o777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0o644)
    }
}

fn replace_file_atomic(
    path: &Path,
    bytes: &[u8],
    mode: u32,
    canonical_root: &Path,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "target has no parent directory".to_string())?;
    let canonical_parent =
        std::fs::canonicalize(parent).map_err(|error| format!("cannot resolve parent: {error}"))?;
    if !canonical_parent.starts_with(canonical_root) {
        return Err("target parent escapes the workspace".into());
    }
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("target became a symlink".into());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "target has no file name".to_string())?;
    let temp = parent.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let result = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(mode))?;
        }
        #[cfg(not(unix))]
        {
            let _ = mode;
        }
        std::fs::rename(&temp, path)?;
        // The rename is committed. A failed directory fsync weakens durability
        // but must not be reported as "not applied" or trigger a rollback.
        let _ = std::fs::File::open(parent).and_then(|dir| dir.sync_all());
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|error| error.to_string())
}

fn merge_outcome(
    risk: String,
    proposal_id: &str,
    path: &Path,
    sha256: &str,
    proposal_sha256: &str,
) -> ToolOutcome {
    ToolOutcome {
        summary: format!("merged approved change {proposal_id}"),
        content: json!({
            "proposal_id": proposal_id,
            "status": "merged",
            "path": path.display().to_string(),
            "sha256": sha256,
        })
        .to_string(),
        metadata: std::collections::BTreeMap::from([
            ("action_type".into(), "code.merge_change".into()),
            ("proposal_id".into(), proposal_id.to_string()),
            ("path".into(), path.display().to_string()),
            ("sha256".into(), sha256.to_string()),
            ("status".into(), "merged".into()),
            ("risk".into(), risk),
            ("reversible".into(), "true".into()),
            ("proposal_sha256".into(), proposal_sha256.to_string()),
        ]),
    }
}

/// Create or verify a private control-plane directory without following
/// symlinked components. Used only for the `.1a` ledger.
fn ensure_control_dir(root: &Path, relative: &str) -> Result<(), ToolError> {
    let canonical_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut current = root.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(ToolError::Failed("invalid control directory".into()));
        }
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(ToolError::Failed(format!(
                        "control path {} must be a real directory",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(|error| {
                    ToolError::Failed(format!(
                        "cannot create control directory {}: {error}",
                        current.display()
                    ))
                })?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&current, std::fs::Permissions::from_mode(0o700))
                        .map_err(|error| {
                            ToolError::Failed(format!(
                                "cannot secure control directory {}: {error}",
                                current.display()
                            ))
                        })?;
                }
            }
            Err(error) => {
                return Err(ToolError::Failed(format!(
                    "cannot stat control directory {}: {error}",
                    current.display()
                )));
            }
        }
    }
    let canonical = std::fs::canonicalize(&current)
        .map_err(|error| ToolError::Failed(format!("cannot resolve control path: {error}")))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(ToolError::Failed(
            "control directory escapes the workspace".into(),
        ));
    }
    Ok(())
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

/// Resolve an existing or new target file. Existing targets must be regular
/// files inside the canonical workspace; new targets need an existing parent
/// directory inside it. Symlinks are rejected instead of followed.
fn target_path(root: &Path, canonical_root: &Path, relative: &str) -> Result<PathBuf, ToolError> {
    validate_relative_path(relative)?;
    let full = root.join(relative);
    match std::fs::symlink_metadata(&full) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ToolError::InvalidArguments(format!(
                    "target `{relative}` must be a regular file, not a symlink or directory"
                )));
            }
            let canonical = std::fs::canonicalize(&full).map_err(|error| {
                ToolError::Failed(format!("cannot resolve target `{relative}`: {error}"))
            })?;
            if !canonical.starts_with(canonical_root) {
                return Err(ToolError::InvalidArguments(format!(
                    "target `{relative}` escapes the workspace"
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = full
                .parent()
                .ok_or_else(|| ToolError::InvalidArguments("target has no parent".into()))?;
            let canonical_parent = std::fs::canonicalize(parent).map_err(|_| {
                ToolError::InvalidArguments(format!(
                    "parent directory for `{relative}` must already exist"
                ))
            })?;
            if !canonical_parent.starts_with(canonical_root) || !canonical_parent.is_dir() {
                return Err(ToolError::InvalidArguments(format!(
                    "parent directory for `{relative}` escapes the workspace"
                )));
            }
        }
        Err(error) => {
            return Err(ToolError::Failed(format!(
                "cannot stat target `{relative}`: {error}"
            )));
        }
    }
    Ok(full)
}

fn target_state(full: &Path) -> Result<(bool, Value), ToolError> {
    let metadata = match std::fs::symlink_metadata(full) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((false, Value::Null));
        }
        Err(error) => {
            return Err(ToolError::Failed(format!(
                "cannot stat {}: {error}",
                full.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ToolError::InvalidArguments(format!(
            "target {} must be a regular file",
            full.display()
        )));
    }
    if metadata.len() > MAX_TARGET_BYTES {
        return Err(ToolError::InvalidArguments(format!(
            "target {} exceeds {MAX_TARGET_BYTES} bytes",
            full.display()
        )));
    }
    let bytes = std::fs::read(full).map_err(|error| {
        ToolError::Failed(format!("cannot read target {}: {error}", full.display()))
    })?;
    Ok((true, Value::String(hex::encode(Sha256::digest(&bytes)))))
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
    if path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_str().is_some_and(|name| matches!(name, ".1a" | ".git")))
    }) {
        return Err(ToolError::InvalidArguments(format!(
            "path `{value}` must not target .1a or .git control directories"
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
        for path in [".1a/proposals/forged.json", "src/.git/config"] {
            let error = tool
                .execute(
                    json!({"summary": "x", "paths": [path]}),
                    &permissions(dir.path()),
                    &tokio_util::sync::CancellationToken::new(),
                )
                .await
                .unwrap_err();
            assert!(error.to_string().contains("control directories"));
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
    async fn staged_change_merges_after_approval_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "old").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "staged change",
                    "paths": ["src/lib.rs", "src/new.rs"],
                    "changes": [
                        {"path": "src/lib.rs", "content": "new"},
                        {"path": "src/new.rs", "content": "created"}
                    ]
                }),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        let proposal_path = PathBuf::from(&proposal.metadata["path"]);
        let proposal_json: Value =
            serde_json::from_slice(&std::fs::read(&proposal_path).unwrap()).unwrap();
        assert_eq!(proposal_json["changes"][0]["before_exists"], true);
        assert_eq!(proposal_json["changes"][1]["before_exists"], false);
        assert_eq!(proposal_json["payload_sha256"].as_str().unwrap().len(), 64);

        let checks = CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(checks.metadata["status"], "passed");
        let check_report: Value = serde_json::from_slice(
            &std::fs::read(
                dir.path()
                    .join(".1a/checks")
                    .join(format!("{proposal_id}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(check_report["mode"], "staged");
        assert_eq!(
            check_report["payload_sha256"],
            proposal_json["payload_sha256"]
        );

        let merge = CodeMergeChangeTool::new(action_by_id("code.merge_change"));
        let missing = merge
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("approval"));

        decide_proposal(&proposal_path, "approve", "alice", Some("merge it")).unwrap();
        let merged = merge
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(merged.metadata["status"], "merged");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap(),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/new.rs")).unwrap(),
            "created"
        );
        let receipt: Value =
            serde_json::from_slice(&std::fs::read(&merged.metadata["path"]).unwrap()).unwrap();
        assert_eq!(receipt["status"], "merged");
        assert_eq!(
            receipt["proposal_sha256"].as_str(),
            Some(proposal.metadata["sha256"].as_str())
        );
        assert_eq!(receipt["files"][1]["before_exists"], false);
        assert!(
            dir.path()
                .join(".1a/merges")
                .join(&proposal_id)
                .join("backup/src/lib.rs")
                .exists()
        );

        let again = merge
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(again.metadata["sha256"], merged.metadata["sha256"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap(),
            "new"
        );
    }

    #[tokio::test]
    async fn merge_rejects_a_target_that_drifted_after_checks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "old").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "drift",
                    "paths": ["src/lib.rs"],
                    "changes": [{"path": "src/lib.rs", "content": "new"}]
                }),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(
            &PathBuf::from(&proposal.metadata["path"]),
            "approve",
            "alice",
            None,
        )
        .unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "drifted").unwrap();
        let error = CodeMergeChangeTool::new(action_by_id("code.merge_change"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("drifted"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap(),
            "drifted"
        );
    }

    #[tokio::test]
    async fn merge_completes_a_crash_after_all_writes_before_the_receipt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "old").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "crash recovery",
                    "paths": ["src/lib.rs"],
                    "changes": [{"path": "src/lib.rs", "content": "new"}]
                }),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(
            &PathBuf::from(&proposal.metadata["path"]),
            "approve",
            "alice",
            None,
        )
        .unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "new").unwrap();
        assert!(
            !dir.path()
                .join(".1a/merges")
                .join(&proposal_id)
                .join("merge.json")
                .exists()
        );
        let merged = CodeMergeChangeTool::new(action_by_id("code.merge_change"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(merged.metadata["status"], "merged");
        assert!(
            dir.path()
                .join(".1a/merges")
                .join(&proposal_id)
                .join("merge.json")
                .exists()
        );
    }

    #[tokio::test]
    async fn merge_fails_closed_on_a_partial_crash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/a.rs"), "old-a").unwrap();
        std::fs::write(dir.path().join("src/b.rs"), "old-b").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "partial",
                    "paths": ["src/a.rs", "src/b.rs"],
                    "changes": [
                        {"path": "src/a.rs", "content": "new-a"},
                        {"path": "src/b.rs", "content": "new-b"}
                    ]
                }),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(
            &PathBuf::from(&proposal.metadata["path"]),
            "approve",
            "alice",
            None,
        )
        .unwrap();
        std::fs::write(dir.path().join("src/a.rs"), "new-a").unwrap();
        let error = CodeMergeChangeTool::new(action_by_id("code.merge_change"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("partial merge"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/b.rs")).unwrap(),
            "old-b"
        );
    }

    #[tokio::test]
    async fn merge_receipt_replay_rejects_diverged_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "old").unwrap();
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "replay",
                    "paths": ["src/lib.rs"],
                    "changes": [{"path": "src/lib.rs", "content": "new"}]
                }),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(
            &PathBuf::from(&proposal.metadata["path"]),
            "approve",
            "alice",
            None,
        )
        .unwrap();
        let merge = CodeMergeChangeTool::new(action_by_id("code.merge_change"));
        merge
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "tampered").unwrap();
        let error = merge
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not at the recorded after hash"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/lib.rs")).unwrap(),
            "tampered"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn propose_rejects_a_symlinked_control_directory() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), dir.path().join(".1a")).unwrap();
        let error = CodeProposeChangeTool::new(action())
            .execute(
                json!({"summary": "symlinked ledger", "paths": ["src/lib.rs"]}),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("real directory"));
    }

    #[tokio::test]
    async fn propose_rejects_change_paths_not_in_the_path_list() {
        let dir = tempfile::tempdir().unwrap();
        let error = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "bad",
                    "paths": ["src/lib.rs"],
                    "changes": [{"path": "src/other.rs", "content": "x"}]
                }),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not listed"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn propose_rejects_symlinked_change_targets_and_missing_parents() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.txt");
        std::fs::write(&victim, "outside").unwrap();
        symlink(&victim, dir.path().join("link.txt")).unwrap();
        let error = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "symlink",
                    "paths": ["link.txt"],
                    "changes": [{"path": "link.txt", "content": "x"}]
                }),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("symlink"));

        let error = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "new file",
                    "paths": ["missing/file.txt"],
                    "changes": [{"path": "missing/file.txt", "content": "x"}]
                }),
                &permissions(dir.path()),
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("must already exist"));
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
        let merge = CodeMergeChangeTool::new(action_by_id("code.merge_change"))
            .execute(
                json!({"proposal_id": "p1"}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(merge, ToolError::PermissionDenied(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn merge_rolls_back_earlier_files_when_a_later_write_fails() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        for sub in ["a", "b"] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::fs::write(dir.path().join(sub).join("file.txt"), "old").unwrap();
        }
        let permissions = permissions(dir.path());
        let proposal = CodeProposeChangeTool::new(action())
            .execute(
                json!({
                    "summary": "rollback",
                    "paths": ["a/file.txt", "b/file.txt"],
                    "changes": [
                        {"path": "a/file.txt", "content": "new-a"},
                        {"path": "b/file.txt", "content": "new-b"}
                    ]
                }),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        let proposal_id = proposal.metadata["proposal_id"].clone();
        CodeRunChecksTool::new(action_by_id("code.run_checks"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap();
        decide_proposal(
            &PathBuf::from(&proposal.metadata["path"]),
            "approve",
            "alice",
            None,
        )
        .unwrap();
        let blocked = dir.path().join("b");
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o500)).unwrap();
        let error = CodeMergeChangeTool::new(action_by_id("code.merge_change"))
            .execute(
                json!({"proposal_id": proposal_id}),
                &permissions,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .unwrap_err();
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(error.to_string().contains("rolled back"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a/file.txt")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b/file.txt")).unwrap(),
            "old"
        );
        assert!(
            !dir.path()
                .join(".1a/merges")
                .join(&proposal_id)
                .join("merge.json")
                .exists()
        );
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
