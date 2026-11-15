//! Independent black-box/CLI QA for the 0.98.2 read-only ontology projection.
//!
//! The tests build synthetic journals/sessions in temporary directories and
//! drive the real `first-principles-agent` binary. Product code is never
//! patched from this file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use first_principles_agent::domain::{
    AgentAnswer, AnswerStatus, Claim, ClaimStatus, CognitiveMode, Evidence, FlipCondition,
    ImpactLevel, ModelResponse, ModelTurn, PermissionSet, RuntimeCheckpoint, StopReason,
    TaskRequest, ToolCall, ToolObservation, ToolStatus, UnknownItem, Usage,
};
use first_principles_agent::journal::{Journal, JournalEventKind};
use first_principles_agent::ontology::sha256_hex;
use first_principles_agent::tools::{AgentTool, EditFileTool, WriteFileTool};
use first_principles_agent::tui_session::TuiSession;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const SECRET_MARKER: &str = "SYNTHETIC_SECRET_MARKER_4f1c9a_DO_NOT_EXPORT";
const RAW_OBJECTIVE: &str =
    "RAW_PROMPT_OBJECTIVE_f1c9a_DO_NOT_EXPORT investigate the internal thing";
const RAW_DELIVERABLE: &str = "RAW_DELIVERABLE_7d2e_DO_NOT_EXPORT";
const RAW_MISSION: &str = "RAW_MISSION_GOAL_3a8b_DO_NOT_EXPORT ship the thing";
const RAW_TOOL_CONTENT: &str = "SENSITIVE_RAW_TOOL_CONTENT_9b3d_DO_NOT_EXPORT page body";
const RAW_TOOL_ARGUMENT: &str = "SENSITIVE_TOOL_ARGUMENT_5c2e_DO_NOT_EXPORT";
const PROMPT_DERIVED_TITLE: &str = "PROMPT_PREFIX_TITLE_2d7c_DO_NOT_EXPORT";
const ARTIFACT_SHA: &str = "abababababababababababababababababababababababababababababababab";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_first-principles-agent")
}

fn command(dir: &Path) -> Command {
    let mut command = Command::new(bin());
    command.current_dir(dir);
    command.env_remove("ONEAGENT_HOME");
    command
}

fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {args:?}: {error}"))
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context}: status={:?}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_no_panic(output: &Output, context: &str) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !text.contains("panicked at") && !text.contains("RUST_BACKTRACE"),
        "{context}: process exposed a panic: {text}"
    );
}

fn parse_json_stdout(output: &Output, context: &str) -> Value {
    assert_success(output, context);
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context}: stdout is not JSON: {error}"))
}

fn parse_compact_document(output: &Output, context: &str) -> Value {
    assert_success(output, context);
    let text = String::from_utf8(output.stdout.clone()).expect("stdout must be utf-8");
    let trimmed = text.trim_end_matches(['\n', '\r']);
    assert!(
        !trimmed.contains('\n'),
        "{context}: expected compact JSON-LD, got multi-line output"
    );
    let document: Value = serde_json::from_str(trimmed)
        .unwrap_or_else(|error| panic!("{context}: invalid JSON-LD: {error}"));
    assert_eq!(
        serde_json::to_string(&document).unwrap(),
        trimmed,
        "{context}: JSON-LD must be serialized compactly without reordering"
    );
    document
}

fn graph(document: &Value) -> &Vec<Value> {
    document["@graph"]
        .as_array()
        .unwrap_or_else(|| panic!("missing @graph: {document}"))
}

fn nodes_of_type<'a>(document: &'a Value, type_name: &str) -> Vec<&'a Value> {
    graph(document)
        .iter()
        .filter(|node| node["@type"] == type_name)
        .collect()
}

fn node_with_suffix<'a>(document: &'a Value, suffix: &str) -> &'a Value {
    graph(document)
        .iter()
        .find(|node| node["@id"].as_str().is_some_and(|id| id.ends_with(suffix)))
        .unwrap_or_else(|| panic!("missing node ending with {suffix}: {document}"))
}

fn has_node_type(document: &Value, type_name: &str) -> bool {
    graph(document)
        .iter()
        .any(|node| node["@type"] == type_name)
}

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

fn base_request(dir: &Path, objective: &str) -> TaskRequest {
    let mut request = TaskRequest::from_objective(objective, dir.to_path_buf());
    request.deliverable = Some(RAW_DELIVERABLE.to_string());
    request.permissions = PermissionSet {
        read_roots: vec![dir.to_path_buf()],
        allow_network: true,
        allow_write: true,
    };
    request
}

fn checkpoint(mode: CognitiveMode) -> RuntimeCheckpoint {
    RuntimeCheckpoint {
        mode,
        messages: Vec::new(),
        tool_calls_used: 1,
        provider_attempts_used: 1,
        unavailable_routes: Vec::new(),
        next_step: 2,
        elapsed_ms: 17,
        route_plan_sha256: None,
        route_policy_sources: None,
        route_objective: None,
        tool_capabilities_sha256: None,
    }
}

