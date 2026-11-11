use std::process::Command;

#[test]
fn unit_economics_cli_reports_margin_and_break_even() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "unit-economics",
            "--revenue-micros",
            "10000",
            "--variable-cost-micros",
            "4000",
            "--fixed-cost-micros",
            "9000",
            "--outcomes",
            "10",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["contribution_micros"], 6000);
    assert_eq!(report["net_micros"], -3000);
    assert_eq!(report["gross_margin_bps"], 6000);
    assert_eq!(report["revenue_per_outcome_micros"], 1000);
    assert_eq!(report["cost_per_outcome_micros"], 400);
    assert_eq!(report["break_even_outcomes"], 15);
}

#[test]
fn unit_economics_cli_handles_losses_and_zero_revenue() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let loss = Command::new(bin)
        .args([
            "unit-economics",
            "--revenue-micros",
            "1000",
            "--variable-cost-micros",
            "2000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(loss.status.success());
    let report: serde_json::Value = serde_json::from_slice(&loss.stdout).unwrap();
    assert_eq!(report["gross_margin_bps"], -10000);
    assert!(report["break_even_outcomes"].is_null());

    let empty = Command::new(bin)
        .args(["unit-economics", "--json"])
        .output()
        .unwrap();
    assert!(empty.status.success());
    let report: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert!(report["gross_margin_bps"].is_null());
    assert!(report["outcomes"].is_null());
}
