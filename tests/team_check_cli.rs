use std::process::Command;

fn check(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("team-check")
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

fn check_denied(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("team-check")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn team_roles_and_scopes_are_enforced() {
    assert_eq!(
        check(&["--role", "owner", "--action", "approve"])["decision"],
        "allow"
    );
    assert_eq!(
        check(&[
            "--role",
            "member",
            "--action",
            "write",
            "--scope",
            "workspace",
            "--granted-scope",
            "workspace",
        ])["decision"],
        "allow"
    );
    assert_eq!(
        check_denied(&[
            "--role",
            "member",
            "--action",
            "read",
            "--scope",
            "release",
            "--granted-scope",
            "workspace",
        ])["decision"],
        "deny"
    );
}

#[test]
fn team_check_denies_and_rejects_bad_input() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let denied = Command::new(bin)
        .args([
            "team-check",
            "--role",
            "member",
            "--action",
            "approve",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!denied.status.success());
    let report: serde_json::Value = serde_json::from_slice(&denied.stdout).unwrap();
    assert_eq!(report["decision"], "deny");

    let bad = Command::new(bin)
        .args(["team-check", "--role", "robot"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
