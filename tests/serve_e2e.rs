use std::io::Write;
use std::process::{Command, Stdio};

use first_principles_agent::{
    HOST_PROTOCOL_VERSION, HostRequest, HostResponse, StopReason, TaskRequest, audit_events,
    read_events,
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
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args([
            "serve",
            "--provider",
            "mock",
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        serde_json::to_string(&request).unwrap()
    )
    .unwrap();
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: HostResponse = serde_json::from_slice(&output.stdout).unwrap();
    assert!(response.ok);
    assert_eq!(response.request_id, "host-1");
    let result = response.result.unwrap();
    assert_eq!(result.stop_reason, StopReason::Completed);
    let audit = audit_events(&read_events(&result.journal_path).unwrap());
    assert!(audit.ok, "{:?}", audit.issues);
}
