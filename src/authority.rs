//! Authority profiles, identity bindings and deterministic policy checks.
//!
//! This is a prerequisite for A-Gate AG-03/AG-04: a mandate or ticket is not
//! enough; the actor must be bound to a live profile that permits the requested
//! scope, strength, tool and risk.

use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::RiskLevel;
use crate::task_graph::MandateStrength;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorityError {
    #[error("authority id is unsafe")]
    UnsafeId,
    #[error("authority scope is empty")]
    EmptyScope,
    #[error("authority profile {0} already exists")]
    ProfileExists(String),
    #[error("authority profile {0} not found")]
    UnknownProfile(String),
    #[error("authority actor {0} is not bound")]
    UnboundActor(String),
    #[error("authority profile {profile} belongs to actor {owner}, not {actor}")]
    ActorMismatch {
        actor: String,
        owner: String,
        profile: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityProfile {
    pub profile_id: String,
    pub actor_id: String,
    pub scopes: Vec<String>,
    pub max_strength: MandateStrength,
    /// Empty means no tools are allowed. `*` allows every non-denied tool.
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    pub max_risk: RiskLevel,
    #[serde(default)]
    pub expires_ms: Option<u128>,
}

impl AuthorityProfile {
    pub fn validate(&self) -> Result<(), AuthorityError> {
        if !safe_id(&self.profile_id) || !safe_id(&self.actor_id) {
            return Err(AuthorityError::UnsafeId);
        }
        if self.scopes.is_empty()
            || self.scopes.iter().any(|scope| {
                scope.trim().is_empty()
                    || scope.chars().count() > 256
                    || scope.chars().any(|ch| ch.is_control())
            })
        {
            return Err(AuthorityError::EmptyScope);
        }
        Ok(())
    }

    pub fn is_live(&self, now_ms: u128) -> bool {
        self.expires_ms.is_none_or(|expires| expires > now_ms)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    Read,
    Write,
    Execute,
    Network,
    Approve,
    Dispatch,
}

impl PolicyAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Execute => "execute",
            Self::Network => "network",
            Self::Approve => "approve",
            Self::Dispatch => "dispatch",
        }
    }
}

