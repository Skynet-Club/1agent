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

    let second = Command::new(bin)
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
            "carol",
            "--summary",
            "second handoff",
            "--evidence",
            "journal:run-2",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(second.status.success());

    let listed = Command::new(bin)
        .args(["handoff", "list", "--store", store, "--json"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let items: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 2);
    assert_eq!(items[0]["status"], "accepted");
    assert_eq!(items[1]["status"], "open");

    let open = Command::new(bin)
        .args(["handoff", "list", "--store", store, "--open", "--json"])
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&open.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["id"], "h2");

    let to_bob = Command::new(bin)
        .args(["handoff", "list", "--store", store, "--to", "bob", "--json"])
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&to_bob.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["id"], "h1");
}

#[test]
fn handoff_cli_cancels_an_open_handoff() {
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
            "--summary",
            "to be cancelled",
            "--evidence",
            "journal:run-1",
        ])
        .output()
        .unwrap();
    assert!(created.status.success());

    let cancelled = Command::new(bin)
        .args([
            "handoff",
            "cancel",
            "--store",
            store,
            "--id",
            "h1",
            "--actor",
            "bob",
            "--reason",
            "not needed",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        cancelled.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cancelled.stderr)
    );
    let handoff: serde_json::Value = serde_json::from_slice(&cancelled.stdout).unwrap();
    assert_eq!(handoff["status"], "cancelled");
    assert_eq!(handoff["cancelled_by"], "bob");
    assert_eq!(handoff["cancelled_reason"], "not needed");

    let accept = Command::new(bin)
        .args([
            "handoff", "accept", "--store", store, "--id", "h1", "--actor", "bob",
        ])
        .output()
        .unwrap();
    assert!(!accept.status.success());

    let open = Command::new(bin)
        .args(["handoff", "list", "--store", store, "--open", "--json"])
        .output()
        .unwrap();
    let items: serde_json::Value = serde_json::from_slice(&open.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 0);
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
