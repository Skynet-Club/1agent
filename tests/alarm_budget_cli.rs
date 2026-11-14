use std::process::Command;

fn run(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("alarm-budget")
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
fn alarm_budget_cli_delivers_digests_and_bypasses_for_critical() {
    assert_eq!(
        run(&["--urgency", "50", "--delivered", "1"])["decision"],
        "deliver"
    );
    assert_eq!(
        run(&["--urgency", "50", "--delivered", "20"])["decision"],
        "digest"
    );
    let noisy = run(&[
        "--urgency",
        "50",
        "--delivered",
        "10",
        "--false-alarms",
        "4",
        "--max-false-alarm-bps",
        "2000",
    ]);
    assert_eq!(noisy["decision"], "digest");
    assert_eq!(noisy["false_alarm_rate_bps"], 4000);
    assert_eq!(
        run(&[
            "--urgency",
            "95",
            "--delivered",
            "100",
            "--false-alarms",
            "100",
        ])["decision"],
        "deliver"
    );
}

#[test]
fn alarm_budget_cli_rejects_out_of_range_input() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    for args in [
        vec!["alarm-budget", "--urgency", "101"],
        vec!["alarm-budget", "--critical-urgency", "101"],
        vec!["alarm-budget", "--max-false-alarm-bps", "10001"],
    ] {
        let output = Command::new(bin).args(args).output().unwrap();
        assert!(!output.status.success());
    }
}
