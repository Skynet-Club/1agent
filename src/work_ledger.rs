//! Append-only work ledger for a workspace. The runtime records what happened
//! (artifacts, hashes, fresh/reused) so a later turn can answer provenance
//! questions without searching or re-auditing the workspace.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

impl WorkLedger {
    pub fn load(workspace: &Path) -> Self {
        let path = ledger_path(workspace);
        let Ok(file) = File::open(path) else {
            return Self::default();
        };
        let mut entries = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<LedgerEntry>(&line).ok())
            .collect::<Vec<_>>();
        if entries.len() > 5 {
            entries = entries.split_off(entries.len() - 5);
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
