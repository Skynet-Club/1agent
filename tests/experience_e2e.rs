use std::process::Command;

use first_principles_agent::{
    ExperienceCandidate, ExperienceStatus, JournalEventKind, RunResult, StopReason, read_events,
};

#[test]
fn experience_candidate_cannot_activate_without_review_and_approval() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "verified lesson").unwrap();
    let journal_dir = workspace.path().join("journal");
    let store = workspace.path().join("experience");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let run_output = Command::new(binary)
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
        run_output.status.success(),
        "{}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    let run: RunResult = serde_json::from_slice(&run_output.stdout).unwrap();
    assert_eq!(run.stop_reason, StopReason::Completed);
    let source_events = read_events(&run.journal_path).unwrap();
    let evidence_seq = source_events
        .iter()
        .find(|event| matches!(&event.event, JournalEventKind::ToolObserved { .. }))
        .unwrap()
        .seq;

    let candidate_path = workspace.path().join("candidate.json");
    std::fs::write(
        &candidate_path,
        serde_json::to_vec(&serde_json::json!({
            "kind": "skill",
            "title": "Inspect verified evidence first",
            "content": "When a decision depends on a repository fact, inspect the cited source before deciding.",
            "scope": "repository decisions",
            "evidence_event_seqs": vec![evidence_seq],
            "sensitive": false,
            "supersedes": null
        }))
        .unwrap(),
    )
    .unwrap();
    let propose_output = Command::new(binary)
        .args([
            "experience",
            "propose",
            "--store",
            &store.display().to_string(),
            "--journal",
            &run.journal_path.display().to_string(),
            "--input",
            &candidate_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        propose_output.status.success(),
        "{}",
        String::from_utf8_lossy(&propose_output.stderr)
    );
    let candidate: ExperienceCandidate = serde_json::from_slice(&propose_output.stdout).unwrap();
    assert_eq!(candidate.status, ExperienceStatus::Draft);

    let premature_activation = Command::new(binary)
        .args([
            "experience",
            "activate",
            "--store",
            &store.display().to_string(),
            "--id",
            &candidate.id.to_string(),
        ])
        .output()
        .unwrap();
    assert!(!premature_activation.status.success());

    let review_output = Command::new(binary)
        .args([
            "experience",
            "review",
            "--store",
            &store.display().to_string(),
            "--id",
            &candidate.id.to_string(),
            "--actor",
            "test-owner",
            "--journal",
            &run.journal_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        review_output.status.success(),
        "review: {}",
        String::from_utf8_lossy(&review_output.stderr)
    );
    let verification = workspace.path().join("verification.json");
    std::fs::write(
        &verification,
        serde_json::to_vec(&serde_json::json!({
            "suite_id": "heldout-e2e-v1",
            "suite_version": "1",
            "case_ids": ["e2e-case-1"],
            "suite_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "training_group": "A",
            "evaluation_group": "B",
            "baseline_success_bps": 5000,
            "learned_success_bps": 6200,
            "baseline_unsupported_bps": 1000,
            "learned_unsupported_bps": 800,
            "cost_delta_bps": 150,
            "evidence": "held-out report"
        }))
        .unwrap(),
    )
    .unwrap();
    let verify_output = Command::new(binary)
        .args([
            "experience",
            "verify",
            "--store",
            &store.display().to_string(),
            "--id",
            &candidate.id.to_string(),
            "--input",
            &verification.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        verify_output.status.success(),
        "verify: {}",
        String::from_utf8_lossy(&verify_output.stderr)
    );
    let approve_output = Command::new(binary)
        .args([
            "experience",
            "approve",
            "--store",
            &store.display().to_string(),
            "--id",
            &candidate.id.to_string(),
            "--actor",
            "test-owner",
            "--journal",
            &run.journal_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        approve_output.status.success(),
        "approve: {}",
        String::from_utf8_lossy(&approve_output.stderr)
    );
    let activate_output = Command::new(binary)
        .args([
            "experience",
            "activate",
            "--store",
            &store.display().to_string(),
            "--id",
            &candidate.id.to_string(),
            "--journal",
            &run.journal_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        activate_output.status.success(),
        "activate: {}",
        String::from_utf8_lossy(&activate_output.stderr)
    );

    let list_output = Command::new(binary)
        .args([
            "experience",
            "list",
            "--store",
            &store.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(list_output.status.success());
    let listed: serde_json::Value = serde_json::from_slice(&list_output.stdout).unwrap();
    assert_eq!(
        listed[&candidate.id.to_string()]["status"],
        serde_json::Value::String("active".into())
    );

    let audit_output = Command::new(binary)
        .args([
            "experience",
            "audit",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        audit_output.status.success(),
        "audit: {}",
        String::from_utf8_lossy(&audit_output.stderr)
    );
    let audit: serde_json::Value = serde_json::from_slice(&audit_output.stdout).unwrap();
    assert_eq!(audit["ok"], serde_json::Value::Bool(true));
    assert_eq!(audit["candidate_count"], serde_json::json!(1));
    assert_eq!(audit["findings"], serde_json::json!([]));

    let audit_text = Command::new(binary)
        .args([
            "experience",
            "audit",
            "--store",
            &store.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(audit_text.status.success());
    let audit_text = String::from_utf8_lossy(&audit_text.stdout);
    assert!(audit_text.contains("experience audit passed"));
    assert!(audit_text.contains("active=1"));
    assert!(audit_text.contains("superseded=0"));
    assert!(audit_text.contains("chains=0"));
    assert!(audit_text.contains("depth=0"));
    assert!(audit_text.contains("expired_active=0"));
}
