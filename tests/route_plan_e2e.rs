use std::process::Command;

#[test]
fn route_plan_is_offline_deterministic_and_explains_rejections() {
    let workspace = tempfile::tempdir().unwrap();
    let config = workspace.path().join("providers.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({
            "providers": [
                {
                    "id": "deterministic",
                    "adapter": "deterministic",
                    "model": "offline-v1",
                    "privacy": "local",
                    "quality_score": 1,
                    "max_risk": "high"
                },
                {
                    "id": "remote",
                    "adapter": "open_ai_compatible",
                    "model": "remote-model",
                    "base_url": "https://example.invalid/v1",
                    "privacy": "remote",
                    "quality_score": 100,
                    "max_risk": "high"
                },
                {
                    "id": "no-tools",
                    "adapter": "open_ai_compatible",
                    "model": "no-tools-v1",
                    "base_url": "http://127.0.0.1:11434/v1",
                    "privacy": "local",
                    "quality_score": 200,
                    "max_risk": "high",
                    "supports_tools": false
                },
                {
                    "id": "expensive",
                    "adapter": "open_ai_compatible",
                    "model": "expensive-model",
                    "base_url": "http://127.0.0.1:11435/v1",
                    "privacy": "local",
                    "quality_score": 0,
                    "input_cost_micros_per_k": 100,
                    "p95_latency_ms": 1000,
                    "max_risk": "high"
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "route-plan",
            "--task",
            "Research evidence from the supplied source",
            "--config",
            &config.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["mode"], "evidence_research");
    assert_eq!(
        report["plan"]["ranked"][0]["manifest"]["provider_id"],
        "deterministic"
    );
    let rejected = report["plan"]["rejected"].as_array().unwrap();
    assert!(rejected.iter().any(|candidate| {
        candidate["provider_id"] == "remote"
            && candidate["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason == "remote_disallowed")
    }));
    assert!(rejected.iter().any(|candidate| {
        candidate["provider_id"] == "no-tools"
            && candidate["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason == "tools_unsupported")
    }));
    assert!(
        report["readiness"]
            .as_array()
            .unwrap()
            .iter()
            .all(|provider| provider["ready"] == true)
    );

    let budgeted = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "route-plan",
            "--task",
            "Research evidence from the supplied source",
            "--config",
            &config.display().to_string(),
            "--max-provider-p95-latency-ms",
            "100",
            "--max-unit-cost-micros-per-k",
            "50",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(budgeted.status.success());
    let budgeted: serde_json::Value = serde_json::from_slice(&budgeted.stdout).unwrap();
    let rejected = budgeted["plan"]["rejected"].as_array().unwrap();
    assert!(rejected.iter().any(|candidate| {
        candidate["provider_id"] == "expensive"
            && candidate["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason == "provider_p95_latency_exceeded")
            && candidate["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .any(|reason| reason == "unit_cost_exceeded")
    }));
}
