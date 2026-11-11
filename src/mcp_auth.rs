//! MCP OAuth claims policy.
//!
//! The caller must verify the token signature with a trusted OAuth/JWT library
//! before constructing `McpToken`. This module only enforces audience, lifetime,
//! TTL and scope constraints on already-verified claims.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum McpAuthError {
    #[error("MCP token claims are invalid: {0}")]
    Invalid(String),
    #[error("MCP token audience does not match the target server")]
    WrongAudience,
    #[error("MCP token is expired")]
    Expired,
    #[error("MCP token is not valid yet")]
    NotYetValid,
    #[error("MCP token lifetime exceeds the policy maximum")]
    TtlTooLong,
    #[error("MCP token lacks the required scope")]
    MissingScope,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpToken {
    pub subject: String,
    pub audience: String,
    pub issued_at_ms: u128,
    pub expires_at_ms: u128,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpTokenPolicy {
    pub expected_audience: String,
    pub max_ttl_ms: u128,
    #[serde(default)]
    pub clock_skew_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpAuthDecision {
    pub authorized: bool,
    pub subject: String,
    pub audience: String,
    pub reason: String,
}

pub fn authorize(
    token: &McpToken,
    policy: &McpTokenPolicy,
    now_ms: u128,
    required_scope: &str,
) -> Result<McpAuthDecision, McpAuthError> {
    if token.subject.trim().is_empty() {
        return Err(McpAuthError::Invalid("subject is empty".into()));
    }
    if token.audience != policy.expected_audience {
        return Err(McpAuthError::WrongAudience);
    }
    if token.expires_at_ms <= token.issued_at_ms {
        return Err(McpAuthError::Invalid(
            "expires_at_ms must be after issued_at_ms".into(),
        ));
    }
    if now_ms > token.expires_at_ms.saturating_add(policy.clock_skew_ms) {
        return Err(McpAuthError::Expired);
    }
    if token.issued_at_ms > now_ms.saturating_add(policy.clock_skew_ms) {
        return Err(McpAuthError::NotYetValid);
    }
    if token.expires_at_ms - token.issued_at_ms > policy.max_ttl_ms {
        return Err(McpAuthError::TtlTooLong);
    }
    if required_scope.trim().is_empty() {
        return Err(McpAuthError::Invalid("required scope is empty".into()));
    }
    if !token.scopes.iter().any(|scope| scope == required_scope) {
        return Err(McpAuthError::MissingScope);
    }
    Ok(McpAuthDecision {
        authorized: true,
        subject: token.subject.clone(),
        audience: token.audience.clone(),
        reason: "audience, lifetime, TTL and scope are valid".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> McpTokenPolicy {
        McpTokenPolicy {
            expected_audience: "mcp://calendar".into(),
            max_ttl_ms: 5 * 60 * 1_000,
            clock_skew_ms: 5_000,
        }
    }

    fn token() -> McpToken {
        McpToken {
            subject: "user-1".into(),
            audience: "mcp://calendar".into(),
            issued_at_ms: 1_000_000,
            expires_at_ms: 1_000_000 + 60_000,
            scopes: vec!["calendar:read".into()],
        }
    }

    #[test]
    fn valid_claims_are_authorized() {
        let decision = authorize(&token(), &policy(), 1_030_000, "calendar:read").unwrap();
        assert!(decision.authorized);
        assert_eq!(decision.subject, "user-1");
    }

    #[test]
    fn audience_expiry_and_skew_are_enforced() {
        let mut wrong = token();
        wrong.audience = "mcp://mail".into();
        assert_eq!(
            authorize(&wrong, &policy(), 1_030_000, "calendar:read"),
            Err(McpAuthError::WrongAudience)
        );
        assert_eq!(
            authorize(&token(), &policy(), 1_066_000, "calendar:read"),
            Err(McpAuthError::Expired)
        );
        assert!(authorize(&token(), &policy(), 1_064_000, "calendar:read").is_ok());
        assert_eq!(
            authorize(&token(), &policy(), 994_000, "calendar:read").unwrap_err(),
            McpAuthError::NotYetValid
        );
        assert!(authorize(&token(), &policy(), 997_000, "calendar:read").is_ok());
    }

    #[test]
    fn ttl_and_scope_are_enforced() {
        let mut long = token();
        long.expires_at_ms = long.issued_at_ms + 10 * 60 * 1_000;
        assert_eq!(
            authorize(&long, &policy(), 1_030_000, "calendar:read"),
            Err(McpAuthError::TtlTooLong)
        );
        assert_eq!(
            authorize(&token(), &policy(), 1_030_000, "calendar:write"),
            Err(McpAuthError::MissingScope)
        );
    }

    #[test]
    fn malformed_claims_are_rejected() {
        let mut empty = token();
        empty.subject = " ".into();
        assert!(matches!(
            authorize(&empty, &policy(), 1_030_000, "calendar:read"),
            Err(McpAuthError::Invalid(_))
        ));
        let mut inverted = token();
        inverted.expires_at_ms = inverted.issued_at_ms;
        assert!(matches!(
            authorize(&inverted, &policy(), 1_030_000, "calendar:read"),
            Err(McpAuthError::Invalid(_))
        ));
        assert!(matches!(
            authorize(&token(), &policy(), 1_030_000, ""),
            Err(McpAuthError::Invalid(_))
        ));
    }
}
