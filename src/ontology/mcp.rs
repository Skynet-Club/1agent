//! Ontology MCP surface.
//!
//! Read tools are always available. Typed write tools require an explicit
//! `--allow-write` scope and a workspace; `code.approve_change` is never
//! exposed, so an MCP client cannot approve its own proposal. Stdio JSON-RPC
//! only; authentication is the local launching user.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{builtin_actions, builtin_types, export_journal};
use crate::domain::PermissionSet;
use crate::tools::{
    AgentTool, CodeMergeChangeTool, CodePrepareReleaseTool, CodeProposeChangeTool,
    CodeRunChecksTool,
};

#[derive(Clone, Debug)]
pub struct McpOptions {
    pub journal_dir: PathBuf,
    pub workspace: Option<PathBuf>,
    pub allow_write: bool,
}

const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_NODE_BYTES: usize = 256 * 1024;

pub fn serve_stdio(options: &McpOptions) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| format!("cannot read MCP input: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > MAX_LINE_BYTES {
            let response = error_response(Some(Value::Null), -32600, "request is too large");
            writeln!(stdout, "{response}").map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                let response =
                    error_response(Some(Value::Null), -32700, &format!("parse error: {error}"));
                writeln!(stdout, "{response}").map_err(|error| error.to_string())?;
                stdout.flush().map_err(|error| error.to_string())?;
                continue;
            }
        };
        if let Some(response) = handle_request(&request, options) {
            writeln!(stdout, "{response}").map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub fn handle_request(request: &Value, options: &McpOptions) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    if method.starts_with("notifications/") {
        return None;
    }
    let response = match method {
        "initialize" => result_response(
            id.clone(),
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {
                    "name": "1agent-ontology",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        ),
        "ping" => result_response(id.clone(), json!({})),
        "tools/list" => result_response(id.clone(), json!({"tools": tools(options)})),
        "tools/call" => match call_tool(request.get("params"), options) {
            Ok(value) => result_response(
                id.clone(),
                json!({
                    "content": [{"type": "text", "text": value.to_string()}],
                    "isError": false
                }),
            ),
            Err(message) => result_response(
                id.clone(),
                json!({
                    "content": [{"type": "text", "text": message}],
                    "isError": true
                }),
            ),
        },
        _ => error_response(id.clone(), -32601, "method not found"),
    };
    if id.is_none() { None } else { Some(response) }
}

fn tools(options: &McpOptions) -> Value {
    let mut tools = vec![
        json!({
            "name": "ontology_types",
            "description": "List the built-in 1AO type registry.",
            "inputSchema": {"type": "object", "additionalProperties": false}
        }),
        json!({
            "name": "action_list",
            "description": "List the read-only ActionType registry and its approval policy.",
            "inputSchema": {"type": "object", "additionalProperties": false}
        }),
        json!({
            "name": "ontology_read",
            "description": "Read one object by IRI from a journal export in the configured journal directory.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["iri"],
                "properties": {
                    "iri": {"type": "string"},
                    "journal": {"type": "string", "description": "Optional journal file name inside the journal directory."}
                }
            }
        }),
        json!({
            "name": "ontology_search",
            "description": "List objects of one @type from a journal export.",
            "inputSchema": {
                "type": "object",
                "additionalProperties": false,
                "required": ["type"],
                "properties": {
                    "type": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                    "journal": {"type": "string"}
                }
            }
        }),
    ];
    if options.allow_write
        && options.workspace.is_some()
        && let Ok(registry) = crate::ontology::try_builtin_action_registry()
    {
        for action in registry.actions {
            if action.id == "code.approve_change" {
                continue;
            }
            let spec = action.tool_spec();
            tools.push(json!({
                "name": spec.name,
                "description": format!(
                    "{} Human approval is CLI-only and is never exposed as an MCP tool.",
                    spec.description
                ),
                "inputSchema": spec.parameters,
            }));
        }
    }
    Value::Array(tools)
}

