//! Black-box CLI tests for `1agent action show|verify`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use first_principles_agent::ActionLogJournal;
use first_principles_agent::action_log::{ActionKind, ActionRecord, ActionStatus};
use first_principles_agent::authority::PolicyOutcome;
use first_principles_agent::domain::{
    CognitiveMode, RiskLevel, StopReason, TaskRequest, ToolCall, ToolObservation, ToolStatus,
};
use first_principles_agent::journal::{Journal, JournalEventKind};
use serde_json::{Value, json};
use uuid::Uuid;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {args:?}: {error}"))
}

fn parse_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not JSON: {error}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn record(run_id: &str, status: ActionStatus, policy: PolicyOutcome) -> ActionRecord {
    ActionRecord {
        action_id: format!("{run_id}:call-1"),
        run_id: run_id.into(),
        session_id: None,
        action_type: "write_file".into(),
        risk: RiskLevel::Medium,
        reversible: false,
        policy_outcome: policy,
        policy_reason: Some("synthetic".into()),
        authority_profile_id: Some("profile-1".into()),
        approval_card_id: None,
        status,
        artifact_sha256: None,
    }
}

fn gated_workflow(
    dir: &Path,
    tool_status: ToolStatus,
    record_status: ActionStatus,
    policy: PolicyOutcome,
) -> (PathBuf, PathBuf, String) {
    let run_id = Uuid::new_v4();
    let journal_path = dir.join("run.jsonl");
    let mut journal = Journal::create(&journal_path, run_id).unwrap();
    journal
        .append(JournalEventKind::RunStarted {
            request: TaskRequest::from_objective("write", dir.to_path_buf()),
            mode: CognitiveMode::Execute,
        })
        .unwrap();
    journal
        .append(JournalEventKind::ToolCalled {
            call: ToolCall {
                id: "call-1".into(),
                name: "write_file".into(),
                arguments: json!({"path": "/tmp/x", "content": "x"}),
            },
        })
        .unwrap();
    let mut metadata = BTreeMap::from([
        ("action_id".into(), format!("{run_id}:call-1")),
        ("policy_outcome".into(), "allow".into()),
    ]);
    if tool_status == ToolStatus::PermissionDenied {
        metadata.insert("policy_outcome".into(), "deny".into());
    }
    journal
        .append(JournalEventKind::ToolObserved {
            observation: ToolObservation {
                call_id: "call-1".into(),
                tool: "write_file".into(),
                status: tool_status,
                summary: "synthetic".into(),
                content: "{}".into(),
                error_code: (tool_status == ToolStatus::PermissionDenied)
                    .then(|| "authority_denied".into()),
                metadata,
            },
        })
        .unwrap();
    journal
        .append(JournalEventKind::RunStopped {
            reason: StopReason::Completed,
        })
        .unwrap();
    drop(journal);

    let log_path = dir.join("action-records.jsonl");
    let mut log = ActionLogJournal::open(&log_path).unwrap();
    log.log_mut()
        .record_action(
            "owner",
            ActionKind::ToolCall,
            "write_file",
            record_status.as_str(),
            "synthetic",
            record(&run_id.to_string(), record_status, policy),
            1,
        )
        .unwrap();
    log.persist().unwrap();
    (journal_path, log_path, run_id.to_string())
}

fn plain_journal(dir: &Path) -> PathBuf {
    let run_id = Uuid::new_v4();
    let path = dir.join("plain.jsonl");
    let mut journal = Journal::create(&path, run_id).unwrap();
    journal
        .append(JournalEventKind::RunStarted {
            request: TaskRequest::from_objective("read", dir.to_path_buf()),
            mode: CognitiveMode::DirectAnswer,
        })
        .unwrap();
    journal
        .append(JournalEventKind::ToolCalled {
            call: ToolCall {
                id: "call-1".into(),
                name: "read_file".into(),
                arguments: json!({"path": "/tmp/x"}),
            },
        })
        .unwrap();
    journal
        .append(JournalEventKind::ToolObserved {
            observation: ToolObservation {
                call_id: "call-1".into(),
                tool: "read_file".into(),
                status: ToolStatus::Failed,
                summary: "no file".into(),
                content: String::new(),
                error_code: Some("tool_failed".into()),
                metadata: BTreeMap::new(),
            },
        })
        .unwrap();
    journal
        .append(JournalEventKind::RunStopped {
            reason: StopReason::Completed,
        })
        .unwrap();
    path
}

