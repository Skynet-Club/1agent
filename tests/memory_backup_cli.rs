use std::process::Command;

use first_principles_agent::{
    MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemoryService, MemorySource,
    MemorySourceKind, MemoryTaint,
};

#[test]
fn memory_backup_export_and_verify_cli() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let mut service = MemoryService::open(&journal, &payloads, &key).unwrap();
    let mut record = MemoryRecord::new(
        "mem-1",
        MemoryLayer::Semantic,
        MemoryKind::Preference,
        MemoryScope::bot("bot-1"),
        "private preference body",
        MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
    );
    record.taint = MemoryTaint::Trusted;
    service.remember_at(record, 10).unwrap();
    drop(service);

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let out = dir.path().join("backup");
    let export = Command::new(bin)
        .args(["memory", "export", "--journal"])
        .arg(&journal)
        .arg("--payloads")
        .arg(&payloads)
        .arg("--out")
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(export.status.success(), "{export:?}");
    let manifest: serde_json::Value = serde_json::from_slice(&export.stdout).unwrap();
    assert_eq!(manifest["key_included"], false);
    assert_eq!(manifest["hard_forget_covers_backup"], false);
    assert!(!out.join("memory.key").exists());

    let verify = Command::new(bin)
        .args(["memory", "verify", "--backup"])
        .arg(&out)
        .arg("--json")
        .output()
        .unwrap();
    assert!(verify.status.success(), "{verify:?}");

    let payload = std::fs::read_dir(out.join("payloads"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.to_string_lossy().ends_with(".payload.json"))
        .unwrap();
    let mut bytes = std::fs::read(&payload).unwrap();
    bytes[0] ^= 0xff;
    std::fs::write(&payload, bytes).unwrap();
    let tampered = Command::new(bin)
        .args(["memory", "verify", "--backup"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!tampered.status.success());
}

#[test]
fn memory_rotate_master_cli_rewraps_the_kek_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let mut service = MemoryService::open(&journal, &payloads, &key).unwrap();
    service
        .remember_at(
            MemoryRecord::new(
                "mem-1",
                MemoryLayer::Semantic,
                MemoryKind::Preference,
                MemoryScope::bot("bot-1"),
                "private preference body",
                MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
            ),
            10,
        )
        .unwrap();
    drop(service);

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let rotate = Command::new(bin)
        .args(["memory", "rotate-master-key", "--json"])
        .arg("--payloads")
        .arg(&payloads)
        .arg("--key")
        .arg(&key)
        .output()
        .unwrap();
    assert!(rotate.status.success(), "{rotate:?}");
    let rotation: serde_json::Value = serde_json::from_slice(&rotate.stdout).unwrap();
    assert_eq!(rotation["new_master_id"], 1);
    assert_eq!(rotation["rewrapped_keks"], 1);

    let reopened = MemoryService::open(&journal, &payloads, &key).unwrap();
    assert_eq!(
        reopened
            .get("mem-1", &MemoryScope::bot("bot-1"))
            .unwrap()
            .content,
        "private preference body"
    );
}

#[test]
fn memory_rotate_key_cli_rewraps_live_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let mut service = MemoryService::open(&journal, &payloads, &key).unwrap();
    service
        .remember_at(
            MemoryRecord::new(
                "mem-1",
                MemoryLayer::Semantic,
                MemoryKind::Preference,
                MemoryScope::bot("bot-1"),
                "private preference body",
                MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
            ),
            10,
        )
        .unwrap();
    drop(service);

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let rotate = Command::new(bin)
        .args(["memory", "rotate-key", "--bot-id", "bot-1", "--json"])
        .arg("--payloads")
        .arg(&payloads)
        .arg("--key")
        .arg(&key)
        .output()
        .unwrap();
    assert!(rotate.status.success(), "{rotate:?}");
    let rotation: serde_json::Value = serde_json::from_slice(&rotate.stdout).unwrap();
    assert_eq!(rotation["new_version"], 2);
    assert_eq!(rotation["rotated"], 1);

    let reopened = MemoryService::open(&journal, &payloads, &key).unwrap();
    assert_eq!(
        reopened
            .get("mem-1", &MemoryScope::bot("bot-1"))
            .unwrap()
            .content,
        "private preference body"
    );
}
