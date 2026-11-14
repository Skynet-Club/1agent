//! Runtime-provided tools for the intelligent loop.
//!
//! These tools are not in `ToolRegistry`: their answers depend on the live run
//! (selected provider, permissions, budget). The runtime adds their specs to
//! the model request and handles their calls itself. Everything they report
//! comes from the same capability snapshot that the journal records.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::{PermissionSet, RiskLevel, RunBudget, ToolSpec};
use crate::tools::{ToolCapability, ToolCapabilityStatus};

pub const DESCRIBE_SELF: &str = "describe_self";
pub const REQUEST_TOOLS: &str = "request_tools";

/// Tools that need network access. Used only to reject an explicit
/// `request_tools` call when network permission is off; execution still
/// enforces its own permission checks.
pub const NETWORK_TOOLS: &[&str] = &["fetch_url", "web_search", "browser_read"];

pub fn virtual_specs() -> Vec<ToolSpec> {
    vec![describe_self_spec(), request_tools_spec()]
}

pub fn is_virtual(name: &str) -> bool {
    name == DESCRIBE_SELF || name == REQUEST_TOOLS
}

pub fn virtual_capabilities() -> Vec<ToolCapability> {
    virtual_specs()
        .into_iter()
        .map(|spec| ToolCapability {
            name: spec.name,
            description: spec.description,
            parameters: spec.parameters,
            risk: RiskLevel::Low,
            requires_write: false,
            timeout_ms: 2_000,
            status: ToolCapabilityStatus::Implemented,
        })
        .collect()
}

pub fn describe_self_spec() -> ToolSpec {
    ToolSpec {
        name: DESCRIBE_SELF.into(),
        description: "Return live facts about this agent: registered tools and their parameters, \
                      permissions, risk, the selected provider/model and the run budgets. Use this \
                      for questions about what you can do; never answer from a memorized list."
            .into(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        }),
    }
}

pub fn request_tools_spec() -> ToolSpec {
    ToolSpec {
        name: REQUEST_TOOLS.into(),
        description: "Ask the runtime to add tools that are not currently offered. The runtime \
                      checks permissions and makes the requested schemas available on the next \
                      step. Use this when the task needs a capability you do not currently have."
            .into(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tools": {
                    "type": "array",
                    "items": {"type": "string"},
                    "minItems": 1,
                    "maxItems": 8
                },
                "reason": {"type": "string"}
            },
            "required": ["tools", "reason"]
        }),
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestToolsArgs {
    pub tools: Vec<String>,
    pub reason: String,
}

