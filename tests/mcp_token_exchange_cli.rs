use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc;

use sha2::Digest;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn start_token_server(response: &'static str) -> (String, mpsc::Receiver<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let read = stream.read(&mut chunk).unwrap_or(0);
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&buffer[..end]).to_string();
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if buffer.len() >= end + 4 + length {
                    let body = buffer[end + 4..end + 4 + length].to_vec();
                    let _ = sender.send(String::from_utf8_lossy(&body).to_string());
                    break;
                }
            }
        }
        let body = response.as_bytes();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .ok();
        stream.write_all(body).ok();
    });
    (format!("http://{address}/token"), receiver)
}

#[test]
fn mcp_token_exchange_writes_a_private_token_file() {
    let workspace = tempfile::tempdir().unwrap();
    let out = workspace.path().join("mcp.token");
    let (endpoint, form) = start_token_server(
        r#"{"access_token":"issued-token","token_type":"Bearer","expires_in":300}"#,
    );
    let output = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint", &endpoint])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(&out)
        .arg("--json")
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("issued-token"));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["token_type"], "Bearer");
    assert_eq!(report["expires_in_seconds"], 300);
    let expected = hex::encode(sha2::Sha256::digest(b"issued-token"));
    assert_eq!(report["access_token_sha256"], expected);

    let written = std::fs::read_to_string(&out).unwrap();
    assert_eq!(written.trim(), "issued-token");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&out).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let form = form
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(form.contains("grant_type=urn"), "form: {form}");
    assert!(
        form.contains("subject_token=subject-secret"),
        "form: {form}"
    );
    assert!(form.contains("audience=mcp%3A%2F%2Fserver"), "form: {form}");
    assert!(form.contains("scope=tools.read"), "form: {form}");
}

#[test]
fn mcp_token_exchange_fails_closed_on_invalid_inputs() {
    let workspace = tempfile::tempdir().unwrap();
    let missing_env = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint"])
        .arg("http://127.0.0.1:9/token")
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "MISSING_SUBJECT_TOKEN_XYZ",
            "--out",
        ])
        .arg(workspace.path().join("missing.token"))
        .output()
        .unwrap();
    assert!(!missing_env.status.success());
    assert!(
        String::from_utf8_lossy(&missing_env.stderr).contains("is not set or empty"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_env.stderr)
    );

    let insecure = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint"])
        .arg("http://example.com/token")
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("insecure.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!insecure.status.success());
    assert!(
        String::from_utf8_lossy(&insecure.stderr).contains("https"),
        "stderr: {}",
        String::from_utf8_lossy(&insecure.stderr)
    );

    let (endpoint, _form) =
        start_token_server(r#"{"access_token":"t","token_type":"mac","expires_in":300}"#);
    let wrong_type = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint", &endpoint])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("wrong.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!wrong_type.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_type.stderr).contains("Bearer"),
        "stderr: {}",
        String::from_utf8_lossy(&wrong_type.stderr)
    );

    let (endpoint, _form) =
        start_token_server(r#"{"access_token":"t","token_type":"Bearer","expires_in":99999}"#);
    let too_long = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint", &endpoint])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("long.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!too_long.status.success());
    assert!(
        String::from_utf8_lossy(&too_long.stderr).contains("expires_in"),
        "stderr: {}",
        String::from_utf8_lossy(&too_long.stderr)
    );

    let partial_client = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint"])
        .arg("http://127.0.0.1:9/token")
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--client-id-env",
            "TEST_CLIENT_ID",
            "--out",
        ])
        .arg(workspace.path().join("partial.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!partial_client.status.success());
    assert!(
        String::from_utf8_lossy(&partial_client.stderr).contains("required together"),
        "stderr: {}",
        String::from_utf8_lossy(&partial_client.stderr)
    );
}

#[test]
fn mcp_token_exchange_refuses_to_overwrite_the_output_file() {
    let workspace = tempfile::tempdir().unwrap();
    let out = workspace.path().join("existing.token");
    std::fs::write(&out, "keep-me").unwrap();
    let (endpoint, _form) =
        start_token_server(r#"{"access_token":"new-token","token_type":"Bearer","expires_in":60}"#);
    let output = Command::new(bin())
        .args(["mcp-token-exchange", "--endpoint", &endpoint])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(&out)
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "keep-me");
}
