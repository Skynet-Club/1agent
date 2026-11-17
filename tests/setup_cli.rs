//! Red tests for the 0.98 setup/status/help surface.
//!
//! The first slice is not implemented yet, so every test is ignored. They
//! compile today and become the acceptance suite for S1-03/S1-08/S1-09.
//!
//! Run once the implementation lands:
//! `cargo test --offline --test setup_cli -- --ignored`

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(args)
        .output()
        .unwrap()
}

fn run_json(args: &[&str]) -> (Output, Value) {
    let output = run(args);
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout is not JSON: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (output, value)
}

fn run_with_home(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .env("ONEAGENT_HOME", home)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn setup_check_on_fresh_home_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let home_arg = home.display().to_string();

    let (output, json) = run_json(&["setup", "--check", "--root", &home_arg, "--json"]);

    assert!(!output.status.success());
    assert_eq!(json["schema"], "1agent.cli/v1");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "setup.not_ready");
    assert!(
        json["error"]["next"]
            .as_str()
            .is_some_and(|next| !next.is_empty())
    );
    assert!(
        !home.exists(),
        "setup --check must not create the runtime home"
    );
}

#[test]
fn setup_non_interactive_requires_explicit_provider() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let home_arg = home.display().to_string();

    let output = run(&["setup", "--non-interactive", "--root", &home_arg, "--json"]);

    assert_eq!(output.status.code(), Some(2));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "setup.missing_provider");
    assert!(
        json["error"]["next"]
            .as_str()
            .is_some_and(|next| !next.is_empty())
    );
}

#[test]
fn setup_check_does_not_write_setup_state() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let home_arg = home.display().to_string();

    let init = run(&["init", "--root", &home_arg, "--json"]);
    assert!(
        init.status.success(),
        "init stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let config = home.join("config.json");
    let before = std::fs::metadata(&config).ok().map(|meta| meta.len());

    let (_output, _json) = run_json(&["setup", "--check", "--root", &home_arg, "--json"]);

    assert!(!home.join("setup.json").exists());
    let after = std::fs::metadata(&config).ok().map(|meta| meta.len());
    assert_eq!(before, after, "setup --check must not rewrite config");
}

#[test]
fn setup_rejects_api_key_argument() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let home_arg = home.display().to_string();
    let workspace_arg = dir.path().display().to_string();

    let output = run(&[
        "setup",
        "--non-interactive",
        "--root",
        &home_arg,
        "--provider",
        "deepseek",
        "--model",
        "deepseek-chat",
        "--permission-preset",
        "safe",
        "--workspace",
        &workspace_arg,
        "--api-key",
        "sk-super-secret",
        "--json",
    ]);

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains("sk-super-secret"));
    assert!(!stderr.contains("sk-super-secret"));

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "setup.api_key_argv_forbidden");
}

#[test]
fn status_json_contract_on_unconfigured_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let home_arg = home.display().to_string();

    let (output, json) = run_json(&["status", "--root", &home_arg, "--json"]);

    assert!(!output.status.success());
    assert_eq!(json["schema"], "1agent.status/v1");
    assert_eq!(json["ok"], false);
    assert!(json["next"].as_str().is_some_and(|next| !next.is_empty()));
}

#[test]
fn help_advanced_redirects_to_all_details() {
    let output = run(&["help", "advanced"]);

    assert!(
        output.status.success(),
        "help advanced stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("高级命令详情已合并"));
    assert!(stdout.contains("1agent help --all"));
    assert!(stdout.contains("1agent help <分类>"));
}

#[test]
fn completions_are_generated() {
    let output = run(&["completions", "bash"]);

    assert!(
        output.status.success(),
        "completions stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1agent"));
    assert!(stdout.contains("setup"));
}

