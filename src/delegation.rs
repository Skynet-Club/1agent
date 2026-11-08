//! Delegation profiles: how much initiative an actor may take in one domain.
//!
//! Delegation is a working style, not a permission source. Every grant is the
//! intersection of the requested mode, the delegation profile and the live
//! `AuthorityRegistry` policy. `act_and_report` additionally needs approval and
//! a reversible action.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::authority::{AuthorityRegistry, PolicyAction, PolicyOutcome, PolicyRequest};
use crate::domain::RiskLevel;
use crate::task_graph::MandateStrength;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DelegationError {
    #[error("delegation profile id must not be empty")]
    EmptyProfileId,
    #[error("delegation actor must not be empty")]
    EmptyActor,
    #[error("delegation domain must not be empty")]
    EmptyDomain,
    #[error("delegation profile already exists for {0}/{1}")]
    DuplicateProfile(String, String),
    #[error("delegation profile not found for {0}/{1}")]
    UnknownProfile(String, String),
    #[error("delegation profile is revoked")]
    Revoked,
    #[error("delegation profile is expired")]
    Expired,
    #[error("requested mode {requested:?} exceeds granted mode {granted:?}")]
    ModeExceeded {
        requested: DelegationMode,
        granted: DelegationMode,
    },
    #[error("action risk {requested:?} exceeds delegation limit {limit:?}")]
    RiskExceeded {
        requested: RiskLevel,
        limit: RiskLevel,
    },
    #[error("tool {0} is not allowed by the delegation profile")]
    ToolNotAllowed(String),
    #[error("act_and_report requires an approval")]
    ApprovalRequired,
    #[error("act_and_report requires a reversible action")]
    ReversibilityRequired,
    #[error("authority policy denied delegation: {0}")]
    AuthorityDenied(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DelegationMode {
    Observe,
    Suggest,
    Draft,
    Ask,
    ActAndReport,
}

impl DelegationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Suggest => "suggest",
            Self::Draft => "draft",
            Self::Ask => "ask",
            Self::ActAndReport => "act_and_report",
        }
    }
}

