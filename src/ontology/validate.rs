//! Internal P0 validation for exported JSON-LD documents.
//!
//! This is a closed-world check: every emitted reference must resolve inside
//! the document. Validation reports errors and never panics.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub ok: bool,
    pub checks: Vec<String>,
    pub errors: Vec<String>,
}

const KNOWN_TYPES: &[&str] = &[
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
    "Goal",
    "Commitment",
    "ActionType",
    "ActionInvocation",
    "PolicyDecision",
    "Approval",
    "DataBinding",
];

const REFERENCE_FIELDS: &[&str] = &[
    "partOfSession",
    "hasRun",
    "hasTask",
    "generated",
    "used",
    "wasGeneratedBy",
    "wasDerivedFrom",
    "observedBy",
    "supportsClaim",
    "contradictsClaim",
    "cites",
    "hasEvidence",
    "hasClaim",
    "hasObservation",
    "hasArtifact",
    "hasGoal",
    "hasSquadMember",
    "parentRun",
    "spawnedBy",
    "authorizedBy",
    "decidedBy",
    "approvedBy",
    "resumedFrom",
    "forkedFrom",
    "parentOf",
    "childOf",
    "supersedes",
    "hasPolicyDecision",
    "hasActionType",
    "hasToolCall",
    "hasApproval",
    "hasActionInvocation",
    "forAction",
    "partOfRun",
    "hasCommitment",
    "belongsToGoal",
];

const SESSION_STATUSES: &[&str] = &["active", "archived"];
const ANSWER_STATUSES: &[&str] = &[
    "completed",
    "conditional",
    "needs_evidence",
    "needs_authorization",
    "failed",
];
const CLAIM_STATUSES: &[&str] = &[
    "verified",
    "user_asserted",
    "derived",
    "assumed",
    "model_prior",
    "conflicting",
    "unknown",
];
const STOP_REASONS: &[&str] = &[
    "completed",
    "needs_evidence",
    "needs_authorization",
    "budget_exhausted",
    "cancelled",
    "provider_error",
    "tool_failure",
    "invalid_output",
    "internal_error",
];
const TOOL_STATUSES: &[&str] = &[
    "ok",
    "invalid_arguments",
    "permission_denied",
    "timed_out",
    "failed",
];
const SQUAD_STATUSES: &[&str] = &[
    "starting",
    "running",
    "completed",
    "failed",
    "timed_out",
    "cancelled",
];
const ACTION_STATUSES: &[&str] = &[
    "proposed",
    "allowed",
    "denied",
    "needs_approval",
    "approved",
    "rejected",
    "executed",
    "failed",
    "cancelled",
];
const POLICY_OUTCOMES: &[&str] = &["allow", "require_approval", "deny"];
const APPROVAL_STATUSES: &[&str] = &["recorded", "pending", "approved", "denied", "consumed"];
const GOAL_STATUSES: &[&str] = &["active", "paused", "achieved", "abandoned"];
const COMMITMENT_STATUSES: &[&str] = &["pending", "kept", "missed", "cancelled"];
const SOURCE_KINDS: &[&str] = &["web", "tool", "model_prior", "file", "unknown"];

