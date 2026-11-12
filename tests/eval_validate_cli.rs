use std::process::Command;

#[test]
fn eval_validate_only_accepts_the_real_task_suite() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let input = concat!(env!("CARGO_MANIFEST_DIR"), "/evals/real-tasks-v1.json");
    let output = Command::new(binary)
        .args(["eval", "--suite", input, "--validate-only", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["cases"], 8);
    assert_eq!(report["kind"], "model_quality");
    assert_eq!(report["execution_status"], "ready");
}

#[test]
fn eval_validate_only_rejects_an_empty_model_quality_suite() {
    let workspace = tempfile::tempdir().unwrap();
    let path = workspace.path().join("empty.json");
    std::fs::write(
        &path,
        r#"{"name":"empty","kind":"model_quality","execution_status":"ready","provider_requirements":["real"],"reviewer":"human","calibration_required":true,"cases":[]}"#,
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "eval",
            "--suite",
            &path.display().to_string(),
            "--validate-only",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no cases"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
