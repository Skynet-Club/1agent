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

#[test]
fn cli_wires_authority_gate_health_and_alert_log() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "verified authority evidence").unwrap();
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
    let authority = workspace.path().join("authority.json");
    std::fs::write(
        &authority,
        serde_json::to_vec_pretty(&serde_json::json!({
            "profiles": [{
                "profile_id": "operator-profile",
                "actor_id": "operator",
                "scopes": ["workspace*"],
                "max_strength": "read_only",
                "allowed_tools": ["read_file"],
                "denied_tools": [],
                "max_risk": "low"
            }],
            "bindings": [{"actor_id": "operator", "profile_id": "operator-profile"}]
        }))
        .unwrap(),
    )
    .unwrap();
    let alert_log = workspace.path().join("alerts.jsonl");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args(["run", "--config", &config.display().to_string()])
        .args([
            "--task",
            &format!("Research @read-file:\"{}\"", evidence.display()),
            "--read-root",
            &workspace.path().display().to_string(),
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
            "--authority",
            &authority.display().to_string(),
            "--actor-id",
            "operator",
            "--authority-scope",
            "workspace",
            "--provider-health",
            "--alert-log",
            &alert_log.display().to_string(),
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
}

#[test]
fn cli_rejects_insecure_alert_webhook() {
    let workspace = tempfile::tempdir().unwrap();
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
        .args(["run", "--config", &config.display().to_string()])
        .args([
            "--task",
            "Compare options",
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
            "--alert-webhook",
            "http://example.com/alerts",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("https"));
}
