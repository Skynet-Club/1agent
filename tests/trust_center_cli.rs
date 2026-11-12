use std::process::Command;

#[test]
fn trust_center_writes_a_disclosure_with_evaluations() {
    let workspace = tempfile::tempdir().unwrap();
    let evaluation = workspace.path().join("eval.json");
    let output_path = workspace.path().join("trust.json");
    let markdown_path = workspace.path().join("trust.md");
    std::fs::write(
        &evaluation,
        serde_json::to_vec(&serde_json::json!({
            "name": "core-v2",
            "version": "1",
            "passed": 6,
            "total": 6,
            "evidence_sha256": "ab".repeat(32),
            "note": "contract fixture"
        }))
        .unwrap(),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let output = Command::new(bin)
        .args([
            "trust-center",
            "--evaluation",
            evaluation.to_str().unwrap(),
            "--security-contact",
            "security@example.com",
            "--incident-url",
            "https://example.com/incidents",
            "--out",
            output_path.to_str().unwrap(),
            "--markdown-out",
            markdown_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&output_path).unwrap()).unwrap();
    assert_eq!(printed, written);
    assert_eq!(printed["product"], "1Agent");
    assert_eq!(printed["audit"]["status"], "not_audited");
    assert_eq!(printed["audit"]["external_audit"], false);
    assert_eq!(printed["data_policy"]["telemetry"], "none");
    assert_eq!(printed["evaluations"][0]["passed"], 6);
    assert_eq!(printed["security_contact"], "security@example.com");
    assert_eq!(printed["incident_url"], "https://example.com/incidents");
    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("# 1Agent Trust Disclosure"));
    assert!(markdown.contains("Status: not_audited"));
    assert!(markdown.contains("core-v2 1: 6/6"));
    let capabilities = &printed["capabilities"];
    assert_eq!(
        capabilities["total"],
        capabilities["verified"].as_u64().unwrap()
            + capabilities["degraded"].as_u64().unwrap()
            + capabilities["unsupported"].as_u64().unwrap()
            + capabilities["untested"].as_u64().unwrap()
    );
}

#[test]
fn trust_center_includes_a_verification_summary() {
    let workspace = tempfile::tempdir().unwrap();
    let input = workspace.path().join("input.json");
    std::fs::write(
        &input,
        serde_json::to_vec(&serde_json::json!({
            "executor_id": "executor",
            "verifier_id": "verifier",
            "acceptance_criteria": ["public"],
            "hidden_criteria": ["hidden"],
            "criteria_results": [
                {"criterion": "public", "passed": true, "evidence": ["log"]},
                {"criterion": "hidden", "passed": false, "evidence": ["log"]}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let store = workspace.path().join("verification.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let recorded = Command::new(bin)
        .args(["verification", "record", "--store"])
        .arg(&store)
        .args(["--verification-id", "v1", "--run-id", "run-1", "--input"])
        .arg(&input)
        .args(["--json"])
        .output()
        .unwrap();
    assert!(recorded.status.success());

    let output = Command::new(bin)
        .args(["trust-center", "--verification-store"])
        .arg(&store)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let disclosure: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(disclosure["verification"]["verified"], 0);
    assert_eq!(disclosure["verification"]["hacked"], 1);
    assert_eq!(disclosure["verification"]["resolved"], 1);
    assert_eq!(disclosure["verification"]["hacked_resolved_bps"], 10000);
}

#[test]
fn trust_center_rejects_invalid_evaluations() {
    let workspace = tempfile::tempdir().unwrap();
    let evaluation = workspace.path().join("eval.json");
    std::fs::write(
        &evaluation,
        serde_json::to_vec(&serde_json::json!({
            "name": "bad",
            "version": "1",
            "passed": 7,
            "total": 6
        }))
        .unwrap(),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "trust-center",
            "--evaluation",
            evaluation.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn trust_center_writes_escaped_static_html() {
    let workspace = tempfile::tempdir().unwrap();
    let html_path = workspace.path().join("trust.html");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "trust-center",
            "--security-contact",
            "<script>alert(1)</script>",
            "--incident-url",
            "javascript:alert(1)",
            "--html-out",
            html_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let html = std::fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(!html.contains("href=\"javascript"));
    assert!(html.contains("not_audited"));
    assert!(html.contains("Static disclosure"));
}

#[test]
fn trust_center_accepts_and_escapes_incident_reports() {
    let workspace = tempfile::tempdir().unwrap();
    let incident = workspace.path().join("incident.json");
    let markdown_path = workspace.path().join("trust.md");
    let html_path = workspace.path().join("trust.html");
    std::fs::write(
        &incident,
        serde_json::to_vec(&serde_json::json!({
            "id": "INC-1",
            "title": "Test incident",
            "status": "resolved",
            "opened_at_ms": 10,
            "resolved_at_ms": 30,
            "summary": "summary <script>alert(1)</script>",
            "link": "https://example.com/incidents/1"
        }))
        .unwrap(),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["trust-center", "--incident-report"])
        .arg(&incident)
        .arg("--markdown-out")
        .arg(&markdown_path)
        .arg("--html-out")
        .arg(&html_path)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let disclosure: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(disclosure["incidents"][0]["id"], "INC-1");
    assert_eq!(disclosure["incidents"][0]["status"], "resolved");
    let markdown = std::fs::read_to_string(&markdown_path).unwrap();
    assert!(markdown.contains("## Incidents"));
    assert!(markdown.contains("[resolved] INC-1"));
    let html = std::fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>alert(1)</script>"));
    assert!(!html.contains("href=\"https://example.com/incidents/1\""));
}

#[test]
fn trust_center_rejects_an_inconsistent_incident_report() {
    let workspace = tempfile::tempdir().unwrap();
    let incident = workspace.path().join("incident.json");
    std::fs::write(
        &incident,
        serde_json::to_vec(&serde_json::json!({
            "id": "INC-2",
            "title": "Open incident",
            "status": "open",
            "opened_at_ms": 10,
            "resolved_at_ms": 20,
            "summary": "should be rejected"
        }))
        .unwrap(),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["trust-center", "--incident-report"])
        .arg(&incident)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("incident"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
