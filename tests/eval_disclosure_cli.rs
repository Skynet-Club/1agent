use std::process::Command;

#[test]
fn eval_writes_a_trust_center_disclosure() {
    let workspace = tempfile::tempdir().unwrap();
    let disclosure_path = workspace.path().join("eval-disclosure.json");
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let eval = Command::new(bin)
        .args(["eval", "--suite", &suite, "--disclosure-out"])
        .arg(&disclosure_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        eval.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&eval.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&eval.stdout).unwrap();
    let disclosure: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&disclosure_path).unwrap()).unwrap();
    assert_eq!(disclosure["name"], report["suite"]);
    assert_eq!(disclosure["passed"], report["passed"]);
    assert_eq!(disclosure["total"], report["total"]);
    let digest = disclosure["evidence_sha256"].as_str().unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let trust = Command::new(bin)
        .args(["trust-center", "--evaluation"])
        .arg(&disclosure_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        trust.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&trust.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(&trust.stdout).unwrap();
    assert_eq!(manifest["evaluations"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["evaluations"][0]["name"], report["suite"]);
}
