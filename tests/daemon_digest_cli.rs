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

#[test]
fn daemon_posts_a_lightweight_digest() {
    let workspace = tempfile::tempdir().unwrap();
    let inbox = workspace.path().join("inbox.jsonl");
    let added = Command::new(bin())
        .args(["inbox", "add", "--store"])
        .arg(&inbox)
        .args(["--kind", "review", "--title", "Approve the deploy"])
        .output()
        .unwrap();
    assert!(added.status.success());
    let (webhook, received) = start_webhook();
    let output = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args(["--once", "--digest-webhook", &webhook])
        .args(["--digest-every-seconds", "60", "--digest-journal-dir"])
        .arg(workspace.path().join("runs"))
        .arg("--digest-inbox-store")
        .arg(&inbox)
        .args(["--digest-escalation-step", "0=urgent"])
        .arg("--digest-state")
        .arg(workspace.path().join("digest-state.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
    let text = payload["text"].as_str().unwrap();
    assert!(text.contains("1Agent Digest (daemon)"), "{text}");
    assert!(text.contains("schedules: 0"), "{text}");
    assert!(text.contains("inbox_open: 1"), "{text}");
    assert!(text.contains("escalation: urgent=1"), "{text}");
}

#[test]
fn daemon_rejects_incomplete_digest_configuration() {
    let workspace = tempfile::tempdir().unwrap();
    let too_short = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args([
            "--once",
            "--digest-webhook",
            "http://127.0.0.1:9/hook",
            "--digest-every-seconds",
            "30",
        ])
        .output()
        .unwrap();
    assert!(!too_short.status.success());
    assert!(
        String::from_utf8_lossy(&too_short.stderr).contains("60..=86400"),
        "stderr: {}",
        String::from_utf8_lossy(&too_short.stderr)
    );

    let missing_webhook = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args(["--once", "--digest-every-seconds", "60"])
        .output()
        .unwrap();
    assert!(!missing_webhook.status.success());
    assert!(
        String::from_utf8_lossy(&missing_webhook.stderr).contains("--digest-webhook"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_webhook.stderr)
    );
}

#[test]
fn daemon_digest_state_suppresses_a_restart_resend() {
    let workspace = tempfile::tempdir().unwrap();
    let state = workspace.path().join("digest-state.json");
    std::fs::write(&state, br#"{"last_sent_ms":9999999999999}"#).unwrap();
    let suppressed = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args([
            "--once",
            "--digest-webhook",
            "http://127.0.0.1:9/hook",
            "--digest-every-seconds",
            "60",
        ])
        .arg("--digest-state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        suppressed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&suppressed.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&suppressed.stderr).contains("cannot send digest"),
        "stderr: {}",
        String::from_utf8_lossy(&suppressed.stderr)
    );
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    assert_eq!(saved["last_sent_ms"], 9999999999999u64);

    std::fs::write(&state, br#"{"last_sent_ms":0}"#).unwrap();
    let attempted = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args([
            "--once",
            "--digest-webhook",
            "http://127.0.0.1:9/hook",
            "--digest-every-seconds",
            "60",
        ])
        .arg("--digest-state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(attempted.status.success());
    assert!(
        String::from_utf8_lossy(&attempted.stderr).contains("cannot send digest"),
        "stderr: {}",
        String::from_utf8_lossy(&attempted.stderr)
    );
}

#[test]
fn daemon_digest_escalation_steps_require_an_inbox_store() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args([
            "--once",
            "--digest-webhook",
            "http://127.0.0.1:9/hook",
            "--digest-every-seconds",
            "60",
            "--digest-escalation-step",
            "0=urgent",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--digest-inbox-store"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn quiet_window_covering_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let minute = ((now / 60_000) % 1_440) as u16;
    let end = (minute + 2) % 1_440;
    format!(
        "{:02}:{:02}-{:02}:{:02}",
        minute / 60,
        minute % 60,
        end / 60,
        end % 60
    )
}

#[test]
fn daemon_quiet_hours_skip_the_digest() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args(["--once", "--quiet-hours", &quiet_window_covering_now()])
        .args([
            "--digest-webhook",
            "http://127.0.0.1:9/hook",
            "--digest-every-seconds",
            "60",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("cannot send digest"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
