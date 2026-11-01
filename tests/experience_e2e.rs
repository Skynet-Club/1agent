use std::process::Command;

use first_principles_agent::{ExperienceCandidate, ExperienceStatus, RunResult, StopReason};

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

    let candidate_path = workspace.path().join("candidate.json");
    std::fs::write(
        &candidate_path,
        serde_json::to_vec(&serde_json::json!({
            "kind": "skill",
            "title": "Inspect verified evidence first",
            "content": "When a decision depends on a repository fact, inspect the cited source before deciding.",
            "scope": "repository decisions",
            "evidence_event_seqs": [2],
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

    for action in ["review", "approve"] {
        let output = Command::new(binary)
            .args([
                "experience",
                action,
                "--store",
                &store.display().to_string(),
                "--id",
                &candidate.id.to_string(),
                "--actor",
                "test-owner",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{action}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let activate_output = Command::new(binary)
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
}
