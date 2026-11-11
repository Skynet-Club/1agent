use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc;

use sha2::Digest;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn start_redirect() -> (String, mpsc::Receiver<bool>) {
    let primary = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = primary.local_addr().unwrap();
    let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target_address = target.local_addr().unwrap();
    target.set_nonblocking(true).unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = primary.accept().unwrap();
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer);
        stream
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .ok();
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
        let mut followed = false;
        while std::time::Instant::now() < deadline {
            match target.accept() {
                Ok(_) => {
                    followed = true;
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
        let _ = sender.send(followed);
    });
    (format!("http://{address}/token"), receiver)
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

    let (endpoint, _form) =
        start_token_server(r#"{"access_token":"bad token","token_type":"Bearer","expires_in":60}"#);
    let both_sources = Command::new(bin())
        .args([
            "mcp-token-exchange",
            "--endpoint",
            "http://127.0.0.1:9/token",
            "--issuer",
            "http://127.0.0.1:9",
        ])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("both-sources.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!both_sources.status.success());

    let bad_token = Command::new(bin())
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
        .arg(workspace.path().join("bad-token.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!bad_token.status.success());
    assert!(
        String::from_utf8_lossy(&bad_token.stderr).contains("access_token"),
        "stderr: {}",
        String::from_utf8_lossy(&bad_token.stderr)
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

#[test]
fn mcp_token_exchange_does_not_follow_redirects() {
    let workspace = tempfile::tempdir().unwrap();
    let (endpoint, followed) = start_redirect();
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
        .arg(workspace.path().join("redirect.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("302"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !followed
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
    );
}

fn start_discovery_server(
    token_endpoint: String,
    issuer_path: &'static str,
    override_issuer: Option<&'static str>,
) -> (String, mpsc::Receiver<Vec<String>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}{issuer_path}");
    let metadata_issuer = override_issuer
        .map(str::to_string)
        .unwrap_or_else(|| issuer.clone());
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut paths = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("")
                .to_string();
            paths.push(path.clone());
            if path.contains("openid-configuration") {
                let body =
                    serde_json::json!({"issuer": metadata_issuer.clone(), "token_endpoint": token_endpoint})
                        .to_string();
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        )
                        .as_bytes(),
                    )
                    .ok();
            } else {
                stream
                    .write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .ok();
            }
        }
        let _ = sender.send(paths);
    });
    (issuer, receiver)
}

#[test]
fn mcp_token_exchange_discovers_the_token_endpoint() {
    let workspace = tempfile::tempdir().unwrap();
    let out = workspace.path().join("discovered.token");
    let (token_endpoint, _token_form) = start_token_server(
        r#"{"access_token":"discovered-token","token_type":"Bearer","expires_in":60}"#,
    );
    let (issuer, paths) = start_discovery_server(token_endpoint, "/tenant", None);
    let output = Command::new(bin())
        .args(["mcp-token-exchange", "--issuer", &issuer])
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
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&out).unwrap().trim(),
        "discovered-token"
    );
    let paths = paths
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert_eq!(paths.len(), 2);
    assert!(paths[0].contains("oauth-authorization-server"), "{paths:?}");
    assert!(paths[1].contains("openid-configuration"), "{paths:?}");
    assert!(
        paths[0].ends_with("/.well-known/oauth-authorization-server/tenant"),
        "{paths:?}"
    );
    assert!(
        paths[1].ends_with("/tenant/.well-known/openid-configuration"),
        "{paths:?}"
    );

    let insecure = Command::new(bin())
        .args(["mcp-token-exchange", "--issuer", "http://example.com"])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("insecure-issuer.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!insecure.status.success());
    assert!(
        String::from_utf8_lossy(&insecure.stderr).contains("https"),
        "stderr: {}",
        String::from_utf8_lossy(&insecure.stderr)
    );
}

#[test]
fn issuer_discovery_does_not_follow_redirects() {
    let workspace = tempfile::tempdir().unwrap();
    let (issuer, followed) = start_redirect();
    let output = Command::new(bin())
        .args(["mcp-token-exchange", "--issuer", &issuer])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("issuer-redirect.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        !followed
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
    );
}

#[test]
fn issuer_discovery_rejects_a_mismatched_metadata_issuer() {
    let workspace = tempfile::tempdir().unwrap();
    let (issuer, _paths) = start_discovery_server(
        "http://127.0.0.1:9/token".into(),
        "/tenant",
        Some("https://evil.example"),
    );
    let output = Command::new(bin())
        .args(["mcp-token-exchange", "--issuer", &issuer])
        .args([
            "--audience",
            "mcp://server",
            "--scope",
            "tools.read",
            "--subject-token-env",
            "TEST_SUBJECT_TOKEN",
            "--out",
        ])
        .arg(workspace.path().join("mismatch.token"))
        .env("TEST_SUBJECT_TOKEN", "subject-secret")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("different issuer"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