#[test]
fn setup_deterministic_is_idempotent_and_status_ready() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let home_arg = home.display().to_string();
    let workspace_arg = workspace.display().to_string();
    let args = [
        "setup",
        "--non-interactive",
        "--root",
        &home_arg,
        "--provider",
        "deterministic",
        "--model",
        "not-a-real-deterministic-model",
        "--permission-preset",
        "safe",
        "--workspace",
        &workspace_arg,
        "--skip-probe",
        "--skip-smoke",
        "--json",
    ];

    let (output, json) = run_json(&args);
    assert!(
        output.status.success(),
        "setup stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(json["schema"], "1agent.cli/v1");
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["provider"]["id"], "deterministic");
    assert!(home.join("config.json").is_file());
    assert!(home.join("setup.json").is_file());

    let config_before = std::fs::read(home.join("config.json")).unwrap();
    let (output, _json) = run_json(&args);
    assert!(
        output.status.success(),
        "second setup stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config_after = std::fs::read(home.join("config.json")).unwrap();
    assert_eq!(config_before, config_after, "setup must be idempotent");

    let (output, status) = run_json(&["status", "--root", &home_arg, "--json"]);
    assert!(output.status.success());
    assert_eq!(status["schema"], "1agent.status/v1");
    assert_eq!(status["ok"], true);
    assert_eq!(status["provider"]["id"], "deterministic");
    assert_eq!(status["provider"]["model"], "offline-v1");
    assert_eq!(status["permissions"]["preset"], "safe");
}

#[test]
fn permissions_show_and_set_are_config_backed() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let home_arg = home.display().to_string();
    let workspace_arg = workspace.display().to_string();
    let setup = run(&[
        "setup",
        "--non-interactive",
        "--root",
        &home_arg,
        "--provider",
        "deterministic",
        "--permission-preset",
        "safe",
        "--workspace",
        &workspace_arg,
        "--skip-probe",
        "--skip-smoke",
        "--json",
    ]);
    assert!(
        setup.status.success(),
        "setup stderr: {}",
        String::from_utf8_lossy(&setup.stderr)
    );

    let (_, shown) = run_json(&["permissions", "show", "--root", &home_arg, "--json"]);
    assert_eq!(shown["preset"], "safe");
    assert_eq!(shown["allow_write"], true);
    assert_eq!(shown["allow_network"], true);

    let (output, changed) = run_json(&[
        "permissions",
        "set",
        "--root",
        &home_arg,
        "--preset",
        "read-only",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "permissions set stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(changed["preset"], "read-only");
    assert_eq!(changed["allow_write"], false);

    let (_, shown) = run_json(&["permissions", "show", "--root", &home_arg, "--json"]);
    assert_eq!(shown["preset"], "read-only");
    assert_eq!(shown["allow_write"], false);
}

#[test]
fn model_list_marks_default_provider() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let home_arg = home.display().to_string();
    let workspace_arg = workspace.display().to_string();
    let setup = run(&[
        "setup",
        "--non-interactive",
        "--root",
        &home_arg,
        "--provider",
        "deterministic",
        "--permission-preset",
        "safe",
        "--workspace",
        &workspace_arg,
        "--skip-probe",
        "--skip-smoke",
        "--json",
    ]);
    assert!(
        setup.status.success(),
        "setup stderr: {}",
        String::from_utf8_lossy(&setup.stderr)
    );

    let (output, list) = run_json(&["model", "list", "--root", &home_arg, "--json"]);
    assert!(output.status.success());
    let providers = list.as_array().expect("model list must be an array");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["id"], "deterministic");
    assert_eq!(providers[0]["default"], true);
}

#[test]
fn setup_check_is_ready_and_run_uses_default_config() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let workspace = dir.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let home_arg = home.display().to_string();
    let workspace_arg = workspace.display().to_string();
    let setup = run(&[
        "setup",
        "--non-interactive",
        "--root",
        &home_arg,
        "--provider",
        "deterministic",
        "--permission-preset",
        "safe",
        "--workspace",
        &workspace_arg,
        "--skip-probe",
        "--skip-smoke",
        "--json",
    ]);
    assert!(
        setup.status.success(),
        "setup stderr: {}",
        String::from_utf8_lossy(&setup.stderr)
    );

    let (output, check) = run_json(&["setup", "--check", "--root", &home_arg, "--json"]);
    assert!(
        output.status.success(),
        "setup --check stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(check["data"]["ok"], true);

    let run = run_with_home(
        &home,
        &[
            "run",
            "--task",
            "Reply with OK",
            "--json",
            "--max-steps",
            "2",
        ],
    );
    assert!(
        run.status.success(),
        "run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let result: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert!(result["journal_path"].as_str().is_some());

    let (_, status) = run_json(&["status", "--root", &home_arg, "--json"]);
    assert_eq!(status["ok"], true);
    assert!(status["last_run"]["journal"].as_str().is_some());
}
