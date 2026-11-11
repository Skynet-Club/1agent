use std::process::Command;

#[test]
fn handoff_create_accept_and_list() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("handoffs.jsonl");
    let store = store.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let created = Command::new(bin)
        .args([
            "handoff",
            "create",
            "--store",
            store,
            "--id",
            "h1",
            "--from",
            "alice",
            "--to",
            "bob",
            "--scope",
            "release",
            "--summary",
            "prepared the release candidate",
            "--open-item",
            "run the smoke test",
            "--evidence",
            "journal:run-1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let handoff: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(handoff["status"], "open");

    let wrong = Command::new(bin)
        .args([
            "handoff", "accept", "--store", store, "--id", "h1", "--actor", "alice",
        ])
        .output()
        .unwrap();
    assert!(!wrong.status.success());

    let accepted = Command::new(bin)
        .args([
            "handoff", "accept", "--store", store, "--id", "h1", "--actor", "bob", "--json",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    let accepted: serde_json::Value = serde_json::from_slice(&accepted.stdout).unwrap();
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(accepted["accepted_by"], "bob");

    let twice = Command::new(bin)
        .args([
            "handoff", "accept", "--store", store, "--id", "h1", "--actor", "bob",
        ])
        .output()
        .unwrap();
    assert!(!twice.status.success());

    let listed = Command::new(bin)
        .args(["handoff", "list", "--store", store, "--json"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["status"], "accepted");
}

#[test]
fn handoff_rejects_ungrounded_and_self_handoffs() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("handoffs.jsonl");
    let store = store.to_str().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let no_evidence = Command::new(bin)
        .args([
            "handoff",
            "create",
            "--store",
            store,
            "--id",
            "h1",
            "--from",
            "alice",
            "--to",
            "bob",
            "--summary",
            "prepared the release candidate",
        ])
        .output()
        .unwrap();
    assert!(!no_evidence.status.success());

    let self_handoff = Command::new(bin)
        .args([
            "handoff",
            "create",
            "--store",
            store,
            "--id",
            "h2",
            "--from",
            "alice",
            "--to",
            "alice",
            "--summary",
            "prepared the release candidate",
            "--evidence",
            "journal:run-1",
        ])
        .output()
        .unwrap();
    assert!(!self_handoff.status.success());
}
