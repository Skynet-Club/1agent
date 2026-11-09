use std::process::{Command, Output};

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

#[test]
fn init_creates_private_home_and_is_idempotent() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    let first = run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(first["created"], true);
    assert!(home.join("home.json").is_file());
    for name in ["journals", "payloads", "backups", "keys"] {
        assert!(home.join(name).is_dir(), "{name} missing");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let root_mode = std::fs::metadata(&home).unwrap().permissions().mode() & 0o777;
        assert_eq!(root_mode, 0o700);
        let marker_mode = std::fs::metadata(home.join("home.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(marker_mode, 0o600);
    }

    let second = run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(second["created"], false);
}

#[test]
fn init_refuses_unmarked_non_empty_directory() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(home.join("user-data.txt"), "keep me").unwrap();
    let output = run(&["init", "--root", &home.display().to_string(), "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not empty"));
    assert!(home.join("user-data.txt").is_file());
}

#[test]
fn doctor_warns_on_missing_layout_and_upgrade_repairs_it() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    std::fs::remove_dir(home.join("backups")).unwrap();

    let report = run_ok(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(report["status"], "warn");

    let upgrade = run_ok(&["upgrade", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(upgrade["changed"], true);
    assert_eq!(upgrade["repaired"][0], "backups");
    assert!(home.join("backups").is_dir());

    let report = run_ok(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(report["status"], "ok");
}

#[test]
fn doctor_fails_for_non_home_directory() {
    let workspace = tempfile::tempdir().unwrap();
    let output = run(&[
        "doctor",
        "--root",
        &workspace.path().display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "fail");
}

#[test]
fn export_excludes_keys_and_restore_rebuilds_home() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    std::fs::write(home.join("keys/memory.key"), "secret-key-material").unwrap();
    std::fs::write(home.join("keys/memory.key-keks.json"), "wrapped-scope-keys").unwrap();
    std::fs::write(home.join("payloads/record.json"), "{\"encrypted\":true}").unwrap();
    std::fs::write(home.join("journals/run.jsonl"), "{\"event\":1}\n").unwrap();

    let bundle = workspace.path().join("bundle");
    let exported = run_ok(&[
        "export",
        "--root",
        &home.display().to_string(),
        "--output",
        &bundle.display().to_string(),
        "--json",
    ]);
    assert_eq!(exported["manifest"]["key_included"], false);
    assert!(!bundle.join("keys/memory.key").exists());
    assert!(!bundle.join("keys/memory.key-keks.json").exists());
    let excluded = exported["manifest"]["excluded"].as_array().unwrap();
    assert!(excluded.iter().any(|value| value == "keys/memory.key"));
    assert!(
        excluded
            .iter()
            .any(|value| value == "keys/memory.key-keks.json")
    );
    assert!(bundle.join("payloads/record.json").is_file());

    let restored_home = workspace.path().join("restored");
    let restored = run_ok(&[
        "restore",
        "--bundle",
        &bundle.display().to_string(),
        "--root",
        &restored_home.display().to_string(),
        "--json",
    ]);
    assert_eq!(restored["files"], 3);
    assert_eq!(
        std::fs::read_to_string(restored_home.join("payloads/record.json")).unwrap(),
        "{\"encrypted\":true}"
    );
    assert!(!restored_home.join("keys/memory.key").exists());
}

#[test]
fn restore_rejects_a_tampered_bundle() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    std::fs::write(home.join("payloads/record.json"), "original").unwrap();
    let bundle = workspace.path().join("bundle");
    run_ok(&[
        "export",
        "--root",
        &home.display().to_string(),
        "--output",
        &bundle.display().to_string(),
        "--json",
    ]);
    std::fs::write(bundle.join("payloads/record.json"), "tampered").unwrap();

    let restored_home = workspace.path().join("restored");
    let output = run(&[
        "restore",
        "--bundle",
        &bundle.display().to_string(),
        "--root",
        &restored_home.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("hash mismatch"));
    assert!(!restored_home.exists());
}

#[test]
fn export_refuses_output_inside_home() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let output = run(&[
        "export",
        "--root",
        &home.display().to_string(),
        "--output",
        &home.join("bundle").display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside"));
}

#[test]
fn upgrade_rejects_a_future_home_version() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let marker = home.join("home.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    manifest["version"] = Value::from(99);
    std::fs::write(&marker, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();

    let output = run(&["upgrade", "--root", &home.display().to_string(), "--json"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported"));
}

#[test]
fn uninstall_keeps_data_until_purge_and_confirmation() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    std::fs::write(home.join("journals/run.jsonl"), "data").unwrap();

    let kept = run_ok(&["uninstall", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(kept["removed"], false);
    assert!(home.is_dir());

    let output = run(&[
        "uninstall",
        "--root",
        &home.display().to_string(),
        "--purge",
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--yes"));
    assert!(home.is_dir());

    let removed = run_ok(&[
        "uninstall",
        "--root",
        &home.display().to_string(),
        "--purge",
        "--yes",
        "--json",
    ]);
    assert_eq!(removed["removed"], true);
    assert!(!home.exists());

    let plain = workspace.path().join("plain");
    std::fs::create_dir(&plain).unwrap();
    std::fs::write(plain.join("keep.txt"), "keep").unwrap();
    let output = run(&[
        "uninstall",
        "--root",
        &plain.display().to_string(),
        "--purge",
        "--yes",
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a 1Agent"));
    assert!(plain.join("keep.txt").is_file());
}

#[test]
fn initialized_home_becomes_default_state_root() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);

    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["task-graph", "demo", "--json"])
        .env("ONEAGENT_HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(home.join("task-graph.jsonl").is_file());

    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "home default journal evidence").unwrap();
    let config = workspace.path().join("agent.json");
    std::fs::write(
        &config,
        serde_json::to_vec_pretty(&serde_json::json!({
            "providers": [{
                "id": "deterministic",
                "adapter": "deterministic",
                "model": "offline-v1",
                "quality_score": 1,
                "max_risk": "high"
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "run",
            "--config",
            &config.display().to_string(),
            "--task",
            &format!("Research @read-file:\"{}\"", evidence.display()),
            "--read-root",
            &workspace.path().display().to_string(),
            "--json",
        ])
        .env("ONEAGENT_HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["stop_reason"], "completed");
    assert!(
        contains_jsonl(&home.join("journals")),
        "run journal was not written under the initialized home"
    );
}

fn contains_jsonl(dir: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && contains_jsonl(&path) {
            return true;
        }
        if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            return true;
        }
    }
    false
}

#[test]
fn doctor_verifies_journals_and_fails_on_corruption() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let store = home.join("task-graph.jsonl");
    run_ok(&[
        "task-graph",
        "demo",
        "--store",
        &store.display().to_string(),
        "--json",
    ]);
    let report = run_ok(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(report["status"], "ok");
    let checks = report["checks"].as_array().unwrap();
    assert!(checks.iter().any(|check| {
        check["id"] == "task_graph_journal"
            && check["status"] == "ok"
            && check["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("events verified"))
    }));

    std::fs::write(home.join("skill-journal.jsonl"), "{broken\n").unwrap();
    let output = run(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "fail");
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| { check["id"] == "skill_journal" && check["status"] == "fail" })
    );
}

#[cfg(unix)]
#[test]
fn doctor_rejects_public_key_ledger() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let ledger = home.join("keys/memory.key-keks.json");
    std::fs::write(&ledger, "wrapped").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&ledger, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
    let output = run(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| { check["id"] == "kek_ledger" && check["status"] == "fail" })
    );
}

#[cfg(unix)]
#[test]
fn export_rejects_a_symlinked_parent_inside_the_home() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let link = workspace.path().join("link");
    std::os::unix::fs::symlink(&home, &link).unwrap();
    let output = run(&[
        "export",
        "--root",
        &home.display().to_string(),
        "--output",
        &link.join("bundle").display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside"));
}

#[test]
fn invalid_home_marker_does_not_become_the_default_state_root() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(
        home.join("home.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "1agent.home.v1",
            "version": 99,
            "created_ms": 1,
            "updated_ms": 1
        }))
        .unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["task-graph", "demo", "--json"])
        .current_dir(workspace.path())
        .env("ONEAGENT_HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!home.join("task-graph.jsonl").exists());
    assert!(workspace.path().join(".1a/task-graph.jsonl").is_file());
}

#[cfg(unix)]
#[test]
fn export_rejects_non_utf8_filenames() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let name = OsStr::from_bytes(b"bad\xff.json");
    std::fs::write(home.join("payloads").join(name), "opaque").unwrap();
    let bundle = workspace.path().join("bundle");
    let output = run(&[
        "export",
        "--root",
        &home.display().to_string(),
        "--output",
        &bundle.display().to_string(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
    assert!(!bundle.join("export.json").exists());
}

#[test]
fn task_graph_inspect_uses_the_initialized_home() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    run_ok(&[
        "task-graph",
        "demo",
        "--store",
        &home.join("task-graph.jsonl").display().to_string(),
        "--json",
    ]);
    let elsewhere = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_task-graph-inspect"))
        .args(["--json"])
        .current_dir(elsewhere.path())
        .env("ONEAGENT_HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(snapshot["threads"].as_array().unwrap().len(), 1);
}

#[test]
fn doctor_fails_on_an_older_home_version_without_a_migration() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    run_ok(&["init", "--root", &home.display().to_string(), "--json"]);
    let marker = home.join("home.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    manifest["version"] = Value::from(0);
    std::fs::write(&marker, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let output = run(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "fail");
    assert!(report["checks"].as_array().unwrap().iter().any(|check| {
        check["id"] == "home_marker"
            && check["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("no migration"))
    }));
}

#[cfg(unix)]
#[test]
fn upgrade_migrates_v1_keys_and_rolls_back() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    std::fs::create_dir(&home).unwrap();
    for name in ["journals", "payloads", "backups"] {
        std::fs::create_dir(home.join(name)).unwrap();
    }
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(
        home.join("home.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "1agent.home.v1",
            "version": 1,
            "created_ms": 1,
            "updated_ms": 1
        }))
        .unwrap(),
    )
    .unwrap();
    for name in ["memory.key", "memory.key-keks.json"] {
        let path = home.join(name);
        std::fs::write(&path, format!("{name}-bytes")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let upgraded = run_ok(&["upgrade", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(upgraded["from_version"], 1);
    assert_eq!(upgraded["to_version"], 2);
    assert_eq!(upgraded["rolled_back"], false);
    assert!(home.join("keys/memory.key").is_file());
    assert!(home.join("keys/memory.key-keks.json").is_file());
    assert!(!home.join("memory.key").exists());

    let report = run_ok(&["doctor", "--root", &home.display().to_string(), "--json"]);
    assert_eq!(report["status"], "ok", "{report:?}");

    let rolled_back = run_ok(&[
        "upgrade",
        "--root",
        &home.display().to_string(),
        "--rollback",
        "--json",
    ]);
    assert_eq!(rolled_back["to_version"], 1);
    assert_eq!(rolled_back["rolled_back"], true);
    assert!(home.join("memory.key").is_file());
    assert!(home.join("memory.key-keks.json").is_file());
    assert!(!home.join("keys").exists());
    let marker: Value =
        serde_json::from_slice(&std::fs::read(home.join("home.json")).unwrap()).unwrap();
    assert_eq!(marker["version"], 1);
}