impl std::str::FromStr for DelegationMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "observe" => Ok(Self::Observe),
            "suggest" => Ok(Self::Suggest),
            "draft" => Ok(Self::Draft),
            "ask" => Ok(Self::Ask),
            "act_and_report" => Ok(Self::ActAndReport),
            other => Err(format!("unknown delegation mode {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DelegationProfile {
    pub profile_id: String,
    pub actor_id: String,
    pub domain: String,
    pub mode: DelegationMode,
    pub max_risk: RiskLevel,
    pub allowed_tools: Vec<String>,
    pub expires_ms: Option<u128>,
}

#[derive(Debug, Clone)]
pub struct DelegationRequest<'a> {
    pub actor_id: &'a str,
    pub domain: &'a str,
    pub requested_mode: DelegationMode,
    pub strength: MandateStrength,
    pub risk: RiskLevel,
    pub tool: Option<&'a str>,
    pub has_approval: bool,
    pub reversible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DelegationGrant {
    pub actor_id: String,
    pub domain: String,
    pub mode: DelegationMode,
    pub policy_outcome: PolicyOutcome,
    pub requires_report: bool,
}

#[derive(Debug, Default)]
pub struct DelegationRegistry {
    profiles: BTreeMap<(String, String), DelegationProfile>,
    revoked: BTreeMap<(String, String), u128>,
}

impl DelegationRegistry {
    pub fn register(&mut self, profile: DelegationProfile) -> Result<(), DelegationError> {
        if profile.profile_id.trim().is_empty() {
            return Err(DelegationError::EmptyProfileId);
        }
        if profile.actor_id.trim().is_empty() {
            return Err(DelegationError::EmptyActor);
        }
        if profile.domain.trim().is_empty() {
            return Err(DelegationError::EmptyDomain);
        }
        let key = (profile.actor_id.clone(), profile.domain.clone());
        if self.profiles.contains_key(&key) {
            return Err(DelegationError::DuplicateProfile(
                profile.actor_id,
                profile.domain,
            ));
        }
        self.profiles.insert(key, profile);
        Ok(())
    }

    pub fn profile(&self, actor_id: &str, domain: &str) -> Option<&DelegationProfile> {
        self.profiles.get(&(actor_id.into(), domain.into()))
    }

    pub fn revoke(
        &mut self,
        actor_id: &str,
        domain: &str,
        now_ms: u128,
    ) -> Result<(), DelegationError> {
        let key = (actor_id.into(), domain.into());
        if !self.profiles.contains_key(&key) {
            return Err(DelegationError::UnknownProfile(
                actor_id.into(),
                domain.into(),
            ));
        }
        if self.revoked.contains_key(&key) {
            return Err(DelegationError::Revoked);
        }
        self.revoked.insert(key, now_ms);
        Ok(())
    }

    pub fn authorize(
        &self,
        request: &DelegationRequest<'_>,
        authority: &AuthorityRegistry,
        now_ms: u128,
    ) -> Result<DelegationGrant, DelegationError> {
        let key = (request.actor_id.into(), request.domain.into());
        let profile = self.profiles.get(&key).ok_or_else(|| {
            DelegationError::UnknownProfile(request.actor_id.into(), request.domain.into())
        })?;
        if self.revoked.contains_key(&key) {
            return Err(DelegationError::Revoked);
        }
        if profile.expires_ms.is_some_and(|expires| now_ms > expires) {
            return Err(DelegationError::Expired);
        }
        if request.requested_mode > profile.mode {
            return Err(DelegationError::ModeExceeded {
                requested: request.requested_mode,
                granted: profile.mode,
            });
        }
        if request.risk > profile.max_risk {
            return Err(DelegationError::RiskExceeded {
                requested: request.risk,
                limit: profile.max_risk,
            });
        }
        if let Some(tool) = request.tool
            && !profile.allowed_tools.iter().any(|allowed| allowed == tool)
        {
            return Err(DelegationError::ToolNotAllowed(tool.into()));
        }
        if request.requested_mode == DelegationMode::ActAndReport && !request.has_approval {
            return Err(DelegationError::ApprovalRequired);
        }
        if request.requested_mode == DelegationMode::ActAndReport && !request.reversible {
            return Err(DelegationError::ReversibilityRequired);
        }

        let action = if request.requested_mode >= DelegationMode::Ask {
            PolicyAction::Write
        } else {
            PolicyAction::Read
        };
        let policy = authority.evaluate(
            &PolicyRequest {
                actor_id: request.actor_id.into(),
                action,
                scope: request.domain.into(),
                strength: request.strength,
                tool: request.tool.map(str::to_string),
                risk: request.risk,
            },
            now_ms,
        );
        match policy.outcome {
            PolicyOutcome::Deny => Err(DelegationError::AuthorityDenied(policy.reason)),
            PolicyOutcome::RequireApproval if !request.has_approval => {
                Err(DelegationError::ApprovalRequired)
            }
            _ => Ok(DelegationGrant {
                actor_id: request.actor_id.into(),
                domain: request.domain.into(),
                mode: request.requested_mode,
                policy_outcome: policy.outcome,
                requires_report: request.requested_mode == DelegationMode::ActAndReport,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::AuthorityProfile;

    fn authority() -> AuthorityRegistry {
        let mut authority = AuthorityRegistry::default();
        authority
            .register(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["repo*".into()],
                max_strength: MandateStrength::Write,
                allowed_tools: vec!["write_file".into()],
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        authority.bind("owner-1", "owner-profile").unwrap();
        authority
    }

    fn profile(mode: DelegationMode) -> DelegationProfile {
        DelegationProfile {
            profile_id: "delegation-1".into(),
            actor_id: "owner-1".into(),
            domain: "repo:1-agent".into(),
            mode,
            max_risk: RiskLevel::Medium,
            allowed_tools: vec!["write_file".into()],
            expires_ms: None,
        }
    }

    fn request(mode: DelegationMode) -> DelegationRequest<'static> {
        DelegationRequest {
            actor_id: "owner-1",
            domain: "repo:1-agent",
            requested_mode: mode,
            strength: MandateStrength::Write,
            risk: RiskLevel::Low,
            tool: Some("write_file"),
            has_approval: false,
            reversible: true,
        }
    }

    #[test]
    fn act_and_report_requires_approval_and_reversibility() {
        let mut registry = DelegationRegistry::default();
        registry
            .register(profile(DelegationMode::ActAndReport))
            .unwrap();
        assert_eq!(
            registry
                .authorize(&request(DelegationMode::ActAndReport), &authority(), 10)
                .unwrap_err(),
            DelegationError::ApprovalRequired
        );
        let mut irreversible = request(DelegationMode::ActAndReport);
        irreversible.has_approval = true;
        irreversible.reversible = false;
        assert_eq!(
            registry
                .authorize(&irreversible, &authority(), 10)
                .unwrap_err(),
            DelegationError::ReversibilityRequired
        );
        let mut allowed = request(DelegationMode::ActAndReport);
        allowed.has_approval = true;
        let grant = registry.authorize(&allowed, &authority(), 10).unwrap();
        assert!(grant.requires_report);
        assert_eq!(grant.mode, DelegationMode::ActAndReport);
    }

    #[test]
    fn mode_risk_tool_and_authority_are_intersected() {
        let mut registry = DelegationRegistry::default();
        registry.register(profile(DelegationMode::Suggest)).unwrap();
        assert!(matches!(
            registry
                .authorize(&request(DelegationMode::ActAndReport), &authority(), 10)
                .unwrap_err(),
            DelegationError::ModeExceeded { .. }
        ));
        let mut risky = request(DelegationMode::Suggest);
        risky.risk = RiskLevel::High;
        assert!(matches!(
            registry.authorize(&risky, &authority(), 10).unwrap_err(),
            DelegationError::RiskExceeded { .. }
        ));
        let mut tool = request(DelegationMode::Suggest);
        tool.tool = Some("unknown_tool");
        assert!(matches!(
            registry.authorize(&tool, &authority(), 10).unwrap_err(),
            DelegationError::AuthorityDenied(_) | DelegationError::ToolNotAllowed(_)
        ));

        let mut attacker = request(DelegationMode::Suggest);
        attacker.actor_id = "attacker";
        assert!(matches!(
            registry.authorize(&attacker, &authority(), 10).unwrap_err(),
            DelegationError::UnknownProfile(_, _)
        ));
    }

    #[test]
    fn revocation_and_expiry_fail_closed() {
        let mut registry = DelegationRegistry::default();
        let mut expiring = profile(DelegationMode::Observe);
        expiring.expires_ms = Some(100);
        registry.register(expiring).unwrap();
        assert_eq!(
            registry
                .authorize(&request(DelegationMode::Observe), &authority(), 101)
                .unwrap_err(),
            DelegationError::Expired
        );
        registry.revoke("owner-1", "repo:1-agent", 101).unwrap();
        assert_eq!(
            registry
                .authorize(&request(DelegationMode::Observe), &authority(), 102)
                .unwrap_err(),
            DelegationError::Revoked
        );
    }
}
