//! Versioned ActionType registry for the first operational vertical.
//!
//! The registry is data, not runtime wiring: it defines typed actions that can
//! later generate tool specs, capability manifests and MCP tools. The journal
//! and the existing ActionCard/ExecutionGate remain the execution facts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability_compiler::{CapabilityDescriptor, CapabilityManifest};
use crate::domain::ToolSpec;

use super::OntologyError;

pub const ACTION_REGISTRY_SCHEMA_VERSION: u64 = 1;
const RISKS: &[&str] = &["low", "medium", "high"];
const APPROVALS: &[&str] = &["none", "human", "authority"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionRegistryFile {
    pub schema_version: u64,
    pub registry_version: String,
    pub vertical: String,
    pub actions: Vec<ActionDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionDefinition {
    pub id: String,
    pub version: String,
    pub title: String,
    pub description: String,
    pub object_types: Vec<String>,
    pub capability: ActionCapability,
    pub parameters: Value,
    pub preconditions: Vec<String>,
    pub risk: String,
    pub reversible: bool,
    pub approval: String,
    pub side_effects: Vec<String>,
    pub postconditions: Vec<String>,
    pub idempotency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionCapability {
    pub verb: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub constraints: Vec<String>,
}

pub fn try_builtin_action_registry() -> Result<ActionRegistryFile, OntologyError> {
    parse_action_registry(super::read_builtin_actions())
}

pub fn parse_action_registry(json: &str) -> Result<ActionRegistryFile, OntologyError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| OntologyError::Json(format!("action registry: {error}")))?;
    validate_action_registry_value(&value)?;
    serde_json::from_value(value)
        .map_err(|error| OntologyError::Json(format!("action registry: {error}")))
}

pub fn builtin_actions() -> Value {
    try_builtin_action_registry()
        .ok()
        .and_then(|registry| serde_json::to_value(registry).ok())
        .unwrap_or(Value::Null)
}

pub fn action_registry_sha256() -> String {
    super::sha256_hex(super::read_builtin_actions().as_bytes())
}

pub fn validate_action_registry_value(value: &Value) -> Result<(), OntologyError> {
    let root = value
        .as_object()
        .ok_or_else(|| invalid("root must be a JSON object"))?;
    if root.get("schema_version").and_then(Value::as_u64) != Some(ACTION_REGISTRY_SCHEMA_VERSION) {
        return Err(invalid("schema_version must be 1"));
    }
    for key in ["registry_version", "vertical"] {
        if root
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(invalid(&format!("{key} must be a non-empty string")));
        }
    }
    let actions = root
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("actions must be an array"))?;
    if actions.is_empty() {
        return Err(invalid("actions must not be empty"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, action) in actions.iter().enumerate() {
        let context = format!("actions[{index}]");
        let action = action
            .as_object()
            .ok_or_else(|| invalid(&format!("{context} must be an object")))?;
        for key in [
            "id",
            "version",
            "title",
            "description",
            "risk",
            "approval",
            "idempotency",
        ] {
            if action
                .get(key)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(invalid(&format!("{context}.{key} must be non-empty")));
            }
        }
        let id = action["id"].as_str().unwrap_or_default();
        if !seen.insert(id.to_string()) {
            return Err(invalid(&format!("duplicate action id `{id}`")));
        }
        if !RISKS.contains(&action["risk"].as_str().unwrap_or_default()) {
            return Err(invalid(&format!(
                "{context}.risk must be low, medium or high"
            )));
        }
        if !APPROVALS.contains(&action["approval"].as_str().unwrap_or_default()) {
            return Err(invalid(&format!(
                "{context}.approval must be none, human or authority"
            )));
        }
        if !action.get("reversible").is_some_and(Value::is_boolean) {
            return Err(invalid(&format!("{context}.reversible must be a boolean")));
        }
        for key in [
            "object_types",
            "preconditions",
            "side_effects",
            "postconditions",
        ] {
            let values = action
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| invalid(&format!("{context}.{key} must be an array")))?;
            if values.is_empty()
                || values
                    .iter()
                    .any(|value| value.as_str().is_none_or(|value| value.trim().is_empty()))
            {
                return Err(invalid(&format!(
                    "{context}.{key} must contain non-empty strings"
                )));
            }
        }
        let parameters = action
            .get("parameters")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid(&format!("{context}.parameters must be an object")))?;
        if parameters.get("type").and_then(Value::as_str) != Some("object") {
            return Err(invalid(&format!(
                "{context}.parameters.type must be object"
            )));
        }
        let capability = action
            .get("capability")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid(&format!("{context}.capability must be an object")))?;
        if capability
            .get("verb")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(invalid(&format!(
                "{context}.capability.verb must be non-empty"
            )));
        }
    }
    Ok(())
}

