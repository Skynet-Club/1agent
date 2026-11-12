use std::process::Command;

#[test]
fn golden_check_validates_the_shipped_set() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let input = concat!(env!("CARGO_MANIFEST_DIR"), "/evals/golden-set-v1.jsonl");
    let output = Command::new(binary)
        .args(["golden-check", "--input", input, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["count"], 40);
    assert_eq!(report["categories"]["chat"], 8);
    assert_eq!(report["categories"]["fact"], 8);
    assert_eq!(report["categories"]["decision"], 8);
    assert_eq!(report["categories"]["research"], 8);
    assert_eq!(report["categories"]["execution"], 8);
}

#[test]
fn golden_check_rejects_an_invalid_set() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("bad.jsonl");
    std::fs::write(
        &input,
        r#"{"id":"one","category":"chat","form":"direct","prompt":"hello"}"#,
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args(["golden-check", "--input", &input.display().to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("30..=50"), "stderr: {stderr}");
}
