//! Append-only work ledger for a workspace. The runtime records what happened
//! (artifacts, hashes, fresh/reused) so a later turn can answer provenance
//! questions without searching or re-auditing the workspace.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::{AnswerStatus, StopReason, ToolStatus};
use crate::journal::{JournalEvent, JournalEventKind};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkLedger {
    #[serde(default)]
    pub entries: Vec<LedgerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub run_id: String,
    #[serde(default)]
    pub session_id: String,
    pub timestamp_unix_ms: u128,
    pub objective: String,
    pub mode: String,
    pub status: String,
    #[serde(default)]
    pub stop_reason: String,
    #[serde(default)]
    pub artifacts: Vec<LedgerArtifact>,
    #[serde(default)]
    pub reused: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerArtifact {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub fresh: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RebuildReport {
    pub scanned: usize,
    pub matched: usize,
    pub appended: usize,
    pub already_present: usize,
    pub unreadable: usize,
}

impl WorkLedger {
    pub fn load(workspace: &Path) -> Self {
        let mut entries = read_entries(&ledger_path(workspace));
        // Keep enough history for the already-done/verify-only gates; the
        // model context still shows only the newest five.
        if entries.len() > 20 {
            entries = entries.split_off(entries.len() - 20);
        }
        Self { entries }
    }

    pub fn append(workspace: &Path, entry: &LedgerEntry) -> std::io::Result<()> {
        let path = ledger_path(workspace);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let line = serde_json::to_string(entry)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        writeln!(file, "{line}")
    }

    pub fn has_active_artifact(&self) -> bool {
        self.entries
            .iter()
            .rev()
            .any(|entry| !entry.artifacts.is_empty())
    }

    /// A recent Act entry with the same objective whose recorded artifacts are
    /// still present and unchanged. Exact objective match only.
    pub fn completed_repeat(&self, objective: &str) -> Option<&LedgerEntry> {
        let objective = objective.trim();
        self.entries.iter().rev().find(|entry| {
            entry.mode == "act"
                && matches!(entry.status.as_str(), "completed" | "conditional")
                && !entry.artifacts.is_empty()
                && entry.objective.trim() == objective
                && entry.artifacts.iter().all(artifact_unchanged)
        })
    }

    /// The newest Act entry's artifact hashes when all of them are unchanged.
    pub fn active_artifacts_unchanged(&self) -> Option<BTreeMap<String, String>> {
        self.entries
            .iter()
            .rev()
            .find(|entry| {
                entry.mode == "act"
                    && !entry.artifacts.is_empty()
                    && entry.artifacts.iter().all(artifact_unchanged)
            })
            .map(|entry| {
                entry
                    .artifacts
                    .iter()
                    .map(|artifact| (artifact.path.clone(), artifact.sha256.clone()))
                    .collect()
            })
    }

    /// Compact context for the model: recent runs plus a live check of the
    /// artifact hashes. This is the answer to "is this the same file?".
    pub fn context(&self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let mut out = String::from(
            "WORK LEDGER (runtime facts from earlier runs in this workspace; newest first)\nThese facts are authoritative for provenance questions. Do not search or re-audit the workspace merely to confirm them. These entries are from earlier runs; this run has not written anything yet.\n",
        );
        for entry in self.entries.iter().rev().take(5) {
            out.push_str(&format!(
                "- run {} · {} · mode={} · status={} · objective: {}\n",
                short_id(&entry.run_id),
                age_label(entry.timestamp_unix_ms, now_ms),
                entry.mode,
                entry.status,
                truncate(&entry.objective, 140)
            ));
            for artifact in &entry.artifacts {
                out.push_str(&format!(
                    "  artifact{}: {} · {} bytes · sha256 {}\n",
                    if artifact.fresh {
                        " (written by that run)"
                    } else {
                        ""
                    },
                    artifact.path,
                    artifact.bytes,
                    short_sha(&artifact.sha256)
                ));
            }
            if !entry.reused.is_empty() {
                out.push_str(&format!("  reused: {}\n", entry.reused.join(", ")));
            }
            if !entry.blockers.is_empty() {
                out.push_str(&format!("  blockers: {}\n", entry.blockers.join(" | ")));
            }
        }
        let mut seen = BTreeSet::new();
        let mut checks = Vec::new();
        'entries: for entry in self.entries.iter().rev() {
            for artifact in &entry.artifacts {
                if !seen.insert(artifact.path.clone()) {
                    continue;
                }
                checks.push(current_artifact_line(artifact));
                if checks.len() >= 5 {
                    break 'entries;
                }
            }
        }
        if !checks.is_empty() {
            out.push_str("CURRENT ARTIFACT CHECK (measured now)\n");
            for check in checks {
                out.push_str(&check);
            }
        }
        Some(out)
    }
}

pub fn ledger_path(workspace: &Path) -> PathBuf {
    let root = std::env::var_os("ONEAGENT_WORK_LEDGER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::runtime_home::default_home().join("work-ledger"));
    let canonical = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    root.join(format!("{}.jsonl", &digest[..16]))
}

/// Backfill a workspace ledger from journals written before the ledger existed.
/// A journal belongs to the workspace when its first read root is the workspace
/// or one of its written artifacts lives under it.
pub fn rebuild_from_journals(
    workspace: &Path,
    journal_dir: &Path,
    ledger_file: &Path,
    requires_write: impl Fn(&str) -> bool,
) -> std::io::Result<RebuildReport> {
    let workspace = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let mut report = RebuildReport::default();
    let mut entries = read_entries(ledger_file);
    let mut seen = entries
        .iter()
        .map(|entry| entry.run_id.clone())
        .collect::<BTreeSet<_>>();
    let mut matched = Vec::new();
    for file in fs::read_dir(journal_dir)? {
        let path = file?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        report.scanned += 1;
        let Ok(events) = crate::journal::read_events(&path) else {
            report.unreadable += 1;
            continue;
        };
        let Some((run_workspace, entry)) = entry_from_events(&events, "", &requires_write) else {
            continue;
        };
        let run_workspace = fs::canonicalize(&run_workspace).unwrap_or(run_workspace);
        let belongs = run_workspace == workspace
            || entry
                .artifacts
                .iter()
                .any(|artifact| Path::new(&artifact.path).starts_with(&workspace));
        if !belongs {
            continue;
        }
        report.matched += 1;
        if !seen.insert(entry.run_id.clone()) {
            report.already_present += 1;
            continue;
        }
        matched.push(entry);
    }
    if matched.is_empty() {
        return Ok(report);
    }
    report.appended = matched.len();
    entries.extend(matched);
    entries.sort_by_key(|entry| entry.timestamp_unix_ms);
    write_entries(ledger_file, &entries)?;
    Ok(report)
}

/// Derive one ledger entry from a finished run's journal. Facts come from tool
/// observations, never from the model's claims.
pub fn entry_from_events(
    events: &[JournalEvent],
    session_id: &str,
    requires_write: impl Fn(&str) -> bool,
) -> Option<(PathBuf, LedgerEntry)> {
    let (start, request) = events.iter().find_map(|event| match &event.event {
        JournalEventKind::RunStarted { request, .. }
        | JournalEventKind::ResumeStarted { request, .. } => Some((event, request)),
        _ => None,
    })?;
    let workspace = request.permissions.read_roots.first()?.clone();
    if workspace.as_os_str().is_empty() {
        return None;
    }
    let mut mode = "unknown".to_string();
    let mut artifacts = std::collections::BTreeMap::new();
    let mut reused = BTreeSet::new();
    let mut status = "unknown";
    let mut stop_reason = String::new();
    let mut blockers = Vec::new();
    for event in events {
        match &event.event {
            JournalEventKind::InteractionModeSelected { decision } => {
                mode = decision.mode.as_str().to_string();
            }
            JournalEventKind::ToolObserved { observation }
                if observation.status == ToolStatus::Ok =>
            {
                let (Some(path), Some(sha256)) = (
                    observation.metadata.get("path"),
                    observation.metadata.get("sha256"),
                ) else {
                    continue;
                };
                let bytes = observation
                    .metadata
                    .get("bytes")
                    .and_then(|value| value.parse::<u64>().ok())
                    .or_else(|| fs::metadata(path).ok().map(|metadata| metadata.len()))
                    .unwrap_or_default();
                if requires_write(&observation.tool) {
                    artifacts.insert(
                        path.clone(),
                        LedgerArtifact {
                            path: path.clone(),
                            sha256: sha256.clone(),
                            bytes,
                            fresh: true,
                        },
                    );
                    reused.remove(path);
                } else if observation.tool == "verify_artifact" {
                    reused.insert(path.clone());
                }
            }
            JournalEventKind::AnswerProduced { answer } => {
                status = answer_status_str(answer.status);
                blockers = answer
                    .unknowns
                    .iter()
                    .map(|unknown| unknown.description.chars().take(160).collect::<String>())
                    .filter(|line| !line.is_empty())
                    .take(3)
                    .collect();
            }
            JournalEventKind::RunStopped { reason } => {
                stop_reason = stop_reason_str(*reason).into();
            }
            _ => {}
        }
    }
    for path in artifacts.keys() {
        reused.remove(path);
    }
    Some((
        workspace,
        LedgerEntry {
            run_id: start.run_id.to_string(),
            session_id: session_id.into(),
            timestamp_unix_ms: start.timestamp_unix_ms,
            objective: request.objective.clone(),
            mode,
            status: status.into(),
            stop_reason,
            artifacts: artifacts.into_values().collect(),
            reused: reused.into_iter().collect(),
            blockers,
        },
    ))
}

fn read_entries(path: &Path) -> Vec<LedgerEntry> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<LedgerEntry>(&line).ok())
        .collect()
}

