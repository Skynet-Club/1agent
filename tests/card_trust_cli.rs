use std::process::Command;

#[test]
fn card_trust_adds_verifies_and_revokes_a_key() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("card-trust.json");
    let card_path = workspace.path().join("card.json");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let private_key_path = workspace.path().join("agent-card.key");
    let public_key_path = workspace.path().join("agent-card.pub");
    let keys = Command::new(bin)
        .args(["agent-card", "--generate-key", "--private-out"])
        .arg(&private_key_path)
        .arg("--public-out")
        .arg(&public_key_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(keys.status.success());
    let keys: serde_json::Value = serde_json::from_slice(&keys.stdout).unwrap();
    let public_key = keys["public_key_hex"].as_str().unwrap();

    let signed = Command::new(bin)
        .args(["agent-card", "--sign-key-file"])
        .arg(&private_key_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(signed.status.success());
    std::fs::write(&card_path, &signed.stdout).unwrap();

    let added = Command::new(bin)
        .args([
            "card-trust",
            "add",
            "--store",
            store.to_str().unwrap(),
            "--public-key",
            public_key,
            "--note",
            "release",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        added.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&added.stderr)
    );

    let verified = Command::new(bin)
        .args([
            "card-trust",
            "verify",
            "--store",
            store.to_str().unwrap(),
            "--card",
            card_path.to_str().unwrap(),
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

    let revoked = Command::new(bin)
        .args([
            "card-trust",
            "revoke",
            "--store",
            store.to_str().unwrap(),
            "--public-key",
            public_key,
            "--reason",
            "retired",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(revoked.status.success());
    let revoked_key: serde_json::Value = serde_json::from_slice(&revoked.stdout).unwrap();
    assert!(revoked_key["revoked_at_ms"].is_number());

    let rejected = Command::new(bin)
        .args([
            "card-trust",
            "verify",
            "--store",
            store.to_str().unwrap(),
            "--card",
            card_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("revoked"));

    let listed = Command::new(bin)
        .args([
            "card-trust",
            "list",
            "--store",
            store.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let trust: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(trust["keys"].as_array().unwrap().len(), 1);
}

#[test]
fn card_trust_rejects_invalid_keys() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("card-trust.json");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "card-trust",
            "add",
            "--store",
            store.to_str().unwrap(),
            "--public-key",
            "not-hex",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
