use std::process::Command;

#[test]
fn provider_inspection_never_prints_secret_values() {
    let workspace = tempfile::tempdir().unwrap();
    let config = workspace.path().join("providers.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({
            "providers": [{
                "id": "remote",
                "adapter": "open_ai_compatible",
                "model": "model-1",
                "base_url": "https://example.invalid/v1",
                "api_key_env": "DEMO_API_KEY",
                "privacy": "remote",
                "quality_score": 10
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "providers",
            "--config",
            &config.display().to_string(),
            "--json",
        ])
        .env("DEMO_API_KEY", "do-not-print-this-secret")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("DEMO_API_KEY"));
    assert!(!stdout.contains("do-not-print-this-secret"));
}
