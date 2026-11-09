//! Bounded standing approvals.
//!
//! A standing approval is a scoped, expiring, use-limited substitute for a
//! one-time approval. It never grants scope or strength beyond the profile that
//! the authority layer already allows.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::authority::{PolicyAction, PolicyRequest};
use crate::domain::RiskLevel;
use crate::task_graph::MandateStrength;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StandingApprovalError {
    #[error("standing approval id is unsafe")]
    UnsafeId,
    #[error("standing approval uses must be 1..=1000")]
    InvalidUses,
    #[error("standing approval scope is empty")]
    EmptyScope,
    #[error("standing approval {0} already exists")]
    AlreadyExists(String),
    #[error("standing approval {0} not found")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StandingApproval {
    pub approval_id: String,
    pub actor_id: String,
    pub scopes: Vec<String>,
    pub max_strength: MandateStrength,
    pub max_risk: RiskLevel,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    pub expires_ms: u128,
    pub max_uses: u32,
    #[serde(default)]
    pub used: u32,
    #[serde(default)]
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StandingApprovalUse {
    pub approval_id: String,
    pub remaining: u32,
    pub expires_ms: u128,
}

#[derive(Debug, Default)]
pub struct StandingApprovalRegistry {
    approvals: BTreeMap<String, StandingApproval>,
}

impl StandingApprovalRegistry {
    pub fn grant(&mut self, approval: StandingApproval) -> Result<(), StandingApprovalError> {
        if !safe_id(&approval.approval_id) || !safe_id(&approval.actor_id) {
            return Err(StandingApprovalError::UnsafeId);
        }
        if !(1..=1_000).contains(&approval.max_uses) {
            return Err(StandingApprovalError::InvalidUses);
        }
        if approval.scopes.is_empty()
            || approval
                .scopes
                .iter()
                .any(|scope| scope.trim().is_empty() || scope.chars().any(char::is_control))
        {
            return Err(StandingApprovalError::EmptyScope);
        }
        if self.approvals.contains_key(&approval.approval_id) {
            return Err(StandingApprovalError::AlreadyExists(approval.approval_id));
        }
        self.approvals
            .insert(approval.approval_id.clone(), approval);
        Ok(())
    }

    pub fn revoke(&mut self, approval_id: &str) -> Result<(), StandingApprovalError> {
        let approval = self
            .approvals
            .get_mut(approval_id)
            .ok_or_else(|| StandingApprovalError::Unknown(approval_id.into()))?;
        approval.revoked = true;
        Ok(())
    }

    /// Consume one use when the request matches; returns None when nothing
    /// matches or the grant is expired/revoked/exhausted.
    pub fn authorize(
        &mut self,
        request: &PolicyRequest,
        now_ms: u128,
    ) -> Option<StandingApprovalUse> {
        let approval = self
            .approvals
            .values_mut()
            .find(|approval| matches(approval, request, now_ms))?;
        approval.used = approval.used.saturating_add(1);
        Some(StandingApprovalUse {
            approval_id: approval.approval_id.clone(),
            remaining: approval.max_uses.saturating_sub(approval.used),
            expires_ms: approval.expires_ms,
        })
    }

    pub fn get(&self, approval_id: &str) -> Option<&StandingApproval> {
        self.approvals.get(approval_id)
    }

    pub fn len(&self) -> usize {
        self.approvals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.approvals.is_empty()
    }
}

fn matches(approval: &StandingApproval, request: &PolicyRequest, now_ms: u128) -> bool {
    approval.actor_id == request.actor_id
        && !approval.revoked
        && approval.expires_ms > now_ms
        && approval.used < approval.max_uses
        && strength_rank(request.strength) <= strength_rank(approval.max_strength)
        && risk_rank(request.risk) <= risk_rank(approval.max_risk)
        && scope_allowed(&approval.scopes, &request.scope)
        && tool_allowed(approval, request.tool.as_deref())
        && !matches!(request.action, PolicyAction::Approve)
}

fn tool_allowed(approval: &StandingApproval, tool: Option<&str>) -> bool {
    let Some(tool) = tool else {
        return true;
    };
    !approval.denied_tools.iter().any(|denied| denied == tool)
        && approval
            .allowed_tools
            .iter()
            .any(|allowed| allowed == "*" || allowed == tool)
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

    fn approval() -> StandingApproval {
        StandingApproval {
            approval_id: "standing-1".into(),
            actor_id: "owner".into(),
            scopes: vec!["repo:1-agent*".into()],
            max_strength: MandateStrength::Write,
            max_risk: RiskLevel::Medium,
            allowed_tools: vec!["echo".into()],
            denied_tools: Vec::new(),
            expires_ms: 100,
            max_uses: 2,
            used: 0,
            revoked: false,
        }
    }

    fn request() -> PolicyRequest {
        PolicyRequest {
            actor_id: "owner".into(),
            action: PolicyAction::Write,
            scope: "repo:1-agent/src".into(),
            strength: MandateStrength::Write,
            tool: Some("echo".into()),
            risk: RiskLevel::Medium,
        }
    }

    #[test]
    fn matching_requests_consume_the_use_budget() {
        let mut registry = StandingApprovalRegistry::default();
        registry.grant(approval()).unwrap();
        assert_eq!(registry.authorize(&request(), 10).unwrap().remaining, 1);
        assert_eq!(registry.authorize(&request(), 11).unwrap().remaining, 0);
        assert!(registry.authorize(&request(), 12).is_none());
    }

    #[test]
    fn expiry_scope_tool_and_revocation_fail_closed() {
        let mut registry = StandingApprovalRegistry::default();
        registry.grant(approval()).unwrap();
        assert!(registry.authorize(&request(), 100).is_none());
        let mut outside = request();
        outside.scope = "repo:other".into();
        assert!(registry.authorize(&outside, 10).is_none());
        let mut denied = request();
        denied.tool = Some("shell".into());
        assert!(registry.authorize(&denied, 10).is_none());
        registry.revoke("standing-1").unwrap();
        assert!(registry.authorize(&request(), 10).is_none());
    }

    #[test]
    fn validation_rejects_unbounded_grants() {
        let mut registry = StandingApprovalRegistry::default();
        let mut bad = approval();
        bad.max_uses = 0;
        assert_eq!(
            registry.grant(bad).unwrap_err(),
            StandingApprovalError::InvalidUses
        );
        let mut bad = approval();
        bad.approval_id = "bad/id".into();
        assert_eq!(
            registry.grant(bad).unwrap_err(),
            StandingApprovalError::UnsafeId
        );
    }

    #[test]
    fn approval_action_is_never_covered_by_standing_approval() {
        let mut registry = StandingApprovalRegistry::default();
        registry.grant(approval()).unwrap();
        let mut request = request();
        request.action = PolicyAction::Approve;
        assert!(registry.authorize(&request, 10).is_none());
    }
}
