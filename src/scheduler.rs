//! Persistent schedule store for the daemon.
//!
//! Jobs are plain JSON records with a next-run timestamp. The daemon runs due
//! jobs as isolated child `1agent run` processes; this module only owns
//! validation, locking and atomic persistence.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::jsonl::FileLock;

const STATE_VERSION: u32 = 1;
const MAX_NAME_CHARS: usize = 64;
const MAX_TASK_CHARS: usize = 8 * 1024;
const MAX_EVERY_SECONDS: u64 = 7 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduleJob {
    pub id: String,
    pub name: String,
    pub task: String,
    pub every_seconds: u64,
    pub read_only: bool,
    pub allow_network: bool,
    pub workspace: Option<PathBuf>,
    #[serde(default)]
    pub trigger_path: Option<PathBuf>,
    /// `name@version` of an active skill composed into the task at run time.
    #[serde(default)]
    pub routine_skill: Option<String>,
    #[serde(default)]
    pub last_trigger_mtime_ms: Option<u128>,
    pub next_run_ms: u128,
    pub last_run_ms: Option<u128>,
    pub last_status: Option<String>,
    pub last_journal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleState {
    pub version: u32,
    pub jobs: Vec<ScheduleJob>,
}

impl Default for ScheduleState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            jobs: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("schedule store io: {0}")]
    Io(#[from] std::io::Error),
    #[error("schedule store json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schedule state version {0}")]
    UnsupportedVersion(u32),
    #[error("schedule job {0} not found")]
    NotFound(String),
    #[error("invalid schedule: {0}")]
    Invalid(String),
}

fn lock_path(path: &Path) -> PathBuf {
    path.with_extension("lock")
}

fn lock(path: &Path) -> Result<FileLock, ScheduleError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    FileLock::exclusive(&lock_path(path)).map_err(ScheduleError::Io)
}

fn read_unlocked(path: &Path) -> Result<ScheduleState, ScheduleError> {
    if !path.exists() {
        return Ok(ScheduleState::default());
    }
    let bytes = std::fs::read(path)?;
    let state: ScheduleState = serde_json::from_slice(&bytes)?;
    validate_state(&state)?;
    Ok(state)
}

fn write_atomic(path: &Path, state: &ScheduleState) -> Result<(), ScheduleError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    std::fs::write(&temporary, serde_json::to_vec_pretty(state)?)?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(ScheduleError::Io(error));
    }
    Ok(())
}

fn update<F, T>(path: &Path, apply: F) -> Result<T, ScheduleError>
where
    F: FnOnce(&mut ScheduleState) -> Result<T, ScheduleError>,
{
    let _lock = lock(path)?;
    let mut state = read_unlocked(path)?;
    let result = apply(&mut state)?;
    write_atomic(path, &state)?;
    Ok(result)
}

fn validate_state(state: &ScheduleState) -> Result<(), ScheduleError> {
    if state.version != STATE_VERSION {
        return Err(ScheduleError::UnsupportedVersion(state.version));
    }
    let mut ids = std::collections::BTreeSet::new();
    for job in &state.jobs {
        validate_job(job)?;
        if !ids.insert(job.id.clone()) {
            return Err(ScheduleError::Invalid(format!(
                "duplicate schedule id {}",
                job.id
            )));
        }
    }
    Ok(())
}

