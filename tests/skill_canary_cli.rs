//! End-to-end coverage for `skill canary`.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn skill_markdown(version: &str) -> String {
    format!(
        "---\nname: release\ndescription: Release procedure\nversion: {version}\nscope: repo:test\n---\n1. verify\n2. build\n"
    )
}

fn register(store: &std::path::Path, skill: &std::path::Path) {
    let output = Command::new(bin())
        .args(["skill", "register", "--store"])
        .arg(store)
        .arg("--path")
        .arg(skill)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn activate(store: &std::path::Path, version: &str) {
    let output = Command::new(bin())
        .args(["skill", "activate", "--store"])
        .arg(store)
        .args(["--name", "release", "--version", version])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn setup(dir: &std::path::Path) -> std::path::PathBuf {
    let store = dir.join("skills.jsonl");
    let baseline = dir.join("baseline.md");
    let candidate = dir.join("candidate.md");
    std::fs::write(&baseline, skill_markdown("1.0.0")).unwrap();
    std::fs::write(&candidate, skill_markdown("1.1.0")).unwrap();
    register(&store, &baseline);
    register(&store, &candidate);
    activate(&store, "1.0.0");
    store
}

fn write_samples(path: &std::path::Path, lines: &[&str]) {
    std::fs::write(path, lines.join("\n")).unwrap();
}

fn canary(store: &std::path::Path, samples: &std::path::Path, apply: bool) -> std::process::Output {
    Command::new(bin())
        .args(["skill", "canary", "--store"])
        .arg(store)
        .args(["--name", "release", "--version", "1.1.0", "--samples"])
        .arg(samples)
        .args([
            "--min-samples",
            "4",
            "--max-failure-bps",
            "0",
            "--max-p95-latency-ms",
            "100",
            "--json",
        ])
        .args(if apply { vec!["--apply"] } else { Vec::new() })
        .output()
        .unwrap()
}

#[test]
fn healthy_samples_promote_the_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let samples = dir.path().join("samples.jsonl");
    write_samples(
        &samples,
        &[
            r#"{"success":true,"latency_ms":10}"#,
            r#"{"success":true,"latency_ms":12}"#,
            r#"{"success":true,"latency_ms":14}"#,
            r#"{"success":true,"latency_ms":16}"#,
        ],
    );
    let output = canary(&store, &samples, true);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "promoted");
    assert_eq!(report["decision"]["kind"], "promote");
    assert_eq!(report["samples"], 4);
    let journal = std::fs::read_to_string(&store).unwrap();
    assert!(
        journal.lines().any(|line| {
            line.contains("\"activated\"") && line.contains("\"version\":\"1.1.0\"")
        })
    );
}

#[test]
fn failures_roll_back_to_the_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let samples = dir.path().join("samples.jsonl");
    write_samples(
        &samples,
        &[
            r#"{"success":true,"latency_ms":10}"#,
            r#"{"success":false,"latency_ms":20}"#,
            r#"{"success":true,"latency_ms":12}"#,
            r#"{"success":false,"latency_ms":22}"#,
        ],
    );
    let output = canary(&store, &samples, true);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "rolled_back");
    assert_eq!(report["decision"]["kind"], "rollback");
    let journal = std::fs::read_to_string(&store).unwrap();
    assert!(
        !journal.lines().any(|line| {
            line.contains("\"activated\"") && line.contains("\"version\":\"1.1.0\"")
        })
    );
}

#[test]
fn too_few_samples_stay_pending_and_do_not_apply() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let samples = dir.path().join("samples.jsonl");
    write_samples(&samples, &[r#"{"success":true,"latency_ms":10}"#]);
    let output = canary(&store, &samples, true);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "running");
    assert_eq!(report["decision"]["kind"], "pending");
    let journal = std::fs::read_to_string(&store).unwrap();
    assert!(
        !journal.lines().any(|line| {
            line.contains("\"activated\"") && line.contains("\"version\":\"1.1.0\"")
        })
    );
}

#[test]
fn malformed_samples_fail_with_the_line_number() {
    let dir = tempfile::tempdir().unwrap();
    let store = setup(dir.path());
    let samples = dir.path().join("samples.jsonl");
    write_samples(
        &samples,
        &[r#"{"success":true,"latency_ms":10}"#, r#"{"success":true}"#],
    );
    let output = canary(&store, &samples, false);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("line 2"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