fn call_tool(params: Option<&Value>, options: &McpOptions) -> Result<Value, String> {
    let params = params.ok_or_else(|| "missing tool parameters".to_string())?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing tool name".to_string())?;
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    match name {
        "ontology_types" => Ok(builtin_types()),
        "action_list" => Ok(builtin_actions()),
        "ontology_read" => {
            let iri = arguments
                .get("iri")
                .and_then(Value::as_str)
                .ok_or_else(|| "ontology_read requires iri".to_string())?;
            let journal = journal_path(&options.journal_dir, arguments.get("journal"))?;
            let document = read_document(&journal)?;
            let node = document
                .get("@graph")
                .and_then(Value::as_array)
                .and_then(|nodes| nodes.iter().find(|node| node["@id"] == iri))
                .ok_or_else(|| format!("object {iri} not found"))?;
            let text = serde_json::to_string_pretty(node).map_err(|error| error.to_string())?;
            if text.len() > MAX_NODE_BYTES {
                return Err("object exceeds the read limit".into());
            }
            Ok(node.clone())
        }
        "ontology_search" => {
            let object_type = arguments
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| "ontology_search requires type".to_string())?;
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(20)
                .clamp(1, 100) as usize;
            let journal = journal_path(&options.journal_dir, arguments.get("journal"))?;
            let document = read_document(&journal)?;
            let nodes = document
                .get("@graph")
                .and_then(Value::as_array)
                .map(|nodes| {
                    nodes
                        .iter()
                        .filter(|node| node["@type"] == object_type)
                        .take(limit)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(json!({"type": object_type, "count": nodes.len(), "objects": nodes}))
        }
        "code_propose_change"
        | "code_run_checks"
        | "code_prepare_release"
        | "code_merge_change" => {
            let workspace = options
                .workspace
                .as_ref()
                .ok_or_else(|| "write scope requires --workspace".to_string())?;
            if !options.allow_write {
                return Err("write scope is not enabled; pass --allow-write".into());
            }
            let permissions = PermissionSet {
                read_roots: vec![workspace.clone()],
                allow_network: false,
                allow_write: true,
            };
            let outcome = match name {
                "code_propose_change" => run_tool(
                    &CodeProposeChangeTool::new(action_definition("code.propose_change")?),
                    arguments,
                    &permissions,
                )?,
                "code_run_checks" => run_tool(
                    &CodeRunChecksTool::new(action_definition("code.run_checks")?),
                    arguments,
                    &permissions,
                )?,
                "code_prepare_release" => run_tool(
                    &CodePrepareReleaseTool::new(action_definition("code.prepare_release")?),
                    arguments,
                    &permissions,
                )?,
                "code_merge_change" => run_tool(
                    &CodeMergeChangeTool::new(action_definition("code.merge_change")?),
                    arguments,
                    &permissions,
                )?,
                _ => unreachable!(),
            };
            Ok(json!({
                "summary": outcome.summary,
                "content": outcome.content,
                "metadata": outcome.metadata,
            }))
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

fn action_definition(id: &str) -> Result<crate::ontology::ActionDefinition, String> {
    crate::ontology::try_builtin_action_registry()
        .map_err(|error| error.to_string())?
        .actions
        .into_iter()
        .find(|action| action.id == id)
        .ok_or_else(|| format!("unknown action {id}"))
}

fn run_tool<T: AgentTool>(
    tool: &T,
    arguments: Value,
    permissions: &PermissionSet,
) -> Result<crate::tools::ToolOutcome, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start action runtime: {error}"))?;
    runtime
        .block_on(tool.execute(
            arguments,
            permissions,
            &tokio_util::sync::CancellationToken::new(),
        ))
        .map_err(|error| error.to_string())
}

fn journal_path(journal_dir: &Path, value: Option<&Value>) -> Result<PathBuf, String> {
    let canonical_dir = std::fs::canonicalize(journal_dir)
        .map_err(|error| format!("cannot open journal dir: {error}"))?;
    let path = match value.and_then(Value::as_str) {
        Some(name) => {
            let name = Path::new(name);
            if name.is_absolute()
                || name.components().any(|component| {
                    matches!(
                        component,
                        std::path::Component::ParentDir | std::path::Component::RootDir
                    )
                })
            {
                return Err("journal name must stay inside the journal directory".into());
            }
            canonical_dir.join(name)
        }
        None => latest_journal(&canonical_dir)?,
    };
    let canonical =
        std::fs::canonicalize(&path).map_err(|error| format!("cannot open journal: {error}"))?;
    if !canonical.starts_with(&canonical_dir) || !canonical.is_file() {
        return Err("journal is outside the journal directory".into());
    }
    let size = std::fs::metadata(&canonical)
        .map_err(|error| format!("cannot stat journal: {error}"))?
        .len();
    if size > MAX_JOURNAL_BYTES {
        return Err("journal is too large".into());
    }
    Ok(canonical)
}

fn latest_journal(dir: &Path) -> Result<PathBuf, String> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).map_err(|error| format!("cannot list journals: {error}"))? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
            newest = Some((modified, path));
        }
    }
    newest
        .map(|(_, path)| path)
        .ok_or_else(|| "no journal found".to_string())
}

