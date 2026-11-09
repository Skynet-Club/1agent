use std::process::Command;

#[test]
fn surface_openapi_and_request_validation_cli() {
    let dir = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");

    let openapi = Command::new(bin)
        .args(["surface", "openapi"])
        .output()
        .unwrap();
    assert!(openapi.status.success(), "{openapi:?}");
    let document: serde_json::Value = serde_json::from_slice(&openapi.stdout).unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    assert!(document["paths"]["/v0/events"].is_object());

    let request_path = dir.path().join("request.json");
    std::fs::write(
        &request_path,
        r#"{
            "protocol_version": "v0",
            "request_id": "request-1",
            "trace_id": "trace-1",
            "idempotency_key": "idem-1",
            "method": {"name": "thread_get", "params": {"thread_id": "thread-1"}}
        }"#,
    )
    .unwrap();
    let valid = Command::new(bin)
        .args(["surface", "validate", "--request"])
        .arg(&request_path)
        .output()
        .unwrap();
    assert!(valid.status.success(), "{valid:?}");
    assert!(String::from_utf8_lossy(&valid.stdout).contains("fingerprint="));

    std::fs::write(
        &request_path,
        r#"{
            "protocol_version": "v0",
            "request_id": "request-1",
            "trace_id": "trace-1",
            "method": {"name": "thread_get", "params": {"thread_id": "bad/id"}}
        }"#,
    )
    .unwrap();
    let invalid = Command::new(bin)
        .args(["surface", "validate", "--request"])
        .arg(&request_path)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
}
