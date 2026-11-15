//! Live Chrome for Testing smoke test for the 0.98 browser sandbox.

use std::path::PathBuf;

use first_principles_agent::browser::{BrowserConfig, BrowserSession};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn start_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = [0u8; 8192];
                let _ = stream.read(&mut buffer).await;
                let body = "<!doctype html><html><head><title>1Agent Test</title></head><body><h1 id=\"heading\">Hello browser</h1><button id=\"button\" onclick=\"document.getElementById('out').textContent='clicked'\">Click</button><a id=\"link\" href=\"/next\">Next</a><div id=\"out\"></div><input id=\"input\"></body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (port, handle)
}

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

#[tokio::test]
async fn browser_records_a_bounded_screencast() {
    let Some(chrome_path) = chrome_path() else {
        eprintln!("skipping: Chrome is not installed");
        return;
    };
    let (port, server) = start_server().await;
    let config = BrowserConfig {
        headless: true,
        allowlist: vec!["127.0.0.1".into()],
        allow_loopback: true,
        chrome_path: Some(chrome_path),
        profile_root: None,
        download_dir: None,
    };
    let session = BrowserSession::launch(config).await.expect("launch Chrome");
    session
        .navigate(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("navigate");
    let dir = tempfile::tempdir().unwrap();
    let frames = session
        .record_screencast(dir.path(), std::time::Duration::from_secs(1))
        .await
        .expect("record");
    assert!(frames > 0, "screencast produced no frames");
    assert!(dir.path().join("index.json").is_file());
    session.close().await;
    server.abort();
}

#[tokio::test]
async fn browser_navigates_reads_clicks_types_and_screenshots() {
    let Some(chrome_path) = chrome_path() else {
        eprintln!("skipping: Chrome is not installed");
        return;
    };
    let (port, server) = start_server().await;
    let config = BrowserConfig {
        headless: true,
        allowlist: vec!["127.0.0.1".into()],
        allow_loopback: true,
        chrome_path: Some(chrome_path),
        profile_root: None,
        download_dir: None,
    };
    let session = BrowserSession::launch(config).await.expect("launch Chrome");
    session
        .navigate(&format!("http://127.0.0.1:{port}/"))
        .await
        .expect("navigate");
    assert_eq!(session.title().await.unwrap(), "1Agent Test");
    assert!(
        session
            .visible_text()
            .await
            .unwrap()
            .contains("Hello browser")
    );
    let input = session.describe_selector("#input").await.expect("input");
    assert!(input.reversible, "plain input focus should be reversible");
    let link = session.describe_selector("#link").await.expect("link");
    assert!(link.reversible, "same-origin link should be reversible");
    let button = session.describe_selector("#button").await.expect("button");
    assert!(!button.reversible, "button must fail closed");
    session.click_selector("#button").await.expect("click");
    let out = session
        .evaluate("document.getElementById('out').textContent")
        .await
        .unwrap();
    assert_eq!(out.as_str(), Some("clicked"));
    session
        .type_selector("#input", "hello")
        .await
        .expect("type");
    let value = session
        .evaluate("document.getElementById('input').value")
        .await
        .unwrap();
    assert_eq!(value.as_str(), Some("hello"));
    session.mask_selector("#input").await.expect("mask");
    let masked = session
        .evaluate(
            "document.getElementById('input').style.getPropertyValue('-webkit-text-security')",
        )
        .await
        .unwrap();
    assert_eq!(masked.as_str(), Some("disc"));
    let shot = tempfile::tempdir().unwrap().path().join("shot.png");
    let bytes = session.screenshot(&shot).await.expect("screenshot");
    assert!(bytes > 0);
    assert!(shot.is_file());
    session.close().await;
    server.abort();
}
