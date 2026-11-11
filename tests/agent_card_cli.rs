use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn agent_card_cli_reports_capabilities_and_limits() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["agent-card", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let card: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(card["schema_version"], 1);
    assert_eq!(card["name"], "1Agent");
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    assert!(card["limits"].as_array().unwrap().len() >= 3);
    let capabilities = card["capabilities"].as_array().unwrap();
    assert!(
        capabilities
            .iter()
            .any(|capability| capability["id"] == "computer_use")
    );
    assert!(
        capabilities
            .iter()
            .any(|capability| capability["id"] == "team_collaboration"
                && capability["status"] == "unsupported")
    );
    let computer_use = capabilities
        .iter()
        .find(|capability| capability["id"] == "computer_use")
        .unwrap();
    assert_ne!(computer_use["status"], "verified");
}

#[test]
fn agent_card_cli_human_output_lists_limits() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin).arg("agent-card").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("1Agent Agent Card"));
    assert!(text.contains("limits:"));
    assert!(text.contains("computer_use:"));
}

#[test]
fn agent_card_cli_signs_and_verifies() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let keys = Command::new(bin)
        .args(["agent-card", "--generate-key"])
        .output()
        .unwrap();
    assert!(keys.status.success());
    let keys: serde_json::Value = serde_json::from_slice(&keys.stdout).unwrap();
    let private_key = keys["private_key_pkcs8_hex"].as_str().unwrap();
    let public_key = keys["public_key_hex"].as_str().unwrap();

    let signed = Command::new(bin)
        .args(["agent-card", "--sign-key", private_key, "--json"])
        .output()
        .unwrap();
    assert!(
        signed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&signed.stderr)
    );
    let card: serde_json::Value = serde_json::from_slice(&signed.stdout).unwrap();
    assert_eq!(card["signature"]["algorithm"], "ed25519");
    assert_eq!(card["signature"]["public_key"], public_key);

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "1agent-agent-card-{}-{unique}.json",
        std::process::id()
    ));
    std::fs::write(&path, &signed.stdout).unwrap();

    let verified = Command::new(bin)
        .args([
            "agent-card",
            "--verify",
            path.to_str().unwrap(),
            "--trusted-key",
            public_key,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        verified.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verified.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(report["verified"], true);

    let mut tampered = card.clone();
    tampered["version"] = "9.9.9".into();
    std::fs::write(&path, serde_json::to_vec(&tampered).unwrap()).unwrap();
    let bad = Command::new(bin)
        .args([
            "agent-card",
            "--verify",
            path.to_str().unwrap(),
            "--trusted-key",
            public_key,
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    std::fs::remove_file(&path).ok();

    let missing_key = Command::new(bin)
        .args(["agent-card", "--verify", "unused.json"])
        .output()
        .unwrap();
    assert!(!missing_key.status.success());
}
