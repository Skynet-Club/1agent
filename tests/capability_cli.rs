use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn capability_resolve_selects_manual_build_and_lists_alternatives() {
    let output = Command::new(binary())
        .args(["capability", "resolve", "--task", "画一张架构图", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["contract"]["deliverable"]["media_type"],
        "image/svg+xml"
    );
    assert_eq!(value["requirements"][0]["selected"]["name"], "manual-svg");
    assert!(
        value["requirements"][0]["alternatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|candidate| candidate["name"] == "archify")
    );
}

#[test]
fn capability_install_proposal_and_human_approval_are_separate() {
    let workspace = tempfile::tempdir().unwrap();
    let proposal = workspace.path().join("proposal.json");
    let output = Command::new(binary())
        .current_dir(workspace.path())
        .args([
            "capability",
            "propose-install",
            "--name",
            "archify",
            "--out",
            &proposal.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&proposal).unwrap()).unwrap();
    assert_eq!(
        value["proposal"]["command"],
        "hermes skills install official/creative/archify"
    );
    assert_eq!(value["approval_required"], true);

    let output = Command::new(binary())
        .current_dir(workspace.path())
        .env_remove("ONEAGENT_HOME")
        .env("HOME", workspace.path())
        .args([
            "capability",
            "approve-install",
            "--proposal",
            &proposal.display().to_string(),
            "--yes",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let approval: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(approval["executed"], false);
    assert_eq!(approval["candidate"], "archify");
    let ledger = workspace.path().join(".1a/capability-approvals.jsonl");
    assert!(ledger.is_file(), "approval ledger must be written");
}
