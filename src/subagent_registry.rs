//! Durable, append-only status registry for bounded child agents.
//!
//! The parent already receives the child's final JSON through the tool result.
//! This registry exists for the live UI: it records the task, workspace,
//! journal directory and lifecycle state before the child finishes, so the
//! TUI can show a running squad member without guessing from stdout.
//!
//! One JSON object per line; the latest record for an id wins. Records are
//! filtered by `root_pid` so concurrent 1Agent processes do not show each
//! other's children. The registry is local state, not evidence: the child
//! journal remains the audit source.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SUBAGENT_REGISTRY_SCHEMA: u32 = 1;
pub const SUBAGENT_ROOT_PID_ENV: &str = "ONEAGENT_SUBAGENT_ROOT_PID";
pub const SUBAGENT_PARENT_RUN_ENV: &str = "ONEAGENT_PARENT_RUN_ID";
pub const SUBAGENT_PARENT_SESSION_ENV: &str = "ONEAGENT_PARENT_SESSION_ID";
pub const SUBAGENT_SPAWN_CALL_ENV: &str = "ONEAGENT_SPAWN_CALL_ID";
const MAX_REGISTRY_BYTES: u64 = 4 * 1024 * 1024;
const MAX_RECORDS: usize = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Starting,
    Running,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
}

impl SubagentStatus {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Starting => "启动中",
            Self::Running => "运行中",
            Self::Completed => "已完成",
            Self::Failed => "失败",
            Self::TimedOut => "超时",
            Self::Cancelled => "已取消",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Starting | Self::Running => "●",
            Self::Completed => "✓",
            Self::Failed => "✗",
            Self::TimedOut => "⏱",
            Self::Cancelled => "·",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubagentRecord {
    pub schema_version: u32,
    pub id: String,
    pub root_pid: u32,
    pub parent_pid: u32,
    #[serde(default)]
    pub child_pid: Option<u32>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub spawn_call_id: Option<String>,
    #[serde(default)]
    pub child_run_id: Option<String>,
    pub depth: u8,
    pub task: String,
    pub workspace: String,
    pub journal_dir: String,
    pub status: SubagentStatus,
    pub started_unix_ms: u128,
    #[serde(default)]
    pub ended_unix_ms: Option<u128>,
    pub allow_write: bool,
    pub allow_network: bool,
    pub max_steps: usize,
    pub timeout_seconds: u64,
    #[serde(default)]
    pub summary: Option<String>,
}

impl SubagentRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        root_pid: u32,
        parent_pid: u32,
        depth: u8,
        task: impl Into<String>,
        workspace: impl Into<String>,
        journal_dir: impl Into<String>,
        allow_write: bool,
        allow_network: bool,
        max_steps: usize,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            schema_version: SUBAGENT_REGISTRY_SCHEMA,
            id: id.into(),
            root_pid,
            parent_pid,
            child_pid: None,
            parent_session_id: None,
            parent_run_id: None,
            spawn_call_id: None,
            child_run_id: None,
            depth,
            task: task.into(),
            workspace: workspace.into(),
            journal_dir: journal_dir.into(),
            status: SubagentStatus::Starting,
            started_unix_ms: now_unix_ms(),
            ended_unix_ms: None,
            allow_write,
            allow_network,
            max_steps,
            timeout_seconds,
            summary: None,
        }
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.ended_unix_ms
            .unwrap_or_else(now_unix_ms)
            .saturating_sub(self.started_unix_ms)
    }
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

pub fn registry_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".cache/1agent-tmp/subagents")
}

pub fn registry_path() -> PathBuf {
    registry_dir().join("registry.jsonl")
}

fn ensure_registry_file(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }
    Ok(())
}

