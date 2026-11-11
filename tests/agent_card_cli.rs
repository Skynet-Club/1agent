use std::process::Command;

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
