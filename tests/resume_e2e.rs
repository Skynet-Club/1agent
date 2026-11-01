use std::process::Command;

use first_principles_agent::{
    Journal, JournalAudit, JournalEventKind, RunResult, StopReason, read_events,
};
use uuid::Uuid;

#[test]
fn cli_resumes_interrupted_journal_from_last_checkpoint() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "verified resume evidence").unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let complete_output = Command::new(binary)
        .args([
            "run",
            "--task",
            &format!("Research @read-file:\"{}\"", evidence.display()),
            "--read-root",
            &workspace.path().display().to_string(),
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        complete_output.status.success(),
        "{}",
        String::from_utf8_lossy(&complete_output.stderr)
    );
    let complete: RunResult = serde_json::from_slice(&complete_output.stdout).unwrap();
    assert_eq!(complete.stop_reason, StopReason::Completed);

    let events = read_events(&complete.journal_path).unwrap();
    let checkpoint_index = events
        .iter()
        .position(|event| matches!(event.event, JournalEventKind::Checkpoint { .. }))
        .unwrap();
    let interrupted_path = workspace.path().join("interrupted.jsonl");
    let mut interrupted = Journal::create(&interrupted_path, Uuid::new_v4()).unwrap();
    for event in &events[..=checkpoint_index] {
        interrupted.append(event.event.clone()).unwrap();
    }
    drop(interrupted);

    let status_output = Command::new(binary)
        .args([
            "resume-status",
            "--journal",
            &interrupted_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        status_output.status.success(),
        "{}",
        String::from_utf8_lossy(&status_output.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status_output.stdout).unwrap();
    assert_eq!(status["route_aware"], true);
    assert_eq!(status["legacy"], false);
    assert_eq!(status["route_objective"], "quality");
    assert!(status["route_plan_sha256"].is_string());

    let conflicting_output = Command::new(binary)
        .args([
            "resume",
            "--journal",
            &interrupted_path.display().to_string(),
            "--journal-dir",
            &workspace.path().join("conflict").display().to_string(),
            "--route-objective",
            "cost",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!conflicting_output.status.success());
    assert!(
        String::from_utf8_lossy(&conflicting_output.stderr)
            .contains("route objective changed since checkpoint")
    );

    let resumed_output = Command::new(binary)
        .args([
            "resume",
            "--journal",
            &interrupted_path.display().to_string(),
            "--journal-dir",
            &workspace.path().join("resumed").display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        resumed_output.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed_output.stderr)
    );
    let resumed: RunResult = serde_json::from_slice(&resumed_output.stdout).unwrap();
    assert_eq!(resumed.stop_reason, complete.stop_reason);
    assert_eq!(resumed.answer.conclusion, complete.answer.conclusion);
    assert_eq!(resumed.answer.evidence, complete.answer.evidence);

    let audit_output = Command::new(binary)
        .args([
            "audit",
            "--journal",
            &resumed.journal_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        audit_output.status.success(),
        "{}",
        String::from_utf8_lossy(&audit_output.stderr)
    );
    let audit: JournalAudit = serde_json::from_slice(&audit_output.stdout).unwrap();
    assert!(audit.ok, "{:?}", audit.issues);
    assert!(audit.findings.is_empty());
}