#[test]
fn action_show_and_verify_pass_for_matching_records() {
    let dir = tempfile::tempdir().unwrap();
    let (journal, log, run_id) = gated_workflow(
        dir.path(),
        ToolStatus::Ok,
        ActionStatus::Executed,
        PolicyOutcome::Allow,
    );

    let show = run(&["action", "show", "--log", log.to_str().unwrap(), "--json"]);
    let shown = parse_stdout(&show);
    assert!(show.status.success(), "{shown}");
    assert_eq!(shown["ok"], true);
    assert_eq!(shown["count"], 1);
    assert_eq!(shown["actions"][0]["action_id"], format!("{run_id}:call-1"));
    assert_eq!(shown["actions"][0]["status"], "executed");

    let verify = run(&[
        "action",
        "verify",
        "--journal",
        journal.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
        "--json",
    ]);
    let report = parse_stdout(&verify);
    assert!(verify.status.success(), "{report}");
    assert_eq!(report["ok"], true);
    assert_eq!(report["checked"], 1);
}

#[test]
fn action_verify_fails_when_the_record_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let (journal, log, _) = gated_workflow(
        dir.path(),
        ToolStatus::Ok,
        ActionStatus::Executed,
        PolicyOutcome::Allow,
    );
    let empty_log = dir.path().join("empty-records.jsonl");
    ActionLogJournal::open(&empty_log).unwrap();

    let verify = run(&[
        "action",
        "verify",
        "--journal",
        journal.to_str().unwrap(),
        "--log",
        empty_log.to_str().unwrap(),
        "--json",
    ]);
    let report = parse_stdout(&verify);
    assert!(!verify.status.success());
    assert_eq!(report["ok"], false);
    assert_eq!(report["issues"][0]["kind"], "missing_action_record");
    assert!(log.exists());
}

#[test]
fn action_verify_fails_on_status_or_policy_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let (journal, log, _) = gated_workflow(
        dir.path(),
        ToolStatus::PermissionDenied,
        ActionStatus::Executed,
        PolicyOutcome::Allow,
    );

    let verify = run(&[
        "action",
        "verify",
        "--journal",
        journal.to_str().unwrap(),
        "--log",
        log.to_str().unwrap(),
        "--json",
    ]);
    let report = parse_stdout(&verify);
    assert!(!verify.status.success());
    let kinds = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["kind"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"status_mismatch".to_string()), "{kinds:?}");
    assert!(kinds.contains(&"policy_mismatch".to_string()), "{kinds:?}");
}

#[test]
fn non_gated_journal_verifies_against_an_empty_log() {
    let dir = tempfile::tempdir().unwrap();
    let journal = plain_journal(dir.path());
    let empty_log = dir.path().join("empty-records.jsonl");
    ActionLogJournal::open(&empty_log).unwrap();

    let verify = run(&[
        "action",
        "verify",
        "--journal",
        journal.to_str().unwrap(),
        "--log",
        empty_log.to_str().unwrap(),
        "--json",
    ]);
    let report = parse_stdout(&verify);
    assert!(verify.status.success(), "{report}");
    assert_eq!(report["ok"], true);
    assert_eq!(report["checked"], 0);
}

#[test]
fn action_show_rejects_a_corrupt_log() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("broken.jsonl");
    std::fs::write(&log, b"not-json\n").unwrap();
    let output = run(&["action", "show", "--log", log.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("action log"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
