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

fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().unwrap()
}

fn add_item(store: &std::path::Path, title: &str) -> serde_json::Value {
    let output = run(&[
        "inbox",
        "add",
        "--store",
        store.to_str().unwrap(),
        "--kind",
        "review",
        "--title",
        title,
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn inbox_escalate_reports_only_open_items_at_their_age_level() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let open = add_item(&store, "needs a decision");
    let resolved = add_item(&store, "already handled");
    let resolve = run(&[
        "inbox",
        "resolve",
        "--store",
        store.to_str().unwrap(),
        "--item-id",
        resolved["item_id"].as_str().unwrap(),
        "--status",
        "dismissed",
    ]);
    assert!(
        resolve.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resolve.stderr)
    );

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + 60_000;
    let output = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "1000=inbox",
        "--step",
        "5000=notify",
        "--step",
        "50000=urgent",
        "--now-ms",
        &now.to_string(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["notification"].is_null());
    let rows = report["items"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "resolved items must not escalate: {rows:?}");
    assert_eq!(rows[0]["item_id"], open["item_id"]);
    assert_eq!(rows[0]["level"], "urgent");
    assert!(rows[0]["elapsed_ms"].as_u64().unwrap() >= 60_000);
    assert!(rows[0]["reason"].as_str().unwrap().contains("50000 ms"));
}

#[test]
fn inbox_escalate_below_the_first_threshold_reports_none() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let item = add_item(&store, "fresh item");
    let output = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "999999999=urgent",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = report["items"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["item_id"], item["item_id"]);
    assert!(rows[0]["level"].is_null());
    assert_eq!(rows[0]["reason"], "no escalation threshold reached");
}

#[test]
fn inbox_escalate_rejects_invalid_ladders() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let missing_equals = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "1000",
    ]);
    assert!(!missing_equals.status.success());
    assert!(
        String::from_utf8_lossy(&missing_equals.stderr).contains("AFTER_MS=ACTION"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_equals.stderr)
    );

    let bad_action = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "1000=pager",
    ]);
    assert!(!bad_action.status.success());
    assert!(
        String::from_utf8_lossy(&bad_action.stderr).contains("inbox, digest, notify or urgent"),
        "stderr: {}",
        String::from_utf8_lossy(&bad_action.stderr)
    );

    let unordered = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "5000=inbox",
        "--step",
        "1000=notify",
    ]);
    assert!(!unordered.status.success());
    assert!(
        String::from_utf8_lossy(&unordered.stderr).contains("strictly increasing"),
        "stderr: {}",
        String::from_utf8_lossy(&unordered.stderr)
    );
}

#[test]
fn inbox_escalate_notifies_once_per_reached_rung() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let state = workspace.path().join("escalation-state.json");
    let item = add_item(&store, "escalate me");

    let (webhook, received) = start_webhook();
    let first = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "0=notify",
        "--webhook",
        &webhook,
        "--state",
        state.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["notification"]["sent"], true);
    assert_eq!(report["notification"]["notified"], 1);
    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(body.contains("[notify]"), "body: {body}");
    assert!(body.contains("escalate me"), "body: {body}");
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&state).unwrap()).unwrap();
    assert_eq!(
        saved["delivered"][item["item_id"].as_str().unwrap()],
        "notify"
    );

    // The same rung must not be delivered twice. A closed port would fail if
    // the command tried to send, so a success proves no request was made.
    let repeat = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "0=notify",
        "--webhook",
        "http://127.0.0.1:9/hook",
        "--state",
        state.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&repeat.stdout).unwrap();
    assert_eq!(report["notification"]["sent"], false);
    assert_eq!(report["notification"]["notified"], 0);

    let (webhook, received) = start_webhook();
    let raised = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "0=urgent",
        "--webhook",
        &webhook,
        "--state",
        state.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        raised.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&raised.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&raised.stdout).unwrap();
    assert_eq!(report["notification"]["sent"], true);
    assert_eq!(report["notification"]["notified"], 1);
    let body = received
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(body.contains("[urgent]"), "body: {body}");

    let missing_state = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "0=notify",
        "--webhook",
        "http://127.0.0.1:9/hook",
    ]);
    assert!(!missing_state.status.success());
    assert!(
        String::from_utf8_lossy(&missing_state.stderr).contains("--webhook/--target and --state"),
        "stderr: {}",
        String::from_utf8_lossy(&missing_state.stderr)
    );
}

#[test]
fn inbox_escalate_fans_out_to_multiple_targets() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("inbox.jsonl");
    let state = workspace.path().join("escalation-state.json");
    add_item(&store, "fan out escalation");

    let (slack_url, slack) = start_webhook();
    let (discord_url, discord) = start_webhook();
    let output = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "0=notify",
        "--target",
        &format!("slack={slack_url}"),
        "--target",
        &format!("discord={discord_url}"),
        "--state",
        state.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["notification"]["sent"], true);
    assert_eq!(report["notification"]["notified"], 1);
    let slack_body = slack
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(slack_body.contains("[notify]"), "body: {slack_body}");
    let discord_body = discord
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(discord_body.contains("[notify]"), "body: {discord_body}");

    let repeat = run(&[
        "inbox",
        "escalate",
        "--store",
        store.to_str().unwrap(),
        "--step",
        "0=notify",
        "--target",
        "slack=http://127.0.0.1:9/hook",
        "--state",
        state.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        repeat.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&repeat.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&repeat.stdout).unwrap();
    assert_eq!(report["notification"]["sent"], false);
    assert_eq!(report["notification"]["notified"], 0);
}
