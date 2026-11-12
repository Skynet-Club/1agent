//! Public trust disclosure manifest.
//!
//! This is a machine-readable disclosure, not a hosted Trust Center page and
//! not an external audit. It states capabilities, limits, evaluation results
//! supplied by the caller, and the current audit status without inventing
//! certifications.

use std::collections::BTreeSet;

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
    #[error("incident disclosure is invalid: {0}")]
    InvalidIncident(String),
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Open,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IncidentDisclosure {
    pub id: String,
    pub title: String,
    pub status: IncidentStatus,
    pub opened_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at_ms: Option<u128>,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub incidents: Vec<IncidentDisclosure>,
}

pub fn build_disclosure(
    version: &str,
    now_ms: u128,
    security_contact: Option<String>,
    incident_url: Option<String>,
    verification: Option<HackSuiteSummary>,
    evaluations: Vec<EvaluationDisclosure>,
) -> Result<TrustDisclosure, TrustCenterError> {
    build_disclosure_with_incidents(
        version,
        now_ms,
        security_contact,
        incident_url,
        verification,
        evaluations,
        Vec::new(),
    )
}

pub fn build_disclosure_with_incidents(
    version: &str,
    now_ms: u128,
    security_contact: Option<String>,
    incident_url: Option<String>,
    verification: Option<HackSuiteSummary>,
    mut evaluations: Vec<EvaluationDisclosure>,
    mut incidents: Vec<IncidentDisclosure>,
) -> Result<TrustDisclosure, TrustCenterError> {
    for evaluation in &evaluations {
        validate_evaluation(evaluation)?;
    }
    evaluations.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.version.cmp(&right.version))
    });
    validate_incidents(&incidents)?;
    incidents.sort_by(|left, right| {
        right
            .opened_at_ms
            .cmp(&left.opened_at_ms)
            .then_with(|| left.id.cmp(&right.id))
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
        incidents,
    })
}

pub fn render_markdown(disclosure: &TrustDisclosure) -> String {
    let mut out = String::new();
    out.push_str("# 1Agent Trust Disclosure\n\n");
    out.push_str(&format!("- Version: {}\n", one_line(&disclosure.version)));
    out.push_str(&format!("- Platform: {}\n", one_line(&disclosure.platform)));
    out.push_str(&format!(
        "- Generated: {} (unix ms)\n\n",
        disclosure.generated_at_ms
    ));
    out.push_str("## Capabilities\n\n");
    out.push_str(&format!(
        "- verified: {}\n- degraded: {}\n- unsupported: {}\n- untested: {}\n- total: {}\n\n",
        disclosure.capabilities.verified,
        disclosure.capabilities.degraded,
        disclosure.capabilities.unsupported,
        disclosure.capabilities.untested,
        disclosure.capabilities.total
    ));
    out.push_str("## Limits\n\n");
    for limit in &disclosure.limits {
        out.push_str(&format!("- {}\n", one_line(limit)));
    }
    out.push('\n');
    out.push_str("## Audit\n\n");
    out.push_str(&format!(
        "- Status: {}\n- External audit: {}\n- Note: {}\n\n",
        one_line(&disclosure.audit.status),
        disclosure.audit.external_audit,
        one_line(&disclosure.audit.note)
    ));
    out.push_str("## Data policy\n\n");
    out.push_str(&format!(
        "- Storage: {}\n",
        one_line(&disclosure.data_policy.storage)
    ));
    out.push_str(&format!(
        "- Telemetry: {}\n",
        one_line(&disclosure.data_policy.telemetry)
    ));
    out.push_str(&format!(
        "- Cloud dependency: {}\n",
        one_line(&disclosure.data_policy.cloud_dependency)
    ));
    out.push_str(&format!(
        "- Network default: {}\n\n",
        one_line(&disclosure.data_policy.network_default)
    ));
    if let Some(verification) = &disclosure.verification {
        out.push_str("## Verification\n\n");
        out.push_str(&format!(
            "- verified: {}\n- hacked: {}\n- rejected: {}\n- resolved: {}\n- hacked_resolved_bps: {}\n\n",
            verification.verified,
            verification.hacked,
            verification.rejected,
            verification.resolved,
            verification.hacked_resolved_bps
        ));
    }
    if !disclosure.evaluations.is_empty() {
        out.push_str("## Evaluations\n\n");
        for evaluation in &disclosure.evaluations {
            out.push_str(&format!(
                "- {} {}: {}/{}{}\n",
                one_line(&evaluation.name),
                one_line(&evaluation.version),
                evaluation.passed,
                evaluation.total,
                evaluation
                    .evidence_sha256
                    .as_ref()
                    .map(|digest| format!(" (sha256 {digest})"))
                    .unwrap_or_default()
            ));
        }
        out.push('\n');
    }
    if !disclosure.incidents.is_empty() {
        out.push_str("## Incidents\n\n");
        for incident in &disclosure.incidents {
            let status = match incident.status {
                IncidentStatus::Open => "open",
                IncidentStatus::Resolved => "resolved",
            };
            let resolved = incident
                .resolved_at_ms
                .map(|ms| format!(", resolved {ms} ms"))
                .unwrap_or_default();
            out.push_str(&format!(
                "- [{status}] {} — {} (opened {} ms{resolved})\n  {}\n",
                one_line(&incident.id),
                one_line(&incident.title),
                incident.opened_at_ms,
                one_line(&incident.summary)
            ));
            if let Some(link) = &incident.link {
                out.push_str(&format!("  Link: {}\n", one_line(link)));
            }
        }
        out.push('\n');
    }
    if disclosure.security_contact.is_some() || disclosure.incident_url.is_some() {
        out.push_str("## Contact\n\n");
        if let Some(contact) = &disclosure.security_contact {
            out.push_str(&format!("- Security: {}\n", one_line(contact)));
        }
        if let Some(url) = &disclosure.incident_url {
            out.push_str(&format!("- Incidents: {}\n", one_line(url)));
        }
    }
    out
}

