use std::process::{Command, Output};

use first_principles_agent::{
    DataClass, MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemoryService, MemorySource,
    MemorySourceKind, MemoryTaint,
};
use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(args)
        .output()
        .unwrap()
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn init(home: &std::path::Path) {
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
}

fn record() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-1",
        MemoryLayer::Semantic,
        MemoryKind::Preference,
        MemoryScope::bot("bot-1"),
        "prefers pnpm",
        MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
    );
    record.taint = MemoryTaint::Trusted;
    record.data_class = DataClass::Personal;
    record
}

fn service(home: &std::path::Path) -> MemoryService {
    MemoryService::open(
        home.join("memory-journal.jsonl"),
        home.join("payloads"),
        home.join("keys/memory.key"),
    )
    .unwrap()
}

#[test]
fn escrow_and_restore_recover_memory_after_key_loss() {
    let workspace = workspace();
    let home = workspace.path().join("home");
    init(&home);
    let mut memory = service(&home);
    memory.remember_at(record(), 10).unwrap();
    drop(memory);

    let escrow = workspace.path().join("keys.escrow.json");
    let recovery = workspace.path().join("recovery.key");
    let outcome = run_ok(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &escrow.display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert_eq!(outcome["created_recovery_key"], true);
    assert_eq!(outcome["files"].as_array().unwrap().len(), 2);
    assert!(recovery.is_file());
    assert!(escrow.is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&escrow, &recovery] {
            let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    std::fs::remove_file(home.join("keys/memory.key")).unwrap();
    std::fs::remove_file(home.join("keys/memory.key-keks.json")).unwrap();
    let restored = run_ok(&[
        "key",
        "restore",
        "--root",
        &home.display().to_string(),
        "--escrow",
        &escrow.display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert_eq!(restored["files"].as_array().unwrap().len(), 2);

    let reopened = service(&home);
    assert_eq!(
        reopened
            .get("mem-1", &MemoryScope::bot("bot-1"))
            .unwrap()
            .content,
        "prefers pnpm"
    );
    assert!(
        reopened
            .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
            .unwrap()
    );

    let output = run(&[
        "key",
        "restore",
        "--root",
        &home.display().to_string(),
        "--escrow",
        &escrow.display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("overwrite"));
}

#[test]
fn restore_rejects_a_tampered_escrow() {
    let workspace = workspace();
    let home = workspace.path().join("home");
    init(&home);
    std::fs::write(home.join("keys/memory.key"), "master-key-bytes").unwrap();
    std::fs::write(home.join("keys/memory.key-keks.json"), "ledger-bytes").unwrap();
    let escrow = workspace.path().join("keys.escrow.json");
    let recovery = workspace.path().join("recovery.key");
    run_ok(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &escrow.display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    let mut envelope: Value = serde_json::from_slice(&std::fs::read(&escrow).unwrap()).unwrap();
    let ciphertext = envelope["ciphertext_hex"].as_str().unwrap();
    let mut chars = ciphertext.chars();
    let first = chars.next().unwrap();
    let replacement = if first == '0' { '1' } else { '0' };
    envelope["ciphertext_hex"] = Value::from(format!("{replacement}{}", chars.as_str()));
    std::fs::write(&escrow, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();

    std::fs::remove_file(home.join("keys/memory.key")).unwrap();
    std::fs::remove_file(home.join("keys/memory.key-keks.json")).unwrap();
    let output = run(&[
        "key",
        "restore",
        "--root",
        &home.display().to_string(),
        "--escrow",
        &escrow.display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be decrypted"));
    assert!(!home.join("keys/memory.key").exists());
}

#[test]
fn escrow_requires_paths_outside_the_home() {
    let workspace = workspace();
    let home = workspace.path().join("home");
    init(&home);
    std::fs::write(home.join("keys/memory.key"), "master-key-bytes").unwrap();
    let recovery = workspace.path().join("recovery.key");

    let output = run(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &home.join("escrow.json").display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside"));

    let output = run(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &workspace.path().join("escrow.json").display().to_string(),
        "--recovery-key",
        &home.join("recovery.key").display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside"));
}

#[cfg(unix)]
#[test]
fn escrow_rejects_a_public_recovery_key() {
    let workspace = workspace();
    let home = workspace.path().join("home");
    init(&home);
    std::fs::write(home.join("keys/memory.key"), "master-key-bytes").unwrap();
    let recovery = workspace.path().join("recovery.key");
    std::fs::write(&recovery, format!("{}\n", "11".repeat(32))).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&recovery, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let output = run(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &workspace.path().join("escrow.json").display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("accessible"));
}

#[cfg(unix)]
#[test]
fn escrow_rejects_symlinked_parents_inside_the_home() {
    let workspace = workspace();
    let home = workspace.path().join("home");
    init(&home);
    std::fs::write(home.join("keys/memory.key"), "master-key-bytes").unwrap();
    let link = workspace.path().join("link");
    std::os::unix::fs::symlink(&home, &link).unwrap();
    let recovery = workspace.path().join("recovery.key");

    let output = run(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &link.join("escrow.json").display().to_string(),
        "--recovery-key",
        &recovery.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside"));

    let output = run(&[
        "key",
        "escrow",
        "--root",
        &home.display().to_string(),
        "--output",
        &workspace.path().join("escrow.json").display().to_string(),
        "--recovery-key",
        &link.join("recovery.key").display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside"));
}
