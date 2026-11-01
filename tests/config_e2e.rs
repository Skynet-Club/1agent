use std::process::Command;

use first_principles_agent::{RunResult, StopReason};

#[test]
fn config_file_builds_provider_candidates_without_keys() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "verified config evidence").unwrap();
    let config = workspace.path().join("agent.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({
            "providers": [{
                "id": "deterministic",
                "adapter": "deterministic",
                "model": "offline-v1",
                "quality_score": 1,
                "max_risk": "high"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "run",
            "--config",
            &config.display().to_string(),
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
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result.stop_reason, StopReason::Completed);
    assert_eq!(result.answer.claims[0].id, "claim-1");
}