fn completed_answer() -> AgentAnswer {
    AgentAnswer {
        schema_version: 2,
        status: AnswerStatus::Completed,
        conclusion: "The exported artifact is deterministic.".into(),
        claims: vec![
            Claim {
                id: "c1".into(),
                text: "The artifact has a stable SHA-256.".into(),
                status: ClaimStatus::Verified,
                sources: vec!["tool:call-1".into()],
                scope: None,
            },
            Claim {
                id: "c2".into(),
                text: "A synthetic derived claim without evidence.".into(),
                status: ClaimStatus::Derived,
                sources: Vec::new(),
                scope: Some("synthetic fixture".into()),
            },
            Claim {
                id: "c3".into(),
                text: "A model prior supported by stale evidence.".into(),
                status: ClaimStatus::ModelPrior,
                sources: Vec::new(),
                scope: None,
            },
        ],
        evidence: vec![
            Evidence {
                source: "tool:call-1".into(),
                statement: "write_file reported a SHA-256.".into(),
                observed_unix_ms: Some(1_700_000_000_000),
                expires_unix_ms: Some(1_800_000_000_000),
                claim_ids: vec!["c1".into()],
            },
            Evidence {
                source: "model:fixture".into(),
                statement: "A stale model-prior observation.".into(),
                observed_unix_ms: None,
                expires_unix_ms: Some(1_600_000_000_000),
                claim_ids: vec!["c3".into()],
            },
        ],
        assumptions: Vec::new(),
        constraints: Vec::new(),
        unknowns: Vec::new(),
        conflicts: Vec::new(),
        flip_conditions: Vec::new(),
        action_receipts: Vec::new(),
        task_need: None,
    }
}

fn conditional_answer() -> AgentAnswer {
    AgentAnswer {
        schema_version: 2,
        status: AnswerStatus::Conditional,
        conclusion: "The deployment is conditional on a log.".into(),
        claims: vec![Claim {
            id: "cq7".into(),
            text: "Deployment status is unknown.".into(),
            status: ClaimStatus::ModelPrior,
            sources: Vec::new(),
            scope: None,
        }],
        evidence: Vec::new(),
        assumptions: Vec::new(),
        constraints: Vec::new(),
        unknowns: vec![UnknownItem {
            id: "u1".into(),
            description: "whether the deployment actually happened".into(),
            impact: ImpactLevel::Medium,
            why_it_matters: None,
            reduction_method: None,
        }],
        conflicts: Vec::new(),
        flip_conditions: vec![FlipCondition {
            if_observed: "a deployment log appears".into(),
            then: "mark the claim verified".into(),
            target_ids: vec!["cq7".into(), "u1".into()],
        }],
        action_receipts: Vec::new(),
        task_need: None,
    }
}

struct RichFixture {
    session_id: String,
    run_completed: String,
    run_conditional: String,
    run_authorization: String,
    run_resumed: String,
}

