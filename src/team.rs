//! Team roles, actions and scope checks for a shared partner.
//!
//! This is a policy module, not an identity provider. The caller supplies the
//! authenticated actor and scope; the policy decides whether the role may act.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    Owner,
    Approver,
    Member,
    Viewer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamAction {
    Read,
    Write,
    Approve,
    Manage,
}

impl TeamRole {
    pub fn allows(self, action: TeamAction) -> bool {
        matches!(
            (self, action),
            (Self::Owner, _)
                | (Self::Approver, TeamAction::Read | TeamAction::Approve)
                | (Self::Member, TeamAction::Read | TeamAction::Write)
                | (Self::Viewer, TeamAction::Read)
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Approver => "approver",
            Self::Member => "member",
            Self::Viewer => "viewer",
        }
    }
}

impl TeamAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Approve => "approve",
            Self::Manage => "manage",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamMember {
    pub member_id: String,
    pub role: TeamRole,
    /// Empty means all scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamPolicy {
    pub team_id: String,
    pub members: Vec<TeamMember>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamVerdict {
    pub decision: TeamDecision,
    pub reason: String,
}

pub fn authorize(policy: &TeamPolicy, actor: &str, action: TeamAction, scope: &str) -> TeamVerdict {
    let Some(member) = policy
        .members
        .iter()
        .find(|member| member.member_id == actor)
    else {
        return verdict(TeamDecision::Deny, "actor is not a team member");
    };
    if !member.role.allows(action) {
        return verdict(
            TeamDecision::Deny,
            &format!(
                "role {} is not allowed to {:?}",
                member.role.as_str(),
                action
            ),
        );
    }
    if !member.scopes.is_empty() && !member.scopes.iter().any(|granted| granted == scope) {
        return verdict(TeamDecision::Deny, "scope is not granted");
    }
    verdict(TeamDecision::Allow, "role and scope allow the action")
}

#[cfg(test)]
pub fn intersect_scopes(requested: &[String], granted: &[String]) -> Vec<String> {
    let mut result = requested
        .iter()
        .filter(|scope| granted.iter().any(|granted| granted == *scope))
        .cloned()
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}

fn verdict(decision: TeamDecision, reason: &str) -> TeamVerdict {
    TeamVerdict {
        decision,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> TeamPolicy {
        TeamPolicy {
            team_id: "team-1".into(),
            members: vec![
                TeamMember {
                    member_id: "owner".into(),
                    role: TeamRole::Owner,
                    scopes: vec![],
                },
                TeamMember {
                    member_id: "approver".into(),
                    role: TeamRole::Approver,
                    scopes: vec!["release".into()],
                },
                TeamMember {
                    member_id: "member".into(),
                    role: TeamRole::Member,
                    scopes: vec!["workspace".into()],
                },
                TeamMember {
                    member_id: "viewer".into(),
                    role: TeamRole::Viewer,
                    scopes: vec![],
                },
            ],
        }
    }

    #[test]
    fn owner_can_do_everything() {
        for action in [
            TeamAction::Read,
            TeamAction::Write,
            TeamAction::Approve,
            TeamAction::Manage,
        ] {
            assert_eq!(
                authorize(&policy(), "owner", action, "release").decision,
                TeamDecision::Allow
            );
        }
    }

    #[test]
    fn roles_are_separated() {
        assert_eq!(
            authorize(&policy(), "member", TeamAction::Write, "workspace").decision,
            TeamDecision::Allow
        );
        assert_eq!(
            authorize(&policy(), "member", TeamAction::Approve, "workspace").decision,
            TeamDecision::Deny
        );
        assert_eq!(
            authorize(&policy(), "approver", TeamAction::Approve, "release").decision,
            TeamDecision::Allow
        );
        assert_eq!(
            authorize(&policy(), "approver", TeamAction::Write, "release").decision,
            TeamDecision::Deny
        );
        assert_eq!(
            authorize(&policy(), "viewer", TeamAction::Write, "workspace").decision,
            TeamDecision::Deny
        );
    }

    #[test]
    fn unknown_actor_and_ungranted_scope_are_denied() {
        assert_eq!(
            authorize(&policy(), "stranger", TeamAction::Read, "workspace").decision,
            TeamDecision::Deny
        );
        assert_eq!(
            authorize(&policy(), "member", TeamAction::Read, "release").decision,
            TeamDecision::Deny
        );
    }

    #[test]
    fn scope_intersection_is_sorted_and_deduplicated() {
        let requested = vec!["b".into(), "a".into(), "a".into()];
        let granted = vec!["a".into(), "c".into()];
        assert_eq!(intersect_scopes(&requested, &granted), vec!["a"]);
    }
}
