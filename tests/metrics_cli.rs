use std::process::Command;

use first_principles_agent::{Journal, JournalEventKind, RunResult, StopReason, read_events};
use uuid::Uuid;

#[test]
fn metrics_summarize_completed_and_interrupted_journals() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "verified evidence").unwrap();
    let journal_dir = workspace.path().join("journals");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "run",
            "--task",
            &format!("Research @read-file:\"{}\"", evidence.display()),
            "--read-root",
            &workspace.path().display().to_string(),
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(run.stop_reason, StopReason::Completed);

    let events = read_events(&run.journal_path).unwrap();
    let cutoff = events
        .iter()
        .position(|event| matches!(event.event, JournalEventKind::ToolObserved { .. }))
        .unwrap();
    let interrupted_path = journal_dir.join("interrupted.jsonl");
    let mut interrupted = Journal::create(&interrupted_path, Uuid::new_v4()).unwrap();
    for event in &events[..=cutoff] {
        interrupted.append(event.event.clone()).unwrap();
    }
    drop(interrupted);

    let report_output = Command::new(binary)
        .args([
            "metrics",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        report_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&report_output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&report_output.stdout).unwrap();
    assert_eq!(report["journals"], 2);
    assert_eq!(report["runs"], 2);
    assert_eq!(report["completed"], 1);
    assert_eq!(report["interrupted"], 1);
    assert_eq!(report["replay_total"], 1);
    assert_eq!(report["replay_bps"], 10_000);
    assert_eq!(report["tool_success_bps"], 10_000);
    assert_eq!(report["audit_complete_bps"], 10_000);
    assert!(report["run_latency"]["p50_ms"].is_number());
    assert!(report["route_latency"]["p50_ms"].is_number());
    assert!(report["context_latency"]["p50_ms"].is_number());
    assert!(report["model_latency"]["p50_ms"].is_number());
    assert!(report["tool_latency"]["p50_ms"].is_number());

    let human = Command::new(binary)
        .args([
            "metrics",
            "--journal-dir",
            &journal_dir.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(human.status.success());
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("journals=2"), "stdout: {stdout}");
    assert!(stdout.contains("recovery="), "stdout: {stdout}");
    assert!(stdout.contains("route latency ms:"), "stdout: {stdout}");
    assert!(stdout.contains("context latency ms:"), "stdout: {stdout}");
    assert!(stdout.contains("model latency ms:"), "stdout: {stdout}");
    assert!(stdout.contains("tool latency ms:"), "stdout: {stdout}");
}
