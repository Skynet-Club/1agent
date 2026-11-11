use std::process::Command;

fn demo_store(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let store = dir.path().join("task-graph.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let demo = Command::new(bin)
        .args(["task-graph", "demo", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(
        demo.status.success(),
        "demo stderr: {}",
        String::from_utf8_lossy(&demo.stderr)
    );
    store
}

#[test]
fn away_digest_cli_surfaces_the_demo_side_effect() {
    let dir = tempfile::tempdir().unwrap();
    let store = demo_store(&dir);
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["away-digest", "--store"])
        .arg(&store)
        .args(["--since-ms", "0", "--until-ms", "99999999999999", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "digest stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let digest: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(digest["applied"].as_array().unwrap().len(), 1);
    assert!(digest["needs_user"].as_array().unwrap().is_empty());
    assert_eq!(
        digest["receipts"][0]["compensation"],
        "remove demo/result.txt"
    );
    assert_eq!(digest["receipts"][0]["reversible"], true);
}

#[test]
fn away_digest_cli_human_output_is_readable() {
    let dir = tempfile::tempdir().unwrap();
    let store = demo_store(&dir);
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["away-digest", "--store"])
        .arg(&store)
        .args(["--since-ms", "0", "--until-ms", "99999999999999"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("1Agent Away Digest"));
    assert!(text.contains("applied=1"));
    assert!(text.contains("undo=remove demo/result.txt"));
}