impl FromStr for PolicyAction {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "execute" => Ok(Self::Execute),
            "network" => Ok(Self::Network),
            "approve" => Ok(Self::Approve),
            "dispatch" => Ok(Self::Dispatch),
            other => Err(format!("unknown policy action {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyRequest {
    pub actor_id: String,
    pub action: PolicyAction,
    pub scope: String,
    pub strength: MandateStrength,
    #[serde(default)]
    pub tool: Option<String>,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyOutcome {
    Allow,
    RequireApproval,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PolicyDecision {
    pub outcome: PolicyOutcome,
    pub reason: String,
}

impl PolicyDecision {
    fn allow(reason: impl Into<String>) -> Self {
        Self {
            outcome: PolicyOutcome::Allow,
            reason: reason.into(),
        }
    }

    fn require_approval(reason: impl Into<String>) -> Self {
        Self {
            outcome: PolicyOutcome::RequireApproval,
            reason: reason.into(),
        }
    }

    fn deny(reason: impl Into<String>) -> Self {
        Self {
            outcome: PolicyOutcome::Deny,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct AuthorityRegistry {
    profiles: BTreeMap<String, AuthorityProfile>,
    bindings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityBinding {
    pub actor_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorityConfig {
    pub profiles: Vec<AuthorityProfile>,
    pub bindings: Vec<AuthorityBinding>,
}

impl AuthorityRegistry {
    pub fn apply_config(&mut self, config: &AuthorityConfig) -> Result<(), AuthorityError> {
        for profile in &config.profiles {
            self.register(profile.clone())?;
        }
        for binding in &config.bindings {
            self.bind(&binding.actor_id, &binding.profile_id)?;
        }
        Ok(())
    }

    pub fn register(&mut self, profile: AuthorityProfile) -> Result<(), AuthorityError> {
        profile.validate()?;
        if self.profiles.contains_key(&profile.profile_id) {
            return Err(AuthorityError::ProfileExists(profile.profile_id));
        }
        self.profiles.insert(profile.profile_id.clone(), profile);
        Ok(())
    }

    pub fn bind(&mut self, actor_id: &str, profile_id: &str) -> Result<(), AuthorityError> {
        if !safe_id(actor_id) {
            return Err(AuthorityError::UnsafeId);
        }
        let Some(profile) = self.profiles.get(profile_id) else {
            return Err(AuthorityError::UnknownProfile(profile_id.into()));
        };
        if profile.actor_id != actor_id {
            return Err(AuthorityError::ActorMismatch {
                actor: actor_id.into(),
                owner: profile.actor_id.clone(),
                profile: profile_id.into(),
            });
        }
        self.bindings.insert(actor_id.into(), profile_id.into());
        Ok(())
    }

    pub fn unbind(&mut self, actor_id: &str) -> Option<String> {
        self.bindings.remove(actor_id)
    }

    pub fn profile_for(&self, actor_id: &str) -> Option<&AuthorityProfile> {
        self.bindings
            .get(actor_id)
            .and_then(|profile_id| self.profiles.get(profile_id))
    }

    pub fn evaluate(&self, request: &PolicyRequest, now_ms: u128) -> PolicyDecision {
        let Some(profile) = self.profile_for(&request.actor_id) else {
            return PolicyDecision::deny(format!(
                "actor {} is not bound to an authority profile",
                request.actor_id
            ));
        };
        if !profile.is_live(now_ms) {
            return PolicyDecision::deny(format!(
                "authority profile {} is expired",
                profile.profile_id
            ));
        }
        if !scope_allowed(&profile.scopes, &request.scope) {
            return PolicyDecision::deny(format!(
                "scope {} is outside profile {}",
                request.scope, profile.profile_id
            ));
        }
        if strength_rank(request.strength) > strength_rank(profile.max_strength) {
            return PolicyDecision::deny(format!(
                "strength {:?} exceeds profile maximum {:?}",
                request.strength, profile.max_strength
            ));
        }
        let required = action_strength(request.action);
        if strength_rank(request.strength) < strength_rank(required) {
            return PolicyDecision::deny(format!(
                "{} requires at least {:?} strength",
                request.action.as_str(),
                required
            ));
        }
        if risk_rank(request.risk) > risk_rank(profile.max_risk) {
            return PolicyDecision::deny(format!(
                "risk {:?} exceeds profile maximum {:?}",
                request.risk, profile.max_risk
            ));
        }
        if let Some(tool) = &request.tool {
            if profile.denied_tools.iter().any(|denied| denied == tool) {
                return PolicyDecision::deny(format!("tool {tool} is explicitly denied"));
            }
            let allowed = profile
                .allowed_tools
                .iter()
                .any(|allowed| allowed == "*" || allowed == tool);
            if !allowed {
                return PolicyDecision::deny(format!(
                    "tool {tool} is not in the profile allowlist"
                ));
            }
        }
        if matches!(
            request.action,
            PolicyAction::Write
                | PolicyAction::Execute
                | PolicyAction::Network
                | PolicyAction::Dispatch
        ) || request.strength != MandateStrength::ReadOnly
            || request.risk == RiskLevel::High
        {
            return PolicyDecision::require_approval(format!(
                "{} at {:?} requires explicit approval",
                request.action.as_str(),
                request.strength
            ));
        }
        PolicyDecision::allow(format!(
            "{} within profile {}",
            request.action.as_str(),
            profile.profile_id
        ))
    }
}

fn scope_allowed(patterns: &[String], requested: &str) -> bool {
    patterns.iter().any(|pattern| {
        pattern == "*"
            || pattern == requested
            || pattern
                .strip_suffix('*')
                .is_some_and(|prefix| requested.starts_with(prefix))
    })
}

fn strength_rank(strength: MandateStrength) -> u8 {
    match strength {
        MandateStrength::ReadOnly => 0,
        MandateStrength::Write => 1,
        MandateStrength::Execute => 2,
        MandateStrength::Network => 3,
    }
}

fn action_strength(action: PolicyAction) -> MandateStrength {
    match action {
        PolicyAction::Read | PolicyAction::Approve | PolicyAction::Dispatch => {
            MandateStrength::ReadOnly
        }
        PolicyAction::Write => MandateStrength::Write,
        PolicyAction::Execute => MandateStrength::Execute,
        PolicyAction::Network => MandateStrength::Network,
    }
}

fn risk_rank(risk: RiskLevel) -> u8 {
    match risk {
        RiskLevel::Low => 0,
        RiskLevel::Medium => 1,
        RiskLevel::High => 2,
    }
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> AuthorityRegistry {
        let mut registry = AuthorityRegistry::default();
        registry
            .register(AuthorityProfile {
                profile_id: "profile-1".into(),
                actor_id: "actor-1".into(),
                scopes: vec!["repo:1-agent*".into()],
                max_strength: MandateStrength::Write,
                allowed_tools: vec!["read-file".into(), "echo".into()],
                denied_tools: vec!["shell".into()],
                max_risk: RiskLevel::Medium,
                expires_ms: Some(100),
            })
            .unwrap();
        registry.bind("actor-1", "profile-1").unwrap();
        registry
    }

    fn request() -> PolicyRequest {
        PolicyRequest {
            actor_id: "actor-1".into(),
            action: PolicyAction::Read,
            scope: "repo:1-agent/src".into(),
            strength: MandateStrength::ReadOnly,
            tool: Some("read-file".into()),
            risk: RiskLevel::Low,
        }
    }

    #[test]
    fn unbound_and_expired_actors_are_denied() {
        let empty = AuthorityRegistry::default();
        assert_eq!(empty.evaluate(&request(), 10).outcome, PolicyOutcome::Deny);
        let registry = registry();
        assert_eq!(
            registry.evaluate(&request(), 100).outcome,
            PolicyOutcome::Deny
        );
    }

    #[test]
    fn scope_strength_risk_and_tools_are_enforced() {
        let registry = registry();
        assert_eq!(
            registry.evaluate(&request(), 10).outcome,
            PolicyOutcome::Allow
        );
        let mut outside = request();
        outside.scope = "repo:other".into();
        assert_eq!(registry.evaluate(&outside, 10).outcome, PolicyOutcome::Deny);
        let mut too_strong = request();
        too_strong.strength = MandateStrength::Execute;
        assert_eq!(
            registry.evaluate(&too_strong, 10).outcome,
            PolicyOutcome::Deny
        );
        let mut high_risk = request();
        high_risk.risk = RiskLevel::High;
        assert_eq!(
            registry.evaluate(&high_risk, 10).outcome,
            PolicyOutcome::Deny
        );
        let mut denied_tool = request();
        denied_tool.tool = Some("shell".into());
        assert_eq!(
            registry.evaluate(&denied_tool, 10).outcome,
            PolicyOutcome::Deny
        );
        let mut unknown_tool = request();
        unknown_tool.tool = Some("browser".into());
        assert_eq!(
            registry.evaluate(&unknown_tool, 10).outcome,
            PolicyOutcome::Deny
        );
    }

    #[test]
    fn write_requires_approval_but_approve_is_a_control_action() {
        let registry = registry();
        let mut write = request();
        write.action = PolicyAction::Write;
        write.strength = MandateStrength::Write;
        write.tool = Some("echo".into());
        assert_eq!(
            registry.evaluate(&write, 10).outcome,
            PolicyOutcome::RequireApproval
        );
        let mut approve = request();
        approve.action = PolicyAction::Approve;
        assert_eq!(
            registry.evaluate(&approve, 10).outcome,
            PolicyOutcome::Allow
        );
    }

    #[test]
    fn bindings_can_be_replaced_and_removed() {
        let mut registry = registry();
        registry
            .register(AuthorityProfile {
                profile_id: "profile-2".into(),
                actor_id: "actor-1".into(),
                scopes: vec!["*".into()],
                max_strength: MandateStrength::Network,
                allowed_tools: vec!["*".into()],
                denied_tools: Vec::new(),
                max_risk: RiskLevel::High,
                expires_ms: None,
            })
            .unwrap();
        registry.bind("actor-1", "profile-2").unwrap();
        assert_eq!(
            registry.evaluate(&request(), 1_000).outcome,
            PolicyOutcome::Allow
        );
        assert_eq!(registry.unbind("actor-1").as_deref(), Some("profile-2"));
        assert_eq!(
            registry.evaluate(&request(), 10).outcome,
            PolicyOutcome::Deny
        );
        assert!(matches!(
            registry.bind("actor-2", "profile-2").unwrap_err(),
            AuthorityError::ActorMismatch { .. }
        ));
    }
}
