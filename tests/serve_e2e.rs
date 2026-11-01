use std::io::Write;
use std::process::{Command, Stdio};

use first_principles_agent::{
    HOST_PROTOCOL_VERSION, HostAction, HostRequest, HostResponse, JournalEventKind, StopReason,
    TaskRequest, audit_events, read_events, scan_history,
};

#[test]
fn stdio_host_protocol_runs_and_returns_auditable_result() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = workspace.path().join("evidence.txt");
    std::fs::write(&evidence, "verified host evidence").unwrap();
    let request = HostRequest {
        protocol_version: HOST_PROTOCOL_VERSION,
        request_id: "host-1".into(),
        task: TaskRequest::from_objective(
            format!("Research @read-file:\"{}\"", evidence.display()),
            workspace.path().to_path_buf(),
        ),
    };
    let second_request = HostRequest {
        protocol_version: HOST_PROTOCOL_VERSION,
        request_id: "host-2".into(),
        task: request.task.clone(),
    };
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args([
            "serve",
            "--provider",
            "mock",
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
            "--max-concurrency",
            "2",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(child.stdin.as_mut().unwrap(), "{{not-json").unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::to_string(&request).unwrap()
    )
    .unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::to_string(&second_request).unwrap()
    )
    .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<_> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(lines.len(), 3);
    let responses: Vec<HostResponse> = lines
        .into_iter()
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    let invalid = responses.iter().find(|response| !response.ok).unwrap();
    assert!(!invalid.ok);
    assert_eq!(invalid.error.as_ref().unwrap().code, "invalid_json");
    let successful: Vec<_> = responses.iter().filter(|response| response.ok).collect();
    assert_eq!(successful.len(), 2);
    let mut ids: Vec<_> = successful
        .iter()
        .map(|response| response.request_id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["host-1", "host-2"]);
    for response in successful {
        let result = response.result.as_ref().unwrap();
        assert_eq!(result.stop_reason, StopReason::Completed);
        let events = read_events(&result.journal_path).unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.event,
            JournalEventKind::ClientRequestLinked { client_request_id }
                if client_request_id == &response.request_id
        )));
        let audit = audit_events(&events);
        assert!(audit.ok, "{:?}", audit.issues);
    }
    let history = scan_history(workspace.path().join("journal"), 10).unwrap();
    let mut client_ids: Vec<_> = history
        .iter()
        .filter_map(|entry| entry.client_request_id.as_deref())
        .collect();
    client_ids.sort_unstable();
    assert_eq!(client_ids, vec!["host-1", "host-2"]);
}

#[test]
fn host_rejects_queue_overflow_and_cancels_by_request_id() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args([
            "serve",
            "--provider",
            "mock",
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
            "--max-concurrency",
            "1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let task = TaskRequest::from_objective("Analyze @sleep-ms:5000", workspace.path().into());
    let first = HostRequest {
        protocol_version: HOST_PROTOCOL_VERSION,
        request_id: "slow-1".into(),
        task: task.clone(),
    };
    let second = HostRequest {
        protocol_version: HOST_PROTOCOL_VERSION,
        request_id: "slow-2".into(),
        task,
    };
    let cancel = serde_json::json!({
        "protocol_version": HOST_PROTOCOL_VERSION,
        "request_id": "slow-1",
        "action": "cancel"
    });
    for value in [&first, &second] {
        writeln!(
            child.stdin.as_mut().unwrap(),
            "{}",
            serde_json::to_string(value).unwrap()
        )
        .unwrap();
    }
    writeln!(child.stdin.as_mut().unwrap(), "{cancel}").unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<HostResponse> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 3);
    assert!(responses.iter().any(|response| {
        response.request_id == "slow-2"
            && response.error.as_ref().map(|error| error.code.as_str()) == Some("queue_full")
    }));
    assert!(responses.iter().any(|response| {
        response.request_id == "slow-1" && response.action == HostAction::Cancel && response.ok
    }));
    assert!(responses.iter().any(|response| {
        response.request_id == "slow-1"
            && response.action == HostAction::Run
            && response.error.as_ref().map(|error| error.code.as_str()) == Some("cancelled")
    }));
}