fn build_rich_session(root: &Path) -> RichFixture {
    let completed_run = Uuid::new_v4();
    let completed_path = root.join(format!("{completed_run}.jsonl"));
    {
        let mut journal = Journal::create(&completed_path, completed_run).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: base_request(root, RAW_OBJECTIVE),
                mode: CognitiveMode::EvidenceResearch,
            })
            .unwrap();
        journal
            .append(JournalEventKind::RouteSelected {
                provider_id: "provider-alpha".into(),
                model: "model-beta".into(),
                reason: "fixture route".into(),
            })
            .unwrap();
        journal
            .append(JournalEventKind::ModelResponded {
                response: ModelResponse {
                    provider_id: "provider-alpha".into(),
                    model: "model-beta".into(),
                    turn: ModelTurn::ToolCalls { calls: Vec::new() },
                    usage: Usage {
                        input_tokens: 100,
                        output_tokens: 20,
                        cost_micros: 7_000,
                        cache_hit_tokens: 0,
                        cache_miss_tokens: 0,
                    },
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::Checkpoint {
                checkpoint: checkpoint(CognitiveMode::EvidenceResearch),
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolCalled {
                call: ToolCall {
                    id: "call-1".into(),
                    name: "write_file".into(),
                    arguments: json!({
                        "path": "report.md",
                        "content": RAW_TOOL_CONTENT,
                        "note": RAW_TOOL_ARGUMENT,
                    }),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "call-1".into(),
                    tool: "write_file".into(),
                    status: ToolStatus::Ok,
                    summary: "wrote report.md (12 bytes)".into(),
                    content: RAW_TOOL_CONTENT.into(),
                    error_code: None,
                    metadata: BTreeMap::from([
                        ("sha256".into(), ARTIFACT_SHA.into()),
                        ("path".into(), "report.md".into()),
                        ("bytes".into(), "12".into()),
                    ]),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: completed_answer(),
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
    }

    let conditional_run = Uuid::new_v4();
    let conditional_path = root.join(format!("{conditional_run}.jsonl"));
    {
        let mut journal = Journal::create(&conditional_path, conditional_run).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: base_request(root, "conditional fixture"),
                mode: CognitiveMode::EvidenceResearch,
            })
            .unwrap();
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: conditional_answer(),
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
    }

    let authorization_run = Uuid::new_v4();
    let authorization_path = root.join(format!("{authorization_run}.jsonl"));
    {
        let mut journal = Journal::create(&authorization_path, authorization_run).unwrap();
        journal
            .append(JournalEventKind::RunStarted {
                request: base_request(root, "authorization fixture"),
                mode: CognitiveMode::Execute,
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolCalled {
                call: ToolCall {
                    id: "call-auth".into(),
                    name: "click".into(),
                    arguments: json!({"target": "pay"}),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "call-auth".into(),
                    tool: "click".into(),
                    status: ToolStatus::PermissionDenied,
                    summary: "approval is required".into(),
                    content: "approval required".into(),
                    error_code: Some("approval_required".into()),
                    metadata: BTreeMap::from([
                        ("policy_outcome".into(), "require_approval".into()),
                        ("policy_reason".into(), "irreversible browser click".into()),
                        ("authority_profile_id".into(), "authority-1".into()),
                        ("approval_card_id".into(), "card-1".into()),
                    ]),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::NeedsAuthorization,
            })
            .unwrap();
    }

    let resumed_run = Uuid::new_v4();
    let resumed_path = root.join(format!("{resumed_run}.jsonl"));
    {
        let mut journal = Journal::create(&resumed_path, resumed_run).unwrap();
        journal
            .append(JournalEventKind::ResumeStarted {
                original_run_id: completed_run,
                checkpoint_seq: 3,
                request: base_request(root, "resume fixture"),
                checkpoint: checkpoint(CognitiveMode::EvidenceResearch),
            })
            .unwrap();
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
    }

    let mut session = TuiSession::open(root, None).unwrap();
    session.rename("Ontology QA synthetic session").unwrap();
    session
        .start_mission(RAW_MISSION, root.to_path_buf())
        .unwrap();
    for path in [
        &completed_path,
        &conditional_path,
        &authorization_path,
        &resumed_path,
    ] {
        session.record_journal(path).unwrap();
    }

    RichFixture {
        session_id: session.id().to_string(),
        run_completed: completed_run.to_string(),
        run_conditional: conditional_run.to_string(),
        run_authorization: authorization_run.to_string(),
        run_resumed: resumed_run.to_string(),
    }
}

fn export_session(root: &Path, id: &str) -> (Value, Output) {
    let output = run(
        root,
        &[
            "ontology",
            "export",
            "--journal-dir",
            root.to_str().unwrap(),
            "--session",
            id,
        ],
    );
    let document = parse_compact_document(&output, "session export");
    (document, output)
}

fn export_journal(root: &Path, journal: &Path) -> (Value, Output) {
    let output = run(
        root,
        &["ontology", "export", "--journal", journal.to_str().unwrap()],
    );
    let document = parse_compact_document(&output, "journal export");
    (document, output)
}

fn spawn_journal(dir: &Path) -> (PathBuf, String) {
    let run_id = Uuid::new_v4();
    let path = dir.join(format!("{run_id}.jsonl"));
    let mut journal = Journal::create(&path, run_id).unwrap();
    journal
        .append(JournalEventKind::RunStarted {
            request: base_request(dir, "spawn fixture"),
            mode: CognitiveMode::Execute,
        })
        .unwrap();
    for (call_id, status, error_code, metadata) in [
        (
            "spawn-ok",
            ToolStatus::Ok,
            None,
            BTreeMap::from([
                ("status".into(), "completed".into()),
                ("child_run_id".into(), "child-run-1".into()),
            ]),
        ),
        (
            "spawn-timeout",
            ToolStatus::TimedOut,
            Some("timed_out"),
            BTreeMap::new(),
        ),
    ] {
        journal
            .append(JournalEventKind::ToolCalled {
                call: ToolCall {
                    id: call_id.into(),
                    name: "spawn_agent".into(),
                    arguments: json!({"task": "child"}),
                },
            })
            .unwrap();
        journal
            .append(JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: call_id.into(),
                    tool: "spawn_agent".into(),
                    status,
                    summary: format!("spawn {call_id}"),
                    content: "{}".into(),
                    error_code: error_code.map(str::to_string),
                    metadata,
                },
            })
            .unwrap();
    }
    journal
        .append(JournalEventKind::RunStopped {
            reason: StopReason::Completed,
        })
        .unwrap();
    (path, run_id.to_string())
}

fn simple_journal(dir: &Path) -> (PathBuf, String, String) {
    let run_id = Uuid::new_v4();
    let path = dir.join(format!("{run_id}.jsonl"));
    let mut journal = Journal::create(&path, run_id).unwrap();
    journal
        .append(JournalEventKind::RunStarted {
            request: base_request(dir, "simple fixture objective"),
            mode: CognitiveMode::EvidenceResearch,
        })
        .unwrap();
    journal
        .append(JournalEventKind::ToolCalled {
            call: ToolCall {
                id: "call-1".into(),
                name: "write_file".into(),
                arguments: json!({"content": RAW_TOOL_CONTENT}),
            },
        })
        .unwrap();
    journal
        .append(JournalEventKind::ToolObserved {
            observation: ToolObservation {
                call_id: "call-1".into(),
                tool: "write_file".into(),
                status: ToolStatus::Ok,
                summary: "wrote report.md".into(),
                content: RAW_TOOL_CONTENT.into(),
                error_code: None,
                metadata: BTreeMap::from([
                    ("sha256".into(), ARTIFACT_SHA.into()),
                    ("path".into(), "report.md".into()),
                    ("bytes".into(), "12".into()),
                ]),
            },
        })
        .unwrap();
    journal
        .append(JournalEventKind::RunStopped {
            reason: StopReason::Completed,
        })
        .unwrap();
    (path, run_id.to_string(), ARTIFACT_SHA.to_string())
}

#[test]
fn ontology_types_and_bindings_are_non_empty_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let types = parse_json_stdout(&run(dir.path(), &["ontology", "types"]), "ontology types");
    assert_eq!(types["schema_version"], 1);
    assert_eq!(types["ontology_version"], "0.98-draft");
    assert!(
        types["namespace"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    let definitions = types["types"].as_array().expect("types array");
    assert!(!definitions.is_empty());
    let ids: BTreeSet<&str> = definitions
        .iter()
        .map(|definition| definition["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), definitions.len(), "duplicate type ids");
    for expected in [
        "Session",
        "Run",
        "Task",
        "ToolInvocation",
        "Observation",
        "Answer",
        "Claim",
        "Evidence",
        "Artifact",
        "SquadMember",
        "ActionType",
        "DataBinding",
    ] {
        assert!(ids.contains(expected), "missing core type {expected}");
    }

    let bindings = parse_json_stdout(
        &run(dir.path(), &["ontology", "bindings"]),
        "ontology bindings",
    );
    assert_eq!(bindings["schema_version"], 1);
    assert_eq!(bindings["mapping_version"], "1");
    let entries = bindings["bindings"].as_array().expect("bindings array");
    assert!(!entries.is_empty());
    for entry in entries {
        assert!(entry["type"].as_str().is_some());
        assert!(entry["property"].as_str().is_some());
        assert!(
            entry["source"].as_str().is_some() || entry["sourcePath"].as_str().is_some(),
            "binding without source: {entry}"
        );
    }
}

#[test]
fn journal_export_is_compact_deterministic_and_embeds_registry_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let (journal_path, run_id, _) = simple_journal(dir.path());
    let first_path = dir.path().join("first.jsonld");
    let second_path = dir.path().join("second.jsonld");

    let first_summary = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "export",
                "--journal",
                journal_path.to_str().unwrap(),
                "--out",
                first_path.to_str().unwrap(),
                "--json",
            ],
        ),
        "first export",
    );
    assert_eq!(first_summary["ok"], true);
    let first_sha = first_summary["sha256"].as_str().unwrap().to_string();
    assert_eq!(first_sha.len(), 64);
    let first_bytes = std::fs::read(&first_path).unwrap();
    assert_eq!(sha256_hex(&first_bytes), first_sha);
    assert!(!first_bytes.contains(&b'\n'), "file must be compact");

    let second_summary = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "export",
                "--journal",
                journal_path.to_str().unwrap(),
                "--out",
                second_path.to_str().unwrap(),
                "--json",
            ],
        ),
        "second export",
    );
    let second_bytes = std::fs::read(&second_path).unwrap();
    assert_eq!(
        first_bytes, second_bytes,
        "export bytes must be deterministic"
    );
    assert_eq!(first_sha, second_summary["sha256"].as_str().unwrap());

    let stdout_document = run(
        dir.path(),
        &[
            "ontology",
            "export",
            "--journal",
            journal_path.to_str().unwrap(),
        ],
    );
    let stdout_document = parse_compact_document(&stdout_document, "stdout export");
    assert_eq!(
        serde_json::to_vec(&stdout_document).unwrap(),
        first_bytes,
        "stdout bytes must match --out bytes"
    );

    let ids: Vec<&str> = graph(&stdout_document)
        .iter()
        .map(|node| node["@id"].as_str().expect("node id"))
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "@graph must be sorted by @id");

    assert!(stdout_document["@context"].is_object());
    assert_eq!(stdout_document["ontologyVersion"], "0.98-draft");
    assert_eq!(stdout_document["bindingVersion"], "1");
    assert!(stdout_document.get("generatedAt").is_none());
    assert!(stdout_document.get("generatedAtUnixMs").is_none());

    let registry_bytes =
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/ontology/types.json")).unwrap();
    assert_eq!(
        stdout_document["registrySha256"].as_str().unwrap(),
        sha256_hex(&registry_bytes),
        "registrySha256 must hash the exact registry bytes"
    );

    let run_node = node_with_suffix(&stdout_document, &format!(":run:{run_id}"));
    assert_eq!(run_node["@type"], "Run");
    assert!(run_node.get("partOfSession").is_none());
    assert_eq!(run_node["hasOutcome"], "completed");
    assert!(
        graph(&stdout_document).iter().all(|node| !node["@id"]
            .as_str()
            .unwrap()
            .starts_with("urn:1agent:session:")),
        "standalone journal export must not invent a session"
    );
}