fn validate_job(job: &ScheduleJob) -> Result<(), ScheduleError> {
    validate_name(&job.name)?;
    validate_task(&job.task)?;
    if job.id.is_empty() || job.id.len() > 128 {
        return Err(ScheduleError::Invalid("invalid schedule id".into()));
    }
    if job.every_seconds == 0 || job.every_seconds > MAX_EVERY_SECONDS {
        return Err(ScheduleError::Invalid(format!(
            "every_seconds must be 1..={MAX_EVERY_SECONDS}"
        )));
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<(), ScheduleError> {
    if value.trim().is_empty() || value.chars().count() > MAX_NAME_CHARS {
        return Err(ScheduleError::Invalid(format!(
            "name must be 1..={MAX_NAME_CHARS} characters"
        )));
    }
    if crate::safety::contains_disallowed_control(value) {
        return Err(ScheduleError::Invalid(
            "name contains control characters".into(),
        ));
    }
    Ok(())
}

fn validate_task(value: &str) -> Result<(), ScheduleError> {
    if value.trim().is_empty() || value.chars().count() > MAX_TASK_CHARS {
        return Err(ScheduleError::Invalid(format!(
            "task must be 1..={MAX_TASK_CHARS} characters"
        )));
    }
    if crate::safety::contains_disallowed_control(value) {
        return Err(ScheduleError::Invalid(
            "task contains control characters".into(),
        ));
    }
    Ok(())
}

pub fn load(path: &Path) -> Result<ScheduleState, ScheduleError> {
    let _lock = lock(path)?;
    read_unlocked(path)
}

#[allow(clippy::too_many_arguments)]
pub fn add(
    path: &Path,
    name: &str,
    task: &str,
    every_seconds: u64,
    read_only: bool,
    allow_network: bool,
    workspace: Option<PathBuf>,
    trigger_path: Option<PathBuf>,
    routine_skill: Option<String>,
    start_now: bool,
    now_ms: u128,
) -> Result<ScheduleJob, ScheduleError> {
    validate_name(name)?;
    validate_task(task)?;
    if every_seconds == 0 || every_seconds > MAX_EVERY_SECONDS {
        return Err(ScheduleError::Invalid(format!(
            "every_seconds must be 1..={MAX_EVERY_SECONDS}"
        )));
    }
    if let Some(skill) = routine_skill.as_deref() {
        let mut parts = skill.split('@');
        let (Some(name), Some(version), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(ScheduleError::Invalid(
                "routine_skill must be name@version".into(),
            ));
        };
        if name.is_empty() || version.is_empty() || skill.len() > 200 {
            return Err(ScheduleError::Invalid(
                "routine_skill must be name@version".into(),
            ));
        }
    }
    let job = ScheduleJob {
        id: format!("schedule-{}", Uuid::new_v4()),
        name: name.trim().to_string(),
        task: task.to_string(),
        every_seconds,
        read_only,
        allow_network,
        workspace,
        trigger_path,
        routine_skill,
        last_trigger_mtime_ms: None,
        next_run_ms: if start_now {
            now_ms
        } else {
            now_ms + u128::from(every_seconds) * 1000
        },
        last_run_ms: None,
        last_status: None,
        last_journal: None,
    };
    update(path, |state| {
        state.jobs.push(job.clone());
        Ok(())
    })?;
    Ok(job)
}

pub fn remove(path: &Path, id: &str) -> Result<(), ScheduleError> {
    update(path, |state| {
        let before = state.jobs.len();
        state.jobs.retain(|job| job.id != id);
        if state.jobs.len() == before {
            return Err(ScheduleError::NotFound(id.to_string()));
        }
        Ok(())
    })
}

pub fn mark_run(
    path: &Path,
    id: &str,
    now_ms: u128,
    status: &str,
    journal: Option<&str>,
    trigger_mtime_ms: Option<u128>,
) -> Result<(), ScheduleError> {
    update(path, |state| {
        let job = state
            .jobs
            .iter_mut()
            .find(|job| job.id == id)
            .ok_or_else(|| ScheduleError::NotFound(id.to_string()))?;
        job.last_run_ms = Some(now_ms);
        job.last_status = Some(status.to_string());
        job.last_journal = journal.map(str::to_string);
        job.last_trigger_mtime_ms = trigger_mtime_ms.or(job.last_trigger_mtime_ms);
        job.next_run_ms = now_ms + u128::from(job.every_seconds) * 1000;
        Ok(())
    })
}

/// Return a changed trigger timestamp, or `None` when the trigger is absent or
/// has not changed since the last run.
pub fn trigger_mtime(job: &ScheduleJob) -> Option<u128> {
    let path = job.trigger_path.as_ref()?;
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let modified_ms = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    if job
        .last_trigger_mtime_ms
        .is_none_or(|last| modified_ms > last)
    {
        Some(modified_ms)
    } else {
        None
    }
}

pub fn is_due(job: &ScheduleJob, now_ms: u128) -> bool {
    job.next_run_ms <= now_ms || trigger_mtime(job).is_some()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JobHealth {
    pub id: String,
    pub name: String,
    pub state: String,
    pub last_status: Option<String>,
    pub next_run_ms: u128,
    pub overdue_ms: u128,
}

/// Read-only health snapshot for operators and watchdogs. `failing` takes
/// precedence over `overdue`; a job is overdue only after the grace window.
pub fn health(path: &Path, now_ms: u128, grace_ms: u128) -> Result<Vec<JobHealth>, ScheduleError> {
    Ok(load(path)?
        .jobs
        .into_iter()
        .map(|job| {
            let late_by = now_ms.saturating_sub(job.next_run_ms);
            let failing = matches!(job.last_status.as_deref(), Some("failed" | "timeout"));
            let state = if failing {
                "failing"
            } else if late_by > grace_ms {
                "overdue"
            } else {
                "ok"
            };
            JobHealth {
                id: job.id,
                name: job.name,
                state: state.into(),
                last_status: job.last_status,
                next_run_ms: job.next_run_ms,
                overdue_ms: if late_by > grace_ms { late_by } else { 0 },
            }
        })
        .collect())
}

pub fn due(path: &Path, now_ms: u128) -> Result<Vec<ScheduleJob>, ScheduleError> {
    Ok(load(path)?
        .jobs
        .into_iter()
        .filter(|job| is_due(job, now_ms))
        .collect())
}

pub fn next_due_ms(path: &Path) -> Result<Option<u128>, ScheduleError> {
    Ok(load(path)?.jobs.iter().map(|job| job.next_run_ms).min())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_job(path: &Path, name: &str, every: u64, start_now: bool, now: u128) -> ScheduleJob {
        add(
            path,
            name,
            &format!("task for {name}"),
            every,
            true,
            false,
            None,
            None,
            None,
            start_now,
            now,
        )
        .unwrap()
    }

    #[test]
    fn add_list_due_mark_and_remove_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schedules.json");
        let first = add_job(&path, "first", 60, true, 1000);
        let second = add_job(&path, "second", 120, false, 1000);
        assert_eq!(load(&path).unwrap().jobs.len(), 2);
        assert_eq!(due(&path, 1000).unwrap().len(), 1);
        assert_eq!(due(&path, 120_999).unwrap().len(), 1);
        assert_eq!(due(&path, 121_000).unwrap().len(), 2);

        mark_run(
            &path,
            &first.id,
            2000,
            "completed",
            Some("journal.jsonl"),
            None,
        )
        .unwrap();
        let state = load(&path).unwrap();
        let updated = state.jobs.iter().find(|job| job.id == first.id).unwrap();
        assert_eq!(updated.next_run_ms, 62_000);
        assert_eq!(updated.last_status.as_deref(), Some("completed"));

        remove(&path, &second.id).unwrap();
        assert_eq!(load(&path).unwrap().jobs.len(), 1);
        assert!(matches!(
            remove(&path, &second.id).unwrap_err(),
            ScheduleError::NotFound(_)
        ));
    }

    #[test]
    fn invalid_jobs_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schedules.json");
        assert!(
            add(
                &path, "", "task", 60, false, false, None, None, None, false, 1
            )
            .is_err()
        );
        assert!(add(&path, "x", "", 60, false, false, None, None, None, false, 1).is_err());
        assert!(
            add(
                &path, "x", "task", 0, false, false, None, None, None, false, 1
            )
            .is_err()
        );
        assert!(
            add(
                &path,
                "x",
                "task",
                60,
                false,
                false,
                None,
                None,
                Some("release".into()),
                false,
                1
            )
            .is_err()
        );
    }

    #[test]
    fn health_reports_ok_overdue_and_failing_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schedules.json");
        let job = add_job(&path, "watch", 60, true, 1000);

        let snapshot = health(&path, 1_000, 100).unwrap();
        assert_eq!(snapshot[0].state, "ok");
        assert_eq!(snapshot[0].overdue_ms, 0);

        let snapshot = health(&path, 2_000, 100).unwrap();
        assert_eq!(snapshot[0].state, "overdue");
        assert_eq!(snapshot[0].overdue_ms, 1_000);

        mark_run(&path, &job.id, 2_000, "failed", None, None).unwrap();
        let snapshot = health(&path, 2_100, 100).unwrap();
        assert_eq!(snapshot[0].state, "failing");
        assert_eq!(snapshot[0].last_status.as_deref(), Some("failed"));
    }

    #[test]
    fn trigger_file_changes_make_a_job_due() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schedules.json");
        let trigger = dir.path().join("trigger.txt");
        let job = add(
            &path,
            "triggered",
            "task",
            3600,
            true,
            false,
            None,
            Some(trigger.clone()),
            None,
            false,
            1000,
        )
        .unwrap();
        assert!(!is_due(&job, 1000));
        std::fs::write(&trigger, b"change").unwrap();
        assert!(trigger_mtime(&job).is_some());
        assert!(is_due(&job, 1000));
    }

    #[test]
    fn corrupt_state_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("schedules.json");
        std::fs::write(&path, b"{not json").unwrap();
        assert!(load(&path).is_err());
    }
}
