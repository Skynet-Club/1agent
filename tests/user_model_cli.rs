use std::process::Command;

use first_principles_agent::{
    MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemoryService, MemorySource,
    MemorySourceKind,
};

fn remember(service: &mut MemoryService, id: &str, kind: MemoryKind, content: &str, at: u128) {
    service
        .remember_at(
            {
                let mut record = MemoryRecord::new(
                    id,
                    MemoryLayer::Semantic,
                    kind,
                    MemoryScope::bot("bot-1"),
                    content,
                    MemorySource::new(MemorySourceKind::UserMessage, format!("event-{id}")),
                );
                record.valid_from_ms = at;
                record
            },
            at,
        )
        .unwrap();
}

#[test]
fn user_model_cli_exposes_relationship_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let mut service = MemoryService::open(&journal, &payloads, &key).unwrap();
    remember(
        &mut service,
        "m1",
        MemoryKind::Preference,
        "editor=pnpm",
        10,
    );
    remember(
        &mut service,
        "m2",
        MemoryKind::Preference,
        "autonomy.workspace=act",
        11,
    );
    remember(
        &mut service,
        "m3",
        MemoryKind::Preference,
        "voice_mode=advisor",
        12,
    );
    remember(
        &mut service,
        "m4",
        MemoryKind::Feedback,
        "consent:preference=deny@2592000000",
        13,
    );
    drop(service);

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["user-model", "--journal"])
        .arg(&journal)
        .arg("--payloads")
        .arg(&payloads)
        .arg("--key")
        .arg(&key)
        .arg("--bot-id")
        .arg("bot-1")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let model: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(model["user_id"], "local-user");
    assert_eq!(model["scope"]["bot_id"], "bot-1");
    assert_eq!(model["preferences"]["editor"]["value"], "pnpm");
    assert_eq!(model["autonomy"]["workspace"]["level"], "act");
    assert_eq!(model["voice"]["mode"], "advisor");
    assert_eq!(model["consents"][0]["memory_kind"], "preference");
    assert_eq!(model["consents"][0]["allow"], false);
    assert_eq!(
        model["consents"][0]["retention_ms"].as_u64(),
        Some(2_592_000_000)
    );
}

#[test]
fn user_model_cli_human_output_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("memory-journal.jsonl");
    let payloads = dir.path().join("payloads");
    let key = dir.path().join("memory.key");
    let mut service = MemoryService::open(&journal, &payloads, &key).unwrap();
    remember(
        &mut service,
        "m1",
        MemoryKind::Preference,
        "autonomy.email=observe",
        10,
    );
    remember(
        &mut service,
        "m2",
        MemoryKind::Preference,
        "voice_mode=companion",
        11,
    );
    drop(service);

    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["user-model", "--journal"])
        .arg(&journal)
        .arg("--payloads")
        .arg(&payloads)
        .arg("--key")
        .arg(&key)
        .arg("--bot-id")
        .arg("bot-1")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("voice: companion"));
    assert!(text.contains("email: observe"));
}