#[test]
fn journal_export_passes_validate_input() {
    let dir = tempfile::tempdir().unwrap();
    let (journal_path, _, _) = simple_journal(dir.path());
    let export_path = dir.path().join("export.jsonld");
    assert_success(
        &run(
            dir.path(),
            &[
                "ontology",
                "export",
                "--journal",
                journal_path.to_str().unwrap(),
                "--out",
                export_path.to_str().unwrap(),
            ],
        ),
        "export",
    );
    let validation = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "validate",
                "--input",
                export_path.to_str().unwrap(),
                "--json",
            ],
        ),
        "validate export",
    );
    assert_eq!(validation["ok"], true, "{validation}");
    assert!(
        validation["checks"]
            .as_array()
            .is_some_and(|checks| !checks.is_empty())
    );
}

#[test]
fn session_export_includes_session_run_task_goal_and_validates() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    let session_id = format!("urn:1agent:session:{}", fixture.session_id);
    let session = node_with_suffix(&document, &session_id);
    assert_eq!(session["@type"], "Session");
    assert_eq!(session["status"], "active");
    assert!(
        session["hasGoal"]["@id"]
            .as_str()
            .unwrap()
            .starts_with("urn:1agent:goal:")
    );
    assert_eq!(
        session["hasRun"].as_array().unwrap().len(),
        4,
        "all synthetic journals should produce runs"
    );

    for run_id in [
        &fixture.run_completed,
        &fixture.run_conditional,
        &fixture.run_authorization,
        &fixture.run_resumed,
    ] {
        let run = node_with_suffix(&document, &format!(":run:{run_id}"));
        assert_eq!(run["@type"], "Run");
        assert_eq!(
            run["partOfSession"]["@id"].as_str().unwrap(),
            session_id,
            "session export must link every run back to the session"
        );
        let task_id = run["hasTask"]["@id"].as_str().unwrap().to_string();
        let task = node_with_suffix(&document, task_id.rsplit(':').next().unwrap());
        assert_eq!(task["@type"], "Task");
        assert_eq!(
            task["objectiveSha256"].as_str().unwrap().len(),
            64,
            "raw objective must be replaced by a digest"
        );
    }

    let validation = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "validate",
                "--journal-dir",
                dir.path().to_str().unwrap(),
                "--session",
                &fixture.session_id,
                "--json",
            ],
        ),
        "validate rich session",
    );
    assert_eq!(validation["ok"], true, "{validation}");
}

#[test]
fn missing_session_export_and_validate_fail_without_creating_directory() {
    let dir = tempfile::tempdir().unwrap();
    let missing = "ses_missing_000000";
    let missing_root = dir.path().join(".sessions").join(missing);
    for args in [
        vec![
            "ontology",
            "export",
            "--journal-dir",
            dir.path().to_str().unwrap(),
            "--session",
            missing,
        ],
        vec![
            "ontology",
            "validate",
            "--journal-dir",
            dir.path().to_str().unwrap(),
            "--session",
            missing,
        ],
    ] {
        let output = run(dir.path(), &args);
        assert!(
            !output.status.success(),
            "missing session should fail: {args:?}"
        );
        assert_no_panic(&output, "missing session");
        assert!(
            !missing_root.exists(),
            "export/validate must never create .sessions/{missing}"
        );
    }
}