fn write_entries(path: &Path, entries: &[LedgerEntry]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("jsonl.tmp");
    let mut lines = String::new();
    for entry in entries {
        lines.push_str(
            &serde_json::to_string(entry)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?,
        );
        lines.push('\n');
    }
    fs::write(&tmp, lines)?;
    fs::rename(tmp, path)
}

pub(crate) fn answer_status_str(status: AnswerStatus) -> &'static str {
    match status {
        AnswerStatus::Completed => "completed",
        AnswerStatus::Conditional => "conditional",
        AnswerStatus::NeedsEvidence => "needs_evidence",
        AnswerStatus::NeedsAuthorization => "needs_authorization",
        AnswerStatus::Failed => "failed",
    }
}

fn stop_reason_str(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Completed => "completed",
        StopReason::NeedsEvidence => "needs_evidence",
        StopReason::NeedsAuthorization => "needs_authorization",
        StopReason::BudgetExhausted => "budget_exhausted",
        StopReason::Cancelled => "cancelled",
        StopReason::ProviderError => "provider_error",
        StopReason::ToolFailure => "tool_failure",
        StopReason::InvalidOutput => "invalid_output",
        StopReason::InternalError => "internal_error",
    }
}

fn current_artifact_line(artifact: &LedgerArtifact) -> String {
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    match fs::metadata(&artifact.path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_BYTES => {
            match sha256_file(&artifact.path) {
                Ok(sha256) => format!(
                    "- {} · {} bytes · sha256 {} · {}\n",
                    artifact.path,
                    metadata.len(),
                    short_sha(&sha256),
                    if artifact.sha256.is_empty() || sha256 == artifact.sha256 {
                        "unchanged since the ledger entry"
                    } else {
                        "CHANGED since the ledger entry"
                    }
                ),
                Err(error) => format!("- {} · cannot hash: {error}\n", artifact.path),
            }
        }
        Ok(metadata) if metadata.is_file() => format!(
            "- {} · {} bytes · sha256 not recomputed (over 8 MiB)\n",
            artifact.path,
            metadata.len()
        ),
        _ => format!("- {} · missing now\n", artifact.path),
    }
}

