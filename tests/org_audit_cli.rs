use std::process::Command;

#[test]
fn org_audit_appends_and_verifies_across_processes() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("org-audit.jsonl");
    let store = store.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    for (actor, action, decision) in [("alice", "approve", "allow"), ("bob", "write", "deny")] {
        let appended = Command::new(bin)
            .args([
                "org-audit",
                "append",
                "--store",
                store,
                "--actor",
                actor,
                "--action",
                action,
                "--scope",
                "workspace",
                "--decision",
                decision,
                "--json",
            ])
            .output()
            .unwrap();
        assert!(
            appended.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&appended.stderr)
        );
    }

    let verified = Command::new(bin)
        .args(["org-audit", "verify", "--store", store, "--json"])
        .output()
        .unwrap();
    assert!(verified.status.success());
    let report: serde_json::Value = serde_json::from_slice(&verified.stdout).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["entries"], 2);

    let raw = std::fs::read_to_string(store).unwrap();
    std::fs::write(
        store,
        raw.replacen("\"actor\":\"alice\"", "\"actor\":\"mallory\"", 1),
    )
    .unwrap();
    let tampered = Command::new(bin)
        .args(["org-audit", "verify", "--store", store, "--json"])
        .output()
        .unwrap();
    assert!(!tampered.status.success());
    let report: serde_json::Value = serde_json::from_slice(&tampered.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(report["first_invalid_seq"], 0);
}

#[test]
fn org_audit_rejects_invalid_decisions_and_empty_logs_verify() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("org-audit.jsonl");
    let store = store.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let missing = Command::new(bin)
        .args(["org-audit", "verify", "--store", store, "--json"])
        .output()
        .unwrap();
    assert!(missing.status.success());
    let report: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["entries"], 0);

    let bad = Command::new(bin)
        .args([
            "org-audit",
            "append",
            "--store",
            store,
            "--actor",
            "alice",
            "--action",
            "approve",
            "--decision",
            "maybe",
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
}
