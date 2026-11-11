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
fn inbox_notify_posts_open_items_to_a_slack_webhook() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    for title in ["Approve the deploy", "Review the draft"] {
        let added = Command::new(bin)
            .args(["inbox", "add", "--store"])
            .arg(&store)
            .args(["--kind", "review", "--title", title])
            .output()
            .unwrap();
        assert!(added.status.success());
    }

    let (webhook, received) = start_webhook();
    let output = Command::new(bin)
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .args(["--webhook", &webhook, "--format", "slack", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["sent"], true);
    assert_eq!(report["open"], 2);

    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
    let text = payload["text"].as_str().unwrap();
    assert!(text.contains("1Agent Inbox: 2 open"));
    assert!(text.contains("Approve the deploy"));
    assert!(text.contains("Review the draft"));
}

#[test]
fn inbox_notify_skips_empty_queues_and_rejects_insecure_urls() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let empty = Command::new(bin)
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .args(["--webhook", "http://127.0.0.1:9/hook", "--json"])
        .output()
        .unwrap();
    assert!(empty.status.success());
    let report: serde_json::Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert_eq!(report["sent"], false);
    assert_eq!(report["open"], 0);

    let insecure = Command::new(bin)
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .args(["--webhook", "http://example.com/hook"])
        .output()
        .unwrap();
    assert!(!insecure.status.success());
}
