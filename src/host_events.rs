//! Best-effort projection of Master journal events into the Workshop host-event log.
//!
//! The Master journal remains the authority. If `SKYTERM_HOST_EVENT_DIR` is absent or
//! the append fails, task execution must continue; the host event stream is an
//! observability surface, not a second state machine.

use crate::master::WorkItemStatus;
use crate::master_coordination::{MasterJournalEvent, MasterJournalEventKind, TaskRuntimeState};
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_EVENT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REASON_CHARS: usize = 512;

pub fn emit(event: &MasterJournalEvent) -> Result<(), String> {
    let Some(dir) = std::env::var_os("SKYTERM_HOST_EVENT_DIR").map(PathBuf::from) else {
        return Ok(());
    };
    emit_to(&dir, event)
}

pub(crate) fn emit_to(dir: &Path, event: &MasterJournalEvent) -> Result<(), String> {
    let Some(kind) = project(&event.kind, task(event)) else {
        return Ok(());
    };
    let value = render(event, kind, task(event))?;
    append(dir, &value)
}

fn task(event: &MasterJournalEvent) -> Option<&TaskRuntimeState> {
    event
        .task_id
        .as_deref()
        .and_then(|task_id| event.state.tasks.get(task_id))
}

fn project(kind: &MasterJournalEventKind, task: Option<&TaskRuntimeState>) -> Option<&'static str> {
    use MasterJournalEventKind::*;
    match kind {
        Initialized => Some("goal.proposed"),
        Approved => Some("goal.approved"),
        PlanRejected => Some("goal.cancelled"),
        Leased => Some("task.queued"),
        Submitted => Some("task.started"),
        Observed => match task.map(|task| task.status) {
            Some(WorkItemStatus::AwaitingInput) => Some("task.awaiting_input"),
            Some(WorkItemStatus::Blocked) => Some("task.blocked"),
            _ => None,
        },
        ReceiptRecorded => match task.map(|task| task.status) {
            Some(WorkItemStatus::ArtifactReady) => Some("task.artifact_ready"),
            Some(WorkItemStatus::Rejected) => Some("task.rejected"),
            _ => None,
        },
        Verified => Some("task.verified"),
        Rejected => Some("task.rejected"),
        RetryRequested | LeaseReclaimed => Some("task.queued"),
        TakenOver => Some("human_takeover.started"),
        TakeoverEnded => Some("human_takeover.ended"),
        GoalAccepted => Some("goal.accepted"),
        Cancelled if task.is_some() => Some("task.cancelled"),
        Cancelled => Some("goal.cancelled"),
        _ => None,
    }
}

fn render(
    event: &MasterJournalEvent,
    kind: &str,
    task: Option<&TaskRuntimeState>,
) -> Result<Value, String> {
    let conversation_id = std::env::var("SKYTERM_MASTER_CONVERSATION_ID")
        .ok()
        .filter(|value| safe_id(value))
        .unwrap_or_else(|| event.state.trace_id.clone());
    if !safe_id(&conversation_id) {
        return Err("conversation_id is not safe".into());
    }
    let goal_id = event.state.plan.goal_id.clone();
    if !safe_id(&goal_id) {
        return Err("goal_id is not safe".into());
    }
    let task_id = event.task_id.clone().filter(|value| safe_id(value));
    let attempt = task.map(|task| task.attempts.max(1));
    if (kind.starts_with("task.") || kind.starts_with("human_takeover."))
        && (task_id.is_none() || attempt.is_none())
    {
        return Err("task event requires task_id and attempt".into());
    }

    let mut payload = serde_json::Map::new();
    payload.insert(
        "journal_kind".into(),
        serde_json::to_value(&event.kind).unwrap_or(Value::Null),
    );
    payload.insert("seq".into(), json!(event.seq));
    payload.insert("plan_revision".into(), json!(event.state.plan.version));
    payload.insert(
        "reason".into(),
        json!(
            event
                .reason
                .chars()
                .take(MAX_REASON_CHARS)
                .collect::<String>()
        ),
    );
    if let Some(task) = task {
        payload.insert(
            "status".into(),
            serde_json::to_value(task.status).unwrap_or(Value::Null),
        );
    }

    let mut object = serde_json::Map::new();
    object.insert("schema".into(), json!("skynet.term.host-event/v1"));
    object.insert("event".into(), json!(kind));
    object.insert(
        "ts_ms".into(),
        json!(u64::try_from(event.timestamp_unix_ms).unwrap_or(u64::MAX)),
    );
    object.insert("conversation_id".into(), json!(conversation_id));
    object.insert("goal_id".into(), json!(goal_id));
    if let Some(task_id) = task_id {
        object.insert("task_id".into(), json!(task_id));
    }
    if let Some(attempt) = attempt {
        object.insert("attempt".into(), json!(attempt));
    }
    if let Some(session_id) = task_session_id(task) {
        object.insert("session_id".into(), json!(session_id));
    }
    object.insert("payload".into(), Value::Object(payload));
    Ok(Value::Object(object))
}

