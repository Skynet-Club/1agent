use std::process::Command;

fn run(args: &[&str]) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .arg("escalate")
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
fn escalate_cli_picks_the_reached_rung() {
    let plan = run(&[
        "--created-ms",
        "1000",
        "--now-ms",
        "901000",
        "--step",
        "0=inbox",
        "--step",
        "900000=notify",
        "--step",
        "3600000=urgent",
    ]);
    assert_eq!(plan["level"], "notify");
    assert_eq!(plan["elapsed_ms"], 900000);

    let resolved = run(&[
        "--created-ms",
        "1000",
        "--now-ms",
        "901000",
        "--step",
        "0=inbox",
        "--resolved",
    ]);
    assert_eq!(resolved["level"], serde_json::Value::Null);
}

#[test]
fn escalate_cli_rejects_bad_steps() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    for args in [
        vec!["escalate"],
        vec!["escalate", "--step", "soon=inbox"],
        vec!["escalate", "--step", "0=teleport"],
        vec!["escalate", "--step", "100=inbox", "--step", "100=notify"],
    ] {
        let output = Command::new(bin).args(args).output().unwrap();
        assert!(!output.status.success());
    }
}
