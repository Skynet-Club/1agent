//! Live acceptance for the terminal tools.
//!
//! Requires a running Skynet Workshop session daemon and the
//! `skynet-workshop-session` CLI on PATH. Run explicitly with:
//!
//! ```bash
//! cargo test --offline --test terminal_tools_live -- --ignored
//! ```

use std::time::Duration;

use first_principles_agent::{
    AgentTool, PermissionSet, TerminalCreateTool, TerminalDestroyTool, TerminalExecTool,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "requires a running Skynet Workshop session daemon"]
async fn creates_executes_and_destroys_a_real_terminal_session() {
    let dir = tempfile::tempdir().unwrap();
    let permissions = PermissionSet {
        read_roots: vec![std::fs::canonicalize(dir.path()).unwrap()],
        allow_network: false,
        allow_write: true,
    };
    let cancellation = CancellationToken::new();

    let create = TerminalCreateTool::from_env();
    let outcome = create
        .execute(
            json!({"cwd": dir.path(), "name": "1a-live-terminal"}),
            &permissions,
            &cancellation,
        )
        .await
        .unwrap();
    let content: serde_json::Value = serde_json::from_str(&outcome.content).unwrap();
    let session_id = content["session_id"].as_str().unwrap().to_string();

    // Give the PTY shell a moment to start before running the command.
    tokio::time::sleep(Duration::from_millis(250)).await;

    let exec = TerminalExecTool::from_env();
    let outcome = exec
        .execute(
            json!({"session_id": session_id, "command": "printf LIVE_TERMINAL_OK"}),
            &permissions,
            &cancellation,
        )
        .await
        .unwrap();
    assert!(
        outcome.content.contains("LIVE_TERMINAL_OK"),
        "unexpected output: {}",
        outcome.content
    );
    assert_eq!(outcome.metadata.get("exit").map(String::as_str), Some("0"));

    let destroy = TerminalDestroyTool::from_env();
    destroy
        .execute(
            json!({"session_id": session_id}),
            &permissions,
            &cancellation,
        )
        .await
        .unwrap();
}
