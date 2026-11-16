//! CLI tests for listing, approving and denying browser action cards.

use std::process::Command;

use first_principles_agent::browser::{ActionCard, ActionCardStore, CardStatus};
use first_principles_agent::domain::RiskLevel;

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn card() -> ActionCard {
    let now = now_ms();
    ActionCard {
        card_id: "card-cli-1".into(),
        action: "click".into(),
        target: "binding-hash".into(),
        risk: RiskLevel::Medium,
        reversible: false,
        nonce: "nonce-1".into(),
        created_at_ms: now,
        expires_at_ms: now + 120_000,
        session_id: "session-cli".into(),
        origin: "https://example.com".into(),
        summary: "click <button> text=\"Submit\" at https://example.com".into(),
        action_id: String::new(),
        expected_artifact_sha256: None,
    }
}

#[test]
fn approvals_are_listed_approved_and_denied_from_the_cli() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("approvals.json");
    let store = ActionCardStore::at(&path);
    store.request(card()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["browser", "approvals", "--json"])
        .env("ONEAGENT_BROWSER_APPROVALS", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cards"][0]["card_id"], "card-cli-1");
    assert_eq!(report["cards"][0]["status"], "pending");

    // Without a TTY an interactive approval is refused.
    let refused = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["browser", "approve", "card-cli-1"])
        .env("ONEAGENT_BROWSER_APPROVALS", &path)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("interactive TTY"),
        "stderr: {}",
        String::from_utf8_lossy(&refused.stderr)
    );

    let approved = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["browser", "approve", "card-cli-1", "--yes", "--json"])
        .env("ONEAGENT_BROWSER_APPROVALS", &path)
        .output()
        .unwrap();
    assert!(
        approved.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&approved.stderr)
    );
    assert_eq!(
        store.get("card-cli-1").unwrap().unwrap().status,
        CardStatus::Approved
    );

    let mut denied_card = card();
    denied_card.card_id = "card-cli-2".into();
    denied_card.target = "binding-hash-2".into();
    store.request(denied_card).unwrap();
    let denied = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["browser", "deny", "card-cli-2", "--json"])
        .env("ONEAGENT_BROWSER_APPROVALS", &path)
        .output()
        .unwrap();
    assert!(
        denied.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&denied.stderr)
    );
    assert_eq!(
        store.get("card-cli-2").unwrap().unwrap().status,
        CardStatus::Denied
    );
}
