use std::process::Command;

fn check(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("budget-check")
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
fn budget_check_reports_ok_near_limit_and_unit_cost() {
    let ok = check(&[
        "--cost-micros",
        "500",
        "--max-cost-micros",
        "1000",
        "--tokens",
        "500",
        "--max-tokens",
        "1000",
        "--tool-calls",
        "5",
        "--max-tool-calls",
        "10",
        "--verified-outcomes",
        "4",
    ]);
    assert_eq!(ok["verdict"]["status"], "ok");
    assert_eq!(ok["verdict"]["allowed"], true);
    assert_eq!(ok["cost_per_verified_outcome"], 125);

    let near = check(&["--cost-micros", "800", "--max-cost-micros", "1000"]);
    assert_eq!(near["verdict"]["status"], "near_limit");
    assert_eq!(near["verdict"]["allowed"], true);
}

#[test]
fn budget_check_exceeds_and_exits_nonzero() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "budget-check",
            "--cost-micros",
            "1001",
            "--max-cost-micros",
            "1000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["verdict"]["status"], "exceeded");
    assert_eq!(report["verdict"]["allowed"], false);
    assert_eq!(report["verdict"]["failures"].as_array().unwrap().len(), 1);
}
