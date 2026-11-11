//! Machine-readable Agent Card.
//!
//! The card states what this build supports, what it only partially supports
//! and what it does not claim. Capability statuses come from the platform
//! registry where the platform is the constraint; core capabilities reflect
//! the contract tests in this repository.

use ring::rand::SystemRandom;
use ring::signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::platform::{self, CapabilityStatus};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum AgentCardError {
    #[error("agent card signing key is invalid: {0}")]
    InvalidKey(String),
    #[error("agent card signature is invalid: {0}")]
    InvalidSignature(String),
    #[error("agent card is unsigned")]
    Unsigned,
    #[error("agent card JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CardCapability {
    pub id: String,
    pub status: CapabilityStatus,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CardSignature {
    pub algorithm: String,
    pub public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentCard {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub platform: String,
    pub capabilities: Vec<CardCapability>,
    pub limits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<CardSignature>,
}

pub fn build_card(version: &str) -> AgentCard {
    let platform_report = platform::report();
    let platform_status = |id: &str| {
        platform_report
            .capabilities
            .iter()
            .find(|capability| capability.name == id)
            .map(|capability| capability.status)
            .unwrap_or(CapabilityStatus::Untested)
    };

    AgentCard {
        schema_version: SCHEMA_VERSION,
        name: "1Agent".into(),
        version: version.into(),
        description:
            "Local-first, evidence-accountable partner runtime with a durable inbox, user model, autonomy policy and voice modes."
                .into(),
        platform: platform_report.platform.as_str().into(),
        capabilities: vec![
            capability(
                "task_evidence_audit",
                CapabilityStatus::Verified,
                "journal, replay, receipts, approval and audit contract tests",
            ),
            capability(
                "user_model",
                CapabilityStatus::Verified,
                "preferences, boundaries, autonomy, voice and memory consent",
            ),
            capability(
                "agent_inbox",
                CapabilityStatus::Verified,
                "append-only store, projections, sync, CLI and TUI actions",
            ),
            capability(
                "autonomy_dial",
                CapabilityStatus::Verified,
                "observe/suggest/act/require_approval policy with risk and reversibility",
            ),
            capability(
                "voice_modes",
                CapabilityStatus::Verified,
                "operator/advisor/companion prompt blocks",
            ),
            capability(
                "onboarding",
                CapabilityStatus::Verified,
                "non-interactive profile, goal and commitment setup",
            ),
            capability(
                "value_report",
                CapabilityStatus::Verified,
                "outcomes, cost, tool use, alerts and open inbox count",
            ),
            capability(
                "independent_verification",
                CapabilityStatus::Verified,
                "hack-verifiable acceptance contract",
            ),
            capability(
                "repair",
                CapabilityStatus::Verified,
                "repair plans, receipts and away digest",
            ),
            capability(
                "signed_agent_card",
                CapabilityStatus::Verified,
                "Ed25519 signing and trusted-key verification",
            ),
            capability(
                "verification_store",
                CapabilityStatus::Verified,
                "persisted verification reports, summaries and inbox alerts",
            ),
            capability(
                "team_handoff",
                CapabilityStatus::Verified,
                "grounded handoffs with addressee acceptance and audit linkage",
            ),
            capability(
                "inbox_notifications",
                CapabilityStatus::Verified,
                "one-way generic/Slack/Discord webhook notifications with a shared watermark and multi-target fan-out",
            ),
            capability(
                "inbox_escalation",
                CapabilityStatus::Verified,
                "ladder projection and one-time notify/urgent delivery from CLI or daemon to one or more targets, including digest counts",
            ),
            capability(
                "repair_runbook",
                CapabilityStatus::Verified,
                "compensation/manual runbook with require-clean, plus repair-mark for verified human clearance",
            ),
            capability(
                "org_audit",
                CapabilityStatus::Verified,
                "hash-chained organization audit with optional Ed25519 seals",
            ),
            capability(
                "mcp_http_transport",
                CapabilityStatus::Verified,
                "streamable HTTP JSON/SSE transport with environment-only bearer tokens",
            ),
            capability(
                "chat_bridge",
                CapabilityStatus::Verified,
                "file-based message→task and run-result→reply adapter",
            ),
            capability(
                "daemon_maintenance",
                CapabilityStatus::Verified,
                "opt-in terminal-handoff reconcile plus alert/goal projection and stale-attention cleanup on every daemon tick",
            ),
            capability(
                "trust_center",
                CapabilityStatus::Verified,
                "JSON/Markdown trust disclosure with not_audited audit status",
            ),
            capability(
                "computer_use",
                platform_status("computer_use"),
                "status comes from the platform registry",
            ),
            capability(
                "pty",
                platform_status("pty"),
                "status comes from the platform registry",
            ),
            capability(
                "cross_platform",
                CapabilityStatus::Untested,
                "macOS/Windows are not exercised; P-Gate is pending",
            ),
            capability(
                "team_collaboration",
                CapabilityStatus::Degraded,
                "role policy, grounded handoffs and local org audit; no IdP, live shared context or multi-tenant isolation",
            ),
            capability(
                "marketplace",
                CapabilityStatus::Unsupported,
                "no signed marketplace or remote index",
            ),
        ],
        limits: vec![
            "No computer-use clicks, typing or screenshots on this build.".into(),
            "macOS and Windows are untested; Linux x86_64 is the verified platform.".into(),
            "No team IdP, live shared context, multi-tenant isolation or marketplace.".into(),
            "No claim of calibrated confidence or model-quality generalization.".into(),
            "Autonomy is a preference; Authority/Policy/ExecutionGate still decide execution.".into(),
        ],
        signature: None,
    }
}

pub fn generate_signing_key() -> Result<(String, String), AgentCardError> {
    let rng = SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|_| AgentCardError::InvalidKey("cannot generate key".into()))?;
    let key = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
        .map_err(|_| AgentCardError::InvalidKey("generated key cannot be parsed".into()))?;
    Ok((
        hex::encode(pkcs8.as_ref()),
        hex::encode(key.public_key().as_ref()),
    ))
}

pub fn sign_card(card: &mut AgentCard, pkcs8_hex: &str) -> Result<(), AgentCardError> {
    let bytes = hex::decode(pkcs8_hex.trim())
        .map_err(|_| AgentCardError::InvalidKey("private key is not hex".into()))?;
    let key = Ed25519KeyPair::from_pkcs8(&bytes)
        .map_err(|_| AgentCardError::InvalidKey("private key is not valid PKCS#8".into()))?;
    let payload = signing_payload(card)?;
    let signature = key.sign(&payload);
    card.signature = Some(CardSignature {
        algorithm: "ed25519".into(),
        public_key: hex::encode(key.public_key().as_ref()),
        signature: hex::encode(signature.as_ref()),
    });
    Ok(())
}

pub fn verify_card(card: &AgentCard, trusted_public_key_hex: &str) -> Result<(), AgentCardError> {
    let Some(signature) = &card.signature else {
        return Err(AgentCardError::Unsigned);
    };
    if signature.algorithm != "ed25519" {
        return Err(AgentCardError::InvalidSignature(
            "unsupported algorithm".into(),
        ));
    }
    let trusted = hex::decode(trusted_public_key_hex.trim())
        .map_err(|_| AgentCardError::InvalidKey("public key is not hex".into()))?;
    if trusted.len() != 32 {
        return Err(AgentCardError::InvalidKey(
            "public key must be 32 bytes".into(),
        ));
    }
    let embedded = hex::decode(signature.public_key.trim())
        .map_err(|_| AgentCardError::InvalidSignature("public key is not hex".into()))?;
    if embedded != trusted {
        return Err(AgentCardError::InvalidSignature(
            "public key does not match the trusted key".into(),
        ));
    }
    let signature_bytes = hex::decode(signature.signature.trim())
        .map_err(|_| AgentCardError::InvalidSignature("signature is not hex".into()))?;
    if signature_bytes.len() != 64 {
        return Err(AgentCardError::InvalidSignature(
            "signature must be 64 bytes".into(),
        ));
    }
    let payload = signing_payload(card)?;
    UnparsedPublicKey::new(&ED25519, trusted)
        .verify(&payload, &signature_bytes)
        .map_err(|_| AgentCardError::InvalidSignature("signature does not verify".into()))
}

fn signing_payload(card: &AgentCard) -> Result<Vec<u8>, AgentCardError> {
    let mut unsigned = card.clone();
    unsigned.signature = None;
    Ok(serde_json::to_vec(&unsigned)?)
}

fn capability(id: &str, status: CapabilityStatus, note: &str) -> CardCapability {
    CardCapability {
        id: id.into(),
        status,
        note: note.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_has_required_fields_and_limits() {
        let card = build_card("0.97.0");
        assert_eq!(card.schema_version, SCHEMA_VERSION);
        assert_eq!(card.name, "1Agent");
        assert_eq!(card.version, "0.97.0");
        assert!(!card.capabilities.is_empty());
        assert!(!card.limits.is_empty());
        assert!(
            card.capabilities
                .iter()
                .any(|capability| capability.id == "computer_use")
        );
        for id in [
            "verification_store",
            "team_handoff",
            "inbox_notifications",
            "repair_runbook",
            "org_audit",
            "mcp_http_transport",
            "chat_bridge",
            "trust_center",
        ] {
            let capability = card
                .capabilities
                .iter()
                .find(|capability| capability.id == id)
                .unwrap();
            assert_eq!(capability.status, CapabilityStatus::Verified);
        }
    }

    #[test]
    fn card_round_trips_through_json() {
        let card = build_card("0.97.0");
        let encoded = serde_json::to_string(&card).unwrap();
        let decoded: AgentCard = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, card);
    }

    #[test]
    fn signed_card_verifies_with_the_trusted_key() {
        let (private_key, public_key) = generate_signing_key().unwrap();
        let mut card = build_card("0.97.0");
        sign_card(&mut card, &private_key).unwrap();
        assert!(card.signature.is_some());
        verify_card(&card, &public_key).unwrap();

        let encoded = serde_json::to_string(&card).unwrap();
        let decoded: AgentCard = serde_json::from_str(&encoded).unwrap();
        verify_card(&decoded, &public_key).unwrap();
    }

    #[test]
    fn tampering_and_wrong_keys_are_rejected() {
        let (private_key, public_key) = generate_signing_key().unwrap();
        let (_, other_public_key) = generate_signing_key().unwrap();
        let mut card = build_card("0.97.0");
        sign_card(&mut card, &private_key).unwrap();

        let mut tampered = card.clone();
        tampered.version = "9.9.9".into();
        assert!(verify_card(&tampered, &public_key).is_err());
        assert!(verify_card(&card, &other_public_key).is_err());

        let unsigned = build_card("0.97.0");
        assert!(matches!(
            verify_card(&unsigned, &public_key),
            Err(AgentCardError::Unsigned)
        ));
    }

    #[test]
    fn unsupported_claims_are_not_marked_verified() {
        let card = build_card("0.97.0");
        let marketplace = card
            .capabilities
            .iter()
            .find(|capability| capability.id == "marketplace")
            .unwrap();
        assert_eq!(marketplace.status, CapabilityStatus::Unsupported);
        let team = card
            .capabilities
            .iter()
            .find(|capability| capability.id == "team_collaboration")
            .unwrap();
        assert_eq!(team.status, CapabilityStatus::Degraded);
        let computer_use = card
            .capabilities
            .iter()
            .find(|capability| capability.id == "computer_use")
            .unwrap();
        assert_ne!(computer_use.status, CapabilityStatus::Verified);
    }
}
