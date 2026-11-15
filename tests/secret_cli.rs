//! CLI wiring for the model-blind browser credential store.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn secret_set_list_and_remove_never_print_the_value() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("secrets.json").display().to_string();
    let secret = "sk-super-secret-value";

    let set = run_with_stdin(
        &[
            "secret",
            "set",
            "--label",
            "vendor/password",
            "--origin",
            "https://vendor.example",
            "--value-stdin",
            "--store",
            &store,
            "--json",
        ],
        secret,
    );
    assert!(
        set.status.success(),
        "set stderr: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    assert!(!String::from_utf8_lossy(&set.stdout).contains(secret));

    let list = run(&["secret", "list", "--store", &store, "--json"]);
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("vendor/password"));
    assert!(stdout.contains("https://vendor.example"));
    assert!(!stdout.contains(secret));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&store).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    let remove = run(&[
        "secret",
        "remove",
        "--label",
        "vendor/password",
        "--store",
        &store,
        "--json",
    ]);
    assert!(remove.status.success());
    let list = run(&["secret", "list", "--store", &store, "--json"]);
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(!stdout.contains("vendor/password"));
}
