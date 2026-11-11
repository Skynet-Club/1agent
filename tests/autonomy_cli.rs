use std::process::Command;

fn run(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("autonomy")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn autonomy_cli_applies_risk_and_reversibility() {
    assert_eq!(
        run(&["--level", "act", "--risk", "low", "--reversible"])["decision"],
        "act"
    );
    assert_eq!(
        run(&["--level", "act", "--risk", "medium", "--reversible"])["decision"],
        "require_approval"
    );
    assert_eq!(
        run(&["--level", "auto", "--risk", "medium", "--reversible"])["decision"],
        "act"
    );
    assert_eq!(
        run(&["--level", "auto", "--risk", "low"])["decision"],
        "require_approval"
    );
    assert_eq!(
        run(&[
            "--level",
            "observe",
            "--risk",
            "low",
            "--reversible",
            "--explicit-approval",
        ])["decision"],
        "observe"
    );
}

#[test]
fn autonomy_cli_rejects_invalid_level_and_risk() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let bad_level = Command::new(bin)
        .args(["autonomy", "--level", "robot"])
        .output()
        .unwrap();
    assert!(!bad_level.status.success());
    let bad_risk = Command::new(bin)
        .args(["autonomy", "--risk", "extreme"])
        .output()
        .unwrap();
    assert!(!bad_risk.status.success());
}