fn artifact_unchanged(artifact: &LedgerArtifact) -> bool {
    const MAX_HASH_BYTES: u64 = 8 * 1024 * 1024;
    let Ok(metadata) = fs::metadata(&artifact.path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != artifact.bytes {
        return false;
    }
    if artifact.sha256.is_empty() || metadata.len() > MAX_HASH_BYTES {
        return true;
    }
    sha256_file(&artifact.path).is_ok_and(|sha256| sha256 == artifact.sha256)
}

fn sha256_file(path: &str) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

fn short_sha(value: &str) -> String {
    if value.is_empty() {
        return "unknown".into();
    }
    format!("{}…", value.get(..12).unwrap_or(value))
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn age_label(timestamp_ms: u128, now_ms: u128) -> String {
    let seconds = now_ms.saturating_sub(timestamp_ms) / 1000;
    if seconds < 60 {
        "just now".into()
    } else if seconds < 3600 {
        format!("{} min ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} h ago", seconds / 3600)
    } else {
        format!("{} d ago", seconds / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_round_trips_and_summarizes_artifacts() {
        let workspace = tempfile::tempdir().unwrap();
        let target = workspace.path().join("index.html");
        fs::write(&target, "<html>old</html>").unwrap();
        let sha = sha256_file(target.to_str().unwrap()).unwrap();
        let entry = LedgerEntry {
            run_id: "12345678-0000".into(),
            session_id: "session".into(),
            timestamp_unix_ms: 1,
            objective: "做一个页面".into(),
            mode: "act".into(),
            status: "completed".into(),
            stop_reason: "completed".into(),
            artifacts: vec![LedgerArtifact {
                path: target.display().to_string(),
                sha256: sha.clone(),
                bytes: 16,
                fresh: true,
            }],
            reused: Vec::new(),
            blockers: Vec::new(),
        };
        WorkLedger::append(workspace.path(), &entry).unwrap();
        let ledger = WorkLedger::load(workspace.path());
        assert_eq!(ledger.entries.len(), 1);
        let context = ledger.context().unwrap();
        assert!(context.contains("mode=act"));
        assert!(context.contains("index.html"));
        assert!(context.contains("unchanged since the ledger entry"));
        assert!(context.contains(&short_sha(&sha)));
    }

    fn write_run(
        journal_dir: &Path,
        read_root: &Path,
        objective: &str,
        artifact: Option<&Path>,
    ) -> String {
        let run_id = uuid::Uuid::new_v4();
        let path = journal_dir.join(format!("{run_id}.jsonl"));
        let mut journal = crate::journal::Journal::create(&path, run_id).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: crate::domain::TaskRequest::from_objective(
                    objective,
                    read_root.to_path_buf(),
                ),
                mode: crate::domain::CognitiveMode::Execute,
            })
            .unwrap();
        if let Some(artifact) = artifact {
            let metadata = std::collections::BTreeMap::from([
                ("path".to_string(), artifact.display().to_string()),
                (
                    "sha256".to_string(),
                    sha256_file(artifact.to_str().unwrap()).unwrap(),
                ),
                (
                    "bytes".to_string(),
                    fs::metadata(artifact).unwrap().len().to_string(),
                ),
            ]);
            journal
                .append(JournalEventKind::ToolObserved {
                    observation: crate::domain::ToolObservation {
                        call_id: "call-1".into(),
                        tool: "write_file".into(),
                        status: ToolStatus::Ok,
                        summary: "wrote the page".into(),
                        content: String::new(),
                        error_code: None,
                        metadata,
                    },
                })
                .unwrap();
        }
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: crate::domain::AgentAnswer {
                    schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                    status: AnswerStatus::Completed,
                    conclusion: "done".into(),
                    claims: Vec::new(),
                    evidence: Vec::new(),
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                    task_need: None,
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        run_id.to_string()
    }

    #[test]
    fn rebuild_backfills_matching_journals_once() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        let journals = root.path().join("journals");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&journals).unwrap();
        let target = project.join("index.html");
        fs::write(&target, "<html>page</html>").unwrap();
        let ledger_file = root.path().join("ledger.jsonl");

        let by_root = write_run(&journals, &project, "直接建页", Some(&target));
        let by_artifact = write_run(&journals, root.path(), "父目录里建页", Some(&target));
        let unrelated_root = tempfile::tempdir().unwrap();
        let unrelated = write_run(&journals, unrelated_root.path(), "别的项目", None);

        let report = rebuild_from_journals(&project, &journals, &ledger_file, |name| {
            name == "write_file"
        })
        .unwrap();
        assert_eq!(report.scanned, 3);
        assert_eq!(report.matched, 2);
        assert_eq!(report.appended, 2);

        let entries = read_entries(&ledger_file);
        let ids = entries
            .iter()
            .map(|entry| entry.run_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            ids,
            BTreeSet::from([by_root.as_str(), by_artifact.as_str()])
        );
        assert!(!ids.contains(unrelated.as_str()));
        let sha = sha256_file(target.to_str().unwrap()).unwrap();
        for entry in &entries {
            assert_eq!(entry.status, "completed");
            assert_eq!(entry.artifacts.len(), 1);
            assert_eq!(entry.artifacts[0].path, target.display().to_string());
            assert_eq!(entry.artifacts[0].sha256, sha);
        }

        let again = rebuild_from_journals(&project, &journals, &ledger_file, |name| {
            name == "write_file"
        })
        .unwrap();
        assert_eq!(again.matched, 2);
        assert_eq!(again.appended, 0);
        assert_eq!(again.already_present, 2);
        assert_eq!(read_entries(&ledger_file).len(), 2);
    }

    #[test]
    fn completed_repeat_requires_unchanged_artifacts() {
        let workspace = tempfile::tempdir().unwrap();
        let target = workspace.path().join("page.html");
        fs::write(&target, "<html>page</html>").unwrap();
        let sha = sha256_file(target.to_str().unwrap()).unwrap();
        let ledger = WorkLedger {
            entries: vec![LedgerEntry {
                run_id: "run-1".into(),
                session_id: String::new(),
                timestamp_unix_ms: 1,
                objective: "做一个页面".into(),
                mode: "act".into(),
                status: "completed".into(),
                stop_reason: "completed".into(),
                artifacts: vec![LedgerArtifact {
                    path: target.display().to_string(),
                    sha256: sha.clone(),
                    bytes: fs::metadata(&target).unwrap().len(),
                    fresh: true,
                }],
                reused: Vec::new(),
                blockers: Vec::new(),
            }],
        };

        assert!(ledger.completed_repeat(" 做一个页面 ").is_some());
        assert_eq!(
            ledger
                .active_artifacts_unchanged()
                .unwrap()
                .get(&target.display().to_string()),
            Some(&sha)
        );
        fs::write(&target, "<html>changed</html>").unwrap();
        assert!(ledger.completed_repeat("做一个页面").is_none());
        assert!(ledger.active_artifacts_unchanged().is_none());
        assert!(ledger.completed_repeat("另一个任务").is_none());
    }

    #[test]
    fn missing_artifact_is_reported() {
        let artifact = LedgerArtifact {
            path: "/tmp/does-not-exist-1agent".into(),
            sha256: "abc".into(),
            bytes: 1,
            fresh: true,
        };
        assert!(current_artifact_line(&artifact).contains("missing now"));
    }
}
