//! Live stdio MCP evidence against two independent local server codebases.
//!
//! Skipped when the checkouts are not present on this machine. The conformance
//! server is run in both modern 2026-07-28 and legacy 2025-11-25 modes.

use std::collections::BTreeMap;
use std::path::Path;

use first_principles_agent::domain::PermissionSet;
use first_principles_agent::tools::load_mcp_tools;
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn calls_three_real_mcp_server_configurations() {
    let conformance = Path::new("/home/skynet/codex-source./scripts/mcp_conformance/server.py");
    let prompt_server = Path::new("/home/skynet/skynet-prompt/skynet_prompt/mcp_server.py");
    if !conformance.is_file() || !prompt_server.is_file() {
        eprintln!("skipping: local real MCP server checkouts are not present");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("mcp-real.json");
    let config_value = json!({
        "servers": [
            {
                "name": "codex-modern",
                "command": [
                    "python3",
                    conformance.display().to_string(),
                    "--mode", "2026-07-28",
                    "--transport", "stdio"
                ],
                "env": {}
            },
            {
                "name": "codex-legacy",
                "command": [
                    "python3",
                    conformance.display().to_string(),
                    "--mode", "2025-11-25",
                    "--transport", "stdio"
                ],
                "env": {}
            },
            {
                "name": "prompt",
                "command": ["python3", "-m", "skynet_prompt.mcp_server"],
                "env": {"PYTHONPATH": "/home/skynet/skynet-prompt"}
            }
        ]
    });
    std::fs::write(&config, serde_json::to_vec(&config_value).unwrap()).unwrap();
    let tools = load_mcp_tools(&config)
        .await
        .expect("load real MCP servers");
    let by_name = tools
        .iter()
        .map(|tool| (tool.spec().name, tool.clone()))
        .collect::<BTreeMap<_, _>>();
    let permissions = PermissionSet {
        allow_network: true,
        ..PermissionSet::default()
    };
    let cancellation = CancellationToken::new();

    for server in ["codex-modern", "codex-legacy"] {
        let tool = by_name
            .get(&format!("mcp_{}_echo", server.replace('-', "_")))
            .unwrap_or_else(|| panic!("{server} did not expose echo"));
        let outcome = tool
            .execute(json!({"text": "real-server"}), &permissions, &cancellation)
            .await
            .unwrap_or_else(|error| panic!("{server} echo failed: {error}"));
        assert!(
            outcome.content.contains("real-server"),
            "{server}: {}",
            outcome.content
        );
    }

    let prompt = by_name
        .get("mcp_prompt_prompt_packs")
        .expect("prompt server did not expose prompt_packs");
    let outcome = prompt
        .execute(json!({}), &permissions, &cancellation)
        .await
        .expect("prompt_packs");
    assert!(!outcome.content.trim().is_empty());
    assert!(
        outcome.content.contains("pack") || outcome.content.contains("version"),
        "unexpected prompt_packs payload: {}",
        outcome.content
    );
}

#[tokio::test]
async fn mirrors_x_mcp_header_parameters_to_a_real_http_server() {
    let conformance = Path::new("/home/skynet/codex-source./scripts/mcp_conformance/server.py");
    if !conformance.is_file() {
        eprintln!("skipping: local conformance server is not present");
        return;
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let _server = KillOnDrop(
        std::process::Command::new("python3")
            .arg(conformance)
            .args([
                "--mode",
                "2026-07-28",
                "--transport",
                "http",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("start conformance HTTP server"),
    );
    let ready = async {
        for _ in 0..100 {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }
    .await;
    assert!(ready, "conformance HTTP server did not start");

    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("mcp-http.json");
    std::fs::write(
        &config,
        serde_json::to_vec(&json!({
            "servers": [{
                "name": "codex-http",
                "url": format!("http://127.0.0.1:{port}/mcp")
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let tools = load_mcp_tools(&config).await.expect("load HTTP server");
    let tool = tools
        .iter()
        .find(|tool| tool.spec().name == "mcp_codex_http_header_echo")
        .expect("header_echo tool");
    let outcome = tool
        .execute(
            json!({"region": "eu", "attempt": 2, "enabled": true, "greeting": "hi"}),
            &PermissionSet {
                allow_network: true,
                ..PermissionSet::default()
            },
            &CancellationToken::new(),
        )
        .await
        .expect("header_echo");
    assert!(
        outcome.content.contains("eu") || outcome.content.contains("region"),
        "unexpected header_echo: {}",
        outcome.content
    );
}
