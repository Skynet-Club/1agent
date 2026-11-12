use std::io::{Read, Write};
use std::process::Command;
use std::time::Duration;

use first_principles_agent::{RunResult, StopReason};

#[test]
fn run_stream_prints_provider_deltas_before_the_final_json() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let answer = serde_json::json!({
        "status": "completed",
        "conclusion": "Streamed CLI answer.",
        "claims": [],
        "evidence": [],
        "assumptions": [],
        "constraints": [],
        "unknowns": [],
        "flip_conditions": [],
        "action_receipts": []
    })
    .to_string();
    let (first, second) = answer.split_at(answer.len() / 2);
    let first_event = serde_json::json!({
        "choices": [{"delta": {"content": first}}]
    })
    .to_string();
    let second_event = serde_json::json!({
        "choices": [{"delta": {"content": second}}]
    })
    .to_string();

    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut stream);
        assert!(request.contains("\"stream\":true"));
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .unwrap();
        stream
            .write_all(format!("data: {first_event}\n\n").as_bytes())
            .unwrap();
        stream.flush().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        stream
            .write_all(format!("data: {second_event}\n\ndata: [DONE]\n\n").as_bytes())
            .unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
    });

    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "run",
            "--task",
            "hello",
            "--provider",
            "openai-compatible",
            "--base-url",
            &format!("http://{address}/v1"),
            "--model",
            "test-model",
            "--allow-network",
            "--stream",
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Streamed CLI answer."),
        "streamed stderr: {stderr}"
    );
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(run.stop_reason, StopReason::Completed);
    assert_eq!(run.answer.conclusion, "Streamed CLI answer.");
    server.join().unwrap();
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let read = stream.read(&mut chunk).unwrap();
        assert!(read > 0, "connection closed before headers");
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_end = position + 4;
            let headers = String::from_utf8_lossy(&bytes[..header_end]).to_ascii_lowercase();
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while bytes.len() < header_end + content_length {
                let read = stream.read(&mut chunk).unwrap();
                assert!(read > 0, "connection closed before body");
                bytes.extend_from_slice(&chunk[..read]);
            }
            return String::from_utf8(bytes).unwrap();
        }
    }
}