pub fn validate_document(document: &Value) -> ValidationReport {
    let mut checks = Vec::new();
    let mut errors = Vec::new();
    let mut reference_failures = false;
    let mut schema_failures = false;
    let mut artifact_failures = false;
    let mut duplicate_failures = false;

    let Some(root) = document.as_object() else {
        return ValidationReport {
            ok: false,
            checks,
            errors: vec!["document must be a JSON object".into()],
        };
    };

    match root.get("@context") {
        Some(Value::Object(context)) if !context.is_empty() => {
            checks.push("embedded @context is present".into());
        }
        Some(Value::Object(_)) => errors.push("@context must not be empty".into()),
        Some(_) => errors.push("@context must be an embedded JSON object".into()),
        None => errors.push("@context is required".into()),
    }

    for field in ["ontologyVersion", "bindingVersion", "registrySha256"] {
        match root.get(field).and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => {
                checks.push(format!("{field} is present"));
            }
            _ => errors.push(format!(
                "{field} is required and must be a non-empty string"
            )),
        }
    }
    if let Some(registry) = root.get("registrySha256").and_then(Value::as_str)
        && !is_sha256_hex(registry)
    {
        errors.push("registrySha256 must be 64 lowercase hex characters".into());
    }

    let Some(graph) = root.get("@graph").and_then(Value::as_array) else {
        errors.push("@graph must be an array".into());
        return ValidationReport {
            ok: false,
            checks,
            errors,
        };
    };
    if graph.is_empty() {
        errors.push("@graph must not be empty".into());
    }
    checks.push(format!("@graph contains {} nodes", graph.len()));

    let mut nodes: BTreeMap<String, (&Map<String, Value>, Option<&'static str>)> = BTreeMap::new();
    for (index, entry) in graph.iter().enumerate() {
        let Some(object) = entry.as_object() else {
            errors.push(format!("@graph[{index}] must be an object"));
            continue;
        };
        let Some(id) = object.get("@id").and_then(Value::as_str) else {
            errors.push(format!("@graph[{index}] is missing a string @id"));
            continue;
        };
        if id.trim().is_empty() {
            errors.push(format!("@graph[{index}] has an empty @id"));
            continue;
        }
        if !id.starts_with("urn:1agent:") {
            errors.push(format!("{id}: @id must use the urn:1agent namespace"));
        }
        let node_type = known_type(object.get("@type"));
        if node_type.is_none() {
            errors.push(format!("{id}: @type must include a known ontology type"));
            schema_failures = true;
        }
        if nodes.insert(id.to_string(), (object, node_type)).is_some() {
            errors.push(format!("duplicate @id: {id}"));
            duplicate_failures = true;
        }
    }
    if !duplicate_failures {
        checks.push("no duplicate @id".into());
    }

    for (id, (object, node_type)) in &nodes {
        for field in REFERENCE_FIELDS {
            let Some(value) = object.get(*field) else {
                continue;
            };
            match reference_ids(value) {
                Some(references) => {
                    for reference in references {
                        if !nodes.contains_key(reference) {
                            errors
                                .push(format!("{id}: {field} references missing @id {reference}"));
                            reference_failures = true;
                        }
                    }
                }
                None => {
                    errors.push(format!(
                        "{id}: {field} must be an @id object or an array of @id objects"
                    ));
                    reference_failures = true;
                }
            }
        }

        match node_type {
            Some("Run") => validate_run(id, object, &mut errors, &mut schema_failures),
            Some("Answer") => {
                require_status(
                    id,
                    object,
                    "status",
                    ANSWER_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
                require_string(id, object, "conclusion", &mut errors, &mut schema_failures);
            }
            Some("Claim") => {
                require_string(id, object, "claimText", &mut errors, &mut schema_failures);
                require_status(
                    id,
                    object,
                    "claimStatus",
                    CLAIM_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("Evidence") => {
                require_status(
                    id,
                    object,
                    "sourceKind",
                    SOURCE_KINDS,
                    &mut errors,
                    &mut schema_failures,
                );
                require_string(id, object, "sourceRef", &mut errors, &mut schema_failures);
                validate_supports_claim(id, object, &mut errors, &mut schema_failures);
            }
            Some("Artifact") => {
                validate_artifact(
                    id,
                    object,
                    &mut errors,
                    &mut artifact_failures,
                    &mut schema_failures,
                );
            }
            Some("SquadMember") => {
                require_status(
                    id,
                    object,
                    "status",
                    SQUAD_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("Observation") => {
                optional_status(
                    id,
                    object,
                    "status",
                    TOOL_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("Session") => {
                optional_status(
                    id,
                    object,
                    "status",
                    SESSION_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("Goal") => {
                optional_status(
                    id,
                    object,
                    "status",
                    GOAL_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("Commitment") => {
                require_status(
                    id,
                    object,
                    "status",
                    COMMITMENT_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("ActionInvocation") => {
                require_status(
                    id,
                    object,
                    "status",
                    ACTION_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("PolicyDecision") => {
                require_status(
                    id,
                    object,
                    "outcome",
                    POLICY_OUTCOMES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            Some("Approval") => {
                require_status(
                    id,
                    object,
                    "status",
                    APPROVAL_STATUSES,
                    &mut errors,
                    &mut schema_failures,
                );
            }
            _ => {}
        }
    }

    if !reference_failures {
        checks.push("all @id references resolve".into());
    }
    if !schema_failures {
        checks.push("required fields and statuses are valid".into());
    }
    if !artifact_failures {
        checks.push("artifact sha256 values are 64 lowercase hex".into());
    }

    ValidationReport {
        ok: errors.is_empty(),
        checks,
        errors,
    }
}

fn validate_run(
    id: &str,
    object: &Map<String, Value>,
    errors: &mut Vec<String>,
    schema_failures: &mut bool,
) {
    let task_count = relation_count(object.get("hasTask"));
    if task_count != 1 {
        errors.push(format!(
            "{id}: Run.hasTask must contain exactly 1 task (found {task_count})"
        ));
        *schema_failures = true;
    }

    let outcome_count = relation_count(object.get("hasOutcome"));
    if outcome_count != 1 {
        errors.push(format!(
            "{id}: Run.hasOutcome must contain exactly 1 outcome (found {outcome_count})"
        ));
        *schema_failures = true;
    }
    if let Some(value) = object.get("hasOutcome") {
        let outcomes = literal_strings(value);
        if outcomes.is_empty() {
            errors.push(format!("{id}: Run.hasOutcome must be a string"));
            *schema_failures = true;
        }
        for outcome in outcomes {
            if !STOP_REASONS.contains(&outcome) {
                errors.push(format!(
                    "{id}: Run.hasOutcome has unknown value {outcome:?}"
                ));
                *schema_failures = true;
            }
        }
    }
}

fn validate_supports_claim(
    id: &str,
    object: &Map<String, Value>,
    errors: &mut Vec<String>,
    schema_failures: &mut bool,
) {
    match object.get("supportsClaim") {
        None => {
            errors.push(format!("{id}: Evidence.supportsClaim is required"));
            *schema_failures = true;
        }
        Some(value) => match reference_ids(value) {
            Some(references) if references.is_empty() => {
                errors.push(format!(
                    "{id}: Evidence.supportsClaim must reference at least one claim"
                ));
                *schema_failures = true;
            }
            Some(_) => {}
            None => {
                // The generic reference pass reports the shape error.
            }
        },
    }
}

fn validate_artifact(
    id: &str,
    object: &Map<String, Value>,
    errors: &mut Vec<String>,
    artifact_failures: &mut bool,
    schema_failures: &mut bool,
) {
    match object.get("sha256").and_then(Value::as_str) {
        Some(sha256) if is_sha256_hex(sha256) => {}
        Some(_) => {
            errors.push(format!(
                "{id}: Artifact.sha256 must be 64 lowercase hex characters"
            ));
            *artifact_failures = true;
        }
        None => {
            errors.push(format!("{id}: Artifact.sha256 is required"));
            *artifact_failures = true;
            *schema_failures = true;
        }
    }
}

fn require_string<'a>(
    id: &str,
    object: &'a Map<String, Value>,
    field: &str,
    errors: &mut Vec<String>,
    schema_failures: &mut bool,
) -> Option<&'a str> {
    match object.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value),
        Some(Value::String(_)) => {
            errors.push(format!("{id}: {field} must not be empty"));
            *schema_failures = true;
            None
        }
        Some(_) => {
            errors.push(format!("{id}: {field} must be a string"));
            *schema_failures = true;
            None
        }
        None => {
            errors.push(format!("{id}: {field} is required"));
            *schema_failures = true;
            None
        }
    }
}

fn require_status(
    id: &str,
    object: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
    errors: &mut Vec<String>,
    schema_failures: &mut bool,
) {
    if let Some(value) = require_string(id, object, field, errors, schema_failures)
        && !allowed.contains(&value)
    {
        errors.push(format!("{id}: {field} has unknown value {value:?}"));
        *schema_failures = true;
    }
}

fn optional_status(
    id: &str,
    object: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
    errors: &mut Vec<String>,
    schema_failures: &mut bool,
) {
    match object.get(field) {
        None => {}
        Some(Value::String(value)) if allowed.contains(&value.as_str()) => {}
        Some(Value::String(value)) => {
            errors.push(format!("{id}: {field} has unknown value {value:?}"));
            *schema_failures = true;
        }
        Some(_) => {
            errors.push(format!("{id}: {field} must be a string"));
            *schema_failures = true;
        }
    }
}

fn known_type(value: Option<&Value>) -> Option<&'static str> {
    let names: Vec<&str> = match value? {
        Value::String(name) => vec![name.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => return None,
    };
    names
        .into_iter()
        .find_map(|name| KNOWN_TYPES.iter().copied().find(|known| *known == name))
}

fn reference_ids(value: &Value) -> Option<Vec<&str>> {
    match value {
        Value::Object(object) => object.get("@id").and_then(Value::as_str).map(|id| vec![id]),
        Value::Array(items) => {
            let mut references = Vec::with_capacity(items.len());
            for item in items {
                let id = item.as_object()?.get("@id")?.as_str()?;
                references.push(id);
            }
            Some(references)
        }
        _ => None,
    }
}

fn relation_count(value: Option<&Value>) -> usize {
    match value {
        None | Some(Value::Null) => 0,
        Some(Value::Array(items)) => items.len(),
        Some(_) => 1,
    }
}

fn literal_strings(value: &Value) -> Vec<&str> {
    match value {
        Value::String(value) => vec![value.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::validate_document;

    const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const RUN: &str = "urn:1agent:run:r1";
    const CLAIM: &str = "urn:1agent:claim:r1:c1";
    const EVIDENCE: &str = "urn:1agent:evidence:r1:4:0";
    const ARTIFACT: &str = "urn:1agent:artifact:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn valid_document() -> Value {
        json!({
            "@context": {
                "@version": 1.1,
                "@vocab": "https://1agent.dev/ontology/0.98#"
            },
            "ontologyVersion": "0.98-draft",
            "bindingVersion": "2",
            "registrySha256": SHA,
            "@graph": [
                {
                    "@id": "urn:1agent:session:s1",
                    "@type": "Session",
                    "title": "Synthetic session",
                    "status": "active",
                    "hasRun": [{"@id": RUN}],
                    "hasGoal": {"@id": "urn:1agent:goal:g1"}
                },
                {
                    "@id": "urn:1agent:goal:g1",
                    "@type": "Goal",
                    "goalText": "Ship it",
                    "status": "active",
                    "revision": 1
                },
                {
                    "@id": RUN,
                    "@type": "Run",
                    "partOfSession": {"@id": "urn:1agent:session:s1"},
                    "hasTask": {"@id": "urn:1agent:task:r1:main"},
                    "hasOutcome": "completed",
                    "used": [{"@id": "urn:1agent:tool-call:r1:c1"}],
                    "generated": {"@id": "urn:1agent:answer:r1"}
                },
                {
                    "@id": "urn:1agent:task:r1:main",
                    "@type": "Task",
                    "objectiveSha256": SHA
                },
                {
                    "@id": "urn:1agent:tool-call:r1:c1",
                    "@type": "ToolInvocation",
                    "name": "read_file",
                    "hasObservation": {"@id": "urn:1agent:observation:r1:c1"}
                },
                {
                    "@id": "urn:1agent:observation:r1:c1",
                    "@type": "Observation",
                    "tool": "read_file",
                    "status": "ok",
                    "summary": "read a file",
                    "observedBy": {"@id": "urn:1agent:tool-call:r1:c1"},
                    "hasArtifact": [{"@id": ARTIFACT}]
                },
                {
                    "@id": "urn:1agent:answer:r1",
                    "@type": "Answer",
                    "status": "completed",
                    "conclusion": "Done",
                    "hasClaim": [{"@id": CLAIM}],
                    "hasEvidence": [{"@id": EVIDENCE}]
                },
                {
                    "@id": CLAIM,
                    "@type": "Claim",
                    "claimText": "The claim holds",
                    "claimStatus": "verified",
                    "cites": [{"@id": EVIDENCE}]
                },
                {
                    "@id": EVIDENCE,
                    "@type": "Evidence",
                    "sourceKind": "tool",
                    "sourceRef": "tool:c1",
                    "statement": "Observed",
                    "supportsClaim": [{"@id": CLAIM}]
                },
                {
                    "@id": ARTIFACT,
                    "@type": "Artifact",
                    "sha256": SHA,
                    "wasGeneratedBy": [{"@id": "urn:1agent:observation:r1:c1"}]
                }
            ]
        })
    }

    fn node_mut<'a>(document: &'a mut Value, id: &str) -> &'a mut Map<String, Value> {
        for node in document["@graph"].as_array_mut().expect("graph") {
            if node.get("@id").and_then(Value::as_str) == Some(id) {
                return node.as_object_mut().expect("node object");
            }
        }
        panic!("missing node {id}");
    }

    #[test]
    fn valid_document_passes() {
        let report = validate_document(&valid_document());
        assert!(report.ok, "validator errors: {:?}", report.errors);
        assert!(!report.checks.is_empty());
    }

    #[test]
    fn dangling_evidence_reference_is_reported() {
        let mut document = valid_document();
        node_mut(&mut document, EVIDENCE).insert(
            "supportsClaim".into(),
            json!([{"@id": "urn:1agent:claim:r1:missing"}]),
        );
        let report = validate_document(&document);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("supportsClaim")
                    && error.contains("urn:1agent:claim:r1:missing")),
            "errors: {:?}",
            report.errors
        );
    }

    #[test]
    fn missing_claim_status_and_bad_artifact_sha_are_reported() {
        let mut document = valid_document();
        node_mut(&mut document, CLAIM).remove("claimStatus");
        node_mut(&mut document, ARTIFACT).insert("sha256".into(), json!("ABC"));
        let report = validate_document(&document);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("claimStatus")),
            "errors: {:?}",
            report.errors
        );
        assert!(
            report.errors.iter().any(|error| error.contains("sha256")),
            "errors: {:?}",
            report.errors
        );
    }

    #[test]
    fn duplicate_ids_and_malformed_documents_are_reported_without_panicking() {
        let mut document = valid_document();
        let duplicate = document["@graph"][0].clone();
        document["@graph"].as_array_mut().unwrap().push(duplicate);
        let report = validate_document(&document);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("duplicate @id")),
            "errors: {:?}",
            report.errors
        );

        for malformed in [
            Value::Null,
            json!({}),
            json!({"@graph": "not an array"}),
            json!({"@context": {}, "ontologyVersion": "", "bindingVersion": "", "registrySha256": "", "@graph": []}),
        ] {
            let report = validate_document(&malformed);
            assert!(!report.ok);
        }
    }

    #[test]
    fn run_requires_exactly_one_task_and_one_outcome() {
        let mut document = valid_document();
        let run = node_mut(&mut document, RUN);
        run.remove("hasTask");
        run.remove("hasOutcome");
        let report = validate_document(&document);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|error| error.contains("hasTask")));
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("hasOutcome"))
        );
    }
}
