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
fn team_check_appends_allow_and_deny_to_the_organization_audit() {
    let workspace = tempfile::tempdir().unwrap();
    let audit = workspace.path().join("org-audit.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let denied = Command::new(bin)
        .args([
            "team-check",
            "--role",
            "member",
            "--action",
            "approve",
            "--scope",
            "release",
            "--granted-scope",
            "workspace",
            "--audit-store",
            audit.to_str().unwrap(),
            "--actor-id",
            "alice",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!denied.status.success());

    let allowed = Command::new(bin)
        .args([
            "team-check",
            "--role",
            "owner",
            "--action",
            "approve",
            "--scope",
            "release",
            "--audit-store",
            audit.to_str().unwrap(),
            "--actor-id",
            "bob",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(allowed.status.success());

    let verified = Command::new(bin)
        .args(["org-audit", "verify", "--store"])
        .arg(&audit)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        verified.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verified.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(report["entries"], 2);

    let entries: Vec<serde_json::Value> = std::fs::read_to_string(&audit)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(entries[0]["actor"], "alice");
    assert_eq!(entries[0]["decision"], "deny");
    assert_eq!(entries[0]["action"], "approve");
    assert_eq!(entries[1]["actor"], "bob");
    assert_eq!(entries[1]["decision"], "allow");
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
