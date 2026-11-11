use std::process::Command;

fn check(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("cua-check")
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
fn cua_check_applies_provenance_and_risk() {
    let trusted = check(&["--action", "click", "--risk", "low", "--reversible"]);
    assert_eq!(trusted["action"]["decision"], "allow");

    let untrusted_medium = check(&[
        "--action",
        "type",
        "--risk",
        "medium",
        "--untrusted",
        "--reversible",
    ]);
    assert_eq!(untrusted_medium["action"]["decision"], "require_approval");

    let untrusted_high = check(&["--action", "pay", "--risk", "high", "--untrusted"]);
    assert_eq!(untrusted_high["action"]["decision"], "deny");
}

#[test]
fn cua_check_blocks_branch_steering() {
    let planned = check(&[
        "--action",
        "click",
        "--risk",
        "low",
        "--reversible",
        "--branch",
        "confirm",
        "--planned-branch",
        "confirm",
        "--planned-branch",
        "cancel",
    ]);
    assert_eq!(planned["branch"]["decision"], "allow");

    let unplanned = check(&[
        "--action",
        "click",
        "--risk",
        "low",
        "--reversible",
        "--branch",
        "pay",
        "--planned-branch",
        "confirm",
    ]);
    assert_eq!(unplanned["branch"]["decision"], "deny");
}
