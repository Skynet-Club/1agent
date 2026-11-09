//! Tool execution gate: capability metadata + Authority/Policy.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::authority::{AuthorityRegistry, PolicyAction, PolicyOutcome, PolicyRequest};
use crate::domain::RiskLevel;
use crate::task_graph::MandateStrength;
use crate::tools::ToolRegistry;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecutionGateError {
    #[error("tool {0} is not registered")]
    UnknownTool(String),
    #[error("tool {0} denied: {1}")]
    Denied(String, String),
}

#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    pub actor_id: String,
    pub tool: String,
    pub scope: String,
    pub strength: MandateStrength,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAuthorization {
    pub tool: String,
    pub outcome: PolicyOutcome,
    pub reason: String,
    pub requires_write: bool,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone)]
pub struct ExecutionGate {
    authority: AuthorityRegistry,
    tools: BTreeMap<String, crate::tools::ToolCapability>,
}

impl ExecutionGate {
    pub fn new(authority: AuthorityRegistry, registry: &ToolRegistry) -> Self {
        let tools = registry
            .capabilities()
            .into_iter()
            .map(|capability| (capability.name.clone(), capability))
            .collect();
        Self { authority, tools }
    }

    pub fn tools(&self) -> impl Iterator<Item = &crate::tools::ToolCapability> {
        self.tools.values()
    }

    pub fn authorize(
        &self,
        request: &ToolCallRequest,
        now_ms: u128,
    ) -> Result<ToolAuthorization, ExecutionGateError> {
        let capability = self
            .tools
            .get(&request.tool)
            .ok_or_else(|| ExecutionGateError::UnknownTool(request.tool.clone()))?;
        let action = if capability.requires_write {
            PolicyAction::Write
        } else {
            PolicyAction::Read
        };
        let policy = self.authority.evaluate(
            &PolicyRequest {
                actor_id: request.actor_id.clone(),
                action,
                scope: request.scope.clone(),
                strength: request.strength,
                tool: Some(request.tool.clone()),
                risk: request.risk.max(capability.risk),
            },
            now_ms,
        );
        if policy.outcome == PolicyOutcome::Deny {
            return Err(ExecutionGateError::Denied(
                request.tool.clone(),
                policy.reason,
            ));
        }
        Ok(ToolAuthorization {
            tool: request.tool.clone(),
            outcome: policy.outcome,
            reason: policy.reason,
            requires_write: capability.requires_write,
            risk: capability.risk,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::AuthorityProfile;
    use crate::tools::default_registry;

    fn authority() -> AuthorityRegistry {
        let mut authority = AuthorityRegistry::default();
        authority
            .register(AuthorityProfile {
                profile_id: "tool-profile".into(),
                actor_id: "actor-1".into(),
                scopes: vec!["repo*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: vec!["read_file".into()],
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Low,
                expires_ms: None,
            })
            .unwrap();
        authority.bind("actor-1", "tool-profile").unwrap();
        authority
    }

    fn request(tool: &str) -> ToolCallRequest {
        ToolCallRequest {
            actor_id: "actor-1".into(),
            tool: tool.into(),
            scope: "repo:1-agent".into(),
            strength: MandateStrength::ReadOnly,
            risk: RiskLevel::Low,
        }
    }

    #[test]
    fn allowed_tool_passes_and_other_tools_fail_closed() {
        let gate = ExecutionGate::new(authority(), &default_registry());
        let allowed = gate.authorize(&request("read_file"), 10).unwrap();
        assert_eq!(allowed.outcome, PolicyOutcome::Allow);
        assert!(!allowed.requires_write);
        assert!(matches!(
            gate.authorize(&request("search_files"), 10).unwrap_err(),
            ExecutionGateError::Denied(_, _)
        ));
        assert_eq!(
            gate.authorize(&request("missing-tool"), 10).unwrap_err(),
            ExecutionGateError::UnknownTool("missing-tool".into())
        );
    }

    #[test]
    fn unknown_actor_and_expired_profile_are_denied() {
        let gate = ExecutionGate::new(authority(), &default_registry());
        let mut unknown = request("read_file");
        unknown.actor_id = "attacker".into();
        assert!(matches!(
            gate.authorize(&unknown, 10).unwrap_err(),
            ExecutionGateError::Denied(_, _)
        ));
    }
}