#[test]
fn validate_rejects_dangling_reference_and_malformed_json_without_panic() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (mut document, _) = export_session(dir.path(), &fixture.session_id);
    let claim_iri = format!("urn:1agent:claim:{}:missing", fixture.run_completed);

    let evidence_nodes: Vec<&mut Value> = document["@graph"]
        .as_array_mut()
        .expect("@graph array")
        .iter_mut()
        .filter(|node| node["@type"] == "Evidence")
        .collect();
    assert!(!evidence_nodes.is_empty());
    for node in evidence_nodes {
        node["supportsClaim"] = json!([{"@id": claim_iri}]);
    }

    let orphan_path = dir.path().join("orphan.jsonld");
    write(&orphan_path, serde_json::to_vec(&document).unwrap());
    let orphan = run(
        dir.path(),
        &[
            "ontology",
            "validate",
            "--input",
            orphan_path.to_str().unwrap(),
        ],
    );
    assert!(
        !orphan.status.success(),
        "dangling claim must fail validation"
    );
    assert_no_panic(&orphan, "dangling reference");

    let malformed_path = dir.path().join("malformed.jsonld");
    write(&malformed_path, b"{\"@graph\": [");
    let malformed = run(
        dir.path(),
        &[
            "ontology",
            "validate",
            "--input",
            malformed_path.to_str().unwrap(),
        ],
    );
    assert!(!malformed.status.success(), "malformed JSON must fail");
    assert_no_panic(&malformed, "malformed JSON");

    let scalar_path = dir.path().join("scalar.jsonld");
    write(&scalar_path, b"[]");
    let scalar = run(
        dir.path(),
        &[
            "ontology",
            "validate",
            "--input",
            scalar_path.to_str().unwrap(),
        ],
    );
    assert!(!scalar.status.success(), "non-object document must fail");
    assert_no_panic(&scalar, "scalar JSON");
}

#[test]
fn export_redacts_raw_prompt_tool_arguments_tool_content_and_secret_marker() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, output) = export_session(dir.path(), &fixture.session_id);
    let text = String::from_utf8(output.stdout.clone()).unwrap();

    for forbidden in [
        RAW_OBJECTIVE,
        RAW_DELIVERABLE,
        RAW_MISSION,
        RAW_TOOL_CONTENT,
        RAW_TOOL_ARGUMENT,
        SECRET_MARKER,
    ] {
        assert!(
            !text.contains(forbidden),
            "export leaked forbidden prompt/tool text: {forbidden}"
        );
    }
    for forbidden_key in [
        "\"arguments\"",
        "\"content\"",
        "\"objective\"",
        "\"deliverable\"",
        "\"goal\"",
        "\"generatedAt\"",
    ] {
        assert!(
            !text.contains(forbidden_key),
            "export leaked forbidden field {forbidden_key}"
        );
    }

    let completed_run = node_with_suffix(
        &document,
        &format!("urn:1agent:run:{}", fixture.run_completed),
    );
    let task_iri = completed_run["hasTask"]["@id"].as_str().unwrap();
    let task = node_with_suffix(&document, task_iri);
    assert_eq!(
        task["objectiveSha256"].as_str().unwrap(),
        sha256_hex(RAW_OBJECTIVE.as_bytes())
    );
    let goal = nodes_of_type(&document, "Goal")
        .into_iter()
        .next()
        .expect("goal node");
    assert_eq!(
        goal["goalSha256"].as_str().unwrap(),
        sha256_hex(RAW_MISSION.as_bytes())
    );
    assert_eq!(goal["goalChars"], RAW_MISSION.chars().count() as u64);

    // Allowed fields remain available for CQ work.
    let claim = node_with_suffix(&document, ":c1");
    assert!(claim["claimText"].as_str().is_some());
    let answer = nodes_of_type(&document, "Answer")
        .into_iter()
        .find(|node| node["status"] == "completed")
        .expect("completed answer");
    assert!(answer["conclusion"].as_str().is_some());
    assert!(has_node_type(&document, "Artifact"));
}

#[test]
fn prompt_derived_session_title_is_hashed_and_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = TuiSession::open(dir.path(), None).unwrap();
    // `main.rs` sets the initial session title from the user prompt. The
    // exporter must hash it rather than writing raw prompt text.
    session.set_title_from_prompt(PROMPT_DERIVED_TITLE);
    let (journal_path, _, _) = simple_journal(dir.path());
    session.record_journal(&journal_path).unwrap();
    let session_id = session.id().to_string();
    drop(session);

    let (document, output) = export_session(dir.path(), &session_id);
    let session_node = node_with_suffix(&document, &format!("urn:1agent:session:{session_id}"));
    assert!(
        session_node.get("title").is_none(),
        "raw title must not be exported"
    );
    assert_eq!(
        session_node["titleSha256"].as_str().unwrap(),
        sha256_hex(PROMPT_DERIVED_TITLE.as_bytes())
    );
    assert_eq!(
        session_node["titleChars"],
        PROMPT_DERIVED_TITLE.chars().count() as u64
    );
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains(PROMPT_DERIVED_TITLE),
        "raw prompt-derived title must not reach CLI output"
    );
}

