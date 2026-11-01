use std::process::Command;

use first_principles_agent::{JournalAudit, ReplayState, RunResult, StopReason};

#[test]
fn cli_run_audit_and_replay_form_a_complete_flywheel() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence_dir = workspace.path().join("evidence dir");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    let evidence = evidence_dir.join("decision.txt");
    std::fs::write(
        &evidence,
        "Verified fact: option A has lower total cost under the stated constraints.",
    )
    .unwrap();
    let journal_dir = workspace.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let output = Command::new(binary)
        .args([
            "run",
            "--task",
            &format!(
                "Compare option A and B using @read-file:\"{}\"",
                evidence.display()
            ),
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
    let result: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result.stop_reason, StopReason::Completed);
    assert_eq!(result.answer.evidence.len(), 1);
    assert!(result.journal_path.exists());

    let audit_output = Command::new(binary)
        .args([
            "audit",
            "--journal",
            &result.journal_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        audit_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&audit_output.stderr)
    );
    let audit: JournalAudit = serde_json::from_slice(&audit_output.stdout).unwrap();
    assert!(audit.ok, "{:?}", audit.issues);

    let replay_output = Command::new(binary)
        .args([
            "replay",
            "--journal",
            &result.journal_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        replay_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&replay_output.stderr)
    );
    let replay: ReplayState = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(replay.stop_reason, Some(StopReason::Completed));
    assert_eq!(replay.tool_calls.len(), 1);
    assert_eq!(replay.observations.len(), 1);
}
