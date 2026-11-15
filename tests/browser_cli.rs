//! CLI smoke test for `1agent browser probe`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn chrome_path() -> Option<PathBuf> {
    for candidate in [
        "/usr/bin/google-chrome-stable",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn start_server() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buffer = [0u8; 8192];
            let _ = stream.read(&mut buffer);
            let body = "<html><head><title>Browser CLI</title></head><body><h1>Hello CLI</h1></body></html>";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    port
}

#[test]
fn browser_probe_returns_title_and_text() {
    if chrome_path().is_none() {
        eprintln!("skipping: Chrome is not installed");
        return;
    }
    let port = start_server();
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args([
            "browser",
            "probe",
            "--url",
            &format!("http://127.0.0.1:{port}/"),
            "--allow",
            "127.0.0.1",
            "--allow-loopback",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["title"], "Browser CLI");
    assert!(
        report["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Hello CLI")
    );
    assert!(report["ax_nodes"].as_u64().unwrap_or(0) > 0);
}
