use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

#[test]
fn inbox_add_list_snooze_resolve_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("inbox.jsonl");
    let add = Command::new(bin())
        .args([
            "inbox",
            "add",
            "--store",
            &store.display().to_string(),
            "--kind",
            "review",
            "--title",
            "Approve the deploy",
            "--why-now",
            "waiting for owner",
            "--source",
            "test",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "add stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let item: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    let item_id = item["item_id"].as_str().unwrap().to_string();

    let list = Command::new(bin())
        .args([
            "inbox",
            "list",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let items: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(items.as_array().unwrap().len(), 1);
    assert_eq!(items[0]["kind"], "review");

    let snooze = Command::new(bin())
        .args([
            "inbox",
            "snooze",
            "--store",
            &store.display().to_string(),
            "--item-id",
            &item_id,
            "--until-ms",
            "99999999999999",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(snooze.status.success());
    let snoozed: serde_json::Value = serde_json::from_slice(&snooze.stdout).unwrap();
    assert_eq!(snoozed["status"], "snoozed");

    let hidden = Command::new(bin())
        .args([
            "inbox",
            "list",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    let hidden: serde_json::Value = serde_json::from_slice(&hidden.stdout).unwrap();
    assert!(hidden.as_array().unwrap().is_empty());

    let resolve = Command::new(bin())
        .args([
            "inbox",
            "resolve",
            "--store",
            &store.display().to_string(),
            "--item-id",
            &item_id,
            "--status",
            "approved",
            "--note",
            "ok",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(resolve.status.success());
    let resolved: serde_json::Value = serde_json::from_slice(&resolve.stdout).unwrap();
    assert_eq!(resolved["status"], "approved");

    let empty = Command::new(bin())
        .args([
            "inbox",
            "list",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    let empty: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert!(empty.as_array().unwrap().is_empty());
}

#[test]
fn inbox_rejects_invalid_kind_and_terminal_transition() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("inbox.jsonl");
    let bad_kind = Command::new(bin())
        .args([
            "inbox",
            "add",
            "--store",
            &store.display().to_string(),
            "--kind",
            "robot",
            "--title",
            "x",
        ])
        .output()
        .unwrap();
    assert!(!bad_kind.status.success());

    let add = Command::new(bin())
        .args([
            "inbox",
            "add",
            "--store",
            &store.display().to_string(),
            "--kind",
            "review",
            "--title",
            "Approve",
            "--json",
        ])
        .output()
        .unwrap();
    let item: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    let item_id = item["item_id"].as_str().unwrap().to_string();
    let resolve = Command::new(bin())
        .args([
            "inbox",
            "resolve",
            "--store",
            &store.display().to_string(),
            "--item-id",
            &item_id,
            "--status",
            "rejected",
        ])
        .output()
        .unwrap();
    assert!(resolve.status.success());

    let again = Command::new(bin())
        .args([
            "inbox",
            "snooze",
            "--store",
            &store.display().to_string(),
            "--item-id",
            &item_id,
            "--until-ms",
            "99999999999999",
        ])
        .output()
        .unwrap();
    assert!(!again.status.success());
}
