//! Agent-facing skill registry tools.
//!
//! The registry and journal already exist; these adapters let the model list,
//! register, evaluate, activate and roll back skills under the same write gate
//! and audit path as the rest of the tool surface.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::skill::{SkillManifest, SkillRisk};
use crate::skill_eval::SkillEvalSuite;
use crate::skill_service::SkillService;

const ENV_STORE: &str = "ONEAGENT_SKILL_JOURNAL";

fn default_store() -> PathBuf {
    std::env::var_os(ENV_STORE)
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::runtime_home::default_home().join("skills.jsonl"))
}

fn open(store: &Path) -> Result<SkillService, ToolError> {
    SkillService::open(store.to_path_buf())
        .map_err(|error| ToolError::Failed(format!("cannot open skill journal: {error}")))
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArguments(format!("{key} is required")))
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

fn manifest(arguments: &Value) -> Result<SkillManifest, ToolError> {
    let risk = match arguments
        .get("risk")
        .and_then(Value::as_str)
        .unwrap_or("low")
    {
        "low" => SkillRisk::Low,
        "medium" => SkillRisk::Medium,
        "high" => SkillRisk::High,
        other => {
            return Err(ToolError::InvalidArguments(format!(
                "unknown risk '{other}'"
            )));
        }
    };
    Ok(SkillManifest {
        name: required_str(arguments, "name")?.into(),
        description: required_str(arguments, "description")?.into(),
        version: required_str(arguments, "version")?.into(),
        platforms: string_list(arguments, "platforms"),
        scope: arguments
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or("global")
            .into(),
        requires_tools: string_list(arguments, "requires_tools"),
        risk,
        body: required_str(arguments, "body")?.into(),
        content_sha256: String::new(),
    })
}

fn suite(arguments: &Value) -> Result<SkillEvalSuite, ToolError> {
    let value = arguments
        .get("suite")
        .cloned()
        .ok_or_else(|| ToolError::InvalidArguments("suite is required".into()))?;
    serde_json::from_value(value)
        .map_err(|error| ToolError::InvalidArguments(format!("invalid suite: {error}")))
}

fn outcome(summary: String, value: Value) -> ToolOutcome {
    ToolOutcome {
        summary,
        content: value.to_string(),
        metadata: Default::default(),
    }
}

pub struct SkillListTool {
    store: PathBuf,
}

impl SkillListTool {
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
impl AgentTool for SkillListTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_list".into(),
            description: "List registered skills with name, version, status and risk.".into(),
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
        let service = open(&self.store)?;
        let skills = service.list();
        Ok(outcome(
            format!("技能列表：{} 个", skills.len()),
            json!({"skills": skills}),
        ))
    }
}

pub struct SkillRegisterTool {
    store: PathBuf,
}

impl SkillRegisterTool {
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
impl AgentTool for SkillRegisterTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_register".into(),
            description: "Register a new immutable skill version. The body is scanned before it enters the registry. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name", "description", "version", "body"],
                "properties": {
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "version": {"type": "string"},
                    "body": {"type": "string"},
                    "platforms": {"type": "array", "items": {"type": "string"}},
                    "scope": {"type": "string"},
                    "requires_tools": {"type": "array", "items": {"type": "string"}},
                    "risk": {"type": "string", "enum": ["low", "medium", "high"]}
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
        let mut service = open(&self.store)?;
        let manifest = manifest(&arguments)?;
        let name = manifest.name.clone();
        let version = manifest.version.clone();
        service
            .register(manifest, crate::current_time_ms())
            .map_err(|error| ToolError::Failed(format!("cannot register skill: {error}")))?;
        Ok(outcome(
            format!("已注册技能 {name} {version}"),
            json!({"name": name, "version": version}),
        ))
    }
}

pub struct SkillEvalTool {
    store: PathBuf,
}

impl SkillEvalTool {
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
impl AgentTool for SkillEvalTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_eval".into(),
            description: "Evaluate a registered skill version against a static held-out suite and return the score and per-case results.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name", "version", "suite"],
                "properties": {
                    "name": {"type": "string"},
                    "version": {"type": "string"},
                    "suite": {"type": "object"}
                }
            }),
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
        arguments: Value,
        _permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        let service = open(&self.store)?;
        let name = required_str(&arguments, "name")?;
        let version = required_str(&arguments, "version")?;
        let report = service
            .evaluate(name, version, &suite(&arguments)?)
            .map_err(|error| ToolError::Failed(format!("skill evaluation failed: {error}")))?;
        Ok(outcome(
            format!(
                "技能 {name} {version} 评测：{}（{} bps）",
                if report.passed { "通过" } else { "未通过" },
                report.score_bps
            ),
            serde_json::to_value(report).map_err(|error| ToolError::Failed(error.to_string()))?,
        ))
    }
}