/// Static, script-free HTML disclosure. Every caller-supplied value is
/// escaped; contact URLs are rendered as text, not links.
pub fn render_html(disclosure: &TrustDisclosure) -> String {
    let mut out = String::from(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>1Agent Trust Disclosure</title><style>\
         body{font-family:system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;line-height:1.5}\
         table{border-collapse:collapse;margin:1rem 0}th,td{border:1px solid #ccc;padding:.35rem .6rem;text-align:left}\
         code{background:#f4f4f4;padding:.1rem .3rem}</style></head><body>",
    );
    out.push_str("<h1>1Agent Trust Disclosure</h1>");
    out.push_str(&format!(
        "<p>Version: <code>{}</code><br>Platform: <code>{}</code><br>Generated: {} (unix ms)</p>",
        escape_html(&disclosure.version),
        escape_html(&disclosure.platform),
        disclosure.generated_at_ms
    ));
    out.push_str("<h2>Capabilities</h2><table><tr><th>Status</th><th>Count</th></tr>");
    for (label, count) in [
        ("verified", disclosure.capabilities.verified),
        ("degraded", disclosure.capabilities.degraded),
        ("unsupported", disclosure.capabilities.unsupported),
        ("untested", disclosure.capabilities.untested),
        ("total", disclosure.capabilities.total),
    ] {
        out.push_str(&format!("<tr><td>{label}</td><td>{count}</td></tr>"));
    }
    out.push_str("</table>");
    out.push_str("<h2>Limits</h2><ul>");
    for limit in &disclosure.limits {
        out.push_str(&format!("<li>{}</li>", escape_html(&one_line(limit))));
    }
    out.push_str("</ul>");
    out.push_str(&format!(
        "<h2>Audit</h2><p>Status: <code>{}</code><br>External audit: {}<br>{}</p>",
        escape_html(&disclosure.audit.status),
        disclosure.audit.external_audit,
        escape_html(&one_line(&disclosure.audit.note))
    ));
    out.push_str("<h2>Data policy</h2><table>");
    for (label, value) in [
        ("Storage", &disclosure.data_policy.storage),
        ("Telemetry", &disclosure.data_policy.telemetry),
        ("Cloud dependency", &disclosure.data_policy.cloud_dependency),
        ("Network default", &disclosure.data_policy.network_default),
    ] {
        out.push_str(&format!(
            "<tr><th>{label}</th><td>{}</td></tr>",
            escape_html(&one_line(value))
        ));
    }
    out.push_str("</table>");
    if let Some(verification) = &disclosure.verification {
        out.push_str(&format!(
            "<h2>Verification</h2><p>verified={} hacked={} rejected={} resolved={} hacked_resolved_bps={}</p>",
            verification.verified,
            verification.hacked,
            verification.rejected,
            verification.resolved,
            verification.hacked_resolved_bps
        ));
    }
    if !disclosure.evaluations.is_empty() {
        out.push_str("<h2>Evaluations</h2><table><tr><th>Name</th><th>Version</th><th>Passed</th><th>Total</th><th>Evidence SHA-256</th><th>Note</th></tr>");
        for evaluation in &disclosure.evaluations {
            out.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                escape_html(&evaluation.name),
                escape_html(&evaluation.version),
                evaluation.passed,
                evaluation.total,
                escape_html(evaluation.evidence_sha256.as_deref().unwrap_or("-")),
                escape_html(&one_line(&evaluation.note))
            ));
        }
        out.push_str("</table>");
    }
    if !disclosure.incidents.is_empty() {
        out.push_str(
            "<h2>Incidents</h2><table><tr><th>ID</th><th>Status</th><th>Opened</th>\
             <th>Resolved</th><th>Summary</th></tr>",
        );
        for incident in &disclosure.incidents {
            let status = match incident.status {
                IncidentStatus::Open => "open",
                IncidentStatus::Resolved => "resolved",
            };
            let resolved = incident
                .resolved_at_ms
                .map(|ms| ms.to_string())
                .unwrap_or_default();
            let mut summary = escape_html(&one_line(&incident.summary));
            if let Some(link) = &incident.link {
                summary.push_str(&format!("<br>{}", escape_html(&one_line(link))));
            }
            out.push_str(&format!(
                "<tr><td>{}</td><td>{status}</td><td>{}</td><td>{resolved}</td><td>{summary}</td></tr>",
                escape_html(&one_line(&incident.id)),
                incident.opened_at_ms
            ));
        }
        out.push_str("</table>");
    }
    if disclosure.security_contact.is_some() || disclosure.incident_url.is_some() {
        out.push_str("<h2>Contact</h2><ul>");
        if let Some(contact) = &disclosure.security_contact {
            out.push_str(&format!(
                "<li>Security: {}</li>",
                escape_html(&one_line(contact))
            ));
        }
        if let Some(url) = &disclosure.incident_url {
            out.push_str(&format!(
                "<li>Incidents: {}</li>",
                escape_html(&one_line(url))
            ));
        }
        out.push_str("</ul>");
    }
    out.push_str(
        "<p><small>Static disclosure generated from the Agent Card and caller-supplied \
         evaluations. It is not a hosted Trust Center or an external audit.</small></p>",
    );
    out.push_str("</body></html>");
    out
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
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

fn validate_incidents(incidents: &[IncidentDisclosure]) -> Result<(), TrustCenterError> {
    let mut ids = BTreeSet::new();
    for incident in incidents {
        if incident.id.trim().is_empty()
            || incident.id.chars().count() > 64
            || !incident.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
        {
            return Err(TrustCenterError::InvalidIncident(
                "id must be 1..=64 safe characters".into(),
            ));
        }
        if !ids.insert(&incident.id) {
            return Err(TrustCenterError::InvalidIncident(format!(
                "duplicate id {}",
                incident.id
            )));
        }
        if incident.title.trim().is_empty() || incident.title.chars().count() > 120 {
            return Err(TrustCenterError::InvalidIncident(
                "title must be 1..=120 characters".into(),
            ));
        }
        if incident.summary.trim().is_empty() || incident.summary.chars().count() > 2000 {
            return Err(TrustCenterError::InvalidIncident(
                "summary must be 1..=2000 characters".into(),
            ));
        }
        for (label, value) in [
            ("title", incident.title.as_str()),
            ("summary", incident.summary.as_str()),
        ] {
            if value
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
            {
                return Err(TrustCenterError::InvalidIncident(format!(
                    "{label} contains control characters"
                )));
            }
        }
        match incident.status {
            IncidentStatus::Open if incident.resolved_at_ms.is_some() => {
                return Err(TrustCenterError::InvalidIncident(
                    "an open incident cannot have resolved_at_ms".into(),
                ));
            }
            IncidentStatus::Resolved
                if incident
                    .resolved_at_ms
                    .is_none_or(|resolved| resolved < incident.opened_at_ms) =>
            {
                return Err(TrustCenterError::InvalidIncident(
                    "a resolved incident needs resolved_at_ms >= opened_at_ms".into(),
                ));
            }
            _ => {}
        }
        if let Some(link) = &incident.link {
            let lower = link.to_ascii_lowercase();
            if !(lower.starts_with("https://") || lower.starts_with("http://"))
                || link
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control())
            {
                return Err(TrustCenterError::InvalidIncident(
                    "link must be a single http(s) URL".into(),
                ));
            }
        }
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
    fn html_escapes_caller_supplied_text() {
        let disclosure = build_disclosure(
            "0.97.0",
            10,
            Some("<script>alert(1)</script>".into()),
            Some("javascript:alert(1)".into()),
            None,
            vec![evaluation()],
        )
        .unwrap();
        let html = render_html(&disclosure);
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains("href=\"javascript"));
        assert!(html.contains("not_audited"));
        assert!(html.contains("core-v2"));
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
    fn markdown_states_the_audit_and_limits() {
        let disclosure = build_disclosure(
            "0.97.0",
            10,
            Some("security@example.com".into()),
            None,
            None,
            vec![evaluation()],
        )
        .unwrap();
        let markdown = render_markdown(&disclosure);
        assert!(markdown.contains("# 1Agent Trust Disclosure"));
        assert!(markdown.contains("Version: 0.97.0"));
        assert!(markdown.contains("Status: not_audited"));
        assert!(markdown.contains("External audit: false"));
        assert!(markdown.contains("core-v2 1: 6/6"));
        assert!(markdown.contains("security@example.com"));
    }

    fn incident(
        id: &str,
        status: IncidentStatus,
        opened_at_ms: u128,
        resolved_at_ms: Option<u128>,
    ) -> IncidentDisclosure {
        IncidentDisclosure {
            id: id.into(),
            title: format!("Incident {id}"),
            status,
            opened_at_ms,
            resolved_at_ms,
            summary: "summary <script>alert(1)</script>".into(),
            link: Some("https://example.com/incidents/1".into()),
        }
    }

    #[test]
    fn incidents_are_validated_sorted_and_rendered() {
        let disclosure = build_disclosure_with_incidents(
            "0.97.0",
            10,
            None,
            None,
            None,
            vec![evaluation()],
            vec![
                incident("old", IncidentStatus::Open, 10, None),
                incident("new", IncidentStatus::Resolved, 20, Some(30)),
            ],
        )
        .unwrap();
        assert_eq!(disclosure.incidents[0].id, "new");
        assert_eq!(disclosure.incidents[1].id, "old");
        let markdown = render_markdown(&disclosure);
        assert!(markdown.contains("## Incidents"));
        assert!(markdown.contains("[resolved] new"));
        assert!(markdown.contains("resolved 30 ms"));
        assert!(markdown.contains("[open] old"));
        let html = render_html(&disclosure);
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(!html.contains("href=\"https://example.com/incidents/1\""));

        let duplicate = build_disclosure_with_incidents(
            "0.97.0",
            10,
            None,
            None,
            None,
            vec![],
            vec![
                incident("same", IncidentStatus::Open, 10, None),
                incident("same", IncidentStatus::Open, 20, None),
            ],
        );
        assert!(matches!(
            duplicate,
            Err(TrustCenterError::InvalidIncident(_))
        ));

        let open_with_resolution = build_disclosure_with_incidents(
            "0.97.0",
            10,
            None,
            None,
            None,
            vec![],
            vec![incident("open", IncidentStatus::Open, 10, Some(20))],
        );
        assert!(matches!(
            open_with_resolution,
            Err(TrustCenterError::InvalidIncident(_))
        ));

        let resolved_missing_time = build_disclosure_with_incidents(
            "0.97.0",
            10,
            None,
            None,
            None,
            vec![],
            vec![incident("resolved", IncidentStatus::Resolved, 10, None)],
        );
        assert!(matches!(
            resolved_missing_time,
            Err(TrustCenterError::InvalidIncident(_))
        ));

        let resolved_before_open = build_disclosure_with_incidents(
            "0.97.0",
            10,
            None,
            None,
            None,
            vec![],
            vec![incident("early", IncidentStatus::Resolved, 20, Some(10))],
        );
        assert!(matches!(
            resolved_before_open,
            Err(TrustCenterError::InvalidIncident(_))
        ));

        let mut bad_link = incident("link", IncidentStatus::Open, 10, None);
        bad_link.link = Some("javascript:alert(1)".into());
        assert!(matches!(
            build_disclosure_with_incidents("0.97.0", 10, None, None, None, vec![], vec![bad_link]),
            Err(TrustCenterError::InvalidIncident(_))
        ));

        let mut bad_id = incident("id", IncidentStatus::Open, 10, None);
        bad_id.id = "bad id".into();
        assert!(matches!(
            build_disclosure_with_incidents("0.97.0", 10, None, None, None, vec![], vec![bad_id]),
            Err(TrustCenterError::InvalidIncident(_))
        ));
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
