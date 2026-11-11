use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

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

fn add_item(store: &std::path::Path, title: &str) {
    let output = Command::new(bin())
        .args(["inbox", "add", "--store"])
        .arg(store)
        .args(["--kind", "review", "--title", title])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn inbox_notify_fans_out_to_targets_with_their_own_formats() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let state = workspace.path().join("notify-state.json");
    let log = workspace.path().join("notify.jsonl");
    add_item(&store, "Approve the deploy");
    add_item(&store, "Review the draft");

    let (slack_url, slack) = start_webhook();
    let (discord_url, discord) = start_webhook();
    let output = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .arg("--target")
        .arg(format!("slack={slack_url}"))
        .arg("--target")
        .arg(format!("discord={discord_url}"))
        .arg("--state")
        .arg(&state)
        .arg("--log")
        .arg(&log)
        .arg("--json")
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
    assert_eq!(report["targets"], 2);

    let slack_body: serde_json::Value = serde_json::from_str(
        &slack
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
    )
    .unwrap();
    assert!(
        slack_body["text"]
            .as_str()
            .unwrap()
            .contains("Approve the deploy")
    );
    let discord_body: serde_json::Value = serde_json::from_str(
        &discord
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap(),
    )
    .unwrap();
    assert!(
        discord_body["content"]
            .as_str()
            .unwrap()
            .contains("Approve the deploy")
    );
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);

    // The shared watermark suppresses both targets on a repeat run.
    let repeat = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .arg("--target")
        .arg("slack=http://127.0.0.1:9/hook")
        .arg("--state")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&repeat.stdout).unwrap();
    assert_eq!(report["sent"], false);
    assert_eq!(report["new_items"], 0);
}

#[test]
fn inbox_notify_partial_target_failure_keeps_the_watermark() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let state = workspace.path().join("notify-state.json");
    add_item(&store, "Needs fan-out");

    let (first_url, first_received) = start_webhook();
    let (second_url, second_received) = start_webhook();
    let baseline = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .arg("--target")
        .arg(format!("slack={first_url}"))
        .arg("--target")
        .arg(format!("discord={second_url}"))
        .arg("--state")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        baseline.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&baseline.stderr)
    );
    let _ = first_received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let _ = second_received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let before = std::fs::read(&state).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    add_item(&store, "Second item");

    let (url, received) = start_webhook();
    let partial = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .arg("--target")
        .arg(format!("slack={url}"))
        .arg("--target")
        .arg("discord=http://127.0.0.1:9/hook")
        .arg("--state")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert!(!partial.status.success());
    let _ = received.recv_timeout(std::time::Duration::from_secs(5));
    assert_eq!(
        std::fs::read(&state).unwrap(),
        before,
        "the failed fan-out must not advance the watermark"
    );

    let (slack_url, slack) = start_webhook();
    let (discord_url, discord) = start_webhook();
    let retry = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .arg("--target")
        .arg(format!("slack={slack_url}"))
        .arg("--target")
        .arg(format!("discord={discord_url}"))
        .arg("--state")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        retry.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&retry.stderr)
    );
    let _ = slack
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let _ = discord
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();

    let neither = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .output()
        .unwrap();
    assert!(!neither.status.success());
    let both = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .args(["--webhook", "http://127.0.0.1:9/hook", "--target"])
        .arg("slack=http://127.0.0.1:9/hook")
        .output()
        .unwrap();
    assert!(!both.status.success());

    let bad_format = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .args(["--target", "pager=http://127.0.0.1:9/hook"])
        .output()
        .unwrap();
    assert!(!bad_format.status.success());

    let duplicate = Command::new(bin())
        .args(["inbox-notify", "--store"])
        .arg(&store)
        .args([
            "--target",
            "slack=http://127.0.0.1:9/hook",
            "--target",
            "slack=http://127.0.0.1:9/hook",
        ])
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert!(
        String::from_utf8_lossy(&duplicate.stderr).contains("duplicate --target"),
        "stderr: {}",
        String::from_utf8_lossy(&duplicate.stderr)
    );
}
