//! End-to-end coverage for `1agent skill capture`.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use first_principles_agent::{
    CognitiveMode, JournalEvent, JournalEventKind, StopReason, TaskRequest, ToolCall,
    ToolObservation, ToolStatus,
};
use serde_json::json;
use uuid::Uuid;

fn event(run_id: Uuid, seq: u64, event: JournalEventKind) -> JournalEvent {
    JournalEvent {
        schema_version: 1,
        run_id,
        seq,
        timestamp_unix_ms: seq as u128,
        event,
    }
}

fn write_journal(path: &Path, events: &[JournalEvent]) {
    let mut file = std::fs::File::create(path).unwrap();
    for event in events {
        writeln!(file, "{}", serde_json::to_string(event).unwrap()).unwrap();
    }
}

fn completed_journal(path: &Path) {
    let run_id = Uuid::new_v4();
    let events = vec![
        event(
            run_id,
            0,
            JournalEventKind::RunStarted {
                request: TaskRequest::from_objective(
                    "Summarize /home/alice/project with ada@example.com",
                    Path::new("/tmp").to_path_buf(),
                ),
                mode: CognitiveMode::Execute,
            },
        ),
        event(
            run_id,
            1,
            JournalEventKind::ToolCalled {
                call: ToolCall {
                    id: "call-1".into(),
                    name: "read_file".into(),
                    arguments: json!({
                        "path": "/home/alice/project/README.md",
                        "token": "super-secret-token"
                    }),
                },
            },
        ),
        event(
            run_id,
            2,
            JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "call-1".into(),
                    tool: "read_file".into(),
                    status: ToolStatus::Ok,
                    summary: "read".into(),
                    content: "never capture this body".into(),
                    error_code: None,
                    metadata: BTreeMap::new(),
                },
            },
        ),
        event(
            run_id,
            3,
            JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            },
        ),
    ];
    write_journal(path, &events);
}

#[test]
fn capture_writes_a_scannable_draft_without_the_run_body() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("run.jsonl");
    let out = dir.path().join("SKILL.md");
    completed_journal(&journal);

    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "capture", "--journal"])
        .arg(&journal)
        .args([
            "--name",
            "summarize-project",
            "--version",
            "1.0.0",
            "--description",
            "Summarize a project README",
            "--scope",
            "repo:test",
            "--out",
        ])
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["steps"], 1);
    assert!(report["redactions"].as_u64().unwrap() >= 1);
    assert!(report["generalizations"].as_u64().unwrap() >= 2);

    let draft = std::fs::read_to_string(&out).unwrap();
    assert!(draft.contains("$HOME/project/README.md"));
    assert!(draft.contains("<redacted>"));
    assert!(draft.contains("<email>"));
    assert!(!draft.contains("/home/alice"));
    assert!(!draft.contains("super-secret-token"));
    assert!(!draft.contains("never capture this body"));

    let scan = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "scan", "--path"])
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        scan.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let scan_report: serde_json::Value = serde_json::from_slice(&scan.stdout).unwrap();
    assert_eq!(scan_report["ok"], true);
}

#[test]
fn capture_refuses_to_overwrite_unless_forced() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("run.jsonl");
    let out = dir.path().join("SKILL.md");
    completed_journal(&journal);
    std::fs::write(&out, "do not replace").unwrap();

    let blocked = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "capture", "--journal"])
        .arg(&journal)
        .args([
            "--name",
            "capture",
            "--version",
            "1.0.0",
            "--description",
            "Captured procedure",
            "--out",
        ])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "do not replace");

    let forced = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "capture", "--journal"])
        .arg(&journal)
        .args([
            "--name",
            "capture",
            "--version",
            "1.0.0",
            "--description",
            "Captured procedure",
            "--out",
        ])
        .arg(&out)
        .arg("--force")
        .output()
        .unwrap();
    assert!(
        forced.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(std::fs::read_to_string(&out).unwrap().starts_with("---\n"));
}

#[test]
fn capture_rejects_an_incomplete_run() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("run.jsonl");
    let out = dir.path().join("SKILL.md");
    let run_id = Uuid::new_v4();
    write_journal(
        &journal,
        &[
            event(
                run_id,
                0,
                JournalEventKind::RunStarted {
                    request: TaskRequest::from_objective("task", Path::new("/tmp").to_path_buf()),
                    mode: CognitiveMode::Execute,
                },
            ),
            event(
                run_id,
                1,
                JournalEventKind::RunStopped {
                    reason: StopReason::Cancelled,
                },
            ),
        ],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["skill", "capture", "--journal"])
        .arg(&journal)
        .args([
            "--name",
            "capture",
            "--version",
            "1.0.0",
            "--description",
            "Captured procedure",
            "--out",
        ])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requires a completed run"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out.exists());
}