#[test]
fn empty_and_malformed_journals_fail_without_panic() {
    let dir = tempfile::tempdir().unwrap();
    let empty = dir.path().join("empty.jsonl");
    write(&empty, b"");
    let whitespace = dir.path().join("whitespace.jsonl");
    write(&whitespace, b" \n\t\n");

    let (valid_path, _, _) = simple_journal(dir.path());
    let mut bad = std::fs::read(&valid_path).unwrap();
    bad.extend_from_slice(b"{not-json}\n");
    let bad_path = dir.path().join("bad-tail.jsonl");
    write(&bad_path, bad);

    let null_path = dir.path().join("null.jsonl");
    write(&null_path, b"null\n");
    let binary_path = dir.path().join("binary.jsonl");
    write(&binary_path, [0xff, 0xfe, 0x00, b'\n']);

    for path in [empty, whitespace, bad_path, null_path, binary_path] {
        let output = run(
            dir.path(),
            &["ontology", "export", "--journal", path.to_str().unwrap()],
        );
        assert!(
            !output.status.success(),
            "invalid journal should fail: {}",
            path.display()
        );
        assert_no_panic(&output, &format!("journal {}", path.display()));
        assert!(
            output.stdout.is_empty(),
            "failed export must not emit a partial document: {}",
            path.display()
        );
    }
}

#[test]
fn v1_session_json_remains_exportable_and_valid() {
    let dir = tempfile::tempdir().unwrap();
    let (journal_path, run_id, _) = simple_journal(dir.path());
    let session_id = "ses_v1compat";
    let session_dir = dir.path().join(".sessions").join(session_id);
    write(
        &session_dir.join("session.json"),
        serde_json::to_vec(&json!({
            "version": 1,
            "id": session_id,
            "title": "Legacy v1 session",
            "journals": [journal_path],
        }))
        .unwrap(),
    );

    let (document, _) = export_session(dir.path(), session_id);
    assert!(has_node_type(&document, "Session"));
    assert!(
        graph(&document)
            .iter()
            .any(|node| node["@id"] == format!("urn:1agent:run:{run_id}")),
        "v1 session journal must be exported"
    );

    let validation = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "validate",
                "--journal-dir",
                dir.path().to_str().unwrap(),
                "--session",
                session_id,
                "--json",
            ],
        ),
        "validate v1 session",
    );
    assert_eq!(validation["ok"], true, "{validation}");
}

#[test]
fn artifact_sha_metadata_joins_to_tool_invocation_and_run() {
    let dir = tempfile::tempdir().unwrap();
    let (journal_path, run_id, sha) = simple_journal(dir.path());
    let (document, _) = export_journal(dir.path(), &journal_path);

    let artifact = node_with_suffix(&document, &format!(":sha256:{sha}"));
    assert_eq!(artifact["@type"], "Artifact");
    assert_eq!(artifact["sha256"], sha);
    assert_eq!(artifact["path"], "report.md");
    assert_eq!(artifact["bytes"], 12);

    let generated_by = artifact["wasGeneratedBy"]
        .as_array()
        .expect("artifact generator")
        .first()
        .expect("artifact must have a generator");
    let observation_iri = generated_by["@id"].as_str().unwrap();
    assert!(observation_iri.starts_with(&format!("urn:1agent:observation:{run_id}:")));

    let observation = node_with_suffix(&document, observation_iri);
    assert_eq!(observation["@type"], "Observation");
    let invocation_iri = observation["observedBy"]["@id"].as_str().unwrap();
    assert!(invocation_iri.starts_with(&format!("urn:1agent:tool-call:{run_id}:")));

    let invocation = node_with_suffix(&document, invocation_iri);
    assert_eq!(invocation["@type"], "ToolInvocation");
    let run = node_with_suffix(&document, &format!("urn:1agent:run:{run_id}"));
    assert!(
        run["used"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| reference["@id"] == invocation_iri),
        "run.used must reference the producing tool call"
    );

    // CQ3 also asks for the squad member. This fixture has no spawn_agent
    // observation, and Artifact has no producer-member relation even when the
    // run does export a SquadMember.
    assert!(!has_node_type(&document, "SquadMember"));
}

#[tokio::test]
async fn write_and_edit_file_tools_emit_artifact_sha_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("artifact.txt");
    let permissions = PermissionSet {
        read_roots: vec![std::fs::canonicalize(dir.path()).unwrap()],
        allow_network: false,
        allow_write: true,
    };

    let write = WriteFileTool
        .execute(
            json!({"path": target, "content": "alpha\n"}),
            &permissions,
            &CancellationToken::new(),
        )
        .await
        .expect("write_file");
    let expected_write = sha256_hex(b"alpha\n");
    assert_eq!(write.metadata["sha256"], expected_write);
    assert_eq!(write.metadata["path"], target.display().to_string());
    assert_eq!(write.metadata["bytes"], "6");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "alpha\n");

    let edit = EditFileTool
        .execute(
            json!({"path": target, "old_string": "alpha", "new_string": "beta gamma"}),
            &permissions,
            &CancellationToken::new(),
        )
        .await
        .expect("edit_file");
    assert_eq!(edit.metadata["sha256_before"], expected_write);
    assert_eq!(edit.metadata["sha256"], sha256_hex(b"beta gamma\n"));
    assert_eq!(edit.metadata["path"], target.display().to_string());
    assert_eq!(edit.metadata["bytes"], "11");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "beta gamma\n");
}

#[test]
fn cq01_completed_answer_claims_without_evidence_are_identifiable() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);
    let answer = nodes_of_type(&document, "Answer")
        .into_iter()
        .find(|node| node["status"] == "completed")
        .expect("completed answer");

    let claims: Vec<&str> = answer["hasClaim"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reference| reference["@id"].as_str().unwrap())
        .collect();
    assert_eq!(claims.len(), 3);

    let mut without_evidence = Vec::new();
    for claim_iri in claims {
        let claim = node_with_suffix(&document, claim_iri);
        let has_cites = claim["cites"]
            .as_array()
            .is_some_and(|cites| !cites.is_empty());
        if !has_cites {
            without_evidence.push(claim["@id"].as_str().unwrap().to_string());
        }
    }
    assert!(
        without_evidence.iter().any(|iri| iri.ends_with(":c2")),
        "c2 deliberately has no evidence: {without_evidence:?}"
    );
    assert!(
        without_evidence
            .iter()
            .all(|iri| !iri.ends_with(":c1") && !iri.ends_with(":c3")),
        "c1/c3 are supported: {without_evidence:?}"
    );
}