pub fn parse_request_tools(arguments: &Value) -> Result<RequestToolsArgs, String> {
    let mut args: RequestToolsArgs =
        serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
    args.tools = args
        .tools
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    if args.tools.is_empty() {
        return Err("tools must name at least one tool".into());
    }
    if args.tools.len() > 8 {
        return Err("at most 8 tools may be requested at once".into());
    }
    if args.reason.trim().is_empty() {
        return Err("reason must not be empty".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    args.tools.retain(|name| seen.insert(name.clone()));
    Ok(args)
}

/// Render the live self model as compact JSON. The caller passes the
/// permission-filtered capability list, so a disabled tool never appears.
pub fn render_self_model(
    provider_id: &str,
    model: &str,
    permissions: &PermissionSet,
    budget: &RunBudget,
    tools: &[ToolCapability],
) -> String {
    let mut limitations = Vec::new();
    if !permissions.allow_write {
        limitations.push("write tools are disabled in this run");
    }
    if !permissions.allow_network {
        limitations.push("network tools are disabled in this run");
    }
    if !tools.iter().any(|tool| tool.requires_write) {
        limitations.push("no write tools are available");
    }
    if !tools
        .iter()
        .any(|tool| NETWORK_TOOLS.contains(&tool.name.as_str()))
    {
        limitations.push("no network tools are available");
    }
    limitations.push(if cfg!(target_os = "linux") {
        "officially supported platform: Linux x86_64"
    } else {
        "this platform is not officially supported"
    });
    limitations.push(
        "browser clicking/typing, computer-use, voice input and image understanding are not implemented",
    );
    limitations.push("no external certification or third-party audit");

    let tools = tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
                "risk": tool.risk,
                "requires_write": tool.requires_write,
                "timeout_ms": tool.timeout_ms,
                "status": tool.status,
            })
        })
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&json!({
        "agent": "1-Agent",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "provider": provider_id,
        "model": model,
        "permissions": {
            "allow_write": permissions.allow_write,
            "allow_network": permissions.allow_network,
        },
        "budgets": {
            "max_steps": budget.max_steps,
            "max_tool_calls": budget.max_tool_calls,
            "max_provider_attempts": budget.max_provider_attempts,
            "timeout_ms": budget.timeout_ms,
            "max_context_tokens": budget.max_context_tokens,
            "auto_compact_tokens": budget.auto_compact_tokens,
            "max_cost_micros": budget.max_cost_micros,
            "max_tokens": budget.max_tokens,
        },
        "tools": tools,
        "limitations": limitations,
    }))
    .unwrap_or_else(|error| format!(r#"{{"error":"cannot render self model: {error}"}}"#))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissions(allow_write: bool, allow_network: bool) -> PermissionSet {
        PermissionSet {
            read_roots: Vec::new(),
            allow_write,
            allow_network,
        }
    }

    #[test]
    fn virtual_specs_are_callable_and_bounded() {
        let specs = virtual_specs();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].name, DESCRIBE_SELF);
        assert_eq!(specs[1].name, REQUEST_TOOLS);
        let request = parse_request_tools(&json!({
            "tools": ["web_search", "web_search", "fetch_url"],
            "reason": "need current facts"
        }))
        .unwrap();
        assert_eq!(request.tools, vec!["web_search", "fetch_url"]);
        assert!(parse_request_tools(&json!({"tools": [], "reason": "x"})).is_err());
        assert!(parse_request_tools(&json!({"tools": ["a"], "reason": " "})).is_err());
    }

    #[test]
    fn self_model_reports_only_available_tools_and_live_budgets() {
        let tools = vec![
            ToolCapability {
                name: "read_file".into(),
                description: "read".into(),
                parameters: json!({"type": "object"}),
                risk: RiskLevel::Low,
                requires_write: false,
                timeout_ms: 30_000,
                status: ToolCapabilityStatus::Implemented,
            },
            ToolCapability {
                name: "web_search".into(),
                description: "search".into(),
                parameters: json!({"type": "object"}),
                risk: RiskLevel::Medium,
                requires_write: false,
                timeout_ms: 30_000,
                status: ToolCapabilityStatus::Implemented,
            },
        ];
        let budget = RunBudget {
            max_steps: 7,
            max_tool_calls: 9,
            max_provider_attempts: 3,
            provider_retry_backoff_ms: 0,
            timeout_ms: 1_000,
            max_context_tokens: 2_000,
            auto_compact_tokens: 1_500,
            max_provider_p95_latency_ms: None,
            max_unit_cost_micros_per_k: None,
            max_cost_micros: Some(42),
            max_tokens: Some(420),
        };
        let rendered = render_self_model(
            "deepseek",
            "deepseek-flash",
            &permissions(false, false),
            &budget,
            &tools,
        );
        let value: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(value["provider"], "deepseek");
        assert_eq!(value["model"], "deepseek-flash");
        assert_eq!(value["budgets"]["max_steps"], 7);
        assert_eq!(value["permissions"]["allow_write"], false);
        assert_eq!(value["tools"].as_array().unwrap().len(), 2);
        let limitations = value["limitations"].as_array().unwrap();
        assert!(
            limitations
                .iter()
                .any(|line| line == "write tools are disabled in this run")
        );
        assert!(
            limitations
                .iter()
                .any(|line| line == "network tools are disabled in this run")
        );
    }
}