fn task_session_id(task: Option<&TaskRuntimeState>) -> Option<String> {
    let env_session_id = std::env::var("SKYTERM_SESSION_ID").ok();
    task_session_id_with(task, env_session_id.as_deref())
}

fn task_session_id_with(
    task: Option<&TaskRuntimeState>,
    env_session_id: Option<&str>,
) -> Option<String> {
    task.and_then(|task| task.execution_target.as_ref())
        .and_then(|target| target.session_id.as_deref())
        .filter(|value| safe_id(value))
        .map(str::to_string)
        .or_else(|| {
            env_session_id
                .filter(|value| safe_id(value))
                .map(str::to_string)
        })
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
}

fn append(dir: &Path, value: &Value) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }

    let lock_path = dir.join("host-events.lock");
    let lock = open_private(&lock_path)?;
    lock_file(&lock)?;

    let path = dir.join("host-events.jsonl");
    let rotated = dir.join("host-events.jsonl.1");
    if std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0) >= MAX_EVENT_BYTES {
        let _ = std::fs::rename(&path, &rotated);
    }

    let mut file = open_private(&path)?;
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_data())
        .map_err(|error| error.to_string())
}

fn open_private(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| error.to_string())
}

fn lock_file(file: &File) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result != 0 {
            return Err(format!(
                "cannot lock host event log: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_kinds_project_to_stable_host_events() {
        let mut task = TaskRuntimeState::default();
        for (status, kind, expected) in [
            (
                WorkItemStatus::ArtifactReady,
                MasterJournalEventKind::ReceiptRecorded,
                Some("task.artifact_ready"),
            ),
            (
                WorkItemStatus::Rejected,
                MasterJournalEventKind::ReceiptRecorded,
                Some("task.rejected"),
            ),
            (
                WorkItemStatus::AwaitingInput,
                MasterJournalEventKind::Observed,
                Some("task.awaiting_input"),
            ),
            (
                WorkItemStatus::Blocked,
                MasterJournalEventKind::Observed,
                Some("task.blocked"),
            ),
            (
                WorkItemStatus::Verified,
                MasterJournalEventKind::Verified,
                Some("task.verified"),
            ),
            (
                WorkItemStatus::Running,
                MasterJournalEventKind::ReceiptRecorded,
                None,
            ),
        ] {
            task.status = status;
            assert_eq!(project(&kind, Some(&task)), expected, "{status:?} {kind:?}");
        }
        assert_eq!(
            project(&MasterJournalEventKind::Approved, None),
            Some("goal.approved")
        );
        assert_eq!(
            project(&MasterJournalEventKind::PlanRejected, None),
            Some("goal.cancelled")
        );
        assert_eq!(
            project(&MasterJournalEventKind::TakeoverEnded, Some(&task)),
            Some("human_takeover.ended")
        );
        assert_eq!(
            project(&MasterJournalEventKind::GoalAccepted, None),
            Some("goal.accepted")
        );
    }

    #[test]
    fn task_session_id_prefers_the_execution_target() {
        let mut task = TaskRuntimeState {
            execution_target: Some(crate::master_coordination::ExecutionTarget {
                session_id: Some("term-1790000000000-1234-1".into()),
                pane_id: None,
                job_id: None,
            }),
            ..Default::default()
        };
        assert_eq!(
            task_session_id_with(Some(&task), None).as_deref(),
            Some("term-1790000000000-1234-1")
        );
        task.execution_target = Some(crate::master_coordination::ExecutionTarget {
            session_id: Some("bad/id".into()),
            pane_id: None,
            job_id: None,
        });
        assert_eq!(task_session_id_with(Some(&task), None), None);
        assert_eq!(task_session_id_with(None, None), None);
        assert_eq!(
            task_session_id_with(None, Some("term-env-1")).as_deref(),
            Some("term-env-1")
        );
        assert_eq!(task_session_id_with(None, Some("bad/id")), None);
    }

    #[test]
    fn host_event_log_is_private_and_append_only() {
        let dir = std::env::temp_dir().join(format!(
            "1agent-host-events-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        append(
            &dir,
            &json!({
                "schema": "skynet.term.host-event/v1",
                "event": "goal.approved"
            }),
        )
        .unwrap();
        append(
            &dir,
            &json!({
                "schema": "skynet.term.host-event/v1",
                "event": "task.started"
            }),
        )
        .unwrap();
        let raw = std::fs::read_to_string(dir.join("host-events.jsonl")).unwrap();
        assert_eq!(raw.lines().count(), 2);
        assert!(raw.contains("goal.approved"));
        assert!(raw.contains("task.started"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("host-events.jsonl"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