fn read_document(path: &Path) -> Result<Value, String> {
    export_journal(path)
        .map(|document| document.value)
        .map_err(|error| error.to_string())
}

fn result_response(id: Option<Value>, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id.unwrap_or(Value::Null), "result": result})
}

fn error_response(id: Option<Value>, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {"code": code, "message": message}
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(dir: &Path) -> McpOptions {
        McpOptions {
            journal_dir: dir.to_path_buf(),
            workspace: None,
            allow_write: false,
        }
    }

    #[test]
    fn initialize_and_tool_list_are_read_only() {
        let dir = tempfile::tempdir().unwrap();
        let init = handle_request(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}),
            &options(dir.path()),
        )
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "1agent-ontology");
        let tools = handle_request(
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            &options(dir.path()),
        )
        .unwrap();
        let names = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "ontology_types",
                "action_list",
                "ontology_read",
                "ontology_search"
            ]
        );
        assert!(!names.iter().any(|name| name.contains("write")));
    }

    #[test]
    fn action_list_and_missing_journal_are_handled() {
        let dir = tempfile::tempdir().unwrap();
        let actions = handle_request(
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {"name": "action_list", "arguments": {}}
            }),
            &options(dir.path()),
        )
        .unwrap();
        assert_eq!(actions["result"]["isError"], false);
        assert!(
            actions["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("code.merge_change")
        );
        let missing = handle_request(
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "ontology_read",
                    "arguments": {"iri": "urn:1agent:run:nope"}
                }
            }),
            &options(dir.path()),
        )
        .unwrap();
        assert_eq!(missing["result"]["isError"], true);
    }

    #[test]
    fn typed_write_tools_require_scope_and_never_expose_approval() {
        let journal = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let options = McpOptions {
            journal_dir: journal.path().to_path_buf(),
            workspace: Some(workspace.path().to_path_buf()),
            allow_write: true,
        };
        let tools = handle_request(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
            &options,
        )
        .unwrap();
        let names = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(names.contains(&"code_propose_change"));
        assert!(names.contains(&"code_merge_change"));
        assert!(!names.contains(&"code_approve_change"));

        let proposed = handle_request(
            &json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "code_propose_change",
                    "arguments": {"summary": "mcp proposal", "paths": ["src/lib.rs"]}
                }
            }),
            &options,
        )
        .unwrap();
        assert_eq!(proposed["result"]["isError"], false);
        assert!(
            proposed["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("proposal_id")
        );

        let denied = handle_request(
            &json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "code_propose_change",
                    "arguments": {"summary": "no scope", "paths": ["src/lib.rs"]}
                }
            }),
            &McpOptions {
                journal_dir: journal.path().to_path_buf(),
                workspace: Some(workspace.path().to_path_buf()),
                allow_write: false,
            },
        )
        .unwrap();
        assert_eq!(denied["result"]["isError"], true);
    }

    #[test]
    fn path_traversal_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let response = handle_request(
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "ontology_read",
                    "arguments": {"iri": "x", "journal": "../outside.jsonl"}
                }
            }),
            &options(dir.path()),
        )
        .unwrap();
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("inside the journal directory")
        );
    }
}