#[test]
fn cq02_claim_evidence_observation_join_uses_explicit_observed_by() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    let claim = node_with_suffix(&document, ":c1");
    let evidence_iri = claim["cites"].as_array().unwrap().first().unwrap()["@id"]
        .as_str()
        .unwrap();
    let evidence = node_with_suffix(&document, evidence_iri);
    assert_eq!(evidence["sourceRef"], "tool:call-1");
    let observation_iri = evidence["observedBy"]["@id"].as_str().unwrap();
    assert_eq!(
        observation_iri,
        format!("urn:1agent:observation:{}:call-1", fixture.run_completed)
    );

    let observation = node_with_suffix(&document, observation_iri);
    assert_eq!(observation["@type"], "Observation");
    assert_eq!(
        observation["observedBy"]["@id"],
        format!("urn:1agent:tool-call:{}:call-1", fixture.run_completed)
    );
}

#[test]
fn cq06_needs_authorization_run_is_identifiable_but_actor_scope_missing() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    let blocked: Vec<&Value> = nodes_of_type(&document, "Run")
        .into_iter()
        .filter(|run| run["hasOutcome"] == "needs_authorization")
        .collect();
    assert_eq!(blocked.len(), 1);
    assert_eq!(
        blocked[0]["@id"],
        format!("urn:1agent:run:{}", fixture.run_authorization)
    );

    // CQ6 also asks which actor/scope blocked. The journal metadata carries
    // policy fields, but the exporter drops them.
    let text = document.to_string();
    assert!(!text.contains("policy_outcome"));
    assert!(!text.contains("authority_profile_id"));
    assert!(!has_node_type(&document, "PolicyDecision"));
}

#[test]
fn cq07_conditional_flip_targets_are_local_ids() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    let conditional = nodes_of_type(&document, "Answer")
        .into_iter()
        .find(|node| node["status"] == "conditional")
        .expect("conditional answer");
    assert_eq!(
        conditional["@id"],
        format!("urn:1agent:answer:{}", fixture.run_conditional)
    );
    let flip = conditional["flipConditions"]
        .as_array()
        .unwrap()
        .first()
        .expect("flip condition");
    let targets: BTreeSet<&str> = flip["targetIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|target| target.as_str().unwrap())
        .collect();
    assert_eq!(targets, BTreeSet::from(["cq7", "u1"]));

    // Targets are answer-local ids, not IRIs; unknown is embedded, so a
    // consumer can only match by id and must not assume a resolvable node.
    let claim = node_with_suffix(&document, ":cq7");
    assert_eq!(claim["@type"], "Claim");
    assert!(conditional["unknowns"][0]["id"] == "u1");
    assert!(!has_node_type(&document, "FlipCondition"));
    assert!(!has_node_type(&document, "Unknown"));
}

#[test]
fn cq08_expired_or_untimed_evidence_is_identifiable() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);
    let evidence = nodes_of_type(&document, "Evidence");
    assert_eq!(evidence.len(), 2);

    let stale = evidence
        .iter()
        .find(|node| node["sourceRef"] == "model:fixture")
        .expect("stale evidence");
    assert_eq!(stale["expiresAtUnixMs"], 1_600_000_000_000u64);
    assert!(
        stale.get("observedAtUnixMs").is_none(),
        "untimed evidence must omit observedAtUnixMs rather than invent a time"
    );
    let fresh = evidence
        .iter()
        .find(|node| node["sourceRef"] == "tool:call-1")
        .expect("fresh evidence");
    assert_eq!(fresh["observedAtUnixMs"], 1_700_000_000_000u64);
    assert!(
        fresh["expiresAtUnixMs"].as_u64().unwrap() > 1_700_000_000_000,
        "fixture expiry is in the future"
    );
}

#[test]
fn cq09_resumed_run_reports_original_run_and_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    let resumed = node_with_suffix(
        &document,
        &format!("urn:1agent:run:{}", fixture.run_resumed),
    );
    assert_eq!(resumed["resumedFromCheckpointSeq"], 3);
    assert_eq!(resumed["resumedFromRunId"], fixture.run_completed);
    let checkpointed = node_with_suffix(
        &document,
        &format!("urn:1agent:run:{}", fixture.run_completed),
    );
    assert_eq!(checkpointed["checkpointSeq"], 3);

    // Duplicate side effects are not a first-class export field; a consumer
    // must compare tool calls/artifacts across the original and resumed runs.
    assert!(resumed.get("duplicateSideEffects").is_none());
}

#[test]
fn cq10_session_fork_is_exported_with_parent_stub_and_validates() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = TuiSession::open(dir.path(), None).unwrap();
    session.rename("Fork source").unwrap();
    let (journal_path, _, _) = simple_journal(dir.path());
    session.record_journal(&journal_path).unwrap();
    let parent_id = session.id().to_string();
    let child = session.fork(Some("Fork child")).unwrap();
    let child_id = child.id().to_string();
    drop(child);

    let (document, _) = export_session(dir.path(), &child_id);
    let child_node = node_with_suffix(&document, &format!("urn:1agent:session:{child_id}"));
    assert_eq!(child_node["@type"], "Session");
    let parent_iri = format!("urn:1agent:session:{parent_id}");
    assert_eq!(
        child_node["forkedFrom"]["@id"], parent_iri,
        "CQ10 is answerable from forkedFrom"
    );

    let parent = node_with_suffix(&document, &parent_iri);
    assert_eq!(parent["@type"], "Session");
    assert_eq!(parent["@id"], parent_iri);

    let validation = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "validate",
                "--journal-dir",
                dir.path().to_str().unwrap(),
                "--session",
                &child_id,
                "--json",
            ],
        ),
        "validate forked session",
    );
    assert_eq!(validation["ok"], true, "{validation}");
}

