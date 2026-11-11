use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc;

fn start_collector_with(status: &'static str) -> (String, mpsc::Receiver<(String, String)>) {
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
                    let _ = sender.send((headers, String::from_utf8_lossy(&body).to_string()));
                    break;
                }
            }
        }
        stream
            .write_all(
                format!("{status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
            )
            .ok();
    });
    (format!("http://{address}/v1/traces"), receiver)
}

fn start_collector() -> (String, mpsc::Receiver<(String, String)>) {
    start_collector_with("HTTP/1.1 200 OK")
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
    (format!("http://{address}/v1/traces"), receiver)
}

fn journal_with_spans(dir: &std::path::Path) -> std::path::PathBuf {
    let workspace = dir.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let note = workspace.join("note.txt");
    std::fs::write(&note, "hello from otel").unwrap();
    let journal_dir = dir.join("journals");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let run = Command::new(bin)
        .args(["run", "--task"])
        .arg(format!("@read-file:\"{}\"", note.display()))
        .arg("--read-root")
        .arg(&workspace)
        .arg("--journal-dir")
        .arg(&journal_dir)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "run stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::read_dir(&journal_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
        .expect("journal")
}

#[test]
fn otel_export_posts_an_otlp_json_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let journal = journal_with_spans(dir.path());
    let (endpoint, collector) = start_collector();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["otel-export", "--journal"])
        .arg(&journal)
        .args([
            "--endpoint",
            &endpoint,
            "--service-name",
            "1agent-test",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["spans"].as_u64().unwrap() >= 2);
    assert_eq!(report["status"], 200);

    let (headers, body) = collector
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        !headers.to_ascii_lowercase().contains("authorization:"),
        "headers: {headers}"
    );
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
    let resource = &payload["resourceSpans"][0]["resource"]["attributes"][0];
    assert_eq!(resource["key"], "service.name");
    assert_eq!(resource["value"]["stringValue"], "1agent-test");
    let spans = payload["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .unwrap();
    let root = spans
        .iter()
        .find(|span| span["name"].as_str().unwrap().starts_with("invoke_agent"))
        .expect("root span");
    assert_eq!(root["traceId"].as_str().unwrap().len(), 32);
    assert_eq!(root["spanId"].as_str().unwrap().len(), 16);
    assert_eq!(root["kind"], 1);
    assert!(
        !root["attributes"][0]["value"]["stringValue"]
            .as_str()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn otel_export_reads_the_bearer_token_only_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let journal = journal_with_spans(dir.path());
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let (endpoint, collector) = start_collector();
    let output = Command::new(bin)
        .args(["otel-export", "--journal"])
        .arg(&journal)
        .args(["--endpoint", &endpoint, "--token-env", "TEST_OTEL_TOKEN"])
        .env("TEST_OTEL_TOKEN", "test-token-value")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let (headers, _) = collector
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("authorization: bearer test-token-value"),
        "headers: {headers}"
    );

    let missing = Command::new(bin)
        .args(["otel-export", "--journal"])
        .arg(&journal)
        .args([
            "--endpoint",
            "http://127.0.0.1:9/v1/traces",
            "--token-env",
            "MISSING_OTEL_TOKEN_XYZ",
        ])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("is not set"),
        "stderr: {}",
        String::from_utf8_lossy(&missing.stderr)
    );

    let insecure = Command::new(bin)
        .args(["otel-export", "--journal"])
        .arg(&journal)
        .args(["--endpoint", "http://example.com/v1/traces"])
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
fn otel_export_does_not_follow_redirects() {
    let dir = tempfile::tempdir().unwrap();
    let journal = journal_with_spans(dir.path());
    let (endpoint, followed) = start_redirect();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["otel-export", "--journal"])
        .arg(&journal)
        .args(["--endpoint", &endpoint])
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

#[test]
fn otel_export_fails_closed_on_a_collector_error() {
    let dir = tempfile::tempdir().unwrap();
    let journal = journal_with_spans(dir.path());
    let (endpoint, _collector) = start_collector_with("HTTP/1.1 503 Service Unavailable");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["otel-export", "--journal"])
        .arg(&journal)
        .args(["--endpoint", &endpoint])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("503"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
