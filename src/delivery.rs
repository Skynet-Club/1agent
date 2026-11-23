//! Post-run delivery acceptance and a durable pending queue.
//!
//! A run can enqueue its journal for acceptance instead of blocking on the
//! model-side acceptance tool. The queue is a directory of JSON files, so it
//! survives process exits and can be drained by `1agent accept --queue`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::{Claim, ClaimStatus, ToolStatus};
use crate::journal::{JournalEventKind, read_events};

pub const RECEIPT_SCHEMA: &str = "1agent.delivery-receipt/v1";
pub const QUEUE_SCHEMA: &str = "1agent.delivery-queue/v1";

#[derive(Debug, Error)]
pub enum DeliveryError {
    #[error("cannot read journal: {0}")]
    Journal(#[from] crate::journal::JournalReadError),
    #[error("cannot read or write acceptance state: {0}")]
    Io(#[from] std::io::Error),
    #[error("cannot serialize acceptance state: {0}")]
    Json(#[from] serde_json::Error),
    #[error("run {0} was already accepted")]
    AlreadyAccepted(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Accepted,
    Conditional,
    Rejected,
}

impl DeliveryStatus {
    fn from_decision(decision: &str) -> Self {
        match decision {
            "accepted" => Self::Accepted,
            "conditional" => Self::Conditional,
            _ => Self::Rejected,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryReceipt {
    pub schema: String,
    pub run_id: String,
    pub journal: String,
    pub objective: Option<String>,
    pub status: DeliveryStatus,
    pub checks: Vec<Value>,
    pub artifacts: Vec<Value>,
    pub workspace_snapshot: Value,
    pub answer: Value,
    pub manifest_sha256: String,
    pub created_unix_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryItem {
    pub schema: String,
    pub run_id: String,
    pub journal: String,
    pub workspace: String,
    pub enqueued_unix_ms: u128,
    pub status: DeliveryStatus,
}

#[derive(Debug, Default)]
pub struct DeliveryBatch {
    pub receipts: Vec<DeliveryReceipt>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
struct ArtifactRecord {
    path: String,
    sha256: String,
    tool: String,
}

pub fn build_receipt(journal_path: &Path) -> Result<DeliveryReceipt, DeliveryError> {
    let events = read_events(journal_path)?;
    let run_id = events
        .first()
        .map(|event| event.run_id.to_string())
        .or_else(|| {
            journal_path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "unknown".into());
    let mut objective = None;
    let mut calls = Vec::new();
    let mut observations = Vec::new();
    let mut artifacts: BTreeMap<String, ArtifactRecord> = BTreeMap::new();
    let mut verified: BTreeMap<String, String> = BTreeMap::new();
    let mut reads: BTreeMap<String, String> = BTreeMap::new();
    let mut terminal_write_possible = false;
    let mut unrecorded_write = false;
    let mut snapshot = None;
    let mut answer = None;

    for event in &events {
        match &event.event {
            JournalEventKind::RunStarted { request, .. } => {
                objective = Some(request.objective.clone());
            }
            JournalEventKind::ToolCalled { call } => calls.push(call.id.clone()),
            JournalEventKind::ToolObserved { observation } => {
                observations.push(observation.call_id.clone());
                if observation.status != ToolStatus::Ok {
                    continue;
                }
                let metadata = &observation.metadata;
                if observation.tool == "write_file" || observation.tool == "edit_file" {
                    let path = metadata.get("path");
                    let digest = metadata
                        .get("sha256")
                        .or_else(|| metadata.get("sha256_after"));
                    match (path, digest) {
                        (Some(path), Some(digest)) if !path.is_empty() && !digest.is_empty() => {
                            artifacts.insert(
                                path.clone(),
                                ArtifactRecord {
                                    path: path.clone(),
                                    sha256: digest.clone(),
                                    tool: observation.tool.clone(),
                                },
                            );
                        }
                        _ => unrecorded_write = true,
                    }
                } else if observation.tool == "terminal_exec"
                    && metadata.get("write").map(String::as_str) != Some("false")
                {
                    terminal_write_possible = true;
                } else if observation.tool == "read_file"
                    && let Some(digest) = metadata.get("sha256")
                    && let Some(path) = metadata
                        .get("path")
                        .or_else(|| metadata.get("relative_path"))
                {
                    reads.insert(path.clone(), digest.clone());
                }
                if observation.tool == "verify_artifact"
                    && metadata.get("decision").map(String::as_str) == Some("verified")
                    && let (Some(path), Some(digest)) =
                        (metadata.get("path"), metadata.get("sha256"))
                {
                    verified.insert(path.clone(), digest.clone());
                }
            }
            JournalEventKind::DeliverySnapshot { snapshot: delivery } => {
                snapshot = Some(delivery.clone());
            }
            JournalEventKind::AnswerProduced { answer: produced } => {
                answer = Some(produced.clone());
            }
            _ => {}
        }
    }

    let conclusion = answer
        .as_ref()
        .map(|answer| answer.conclusion.as_str())
        .unwrap_or("");
    let derived_claims = derive_claims(&artifacts, &verified, &reads, conclusion);
    let (workspace_snapshot, snapshot_ok, snapshot_stale, snapshot_incomplete) =
        verify_snapshot(snapshot.as_ref());
    let unpinned_write = artifacts.is_empty() && terminal_write_possible && !snapshot_ok;

    let mut checks = Vec::new();
    checks.push(json!({
        "id": "journal:run_started",
        "passed": objective.as_ref().is_some_and(|value| !value.trim().is_empty()),
        "detail": {"objective": objective},
    }));
    checks.push(pairing_check(&calls, &observations));

    let mut artifact_rows = Vec::new();
    for artifact in artifacts.values() {
        let path = Path::new(&artifact.path);
        let current = sha256_file(path);
        let exists = path.is_file();
        let matched = current.as_deref() == Some(artifact.sha256.as_str());
        artifact_rows.push(json!({
            "path": artifact.path,
            "recorded_sha256": artifact.sha256,
            "current_sha256": current,
            "exists": exists,
            "match": matched,
            "tool": artifact.tool,
        }));
        checks.push(json!({
            "id": format!("artifact:{}", artifact.path),
            "passed": exists && matched,
            "detail": {
                "path": artifact.path,
                "recorded_sha256": artifact.sha256,
                "current_sha256": current,
                "exists": exists,
                "match": matched,
            },
        }));
    }
    if unrecorded_write {
        checks.push(json!({
            "id": "journal:recorded_write_artifact",
            "passed": false,
            "detail": "a write/edit observation has no recorded path/sha256",
        }));
    }
    if snapshot.as_ref().is_some() {
        checks.push(json!({
            "id": "workspace:delivery_snapshot",
            "passed": snapshot_ok,
            "detail": {
                "truncated": workspace_snapshot["truncated"],
                "files": workspace_snapshot["files"],
                "mismatches": workspace_snapshot["mismatch_count"],
            },
        }));
    }
    if unpinned_write {
        checks.push(json!({
            "id": "journal:terminal_write_unpinned",
            "passed": false,
            "detail": "a terminal command may have written, but no write/edit artifact or snapshot was recorded",
        }));
    }

    let mut answer_value = json!({
        "status": null,
        "claims": 0,
        "derived_claims": derived_claims.len(),
        "evidence_source": "none",
        "sources": [],
    });
    let mut answer_ok = false;
    if let Some(answer) = &answer {
        let model_verified = answer
            .claims
            .iter()
            .filter(|claim| claim.status == ClaimStatus::Verified)
            .collect::<Vec<_>>();
        let effective_claims: Vec<Claim> = if model_verified.is_empty() {
            derived_claims.clone()
        } else {
            let mut claims = answer.claims.clone();
            for derived in &derived_claims {
                if !claims.iter().any(|claim| {
                    claim
                        .sources
                        .iter()
                        .any(|source| derived.sources.contains(source))
                }) {
                    claims.push(derived.clone());
                }
            }
            claims
        };
        let effective_verified = effective_claims
            .iter()
            .filter(|claim| claim.status == ClaimStatus::Verified)
            .count();
        let evidence_source = if !model_verified.is_empty() {
            "model"
        } else if !derived_claims.is_empty() {
            "journal"
        } else {
            "none"
        };
        let mut sources = effective_claims
            .iter()
            .flat_map(|claim| claim.sources.iter().cloned())
            .chain(
                answer
                    .evidence
                    .iter()
                    .map(|evidence| evidence.source.clone()),
            )
            .filter(|source| !source.is_empty())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        sources.sort();
        let missing_sources = sources
            .iter()
            .filter(|source| source.starts_with('/') && !Path::new(source).exists())
            .cloned()
            .collect::<Vec<_>>();
        let present = !answer.conclusion.trim().is_empty();
        let status_ready = matches!(
            answer.status,
            crate::domain::AnswerStatus::Completed | crate::domain::AnswerStatus::Conditional
        );
        checks.push(json!({
            "id": "answer:present",
            "passed": present,
            "detail": {"status": answer.status, "conclusion_chars": answer.conclusion.chars().count()},
        }));
        checks.push(json!({
            "id": "answer:status_ready",
            "passed": status_ready,
            "detail": {"status": answer.status},
        }));
        checks.push(json!({
            "id": "answer:claims",
            "passed": effective_verified > 0,
            "detail": {
                "claims": answer.claims.len(),
                "verified": model_verified.len(),
                "derived": derived_claims.len(),
                "evidence_source": evidence_source,
            },
        }));
        checks.push(json!({
            "id": "answer:sources_exist",
            "passed": missing_sources.is_empty(),
            "detail": {"sources": sources, "missing": missing_sources},
        }));
        answer_ok = present && status_ready && effective_verified > 0 && missing_sources.is_empty();
        answer_value = json!({
            "status": answer.status,
            "claims": answer.claims.len(),
            "derived_claims": derived_claims.len(),
            "evidence_source": evidence_source,
            "sources": sources,
        });
    } else {
        checks.push(json!({
            "id": "answer:present",
            "passed": false,
            "detail": "no answer_produced event",
        }));
    }

    let proof_ok = checks.iter().all(|check| {
        let id = check.get("id").and_then(Value::as_str).unwrap_or_default();
        if matches!(
            id,
            "answer:claims"
                | "answer:sources_exist"
                | "answer:status_ready"
                | "journal:terminal_write_unpinned"
                | "workspace:delivery_snapshot"
        ) {
            return true;
        }
        check
            .get("passed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    });
    let decision = if snapshot_stale || !proof_ok {
        "rejected"
    } else if unpinned_write || snapshot_incomplete || !answer_ok {
        "conditional"
    } else {
        "accepted"
    };
    let status = DeliveryStatus::from_decision(decision);
    let created_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let canonical = json!({
        "run_id": run_id,
        "objective": objective,
        "status": status,
        "checks": checks,
        "artifacts": artifact_rows,
        "workspace_snapshot": workspace_snapshot,
        "answer": answer_value,
    });
    let manifest_sha256 = hex::encode(Sha256::digest(
        serde_json::to_vec(&canonical).unwrap_or_default(),
    ));
    Ok(DeliveryReceipt {
        schema: RECEIPT_SCHEMA.into(),
        run_id,
        journal: journal_path.to_string_lossy().into_owned(),
        objective,
        status,
        checks,
        artifacts: artifact_rows,
        workspace_snapshot,
        answer: answer_value,
        manifest_sha256,
        created_unix_ms,
    })
}

fn pairing_check(calls: &[String], observations: &[String]) -> Value {
    let call_set = calls.iter().cloned().collect::<BTreeSet<_>>();
    let observation_set = observations.iter().cloned().collect::<BTreeSet<_>>();
    let missing = call_set
        .difference(&observation_set)
        .cloned()
        .collect::<Vec<_>>();
    let extra = observation_set
        .difference(&call_set)
        .cloned()
        .collect::<Vec<_>>();
    let mut duplicates = Vec::new();
    for call in call_set {
        if calls.iter().filter(|candidate| **candidate == call).count() > 1 {
            duplicates.push(call);
        }
    }
    json!({
        "id": "journal:tool_calls_paired",
        "passed": missing.is_empty() && extra.is_empty() && duplicates.is_empty(),
        "detail": {
            "calls": calls.len(),
            "observations": observations.len(),
            "missing_observations": missing,
            "extra_observations": extra,
            "duplicate_calls": duplicates,
        },
    })
}

fn derive_claims(
    artifacts: &BTreeMap<String, ArtifactRecord>,
    verified: &BTreeMap<String, String>,
    reads: &BTreeMap<String, String>,
    conclusion: &str,
) -> Vec<Claim> {
    let mut claims = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, artifact) in artifacts.values().enumerate() {
        seen.insert(artifact.path.clone());
        claims.push(Claim {
            id: format!("derived-artifact-{}", index + 1),
            text: format!(
                "recorded write: {} (sha256 {})",
                artifact.path, artifact.sha256
            ),
            status: ClaimStatus::Verified,
            sources: vec![artifact.path.clone()],
            scope: None,
        });
    }
    for (index, (path, digest)) in verified.iter().enumerate() {
        if seen.contains(path) {
            continue;
        }
        claims.push(Claim {
            id: format!("derived-verified-{}", index + 1),
            text: format!("verified artifact: {path} (sha256 {digest})"),
            status: ClaimStatus::Verified,
            sources: vec![path.clone()],
            scope: None,
        });
    }
    let text = conclusion.to_lowercase();
    for (index, (path, digest)) in reads.iter().enumerate() {
        let name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(path)
            .to_lowercase();
        if !text.contains(&name) {
            continue;
        }
        claims.push(Claim {
            id: format!("derived-read-{}", index + 1),
            text: format!("read source: {path} (sha256 {digest})"),
            status: ClaimStatus::Verified,
            sources: vec![path.clone()],
            scope: None,
        });
    }
    claims
}

fn verify_snapshot(
    snapshot: Option<&crate::workspace_snapshot::WorkspaceSnapshot>,
) -> (Value, bool, bool, bool) {
    let Some(snapshot) = snapshot else {
        return (
            json!({"present": false, "truncated": false, "files": 0, "mismatch_count": 0}),
            false,
            false,
            false,
        );
    };
    let mut mismatches = Vec::new();
    for file in &snapshot.files {
        let path = Path::new(&file.path);
        let current = sha256_file(path);
        if current.as_deref() != Some(file.sha256.as_str()) || !path.is_file() {
            mismatches.push(json!({
                "path": file.path,
                "recorded_sha256": file.sha256,
                "current_sha256": current,
            }));
        }
    }
    let ok = !snapshot.truncated && mismatches.is_empty();
    let stale = !snapshot.truncated && !mismatches.is_empty();
    let incomplete = snapshot.truncated;
    (
        json!({
            "present": true,
            "truncated": snapshot.truncated,
            "files": snapshot.files.len(),
            "mismatch_count": mismatches.len(),
            "mismatches": mismatches,
        }),
        ok,
        stale,
        incomplete,
    )
}

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(hex::encode(Sha256::digest(bytes)))
}

pub struct DeliveryQueue {
    root: PathBuf,
}

impl Default for DeliveryQueue {
    fn default() -> Self {
        Self {
            root: crate::runtime_home::default_home().join("acceptance"),
        }
    }
}

impl DeliveryQueue {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn queue_dir(&self) -> PathBuf {
        self.root.join("queue")
    }

    pub fn done_dir(&self) -> PathBuf {
        self.root.join("done")
    }

    fn ledger_path(&self) -> PathBuf {
        self.root.join("ledger.jsonl")
    }

    pub fn enqueue(&self, journal: &Path) -> Result<DeliveryItem, DeliveryError> {
        let run_id = read_events(journal)
            .ok()
            .and_then(|events| events.first().map(|event| event.run_id.to_string()))
            .or_else(|| {
                journal
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "unknown".into());
        if self.done_dir().join(format!("{run_id}.json")).is_file() {
            return Err(DeliveryError::AlreadyAccepted(run_id));
        }
        fs::create_dir_all(self.queue_dir())?;
        let path = self.queue_dir().join(format!("{run_id}.json"));
        if path.is_file() {
            return Ok(serde_json::from_slice(&fs::read(path)?)?);
        }
        let item = DeliveryItem {
            schema: QUEUE_SCHEMA.into(),
            run_id,
            journal: journal.to_string_lossy().into_owned(),
            workspace: std::env::current_dir()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            enqueued_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
            status: DeliveryStatus::Pending,
        };
        write_json_atomic(&path, &item)?;
        Ok(item)
    }

    pub fn pending(&self) -> Result<Vec<DeliveryItem>, DeliveryError> {
        let dir = self.queue_dir();
        let mut items = Vec::new();
        if !dir.is_dir() {
            return Ok(items);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            items.push(serde_json::from_slice(&fs::read(entry.path())?)?);
        }
        items.sort_by_key(|item| item.enqueued_unix_ms);
        Ok(items)
    }

    pub fn done(&self) -> Result<Vec<DeliveryReceipt>, DeliveryError> {
        let dir = self.done_dir();
        let mut receipts = Vec::new();
        if !dir.is_dir() {
            return Ok(receipts);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            receipts.push(serde_json::from_slice(&fs::read(entry.path())?)?);
        }
        receipts.sort_by_key(|receipt| std::cmp::Reverse(receipt.created_unix_ms));
        Ok(receipts)
    }

    pub fn accept_journal(&self, journal: &Path) -> Result<DeliveryReceipt, DeliveryError> {
        let receipt = build_receipt(journal)?;
        fs::create_dir_all(self.done_dir())?;
        write_json_atomic(
            &self.done_dir().join(format!("{}.json", receipt.run_id)),
            &receipt,
        )?;
        self.append_ledger(&receipt)?;
        Ok(receipt)
    }

    pub fn process_pending(&self) -> Result<DeliveryBatch, DeliveryError> {
        let mut batch = DeliveryBatch::default();
        for item in self.pending()? {
            let journal = PathBuf::from(&item.journal);
            match build_receipt(&journal) {
                Ok(receipt) => {
                    fs::create_dir_all(self.done_dir())?;
                    write_json_atomic(
                        &self.done_dir().join(format!("{}.json", receipt.run_id)),
                        &receipt,
                    )?;
                    self.append_ledger(&receipt)?;
                    fs::remove_file(self.queue_dir().join(format!("{}.json", item.run_id)))?;
                    batch.receipts.push(receipt);
                }
                Err(error) => batch.errors.push(format!("{}: {error}", item.journal)),
            }
        }
        Ok(batch)
    }

    fn append_ledger(&self, receipt: &DeliveryReceipt) -> Result<(), DeliveryError> {
        use std::io::Write;
        fs::create_dir_all(&self.root)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.ledger_path())?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(&json!({
                "schema": "1agent.delivery-ledger/v1",
                "run_id": receipt.run_id,
                "status": receipt.status,
                "manifest_sha256": receipt.manifest_sha256,
                "journal": receipt.journal,
                "created_unix_ms": receipt.created_unix_ms,
            }))?
        )?;
        Ok(())
    }
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), DeliveryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentAnswer, AnswerStatus, TaskRequest};
    use crate::journal::{JournalEvent, JournalEventKind};
    use uuid::Uuid;

    fn write_journal(path: &Path, events: Vec<JournalEventKind>) -> String {
        let run_id = Uuid::new_v4();
        let mut text = String::new();
        for (seq, event) in events.into_iter().enumerate() {
            let record = JournalEvent {
                schema_version: 1,
                run_id,
                seq: seq as u64,
                timestamp_unix_ms: seq as u128,
                event,
            };
            text.push_str(&serde_json::to_string(&record).unwrap());
            text.push('\n');
        }
        fs::write(path, text).unwrap();
        run_id.to_string()
    }

    fn artifact_journal(dir: &Path) -> (PathBuf, PathBuf, String) {
        artifact_journal_with_status(dir, AnswerStatus::Completed)
    }

    fn artifact_journal_with_status(
        dir: &Path,
        answer_status: AnswerStatus,
    ) -> (PathBuf, PathBuf, String) {
        let artifact = dir.join("out.txt");
        fs::write(&artifact, "evidence").unwrap();
        let digest = sha256_file(&artifact).unwrap();
        let input = dir.join("input.txt");
        fs::write(&input, "source").unwrap();
        let input_digest = sha256_file(&input).unwrap();
        let journal = dir.join("run.jsonl");
        let request = TaskRequest::from_objective("write out.txt", dir.to_path_buf());
        let read_call = crate::domain::ToolCall {
            id: "c0".into(),
            name: "read_file".into(),
            arguments: json!({"path": input}),
        };
        let read_observation = crate::domain::ToolObservation {
            call_id: "c0".into(),
            tool: "read_file".into(),
            status: ToolStatus::Ok,
            summary: String::new(),
            content: String::new(),
            error_code: None,
            metadata: BTreeMap::from([
                (
                    "relative_path".into(),
                    input.file_name().unwrap().to_string_lossy().into_owned(),
                ),
                ("sha256".into(), input_digest),
            ]),
        };
        let call = crate::domain::ToolCall {
            id: "c1".into(),
            name: "write_file".into(),
            arguments: json!({"path": artifact}),
        };
        let observation = crate::domain::ToolObservation {
            call_id: "c1".into(),
            tool: "write_file".into(),
            status: ToolStatus::Ok,
            summary: String::new(),
            content: String::new(),
            error_code: None,
            metadata: BTreeMap::from([
                ("path".into(), artifact.to_string_lossy().into_owned()),
                ("sha256".into(), digest.clone()),
                ("bytes".into(), "8".into()),
            ]),
        };
        let answer = AgentAnswer {
            schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
            status: answer_status,
            conclusion: "wrote evidence from input.txt".into(),
            claims: vec![Claim {
                id: "c1".into(),
                text: "out.txt is evidence".into(),
                status: ClaimStatus::Verified,
                sources: vec![artifact.to_string_lossy().into_owned()],
                scope: None,
            }],
            evidence: vec![],
            assumptions: vec![],
            constraints: vec![],
            unknowns: vec![],
            conflicts: vec![],
            flip_conditions: vec![],
            action_receipts: vec![],
            task_need: None,
        };
        let snapshot = crate::workspace_snapshot::capture(dir);
        let run_id = write_journal(
            &journal,
            vec![
                JournalEventKind::RunStarted {
                    request,
                    mode: crate::domain::CognitiveMode::Execute,
                },
                JournalEventKind::ToolCalled { call: read_call },
                JournalEventKind::ToolObserved {
                    observation: read_observation,
                },
                JournalEventKind::ToolCalled { call },
                JournalEventKind::ToolObserved { observation },
                JournalEventKind::DeliverySnapshot { snapshot },
                JournalEventKind::AnswerProduced { answer },
            ],
        );
        (journal, artifact, run_id)
    }

    #[test]
    fn accepts_a_recorded_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let (journal, _artifact, _run_id) = artifact_journal(dir.path());
        let receipt = build_receipt(&journal).unwrap();
        assert_eq!(receipt.status, DeliveryStatus::Accepted);
        assert_eq!(receipt.artifacts[0]["match"], true);
        assert!(
            receipt.answer["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source.as_str().is_some_and(|s| s.ends_with("input.txt")))
        );
    }

    #[test]
    fn needs_evidence_answer_is_not_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let (journal, _artifact, _run_id) =
            artifact_journal_with_status(dir.path(), AnswerStatus::NeedsEvidence);
        let receipt = build_receipt(&journal).unwrap();
        assert_eq!(receipt.status, DeliveryStatus::Conditional);
    }

    #[test]
    fn tampered_artifact_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let (journal, artifact, _run_id) = artifact_journal(dir.path());
        fs::write(artifact, "tampered").unwrap();
        let receipt = build_receipt(&journal).unwrap();
        assert_eq!(receipt.status, DeliveryStatus::Rejected);
    }

    #[test]
    fn queue_is_durable_and_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (journal, _artifact, run_id) = artifact_journal(dir.path());
        let queue = DeliveryQueue::new(dir.path().join("acceptance"));
        let item = queue.enqueue(&journal).unwrap();
        assert_eq!(item.run_id, run_id);
        assert_eq!(queue.pending().unwrap().len(), 1);
        let batch = queue.process_pending().unwrap();
        assert_eq!(batch.receipts.len(), 1);
        assert!(queue.pending().unwrap().is_empty());
        assert!(queue.done_dir().join(format!("{run_id}.json")).is_file());
        assert_eq!(queue.done().unwrap().len(), 1);
        assert!(queue.enqueue(&journal).is_err());
    }
}
