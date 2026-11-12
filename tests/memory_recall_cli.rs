use std::process::Command;

use first_principles_agent::{
    MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemoryService, MemorySource,
    MemorySourceKind, MemoryTaint,
};

fn memory_service(dir: &std::path::Path) -> MemoryService {
    MemoryService::open(
        dir.join("memory-journal.jsonl"),
        dir.join("payloads"),
        dir.join("memory.key"),
    )
    .unwrap()
}

fn record(id: &str, bot: &str, content: &str) -> MemoryRecord {
    let mut record = MemoryRecord::new(
        id,
        MemoryLayer::Semantic,
        MemoryKind::Preference,
        MemoryScope::bot(bot),
        content,
        MemorySource::new(MemorySourceKind::UserMessage, format!("event-{id}")),
    );
    record.taint = MemoryTaint::Trusted;
    record
}

#[test]
fn memory_recall_reports_explainable_scoped_hits() {
    let dir = tempfile::tempdir().unwrap();
    let mut service = memory_service(dir.path());
    service
        .remember_at(record("mem-1", "bot-1", "prefers short answers"), 10)
        .unwrap();
    service
        .remember_at(record("mem-2", "bot-2", "prefers long answers"), 11)
        .unwrap();
    drop(service);

    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "memory",
            "recall",
            "--journal",
            &dir.path()
                .join("memory-journal.jsonl")
                .display()
                .to_string(),
            "--payloads",
            &dir.path().join("payloads").display().to_string(),
            "--key",
            &dir.path().join("memory.key").display().to_string(),
            "--query",
            "short",
            "--bot-id",
            "bot-1",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scope"]["bot_id"], "bot-1");
    let hits = report["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["record"]["memory_id"], "mem-1");
    assert_eq!(hits[0]["record"]["content"], "prefers short answers");
    assert_eq!(hits[0]["record"]["source"]["kind"], "user_message");
    assert_eq!(hits[0]["record"]["source"]["event_id"], "event-mem-1");
    assert_eq!(hits[0]["record"]["taint"], "trusted");
    assert!(hits[0]["record"]["confidence"].is_number());
    assert!(hits[0]["score"].is_number());

    let isolated = Command::new(binary)
        .args([
            "memory",
            "recall",
            "--journal",
            &dir.path()
                .join("memory-journal.jsonl")
                .display()
                .to_string(),
            "--payloads",
            &dir.path().join("payloads").display().to_string(),
            "--key",
            &dir.path().join("memory.key").display().to_string(),
            "--query",
            "short",
            "--bot-id",
            "bot-2",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(isolated.status.success());
    let report: serde_json::Value = serde_json::from_slice(&isolated.stdout).unwrap();
    assert_eq!(report["hits"].as_array().unwrap().len(), 0);

    let empty = Command::new(binary)
        .args([
            "memory",
            "recall",
            "--journal",
            &dir.path()
                .join("memory-journal.jsonl")
                .display()
                .to_string(),
            "--payloads",
            &dir.path().join("payloads").display().to_string(),
            "--key",
            &dir.path().join("memory.key").display().to_string(),
            "--query",
            "   ",
        ])
        .output()
        .unwrap();
    assert!(!empty.status.success());
}
