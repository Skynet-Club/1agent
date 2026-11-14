use std::process::Command;

fn claims_path(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("claims.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "subject": "user-1",
            "audience": "mcp://calendar",
            "issued_at_ms": 1_000_000,
            "expires_at_ms": 1_060_000,
            "scopes": ["calendar:read"],
        }))
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn mcp_token_check_accepts_valid_claims() {
    let workspace = tempfile::tempdir().unwrap();
    let claims = claims_path(workspace.path());
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "mcp-token-check",
            "--claims",
            claims.to_str().unwrap(),
            "--audience",
            "mcp://calendar",
            "--scope",
            "calendar:read",
            "--now-ms",
            "1030000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(decision["authorized"], true);
    assert_eq!(decision["subject"], "user-1");
}

#[test]
fn mcp_token_check_rejects_audience_scope_ttl_and_expiry() {
    let workspace = tempfile::tempdir().unwrap();
    let claims = claims_path(workspace.path());
    let base = ["mcp-token-check", "--claims", claims.to_str().unwrap()];
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let cases: [&[&str]; 4] = [
        &[
            "--audience",
            "mcp://mail",
            "--scope",
            "calendar:read",
            "--now-ms",
            "1030000",
        ],
        &[
            "--audience",
            "mcp://calendar",
            "--scope",
            "calendar:write",
            "--now-ms",
            "1030000",
        ],
        &[
            "--audience",
            "mcp://calendar",
            "--scope",
            "calendar:read",
            "--now-ms",
            "1030000",
            "--max-ttl-ms",
            "1000",
        ],
        &[
            "--audience",
            "mcp://calendar",
            "--scope",
            "calendar:read",
            "--now-ms",
            "2000000",
        ],
    ];
    for args in cases {
        let output = Command::new(bin)
            .args(base)
            .args(args)
            .arg("--json")
            .output()
            .unwrap();
        assert!(!output.status.success(), "unexpected success for {args:?}");
        let decision: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(decision["authorized"], false);
        assert!(!String::from_utf8_lossy(&output.stderr).contains("user-1"));
    }
}

#[test]
fn mcp_token_check_rejects_bad_claims_json() {
    let workspace = tempfile::tempdir().unwrap();
    let claims = workspace.path().join("claims.json");
    std::fs::write(&claims, b"not-json").unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args([
            "mcp-token-check",
            "--claims",
            claims.to_str().unwrap(),
            "--audience",
            "mcp://calendar",
            "--scope",
            "calendar:read",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
