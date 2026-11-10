//! Agent-facing delegation profile tools.
//!
//! Profiles select a working mode for one actor/domain; they never grant
//! authority by themselves. Register/revoke are write-gated and persisted
//! through the existing `DelegationStore` snapshot locks.

use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::delegation::{DelegationMode, DelegationProfile, DelegationStore};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

const ENV_STORE: &str = "ONEAGENT_DELEGATION_STORE";

fn default_store() -> PathBuf {
    std::env::var_os(ENV_STORE)
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::runtime_home::default_home().join("delegation.json"))
}

fn open(store: &std::path::Path) -> Result<DelegationStore, ToolError> {
    DelegationStore::open(store.to_path_buf())
        .map_err(|error| ToolError::Failed(format!("cannot open delegation store: {error}")))
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("{key} is required")))
}

fn parse_risk(value: &str) -> Result<RiskLevel, ToolError> {
    match value {
        "low" => Ok(RiskLevel::Low),
        "medium" => Ok(RiskLevel::Medium),
        "high" => Ok(RiskLevel::High),
        other => Err(ToolError::InvalidArguments(format!(
            "unknown max_risk '{other}'"
        ))),
    }
}

fn string_list(arguments: &Value, key: &str) -> Vec<String> {
    arguments
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn outcome(summary: String, value: Value) -> ToolOutcome {
    ToolOutcome {
        summary,
        content: value.to_string(),
        metadata: Default::default(),
    }
}

pub struct DelegationListTool {
    store: PathBuf,
}

impl DelegationListTool {
    pub fn new(store: impl Into<PathBuf>) -> Self {
        Self {
            store: store.into(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(default_store())
    }
}

#[async_trait]
impl AgentTool for DelegationListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "delegation_list".into(),
            description: "List delegation profiles and revocations.".into(),
            parameters: json!({"type": "object", "additionalProperties": false, "properties": {}}),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }

    async fn execute(
        &self,
        _arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let store = open(&self.store)?;
        let snapshot = store.registry().snapshot();
        Ok(outcome(
            format!(
                "委托配置：{} 个 profile，{} 个撤销",
                snapshot.profiles.len(),
                snapshot.revoked.len()
            ),
            serde_json::to_value(snapshot).map_err(|error| ToolError::Failed(error.to_string()))?,
        ))
    }
}

pub struct DelegationRegisterTool {
    store: PathBuf,
}

impl DelegationRegisterTool {
    pub fn new(store: impl Into<PathBuf>) -> Self {
        Self {
            store: store.into(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(default_store())
    }
}

#[async_trait]
impl AgentTool for DelegationRegisterTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "delegation_register".into(),
            description: "Register a delegation profile for one actor/domain. The mode only selects how work is proposed; Authority/Policy are separate. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["profile_id", "actor_id", "domain", "mode", "max_risk"],
                "properties": {
                    "profile_id": {"type": "string"},
                    "actor_id": {"type": "string"},
                    "domain": {"type": "string"},
                    "mode": {"type": "string", "enum": ["observe", "suggest", "draft", "ask", "act_and_report"]},
                    "max_risk": {"type": "string", "enum": ["low", "medium", "high"]},
                    "allowed_tools": {"type": "array", "items": {"type": "string"}},
                    "expires_ms": {"type": "integer", "minimum": 0}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let mut store = open(&self.store)?;
        let mode = DelegationMode::from_str(required_str(&arguments, "mode")?)
            .map_err(|error| ToolError::InvalidArguments(format!("invalid mode: {error}")))?;
        let profile = DelegationProfile {
            profile_id: required_str(&arguments, "profile_id")?.into(),
            actor_id: required_str(&arguments, "actor_id")?.into(),
            domain: required_str(&arguments, "domain")?.into(),
            mode,
            max_risk: parse_risk(required_str(&arguments, "max_risk")?)?,
            allowed_tools: string_list(&arguments, "allowed_tools"),
            expires_ms: arguments
                .get("expires_ms")
                .and_then(Value::as_u64)
                .map(u128::from),
        };
        let profile_id = profile.profile_id.clone();
        store
            .with_registry_mut(|registry| registry.register(profile))
            .map_err(|error| ToolError::Failed(format!("cannot register delegation: {error}")))?;
        Ok(outcome(
            format!("已注册委托 {profile_id}"),
            json!({"profile_id": profile_id}),
        ))
    }
}

pub struct DelegationRevokeTool {
    store: PathBuf,
}

impl DelegationRevokeTool {
    pub fn new(store: impl Into<PathBuf>) -> Self {
        Self {
            store: store.into(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(default_store())
    }
}

#[async_trait]
impl AgentTool for DelegationRevokeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "delegation_revoke".into(),
            description:
                "Revoke a delegation profile for one actor/domain. Requires write permission."
                    .into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["actor_id", "domain"],
                "properties": {
                    "actor_id": {"type": "string"},
                    "domain": {"type": "string"}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }

    async fn execute(
        &self,
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let mut store = open(&self.store)?;
        let actor_id = required_str(&arguments, "actor_id")?;
        let domain = required_str(&arguments, "domain")?;
        store
            .with_registry_mut(|registry| {
                registry.revoke(actor_id, domain, crate::current_time_ms())
            })
            .map_err(|error| ToolError::Failed(format!("cannot revoke delegation: {error}")))?;
        Ok(outcome(
            format!("已撤销委托 {actor_id}/{domain}"),
            json!({"actor_id": actor_id, "domain": domain, "revoked": true}),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::domain::{ToolCall, ToolStatus};
    use crate::tools::ToolRegistry;

    fn profile_json() -> Value {
        json!({
            "profile_id": "profile-1",
            "actor_id": "owner",
            "domain": "repo",
            "mode": "suggest",
            "max_risk": "low",
            "allowed_tools": ["read_file"]
        })
    }

    #[tokio::test]
    async fn registers_lists_and_revokes_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("delegation.json");
        let permissions = PermissionSet {
            allow_write: true,
            ..PermissionSet::default()
        };
        let cancellation = CancellationToken::new();
        DelegationRegisterTool::new(store.clone())
            .execute(profile_json(), &permissions, &cancellation)
            .await
            .unwrap();
        let list = DelegationListTool::new(store.clone())
            .execute(json!({}), &permissions, &cancellation)
            .await
            .unwrap();
        assert!(list.content.contains("profile-1"));
        assert!(list.content.contains("suggest"));

        DelegationRevokeTool::new(store.clone())
            .execute(
                json!({"actor_id": "owner", "domain": "repo"}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        let list = DelegationListTool::new(store)
            .execute(json!({}), &permissions, &cancellation)
            .await
            .unwrap();
        assert!(list.content.contains("\"revoked\""));
    }

    #[tokio::test]
    async fn register_is_write_gated() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("delegation.json");
        let mut registry = ToolRegistry::default();
        registry.register(Arc::new(DelegationRegisterTool::new(store)));
        let observation = registry
            .execute(
                &ToolCall {
                    id: "delegation-1".into(),
                    name: "delegation_register".into(),
                    arguments: profile_json(),
                },
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await;
        assert_eq!(observation.status, ToolStatus::PermissionDenied);
    }
}