pub struct SkillActivateTool {
    store: PathBuf,
}

impl SkillActivateTool {
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
impl AgentTool for SkillActivateTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_activate".into(),
            description: "Activate a registered skill version. When a suite is supplied the held-out gate must pass first. Requires write permission.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name", "version"],
                "properties": {
                    "name": {"type": "string"},
                    "version": {"type": "string"},
                    "suite": {"type": "object"}
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
        let mut service = open(&self.store)?;
        let name = required_str(&arguments, "name")?;
        let version = required_str(&arguments, "version")?;
        if arguments.get("suite").is_some() {
            service
                .activate_if_passed(name, version, &suite(&arguments)?, crate::current_time_ms())
                .map_err(|error| ToolError::Failed(format!("skill activation failed: {error}")))?;
        } else {
            service
                .activate(name, version, crate::current_time_ms())
                .map_err(|error| ToolError::Failed(format!("skill activation failed: {error}")))?;
        }
        Ok(outcome(
            format!("已激活技能 {name} {version}"),
            json!({"name": name, "version": version}),
        ))
    }
}

pub struct SkillRollbackTool {
    store: PathBuf,
}

impl SkillRollbackTool {
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
impl AgentTool for SkillRollbackTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "skill_rollback".into(),
            description:
                "Roll a skill back to a previous registered version. Requires write permission."
                    .into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["name", "version"],
                "properties": {
                    "name": {"type": "string"},
                    "version": {"type": "string"}
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
        let mut service = open(&self.store)?;
        let name = required_str(&arguments, "name")?;
        let version = required_str(&arguments, "version")?;
        service
            .rollback(name, version, crate::current_time_ms())
            .map_err(|error| ToolError::Failed(format!("skill rollback failed: {error}")))?;
        Ok(outcome(
            format!("已回滚技能 {name} 到 {version}"),
            json!({"name": name, "version": version}),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::domain::{ToolCall, ToolStatus};
    use crate::tools::ToolRegistry;

    fn manifest_json(version: &str) -> Value {
        json!({
            "name": "release",
            "description": "release process",
            "version": version,
            "body": "1. verify\n2. build",
            "platforms": ["linux"],
            "scope": "repo",
            "requires_tools": ["shell"],
            "risk": "low"
        })
    }

    fn suite_json() -> Value {
        json!({
            "suite_id": "release-suite",
            "min_score_bps": 10000,
            "cases": [{
                "case_id": "verify",
                "required_markers": ["verify"],
                "forbidden_markers": ["rm -rf"]
            }]
        })
    }

    #[tokio::test]
    async fn registers_evaluates_activates_and_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("skills.jsonl");
        let permissions = PermissionSet {
            allow_write: true,
            ..PermissionSet::default()
        };
        let cancellation = CancellationToken::new();
        SkillRegisterTool::new(store.clone())
            .execute(manifest_json("1.0.0"), &permissions, &cancellation)
            .await
            .unwrap();
        SkillRegisterTool::new(store.clone())
            .execute(manifest_json("1.1.0"), &permissions, &cancellation)
            .await
            .unwrap();
        let list = SkillListTool::new(store.clone())
            .execute(json!({}), &permissions, &cancellation)
            .await
            .unwrap();
        assert!(list.content.contains("1.1.0"));

        let eval = SkillEvalTool::new(store.clone())
            .execute(
                json!({"name": "release", "version": "1.1.0", "suite": suite_json()}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        assert!(eval.content.contains("\"passed\":true"));

        SkillActivateTool::new(store.clone())
            .execute(
                json!({"name": "release", "version": "1.1.0", "suite": suite_json()}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        SkillRollbackTool::new(store.clone())
            .execute(
                json!({"name": "release", "version": "1.0.0"}),
                &permissions,
                &cancellation,
            )
            .await
            .unwrap();
        let service = SkillService::open(store.clone()).unwrap();
        assert_eq!(service.active("release").unwrap().version, "1.0.0");
    }

    #[tokio::test]
    async fn activation_is_write_gated() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("skills.jsonl");
        let mut registry = ToolRegistry::default();
        registry.register(Arc::new(SkillActivateTool::new(store)));
        let observation = registry
            .execute(
                &ToolCall {
                    id: "skill-1".into(),
                    name: "skill_activate".into(),
                    arguments: json!({"name": "release", "version": "1.0.0"}),
                },
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await;
        assert_eq!(observation.status, ToolStatus::PermissionDenied);
    }
}
