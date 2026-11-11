use std::process::Command;

#[test]
fn seals_and_detects_appends_and_tampering() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("org-audit.jsonl");
    let private_key = workspace.path().join("org-audit.key");
    let public_key_path = workspace.path().join("org-audit.pub");
    let seal_path = workspace.path().join("org-audit.seal.json");
    let store_s = store.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let keys = Command::new(bin)
        .args([
            "org-audit",
            "generate-key",
            "--private-out",
            private_key.to_str().unwrap(),
            "--public-out",
            public_key_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        keys.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&keys.stderr)
    );
    let keys: serde_json::Value = serde_json::from_slice(&keys.stdout).unwrap();
    let public_key = keys["public_key"].as_str().unwrap();
    assert!(private_key.exists());
    assert!(public_key_path.exists());

    for actor in ["alice", "bob"] {
        let appended = Command::new(bin)
            .args([
                "org-audit",
                "append",
                "--store",
                store_s,
                "--actor",
                actor,
                "--action",
                "approve",
                "--decision",
                "allow",
            ])
            .output()
            .unwrap();
        assert!(appended.status.success());
    }

    let sealed = Command::new(bin)
        .args([
            "org-audit",
            "seal",
            "--store",
            store_s,
            "--key",
            private_key.to_str().unwrap(),
            "--out",
            seal_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        sealed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sealed.stderr)
    );
    let seal: serde_json::Value = serde_json::from_slice(&sealed.stdout).unwrap();
    assert_eq!(seal["entries"], 2);
    assert_eq!(seal["algorithm"], "ed25519");

    let verified = Command::new(bin)
        .args([
            "org-audit",
            "verify",
            "--store",
            store_s,
            "--seal",
            seal_path.to_str().unwrap(),
            "--public-key",
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
    assert_eq!(report["valid"], true);
    assert_eq!(report["seal_verified"], true);

    let appended = Command::new(bin)
        .args([
            "org-audit",
            "append",
            "--store",
            store_s,
            "--actor",
            "carol",
            "--action",
            "approve",
            "--decision",
            "deny",
        ])
        .output()
        .unwrap();
    assert!(appended.status.success());
    let stale = Command::new(bin)
        .args([
            "org-audit",
            "verify",
            "--store",
            store_s,
            "--seal",
            seal_path.to_str().unwrap(),
            "--public-key",
            public_key,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    let report: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["seal_verified"], false);

    let resealed = Command::new(bin)
        .args([
            "org-audit",
            "seal",
            "--store",
            store_s,
            "--key",
            private_key.to_str().unwrap(),
            "--out",
            seal_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(resealed.status.success());
    let mut tampered: serde_json::Value = serde_json::from_slice(&resealed.stdout).unwrap();
    assert_eq!(tampered["entries"], 3);
    tampered["head_hash"] = serde_json::json!("00".repeat(32));
    std::fs::write(&seal_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
    let rejected = Command::new(bin)
        .args([
            "org-audit",
            "verify",
            "--store",
            store_s,
            "--seal",
            seal_path.to_str().unwrap(),
            "--public-key",
            public_key,
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());

    let missing_key = Command::new(bin)
        .args([
            "org-audit",
            "verify",
            "--store",
            store_s,
            "--seal",
            seal_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!missing_key.status.success());
}