#[test]
fn cq14_run_route_and_cost_are_identifiable_for_claim() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    let claim = node_with_suffix(&document, ":c1");
    let claim_iri = claim["@id"].as_str().unwrap();
    assert!(claim_iri.contains(&fixture.run_completed));
    let run = node_with_suffix(
        &document,
        &format!("urn:1agent:run:{}", fixture.run_completed),
    );
    assert_eq!(run["providerId"], "provider-alpha");
    assert_eq!(run["model"], "model-beta");
    assert_eq!(run["inputTokens"], 100);
    assert_eq!(run["outputTokens"], 20);
    assert_eq!(run["costMicros"], 7_000);
}

#[test]
fn cq04_successful_spawn_exports_squad_lineage() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = TuiSession::open(dir.path(), None).unwrap();
    session.rename("Squad fixture").unwrap();
    let (journal_path, run_id) = spawn_journal(dir.path());
    session.record_journal(&journal_path).unwrap();
    let session_id = session.id().to_string();
    drop(session);

    let (document, _) = export_session(dir.path(), &session_id);
    let squad = node_with_suffix(
        &document,
        &format!("urn:1agent:squad-member:{run_id}:spawn-ok"),
    );
    assert_eq!(squad["@type"], "SquadMember");
    assert_eq!(squad["status"], "completed");
    assert_eq!(squad["childRunId"], "child-run-1");
    assert_eq!(
        squad["parentRun"]["@id"],
        format!("urn:1agent:run:{run_id}")
    );
    assert_eq!(
        squad["partOfSession"]["@id"],
        format!("urn:1agent:session:{session_id}")
    );
    assert_eq!(
        squad["spawnedBy"]["@id"],
        format!("urn:1agent:tool-call:{run_id}:spawn-ok")
    );
    let run = node_with_suffix(&document, &format!("urn:1agent:run:{run_id}"));
    assert_eq!(run["hasSquadMember"], json!([{"@id": squad["@id"]}]));

    // CQ13 gap: the timed-out spawn has an Observation but no SquadMember,
    // so failed/timed-out members cannot be queried as squad members.
    let timeout = graph(&document)
        .iter()
        .find(|node| {
            node["@type"] == "Observation"
                && node["@id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with(":spawn-timeout"))
        })
        .expect("timed-out spawn observation");
    assert_eq!(timeout["status"], "timed_out");
    assert!(
        !graph(&document).iter().any(|node| {
            node["@type"] == "SquadMember"
                && node["@id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with(":spawn-timeout"))
        }),
        "characterization: failed/timed-out spawns are not SquadMember nodes"
    );
}

#[test]
fn cq_gap_policy_and_approval_metadata_is_not_exported() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);
    let text = document.to_string();
    // The journal really contains these metadata keys (see build_rich_session),
    // but the exporter drops them. CQ5/CQ12/CQ16 cannot be answered.
    assert!(!text.contains("policy_outcome"));
    assert!(!text.contains("approval_card_id"));
    assert!(!text.contains("authority_profile_id"));
    assert!(!has_node_type(&document, "PolicyDecision"));
    assert!(!has_node_type(&document, "Approval"));
    assert!(!has_node_type(&document, "ActionInvocation"));
}

#[test]
fn cq_gap_eval_and_audit_projections_are_absent() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);
    // CQ11 and CQ15 have no exporter input at all.
    assert!(!has_node_type(&document, "EvalCase"));
    assert!(!has_node_type(&document, "EvalRun"));
    assert!(!has_node_type(&document, "Audit"));
    assert!(!has_node_type(&document, "Journal"));
}

#[test]
fn binding_coverage_gap_is_documented() {
    let dir = tempfile::tempdir().unwrap();
    let types = parse_json_stdout(&run(dir.path(), &["ontology", "types"]), "types");
    let bindings = parse_json_stdout(&run(dir.path(), &["ontology", "bindings"]), "bindings");
    let bound: BTreeSet<String> = bindings["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            entry["type"]
                .as_str()
                .unwrap()
                .trim_start_matches("one:")
                .to_string()
        })
        .collect();
    let uncovered: Vec<&str> = types["types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|definition| definition["id"].as_str().unwrap())
        .filter(|id| !bound.contains(*id))
        .collect();
    assert!(
        uncovered.len() >= 8,
        "characterization: many core types have no binding: {uncovered:?}"
    );
    for expected in [
        "Task",
        "Observation",
        "Answer",
        "Goal",
        "Commitment",
        "ActionType",
        "PolicyDecision",
        "Approval",
        "DataBinding",
    ] {
        assert!(
            uncovered.contains(&expected),
            "expected {expected} to be unbound in the current P0 registry"
        );
    }
}

#[test]
fn validator_accepts_a_completed_claim_without_evidence_known_gap() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = build_rich_session(dir.path());
    let (document, _) = export_session(dir.path(), &fixture.session_id);

    // c2 has no cites, and the completed Answer is still reported valid.
    let claim = node_with_suffix(&document, ":c2");
    assert!(claim.get("cites").is_none());
    let path = dir.path().join("claim-without-evidence.jsonld");
    write(&path, serde_json::to_vec(&document).unwrap());
    let report = parse_json_stdout(
        &run(
            dir.path(),
            &[
                "ontology",
                "validate",
                "--input",
                path.to_str().unwrap(),
                "--json",
            ],
        ),
        "validate claim without evidence",
    );
    assert_eq!(
        report["ok"], true,
        "characterization: the validator does not detect a completed claim without evidence"
    );
}
