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

fn add_item(store: &std::path::Path, title: &str) -> serde_json::Value {
    let output = Command::new(bin())
        .args(["inbox", "add", "--store"])
        .arg(store)
        .args(["--kind", "review", "--title", title, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
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
fn daemon_delivers_each_escalation_rung_once() {
    let workspace = tempfile::tempdir().unwrap();
    let inbox_store = workspace.path().join("inbox.jsonl");
    let state = workspace.path().join("escalation-state.json");
    let schedules = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");
    let item = add_item(&inbox_store, "daemon escalation");

    let (webhook, received) = start_webhook();
    let first = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(&schedules)
        .arg("--journal-dir")
        .arg(&journals)
        .args(["--once", "--escalate-inbox-store"])
        .arg(&inbox_store)
        .args([
            "--escalate-step",
            "0=notify",
            "--escalate-webhook",
            &webhook,
        ])
        .arg("--escalate-state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(body.contains("[notify]"), "body: {body}");
    assert!(body.contains("daemon escalation"), "body: {body}");
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    assert_eq!(
        saved["delivered"][item["item_id"].as_str().unwrap()],
        "notify"
    );

    let (webhook, received) = start_webhook();
    let raised = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(&schedules)
        .arg("--journal-dir")
        .arg(&journals)
        .args(["--once", "--escalate-inbox-store"])
        .arg(&inbox_store)
        .args([
            "--escalate-step",
            "0=urgent",
            "--escalate-webhook",
            &webhook,
        ])
        .arg("--escalate-state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        raised.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&raised.stderr)
    );
    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(body.contains("[urgent]"), "body: {body}");

    // The same rung must not be retried; a closed port would log a send
    // failure if the daemon tried to deliver it again.
    let repeat = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(&schedules)
        .arg("--journal-dir")
        .arg(&journals)
        .args(["--once", "--escalate-inbox-store"])
        .arg(&inbox_store)
        .args([
            "--escalate-step",
            "0=urgent",
            "--escalate-webhook",
            "http://127.0.0.1:9/hook",
        ])
        .arg("--escalate-state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&repeat.stderr).contains("inbox escalation failed"),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
}

#[test]
fn daemon_quiet_hours_suppress_escalation_delivery() {
    let workspace = tempfile::tempdir().unwrap();
    let inbox_store = workspace.path().join("inbox.jsonl");
    let state = workspace.path().join("escalation-state.json");
    let schedules = workspace.path().join("schedules.json");
    let journals = workspace.path().join("journals");
    add_item(&inbox_store, "quiet escalation");

    let output = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(&schedules)
        .arg("--journal-dir")
        .arg(&journals)
        .args(["--once", "--quiet-hours", &quiet_window_covering_now()])
        .arg("--escalate-inbox-store")
        .arg(&inbox_store)
        .args([
            "--escalate-step",
            "0=notify",
            "--escalate-webhook",
            "http://127.0.0.1:9/hook",
        ])
        .arg("--escalate-state")
        .arg(&state)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!state.exists(), "quiet hours must not advance the state");
}

#[test]
fn daemon_rejects_partial_escalation_configuration() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(bin())
        .args(["daemon", "--store"])
        .arg(workspace.path().join("schedules.json"))
        .arg("--journal-dir")
        .arg(workspace.path().join("journals"))
        .args(["--once", "--escalate-step", "0=notify"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--escalate-inbox-store"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
