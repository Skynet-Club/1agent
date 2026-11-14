use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc;

fn start_webhook() -> (String, mpsc::Receiver<String>) {
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
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .ok();
    });
    (format!("http://{address}/hook"), receiver)
}

#[test]
fn digest_posts_its_text_summary() {
    let workspace = tempfile::tempdir().unwrap();
    let receipt_log = workspace.path().join("digest-notify.jsonl");
    let (webhook, received) = start_webhook();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["digest", "--journal-dir"])
        .arg(workspace.path().join("journals"))
        .args(["--schedule-store"])
        .arg(workspace.path().join("schedules.json"))
        .args(["--inbox-store"])
        .arg(workspace.path().join("inbox.jsonl"))
        .args(["--notify-webhook", &webhook, "--notify-format", "slack"])
        .arg("--notify-log")
        .arg(&receipt_log)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["inbox"]["open"], 0);

    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
    let text = payload["text"].as_str().unwrap();
    assert!(text.contains("1Agent Digest"));
    assert!(text.contains("Inbox 待办：0 条"));

    let receipt: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(&receipt_log).unwrap().trim()).unwrap();
    assert_eq!(receipt["format"], "slack");
    assert_eq!(receipt["inbox_open"], 0);
    assert_eq!(receipt["runs"], 0);
    assert_eq!(receipt["alerts"], 0);
}

#[test]
fn digest_rejects_a_receipt_log_without_a_webhook() {
    let workspace = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["digest", "--journal-dir"])
        .arg(workspace.path().join("journals"))
        .args(["--schedule-store"])
        .arg(workspace.path().join("schedules.json"))
        .args(["--inbox-store"])
        .arg(workspace.path().join("inbox.jsonl"))
        .arg("--notify-log")
        .arg(workspace.path().join("receipts.jsonl"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--notify-log requires --notify-webhook"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn digest_rejects_an_insecure_webhook() {
    let workspace = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["digest", "--journal-dir"])
        .arg(workspace.path().join("journals"))
        .args(["--schedule-store"])
        .arg(workspace.path().join("schedules.json"))
        .args(["--inbox-store"])
        .arg(workspace.path().join("inbox.jsonl"))
        .args(["--notify-webhook", "http://example.com/hook"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
