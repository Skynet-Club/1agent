//! Public trust disclosure manifest.
//!
//! This is a machine-readable disclosure, not a hosted Trust Center page and
//! not an external audit. It states capabilities, limits, evaluation results
//! supplied by the caller, and the current audit status without inventing
//! certifications.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::agent_card;
use crate::platform::CapabilityStatus;
use crate::verification::HackSuiteSummary;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum TrustCenterError {
    #[error("evaluation disclosure is invalid: {0}")]
    InvalidEvaluation(String),
    #[error("trust disclosure JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvaluationDisclosure {
    pub name: String,
    pub version: String,
    pub passed: u32,
    pub total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_sha256: Option<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySummary {
    pub verified: usize,
    pub degraded: usize,
    pub unsupported: usize,
    pub untested: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditDisclosure {
    pub status: String,
    pub external_audit: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataPolicy {
    pub storage: String,
    pub telemetry: String,
    pub cloud_dependency: String,
    pub network_default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrustDisclosure {
    pub schema_version: u32,
    pub generated_at_ms: u128,
    pub product: String,
    pub version: String,
    pub platform: String,
    pub capabilities: CapabilitySummary,
    pub limits: Vec<String>,
    pub audit: AuditDisclosure,
    pub data_policy: DataPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<HackSuiteSummary>,
    pub evaluations: Vec<EvaluationDisclosure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_contact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident_url: Option<String>,
}

pub fn build_disclosure(
    version: &str,
    now_ms: u128,
    security_contact: Option<String>,
    incident_url: Option<String>,
    verification: Option<HackSuiteSummary>,
    mut evaluations: Vec<EvaluationDisclosure>,
) -> Result<TrustDisclosure, TrustCenterError> {
    for evaluation in &evaluations {
        validate_evaluation(evaluation)?;
    }
    evaluations.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.version.cmp(&right.version))
    });
    let card = agent_card::build_card(version);
    let mut capabilities = CapabilitySummary {
        verified: 0,
        degraded: 0,
        unsupported: 0,
        untested: 0,
        total: card.capabilities.len(),
    };
    for capability in &card.capabilities {
        match capability.status {
            CapabilityStatus::Verified => capabilities.verified += 1,
            CapabilityStatus::Degraded => capabilities.degraded += 1,
            CapabilityStatus::Unsupported => capabilities.unsupported += 1,
            CapabilityStatus::Untested => capabilities.untested += 1,
        }
    }
    Ok(TrustDisclosure {
        schema_version: SCHEMA_VERSION,
        generated_at_ms: now_ms,
        product: card.name,
        version: card.version,
        platform: card.platform,
        capabilities,
        limits: card.limits,
        audit: AuditDisclosure {
            status: "not_audited".into(),
            external_audit: false,
            note: "No third-party security or compliance audit has been completed.".into(),
        },
        data_policy: DataPolicy {
            storage: "local files; the operator controls the runtime home".into(),
            telemetry: "none".into(),
            cloud_dependency: "none by default; providers are opt-in".into(),
            network_default: "off unless --allow-network or a configured provider/tool needs it"
                .into(),
        },
        verification,
        evaluations,
        security_contact,
        incident_url,
    })
}

fn validate_evaluation(evaluation: &EvaluationDisclosure) -> Result<(), TrustCenterError> {
    if evaluation.name.trim().is_empty() || evaluation.version.trim().is_empty() {
        return Err(TrustCenterError::InvalidEvaluation(
            "name and version are required".into(),
        ));
    }
    if evaluation.total == 0 || evaluation.passed > evaluation.total {
        return Err(TrustCenterError::InvalidEvaluation(
            "total must be positive and passed must not exceed total".into(),
        ));
    }
    if let Some(digest) = &evaluation.evidence_sha256
        && (digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(TrustCenterError::InvalidEvaluation(
            "evidence_sha256 must be 64 hex characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluation() -> EvaluationDisclosure {
        EvaluationDisclosure {
            name: "core-v2".into(),
            version: "1".into(),
            passed: 6,
            total: 6,
            evidence_sha256: Some("ab".repeat(32)),
            note: "contract fixture".into(),
        }
    }

    #[test]
    fn disclosure_counts_capabilities_without_overclaiming_audit() {
        let disclosure = build_disclosure(
            "0.97.0",
            10,
            Some("security@example.com".into()),
            None,
            None,
            vec![],
        )
        .unwrap();
        assert_eq!(disclosure.product, "1Agent");
        assert_eq!(
            disclosure.capabilities.total,
            disclosure.capabilities.verified
                + disclosure.capabilities.degraded
                + disclosure.capabilities.unsupported
                + disclosure.capabilities.untested
        );
        assert!(disclosure.capabilities.verified > 0);
        assert!(!disclosure.audit.external_audit);
        assert_eq!(disclosure.audit.status, "not_audited");
        assert_eq!(disclosure.data_policy.telemetry, "none");
        assert_eq!(
            disclosure.security_contact.as_deref(),
            Some("security@example.com")
        );
    }

    #[test]
    fn evaluations_are_validated_and_sorted() {
        let mut other = evaluation();
        other.name = "alpha".into();
        let disclosure =
            build_disclosure("0.97.0", 10, None, None, None, vec![evaluation(), other]).unwrap();
        assert_eq!(disclosure.evaluations[0].name, "alpha");

        let mut invalid = evaluation();
        invalid.passed = 7;
        assert!(build_disclosure("0.97.0", 10, None, None, None, vec![invalid]).is_err());

        let mut bad_digest = evaluation();
        bad_digest.evidence_sha256 = Some("xyz".into());
        assert!(build_disclosure("0.97.0", 10, None, None, None, vec![bad_digest]).is_err());
    }

    #[test]
    fn verification_summary_is_included_when_supplied() {
        let summary = HackSuiteSummary {
            resolved: 2,
            verified: 1,
            hacked: 1,
            rejected: 0,
            hacked_resolved_bps: 5_000,
        };
        let disclosure = build_disclosure("0.97.0", 10, None, None, Some(summary), vec![]).unwrap();
        assert_eq!(disclosure.verification, Some(summary));
    }

    #[test]
    fn disclosure_round_trips_through_json() {
        let disclosure = build_disclosure(
            "0.97.0",
            10,
            None,
            Some("https://example.com/incidents".into()),
            None,
            vec![evaluation()],
        )
        .unwrap();
        let encoded = serde_json::to_string(&disclosure).unwrap();
        let decoded: TrustDisclosure = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, disclosure);
    }
}
