use std::process::Command;

fn check(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("cascade-check")
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
fn containment_at_b_stops_the_chain() {
    let result = check(&[
        "--root",
        "a",
        "--edge",
        "a>b",
        "--edge",
        "b>c",
        "--edge",
        "c>d",
        "--contain",
        "b",
    ]);
    assert_eq!(result["report"]["affected"].as_array().unwrap().len(), 2);
    assert_eq!(result["report"]["containment_rate_bps"], 5_000);
    assert_eq!(result["report"]["cascade_depth"], 1);
    assert_eq!(result["report"]["blast_radius"], 2);
    assert_eq!(result["verdict"]["passed"], true);
}

#[test]
fn no_containment_fails_the_default_policy() {
    let result = check(&[
        "--root", "a", "--edge", "a>b", "--edge", "b>c", "--edge", "c>d",
    ]);
    assert_eq!(result["report"]["affected"].as_array().unwrap().len(), 4);
    assert_eq!(result["report"]["containment_rate_bps"], 0);
    assert_eq!(result["verdict"]["passed"], false);
}

#[test]
fn malformed_edge_fails_closed() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["cascade-check", "--root", "a", "--edge", "ab"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