impl ActionDefinition {
    pub fn capability_manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: format!("action.{}", self.id),
            name: self.id.clone(),
            kind: "action".into(),
            source: "1ao.action-registry".into(),
            installed: false,
            trust: "core".into(),
            license: "internal".into(),
            capabilities: vec![CapabilityDescriptor {
                verb: self.capability.verb.clone(),
                inputs: self.capability.inputs.clone(),
                outputs: self.capability.outputs.clone(),
                constraints: self.capability.constraints.clone(),
            }],
            verification: vec!["preconditions".into(), "postconditions".into()],
            install: None,
        }
    }

    pub fn tool_spec(&self) -> ToolSpec {
        let mut description = format!("{}: {}", self.title, self.description);
        if self.approval != "none" {
            description.push_str(&format!(" Requires {} approval.", self.approval));
        }
        ToolSpec {
            name: self.id.replace('.', "_"),
            description,
            parameters: self.parameters.clone(),
        }
    }
}

fn invalid(message: &str) -> OntologyError {
    OntologyError::Validation(format!("action registry: {message}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_parses_and_has_the_code_release_vertical() {
        let registry = try_builtin_action_registry().unwrap();
        assert_eq!(registry.schema_version, 1);
        assert_eq!(registry.vertical, "code-change-release");
        assert_eq!(registry.actions.len(), 5);
        let ids = registry
            .actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(ids.len(), 5);
        for id in [
            "code.propose_change",
            "code.run_checks",
            "code.approve_change",
            "code.prepare_release",
            "code.merge_change",
        ] {
            assert!(ids.contains(id), "missing {id}");
        }
        assert_eq!(action_registry_sha256().len(), 64);
    }

    #[test]
    fn action_generates_tool_and_capability_manifest() {
        let registry = try_builtin_action_registry().unwrap();
        let action = &registry.actions[0];
        let tool = action.tool_spec();
        assert_eq!(tool.name, "code_propose_change");
        assert!(tool.description.contains("Propose change"));
        let manifest = action.capability_manifest();
        assert_eq!(manifest.id, "action.code.propose_change");
        assert_eq!(manifest.kind, "action");
        assert_eq!(manifest.capabilities[0].verb, "propose");
        assert_eq!(manifest.capabilities[0].outputs, ["change_proposal"]);
    }

    #[test]
    fn invalid_registry_fails_closed() {
        let duplicate = r#"{
            "schema_version": 1,
            "registry_version": "1",
            "vertical": "test",
            "actions": [
                {"id":"a","version":"1","title":"A","description":"a","object_types":["X"],"capability":{"verb":"do","inputs":[],"outputs":[],"constraints":[]},"parameters":{"type":"object"},"preconditions":["p"],"risk":"low","reversible":true,"approval":"none","side_effects":["s"],"postconditions":["q"],"idempotency":"k"},
                {"id":"a","version":"1","title":"A","description":"a","object_types":["X"],"capability":{"verb":"do","inputs":[],"outputs":[],"constraints":[]},"parameters":{"type":"object"},"preconditions":["p"],"risk":"low","reversible":true,"approval":"none","side_effects":["s"],"postconditions":["q"],"idempotency":"k"}
            ]
        }"#;
        assert!(parse_action_registry(duplicate).is_err());

        let bad_risk = duplicate.replace("\"risk\":\"low\"", "\"risk\":\"extreme\"");
        assert!(parse_action_registry(&bad_risk).is_err());
    }
}
