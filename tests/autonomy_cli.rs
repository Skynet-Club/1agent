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
fn autonomy_cli_defers_proactive_work_in_quiet_hours() {
    assert_eq!(
        run(&[
            "--level",
            "auto",
            "--risk",
            "low",
            "--reversible",
            "--quiet",
            "22:00-07:00",
            "--now",
            "23:30",
        ])["decision"],
        "defer"
    );
    assert_eq!(
        run(&[
            "--level",
            "auto",
            "--risk",
            "low",
            "--reversible",
            "--quiet",
            "22:00-07:00",
            "--now",
            "23:30",
            "--user-initiated",
        ])["decision"],
        "act"
    );
    assert_eq!(
        run(&[
            "--level",
            "auto",
            "--risk",
            "low",
            "--reversible",
            "--quiet",
            "22:00-07:00",
            "--now",
            "12:00",
        ])["decision"],
        "act"
    );
    assert_eq!(
        run(&[
            "--level",
            "auto",
            "--risk",
            "high",
            "--quiet",
            "22:00-07:00",
            "--now",
            "23:30",
            "--explicit-approval",
        ])["decision"],
        "act"
    );

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let bad = Command::new(bin)
        .args([
            "autonomy", "--level", "auto", "--quiet", "bedtime", "--now", "23:30",
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}

#[test]
fn autonomy_for_reads_the_user_model_and_quiet_hours() {
    let workspace = tempfile::tempdir().unwrap();
    let journal = workspace.path().join("memory-journal.jsonl");
    let payloads = workspace.path().join("payloads");
    let key = workspace.path().join("memory.key");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let onboard = Command::new(bin)
        .args(["onboard", "--memory-journal"])
        .arg(&journal)
        .arg("--memory-payloads")
        .arg(&payloads)
        .arg("--memory-key")
        .arg(&key)
        .args([
            "--bot-id",
            "bot-1",
            "--autonomy",
            "workspace=act",
            "--quiet-hours",
            "22:00-07:00",
        ])
        .output()
        .unwrap();
    assert!(onboard.status.success());

    let base = [
        "autonomy-for",
        "--journal",
        journal.to_str().unwrap(),
        "--payloads",
        payloads.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
        "--bot-id",
        "bot-1",
        "--domain",
        "workspace",
        "--risk",
        "low",
        "--reversible",
        "--json",
    ];
    let deferred = Command::new(bin)
        .args(base)
        .args(["--now", "23:30"])
        .output()
        .unwrap();
    assert!(
        deferred.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&deferred.stderr)
    );
    let plan: serde_json::Value = serde_json::from_slice(&deferred.stdout).unwrap();
    assert_eq!(plan["decision"], "defer");
    assert!(plan["reason"].as_str().unwrap().contains("quiet hours"));

    for args in [
        &["--now", "12:00"][..],
        &["--now", "23:30", "--user-initiated"][..],
    ] {
        let output = Command::new(bin).args(base).args(args).output().unwrap();
        assert!(output.status.success());
        let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(plan["decision"], "act");
    }
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