pub fn append_record(path: &Path, record: &SubagentRecord) -> std::io::Result<()> {
    ensure_registry_file(path)?;
    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && metadata.file_type().is_symlink()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "subagent registry path must not be a symlink",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    // One `write_all` per record: concurrent 1Agent processes append to the
    // same registry file, and a formatted write can split the JSON and the
    // newline into separate syscalls that interleave with another record.
    let mut line = serde_json::to_string(record)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    line.push('\n');
    file.write_all(line.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn load_recent(path: &Path, root_pid: u32) -> Vec<SubagentRecord> {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return Vec::new();
    };
    if metadata.file_type().is_symlink() || metadata.len() > MAX_REGISTRY_BYTES {
        return Vec::new();
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut latest: BTreeMap<String, SubagentRecord> = BTreeMap::new();
    for line in content.lines() {
        let Ok(record) = serde_json::from_str::<SubagentRecord>(line) else {
            continue;
        };
        if record.schema_version != SUBAGENT_REGISTRY_SCHEMA || record.root_pid != root_pid {
            continue;
        }
        latest.insert(record.id.clone(), record);
    }
    let mut records = latest.into_values().collect::<Vec<_>>();
    records.sort_by(|left, right| {
        right
            .started_unix_ms
            .cmp(&left.started_unix_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    records.truncate(MAX_RECORDS);
    records
}

pub fn load_recent_current() -> Vec<SubagentRecord> {
    load_recent(&registry_path(), std::process::id())
}

/// Writes a final `Failed` record if the owner is dropped without finishing.
/// This is what closes the UI gap when the parent run is interrupted.
pub struct RegistryGuard {
    path: PathBuf,
    record: SubagentRecord,
    finished: bool,
}

impl RegistryGuard {
    pub fn start(record: SubagentRecord) -> Self {
        let path = registry_path();
        let _ = append_record(&path, &record);
        Self {
            path,
            record,
            finished: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.record.id
    }

    pub fn running(&mut self, child_pid: u32) {
        self.record.status = SubagentStatus::Running;
        self.record.child_pid = Some(child_pid);
        let _ = append_record(&self.path, &self.record);
    }

    /// Backfill the child run id once the child reports it. The value is
    /// persisted by the next registry write (`finish` or `Drop`).
    pub fn set_child_run_id(&mut self, child_run_id: impl Into<String>) {
        self.record.child_run_id = Some(child_run_id.into());
    }

    pub fn finish(&mut self, status: SubagentStatus, summary: Option<String>) {
        self.record.status = status;
        self.record.summary = summary;
        self.record.ended_unix_ms = Some(now_unix_ms());
        let _ = append_record(&self.path, &self.record);
        self.finished = true;
    }
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(
                SubagentStatus::Cancelled,
                Some("owner dropped before the child reported".into()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, root_pid: u32, status: SubagentStatus, started: u128) -> SubagentRecord {
        let mut record = SubagentRecord::new(
            id,
            root_pid,
            root_pid,
            1,
            "inspect the repo",
            "/tmp/workspace",
            "/tmp/workspace/.journal",
            true,
            false,
            8,
            120,
        );
        record.status = status;
        record.started_unix_ms = started;
        record
    }

    #[test]
    fn latest_record_wins_and_root_pid_filters_other_processes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.jsonl");
        append_record(&path, &record("sub-a", 10, SubagentStatus::Starting, 1)).unwrap();
        append_record(&path, &record("sub-a", 10, SubagentStatus::Running, 1)).unwrap();
        append_record(&path, &record("sub-b", 20, SubagentStatus::Running, 2)).unwrap();
        append_record(&path, &record("sub-c", 10, SubagentStatus::Completed, 3)).unwrap();

        let records = load_recent(&path, 10);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, "sub-c");
        assert_eq!(records[1].id, "sub-a");
        assert_eq!(records[1].status, SubagentStatus::Running);
    }

    #[test]
    fn guard_finishes_or_marks_a_dropped_run_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.jsonl");
        let mut guard = RegistryGuard {
            path: path.clone(),
            record: record("sub-d", 10, SubagentStatus::Starting, 1),
            finished: false,
        };
        guard.running(1234);
        guard.finish(SubagentStatus::Completed, Some("done".into()));
        drop(guard);
        let records = load_recent(&path, 10);
        assert_eq!(records[0].status, SubagentStatus::Completed);
        assert_eq!(records[0].child_pid, Some(1234));
        assert_eq!(records[0].summary.as_deref(), Some("done"));

        let guard = RegistryGuard {
            path: path.clone(),
            record: record("sub-e", 10, SubagentStatus::Starting, 2),
            finished: false,
        };
        drop(guard);
        let records = load_recent(&path, 10);
        assert_eq!(
            records
                .iter()
                .find(|record| record.id == "sub-e")
                .unwrap()
                .status,
            SubagentStatus::Cancelled
        );
    }

    #[test]
    fn lineage_fields_round_trip_and_child_run_is_backfilled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.jsonl");

        let mut record = record("sub-lineage", 10, SubagentStatus::Starting, 1);
        assert!(record.parent_session_id.is_none());
        assert!(record.parent_run_id.is_none());
        assert!(record.spawn_call_id.is_none());
        assert!(record.child_run_id.is_none());
        record.parent_session_id = Some("ses_parent".into());
        record.parent_run_id = Some("run-parent".into());
        record.spawn_call_id = Some("call-7".into());

        let mut guard = RegistryGuard {
            path: path.clone(),
            record,
            finished: false,
        };
        guard.running(1234);
        guard.set_child_run_id("run-child");
        guard.finish(SubagentStatus::Completed, Some("done".into()));
        drop(guard);

        let records = load_recent(&path, 10);
        let latest = records
            .iter()
            .find(|record| record.id == "sub-lineage")
            .unwrap();
        assert_eq!(latest.parent_session_id.as_deref(), Some("ses_parent"));
        assert_eq!(latest.parent_run_id.as_deref(), Some("run-parent"));
        assert_eq!(latest.spawn_call_id.as_deref(), Some("call-7"));
        assert_eq!(latest.child_run_id.as_deref(), Some("run-child"));
    }

    #[test]
    fn concurrent_appends_keep_one_parseable_record_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.jsonl");
        let handles = (0..8)
            .map(|worker| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for index in 0..50 {
                        append_record(
                            &path,
                            &record(
                                &format!("sub-{worker}-{index}"),
                                10,
                                SubagentStatus::Running,
                                index,
                            ),
                        )
                        .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }

        let text = std::fs::read_to_string(&path).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 400);
        for line in lines {
            serde_json::from_str::<SubagentRecord>(line).unwrap();
        }
    }

    #[test]
    fn legacy_records_without_lineage_fields_remain_readable() {
        let mut value =
            serde_json::to_value(record("sub-legacy", 10, SubagentStatus::Running, 1)).unwrap();
        let object = value.as_object_mut().unwrap();
        for key in [
            "parent_session_id",
            "parent_run_id",
            "spawn_call_id",
            "child_run_id",
        ] {
            assert!(object.remove(key).is_some(), "expected {key} in old record");
        }

        let parsed: SubagentRecord = serde_json::from_value(value).unwrap();
        assert!(parsed.parent_session_id.is_none());
        assert!(parsed.parent_run_id.is_none());
        assert!(parsed.spawn_call_id.is_none());
        assert!(parsed.child_run_id.is_none());
    }
}
