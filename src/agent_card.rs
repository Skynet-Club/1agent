//! Machine-readable Agent Card.
//!
//! The card states what this build supports, what it only partially supports
//! and what it does not claim. Capability statuses come from the platform
//! registry where the platform is the constraint; core capabilities reflect
//! the contract tests in this repository.

use serde::{Deserialize, Serialize};

use crate::platform::{self, CapabilityStatus};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CardCapability {
    pub id: String,
    pub status: CapabilityStatus,
    pub note: String,
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
                CapabilityStatus::Unsupported,
                "shared team partner is not implemented",
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
            "No team collaboration, multi-tenant identity or marketplace.".into(),
            "No claim of calibrated confidence or model-quality generalization.".into(),
            "Autonomy is a preference; Authority/Policy/ExecutionGate still decide execution.".into(),
        ],
    }
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
    }

    #[test]
    fn card_round_trips_through_json() {
        let card = build_card("0.97.0");
        let encoded = serde_json::to_string(&card).unwrap();
        let decoded: AgentCard = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, card);
    }

    #[test]
    fn unsupported_claims_are_not_marked_verified() {
        let card = build_card("0.97.0");
        for id in ["team_collaboration", "marketplace"] {
            let capability = card
                .capabilities
                .iter()
                .find(|capability| capability.id == id)
                .unwrap();
            assert_eq!(capability.status, CapabilityStatus::Unsupported);
        }
        let computer_use = card
            .capabilities
            .iter()
            .find(|capability| capability.id == "computer_use")
            .unwrap();
        assert_ne!(computer_use.status, CapabilityStatus::Verified);
    }
}
