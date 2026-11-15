//! Read-only JSON-LD projection of journals and sessions.
//!
//! The exporter only reads existing files. It never creates a session and never
//! writes to a journal. Raw prompts, tool arguments, tool content and page text
//! are excluded; prose fields are truncated to [`MAX_SUMMARY_CHARS`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::domain::{
    AgentAnswer, AnswerStatus, ClaimStatus, StopReason, TaskRequest, ToolCall, ToolObservation,
    ToolStatus,
};
use crate::journal::{JournalEvent, JournalEventKind, read_events};
use crate::tui_session::{SessionMission, TuiSession};

use super::{ExportDocument, OntologyError, ids};

/// Maximum number of characters exported for a prose summary field.
pub const MAX_SUMMARY_CHARS: usize = 500;

pub fn export_session(
    session_root: &Path,
    session_id: &str,
) -> Result<ExportDocument, OntologyError> {
    let sessions = TuiSession::list(session_root)
        .map_err(|error| OntologyError::Session(error.to_string()))?;
    if !sessions.iter().any(|session| session.id == session_id) {
        return Err(OntologyError::Session(format!(
            "session {session_id} does not exist"
        )));
    }
    let session = TuiSession::open(session_root, Some(session_id))
        .map_err(|error| OntologyError::Session(error.to_string()))?;

    let session_iri = ids::session_id(session.id());
    let mut nodes = BTreeMap::new();
    let mut run_iris = BTreeSet::new();
    for journal_path in session.journals() {
        let events = read_journal(journal_path)?;
        if let Some(run_iri) = build_run(&events, Some(&session_iri), &mut nodes)? {
            run_iris.insert(run_iri);
        }
    }

    let mut session_object = Map::new();
    session_object.insert("@id".into(), Value::String(session_iri.clone()));
    session_object.insert("@type".into(), Value::String("Session".into()));
    session_object.insert(
        "titleSha256".into(),
        Value::String(super::sha256_hex(session.title().as_bytes())),
    );
    session_object.insert(
        "titleChars".into(),
        Value::from(session.title().chars().count() as u64),
    );
    session_object.insert(
        "status".into(),
        Value::String(
            if session.archived() {
                "archived"
            } else {
                "active"
            }
            .into(),
        ),
    );
    session_object.insert("hasRun".into(), refs_array(run_iris.iter().cloned()));
    if let Some(parent) = session.forked_from_session_id() {
        let parent_iri = ids::session_id(parent);
        session_object.insert("forkedFrom".into(), ref_iri(&parent_iri));
        // Keep the reference closed without recursively exporting the parent's
        // runs. The stub is a real Session identity, not a guessed entity.
        nodes.entry(parent_iri.clone()).or_insert_with(|| {
            json!({
                "@id": parent_iri,
                "@type": "Session",
            })
        });
    }

    if let Some(mission) = session.mission() {
        let goal_iri = ids::goal_id(&mission.id);
        session_object.insert("hasGoal".into(), ref_iri(&goal_iri));
        nodes.insert(goal_iri, goal_node(mission, &run_iris));
    }
    nodes.insert(session_iri, Value::Object(session_object));

    finish(nodes)
}

pub fn export_journal(journal_path: &Path) -> Result<ExportDocument, OntologyError> {
    let events = read_journal(journal_path)?;
    if events.is_empty() {
        return Err(OntologyError::Journal(format!(
            "{} contains no events",
            journal_path.display()
        )));
    }
    let mut nodes = BTreeMap::new();
    build_run(&events, None, &mut nodes)?;
    finish(nodes)
}

fn build_run(
    events: &[JournalEvent],
    part_of_session: Option<&str>,
    nodes: &mut BTreeMap<String, Value>,
) -> Result<Option<String>, OntologyError> {
    let Some(first) = events.first() else {
        return Ok(None);
    };
    let run_segment = first.run_id.to_string();
    let run_iri = ids::run_id(&run_segment);
    let task_iri = ids::task_id(&run_segment);

    let mut request: Option<&TaskRequest> = None;
    let mut answer: Option<(&AgentAnswer, u64, u128)> = None;
    let mut stop_reason: Option<&StopReason> = None;
    let mut started_at: Option<u128> = None;
    let mut stopped_at: Option<u128> = None;
    let mut calls: Vec<(&ToolCall, u128)> = Vec::new();
    let mut observations: Vec<(&ToolObservation, u128)> = Vec::new();
    let mut route: Option<(String, String, String)> = None;
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut cost_micros = 0u64;
    let mut checkpoint_seq: Option<u64> = None;
    let mut resumed_from_checkpoint: Option<u64> = None;
    let mut resumed_from_run: Option<String> = None;

    for event in events {
        match &event.event {
            JournalEventKind::RunStarted {
                request: started, ..
            } => {
                if request.is_none() {
                    request = Some(started);
                    started_at = Some(event.timestamp_unix_ms);
                }
            }
            JournalEventKind::ResumeStarted {
                original_run_id,
                request: resumed,
                checkpoint_seq: resumed_seq,
                ..
            } => {
                if request.is_none() {
                    request = Some(resumed);
                }
                resumed_from_checkpoint = Some(*resumed_seq);
                resumed_from_run = Some(original_run_id.to_string());
            }
            JournalEventKind::RouteSelected {
                provider_id,
                model,
                reason,
            } => {
                route = Some((provider_id.clone(), model.clone(), reason.clone()));
            }
            JournalEventKind::ModelResponded { response } => {
                input_tokens = input_tokens.saturating_add(response.usage.input_tokens);
                output_tokens = output_tokens.saturating_add(response.usage.output_tokens);
                cost_micros = cost_micros.saturating_add(response.usage.cost_micros);
                if route.is_none() {
                    route = Some((
                        response.provider_id.clone(),
                        response.model.clone(),
                        "model_response".into(),
                    ));
                }
            }
            JournalEventKind::Checkpoint { .. } => {
                checkpoint_seq = Some(event.seq);
            }
            JournalEventKind::CheckpointDelta { .. } => {
                checkpoint_seq = Some(event.seq);
            }
            JournalEventKind::ToolCalled { call } => {
                calls.push((call, event.timestamp_unix_ms));
            }
            JournalEventKind::ToolObserved { observation } => {
                observations.push((observation, event.timestamp_unix_ms));
            }
            JournalEventKind::AnswerProduced { answer: produced } => {
                answer = Some((produced, event.seq, event.timestamp_unix_ms));
            }
            JournalEventKind::RunStopped { reason } => {
                stop_reason = Some(reason);
                stopped_at = Some(event.timestamp_unix_ms);
            }
            _ => {}
        }
    }

    let (mut artifacts, observation_artifacts) = collect_artifacts(&run_segment, &observations);

    let mut tool_call_iris = BTreeSet::new();
    let called: BTreeSet<&str> = calls.iter().map(|(call, _)| call.id.as_str()).collect();
    for (call, timestamp) in &calls {
        let invocation_iri = ids::tool_call_id(&run_segment, &call.id);
        tool_call_iris.insert(invocation_iri.clone());
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(invocation_iri.clone()));
        object.insert("@type".into(), Value::String("ToolInvocation".into()));
        object.insert("name".into(), Value::String(bounded(&call.name)));
        object.insert("calledAtUnixMs".into(), unix_ms_value(*timestamp));
        if observations
            .iter()
            .any(|(observation, _)| observation.call_id == call.id)
        {
            object.insert(
                "hasObservation".into(),
                ref_iri(&ids::observation_id(&run_segment, &call.id)),
            );
        }
        nodes.insert(invocation_iri, Value::Object(object));
    }

    for (observation, timestamp) in &observations {
        let observation_iri = ids::observation_id(&run_segment, &observation.call_id);
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(observation_iri.clone()));
        object.insert("@type".into(), Value::String("Observation".into()));
        object.insert("tool".into(), Value::String(bounded(&observation.tool)));
        object.insert(
            "status".into(),
            Value::String(tool_status_text(observation.status).into()),
        );
        object.insert(
            "summary".into(),
            Value::String(bounded(&observation.summary)),
        );
        object.insert("recordedAtUnixMs".into(), unix_ms_value(*timestamp));
        if called.contains(observation.call_id.as_str()) {
            object.insert(
                "observedBy".into(),
                ref_iri(&ids::tool_call_id(&run_segment, &observation.call_id)),
            );
        }
        if let Some(error_code) = observation.error_code.as_deref() {
            object.insert("errorCode".into(), Value::String(bounded(error_code)));
        }
        if let Some(iris) = observation_artifacts.get(&observation_iri)
            && !iris.is_empty()
        {
            object.insert("hasArtifact".into(), refs_array(iris.clone()));
        }
        nodes.insert(observation_iri, Value::Object(object));
    }

    let mut squad_iris = BTreeSet::new();
    for (observation, _) in &observations {
        if observation.tool != "spawn_agent" {
            continue;
        }
        let Some(child_run_id) = observation.metadata.get("child_run_id") else {
            continue;
        };
        if child_run_id.trim().is_empty() {
            continue;
        }
        let squad_iri = ids::squad_member_id(&run_segment, &observation.call_id);
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(squad_iri.clone()));
        object.insert("@type".into(), Value::String("SquadMember".into()));
        object.insert(
            "status".into(),
            Value::String(bounded(
                observation
                    .metadata
                    .get("status")
                    .map(String::as_str)
                    .unwrap_or("completed"),
            )),
        );
        object.insert("parentRun".into(), ref_iri(&run_iri));
        if let Some(session_iri) = part_of_session {
            object.insert("partOfSession".into(), ref_iri(session_iri));
        }
        object.insert(
            "spawnedBy".into(),
            ref_iri(&ids::tool_call_id(&run_segment, &observation.call_id)),
        );
        object.insert("childRunId".into(), Value::String(bounded(child_run_id)));
        nodes.insert(squad_iri.clone(), Value::Object(object));
        squad_iris.insert(squad_iri);
    }

    for (artifact_iri, artifact) in artifacts.iter_mut() {
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(artifact_iri.clone()));
        object.insert("@type".into(), Value::String("Artifact".into()));
        object.insert("sha256".into(), Value::String(artifact.sha256.clone()));
        if let Some(path) = artifact.paths.iter().next() {
            object.insert("path".into(), Value::String(bounded(path)));
        }
        if let Some(bytes) = artifact.bytes {
            object.insert("bytes".into(), Value::from(bytes));
        }
        if !artifact.generated_by.is_empty() {
            object.insert(
                "wasGeneratedBy".into(),
                refs_array(std::mem::take(&mut artifact.generated_by)),
            );
        }
        nodes.insert(artifact_iri.clone(), Value::Object(object));
    }

    if let Some((answer, seq, timestamp)) = answer {
        let answer_iri = ids::answer_id(&run_segment);
        let claim_iris: Vec<String> = answer
            .claims
            .iter()
            .map(|claim| ids::claim_id(&run_segment, &claim.id))
            .collect();
        let evidence_iris: Vec<String> = answer
            .evidence
            .iter()
            .enumerate()
            .map(|(index, _)| ids::evidence_id(&run_segment, seq, index))
            .collect();

        let mut evidence_for_claim: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (index, evidence) in answer.evidence.iter().enumerate() {
            let evidence_iri = &evidence_iris[index];
            for claim in &evidence.claim_ids {
                evidence_for_claim
                    .entry(ids::claim_id(&run_segment, claim))
                    .or_default()
                    .insert(evidence_iri.clone());
            }
        }

        for claim in &answer.claims {
            let claim_iri = ids::claim_id(&run_segment, &claim.id);
            let mut object = Map::new();
            object.insert("@id".into(), Value::String(claim_iri.clone()));
            object.insert("@type".into(), Value::String("Claim".into()));
            object.insert("claimText".into(), Value::String(bounded(&claim.text)));
            object.insert(
                "claimStatus".into(),
                Value::String(claim_status_text(claim.status).into()),
            );
            if let Some(scope) = claim.scope.as_deref() {
                object.insert("scope".into(), Value::String(bounded(scope)));
            }
            if let Some(cites) = evidence_for_claim.remove(&claim_iri) {
                object.insert("cites".into(), refs_array(cites));
            }
            nodes.insert(claim_iri, Value::Object(object));
        }

        for (index, evidence) in answer.evidence.iter().enumerate() {
            let evidence_iri = ids::evidence_id(&run_segment, seq, index);
            let supports: BTreeSet<String> = evidence
                .claim_ids
                .iter()
                .map(|claim| ids::claim_id(&run_segment, claim))
                .collect();
            let mut object = Map::new();
            object.insert("@id".into(), Value::String(evidence_iri.clone()));
            object.insert("@type".into(), Value::String("Evidence".into()));
            object.insert(
                "sourceKind".into(),
                Value::String(classify_source(&evidence.source).into()),
            );
            object.insert("sourceRef".into(), Value::String(bounded(&evidence.source)));
            if let Some(call_id) = evidence.source.strip_prefix("tool:")
                && observations
                    .iter()
                    .any(|(observation, _)| observation.call_id == call_id)
            {
                object.insert(
                    "observedBy".into(),
                    ref_iri(&ids::observation_id(&run_segment, call_id)),
                );
            }
            object.insert(
                "statement".into(),
                Value::String(bounded(&evidence.statement)),
            );
            object.insert("supportsClaim".into(), refs_array(supports));
            if let Some(observed) = evidence.observed_unix_ms {
                object.insert("observedAtUnixMs".into(), Value::from(observed));
            }
            if let Some(expires) = evidence.expires_unix_ms {
                object.insert("expiresAtUnixMs".into(), Value::from(expires));
            }
            object.insert("recordedAtUnixMs".into(), unix_ms_value(timestamp));
            nodes.insert(evidence_iri, Value::Object(object));
        }

        let mut object = Map::new();
        object.insert("@id".into(), Value::String(answer_iri.clone()));
        object.insert("@type".into(), Value::String("Answer".into()));
        object.insert(
            "status".into(),
            Value::String(answer_status_text(answer.status).into()),
        );
        object.insert(
            "conclusion".into(),
            Value::String(bounded(&answer.conclusion)),
        );
        object.insert("recordedAtUnixMs".into(), unix_ms_value(timestamp));
        object.insert(
            "hasClaim".into(),
            refs_array(claim_iris.into_iter().collect::<BTreeSet<_>>()),
        );
        object.insert(
            "hasEvidence".into(),
            refs_array(evidence_iris.into_iter().collect::<BTreeSet<_>>()),
        );
        if !answer.unknowns.is_empty() {
            object.insert(
                "unknowns".into(),
                Value::Array(
                    answer
                        .unknowns
                        .iter()
                        .map(|unknown| {
                            json!({
                                "id": bounded(&unknown.id),
                                "description": bounded(&unknown.description),
                                "impact": format!("{:?}", unknown.impact).to_lowercase(),
                            })
                        })
                        .collect(),
                ),
            );
        }
        if !answer.flip_conditions.is_empty() {
            object.insert(
                "flipConditions".into(),
                Value::Array(
                    answer
                        .flip_conditions
                        .iter()
                        .map(|condition| {
                            json!({
                                "ifObserved": bounded(&condition.if_observed),
                                "then": bounded(&condition.then),
                                "targetIds": condition.target_ids
                                    .iter()
                                    .map(|id| bounded(id))
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect(),
                ),
            );
        }
        nodes.insert(answer_iri, Value::Object(object));
    }

    let mut run_object = Map::new();
    run_object.insert("@id".into(), Value::String(run_iri.clone()));
    run_object.insert("@type".into(), Value::String("Run".into()));
    if let Some(session_iri) = part_of_session {
        run_object.insert("partOfSession".into(), ref_iri(session_iri));
    }
    if request.is_some() {
        run_object.insert("hasTask".into(), ref_iri(&task_iri));
    }
    if let Some(reason) = stop_reason {
        run_object.insert(
            "hasOutcome".into(),
            Value::String(stop_reason_text(reason).into()),
        );
    }
    if !tool_call_iris.is_empty() {
        run_object.insert("used".into(), refs_array(tool_call_iris));
    }
    if !squad_iris.is_empty() {
        run_object.insert("hasSquadMember".into(), refs_array(squad_iris));
    }
    if answer.is_some() {
        run_object.insert("generated".into(), ref_iri(&ids::answer_id(&run_segment)));
    }
    if let Some(started_at) = started_at {
        run_object.insert("startedAtUnixMs".into(), unix_ms_value(started_at));
    }
    if let Some(stopped_at) = stopped_at {
        run_object.insert("completedAtUnixMs".into(), unix_ms_value(stopped_at));
    }
    if let Some((provider_id, model, reason)) = route {
        run_object.insert("providerId".into(), Value::String(bounded(&provider_id)));
        run_object.insert("model".into(), Value::String(bounded(&model)));
        run_object.insert("routeReason".into(), Value::String(bounded(&reason)));
    }
    if input_tokens > 0 || output_tokens > 0 || cost_micros > 0 {
        run_object.insert("inputTokens".into(), Value::from(input_tokens));
        run_object.insert("outputTokens".into(), Value::from(output_tokens));
        run_object.insert("costMicros".into(), Value::from(cost_micros));
    }
    if let Some(seq) = checkpoint_seq {
        run_object.insert("checkpointSeq".into(), Value::from(seq));
    }
    if let Some(seq) = resumed_from_checkpoint {
        run_object.insert("resumedFromCheckpointSeq".into(), Value::from(seq));
    }
    if let Some(original) = resumed_from_run {
        // The original journal is not always in the same export. Keep this as
        // an identity string, not a dangling @id reference.
        run_object.insert("resumedFromRunId".into(), Value::String(original));
    }
    nodes.insert(run_iri.clone(), Value::Object(run_object));

    if let Some(request) = request {
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(task_iri.clone()));
        object.insert("@type".into(), Value::String("Task".into()));
        object.insert(
            "objectiveSha256".into(),
            Value::String(super::sha256_hex(request.objective.as_bytes())),
        );
        object.insert(
            "objectiveChars".into(),
            Value::from(request.objective.chars().count() as u64),
        );
        if let Some(deliverable) = request.deliverable.as_deref()
            && !deliverable.trim().is_empty()
        {
            object.insert(
                "deliverableSha256".into(),
                Value::String(super::sha256_hex(deliverable.as_bytes())),
            );
        }
        object.insert(
            "hasPermission".into(),
            json!({
                "@type": "PermissionSet",
                "allowNetwork": request.permissions.allow_network,
                "allowWrite": request.permissions.allow_write,
            }),
        );
        nodes.insert(task_iri, Value::Object(object));
    }

    Ok(Some(run_iri))
}

struct ArtifactRecord {
    sha256: String,
    paths: BTreeSet<String>,
    bytes: Option<u64>,
    generated_by: BTreeSet<String>,
}

fn collect_artifacts(
    run_segment: &str,
    observations: &[(&ToolObservation, u128)],
) -> (
    BTreeMap<String, ArtifactRecord>,
    BTreeMap<String, BTreeSet<String>>,
) {
    let mut artifacts: BTreeMap<String, ArtifactRecord> = BTreeMap::new();
    let mut observation_artifacts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (observation, _) in observations {
        let Some(sha256) = observation.metadata.get("sha256") else {
            continue;
        };
        if sha256.trim().is_empty() {
            continue;
        }
        let artifact_iri = ids::artifact_id(sha256);
        let artifact = artifacts
            .entry(artifact_iri.clone())
            .or_insert_with(|| ArtifactRecord {
                sha256: sha256.clone(),
                paths: BTreeSet::new(),
                bytes: None,
                generated_by: BTreeSet::new(),
            });
        if let Some(path) = observation
            .metadata
            .get("path")
            .or_else(|| observation.metadata.get("relative_path"))
            && !path.trim().is_empty()
        {
            artifact.paths.insert(path.clone());
        }
        if artifact.bytes.is_none() {
            artifact.bytes = observation
                .metadata
                .get("bytes")
                .and_then(|value| value.parse::<u64>().ok());
        }
        let observation_iri = ids::observation_id(run_segment, &observation.call_id);
        artifact.generated_by.insert(observation_iri.clone());
        observation_artifacts
            .entry(observation_iri)
            .or_default()
            .insert(artifact_iri);
    }
    (artifacts, observation_artifacts)
}

fn goal_node(mission: &SessionMission, run_iris: &BTreeSet<String>) -> Value {
    json!({
        "@id": ids::goal_id(&mission.id),
        "@type": "Goal",
        "goalSha256": super::sha256_hex(mission.goal.as_bytes()),
        "goalChars": mission.goal.chars().count() as u64,
        "status": "active",
        "revision": mission.revision,
        "hasRun": refs_array(run_iris.iter().cloned()),
    })
}

fn finish(nodes: BTreeMap<String, Value>) -> Result<ExportDocument, OntologyError> {
    let value = json!({
        "@context": {
            "@version": 1.1,
            "@vocab": "https://1agent.dev/ontology/0.98#",
            "prov": "http://www.w3.org/ns/prov#",
            "xsd": "http://www.w3.org/2001/XMLSchema#"
        },
        "ontologyVersion": super::ONTOLOGY_VERSION,
        "bindingVersion": super::BINDING_VERSION,
        "registrySha256": super::registry_sha256(),
        "@graph": nodes.into_values().collect::<Vec<_>>(),
    });
    let bytes =
        serde_json::to_vec(&value).map_err(|error| OntologyError::Json(error.to_string()))?;
    Ok(ExportDocument {
        value,
        sha256: super::sha256_hex(&bytes),
    })
}

fn read_journal(path: &Path) -> Result<Vec<JournalEvent>, OntologyError> {
    read_events(path)
        .map_err(|error| OntologyError::Journal(format!("{}: {error}", path.display())))
}

fn bounded(value: &str) -> String {
    value.chars().take(MAX_SUMMARY_CHARS).collect()
}

fn unix_ms_value(value: u128) -> Value {
    Value::from(u64::try_from(value).unwrap_or(u64::MAX))
}

fn ref_iri(id: &str) -> Value {
    json!({ "@id": id })
}

fn refs_array(ids: impl IntoIterator<Item = String>) -> Value {
    Value::Array(ids.into_iter().map(|id| ref_iri(&id)).collect())
}

/// Contract §5 source classification, in priority order.
fn classify_source(source: &str) -> &'static str {
    if source.starts_with("http://") || source.starts_with("https://") {
        "web"
    } else if source.starts_with("tool:") {
        "tool"
    } else if source.starts_with("model:") {
        "model_prior"
    } else if source.contains('/') {
        "file"
    } else {
        "unknown"
    }
}

fn answer_status_text(status: AnswerStatus) -> &'static str {
    match status {
        AnswerStatus::Completed => "completed",
        AnswerStatus::Conditional => "conditional",
        AnswerStatus::NeedsEvidence => "needs_evidence",
        AnswerStatus::NeedsAuthorization => "needs_authorization",
        AnswerStatus::Failed => "failed",
    }
}

fn claim_status_text(status: ClaimStatus) -> &'static str {
    match status {
        ClaimStatus::Verified => "verified",
        ClaimStatus::UserAsserted => "user_asserted",
        ClaimStatus::Derived => "derived",
        ClaimStatus::Assumed => "assumed",
        ClaimStatus::ModelPrior => "model_prior",
        ClaimStatus::Conflicting => "conflicting",
        ClaimStatus::Unknown => "unknown",
    }
}

fn tool_status_text(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Ok => "ok",
        ToolStatus::InvalidArguments => "invalid_arguments",
        ToolStatus::PermissionDenied => "permission_denied",
        ToolStatus::TimedOut => "timed_out",
        ToolStatus::Failed => "failed",
    }
}

fn stop_reason_text(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Completed => "completed",
        StopReason::NeedsEvidence => "needs_evidence",
        StopReason::NeedsAuthorization => "needs_authorization",
        StopReason::BudgetExhausted => "budget_exhausted",
        StopReason::Cancelled => "cancelled",
        StopReason::ProviderError => "provider_error",
        StopReason::ToolFailure => "tool_failure",
        StopReason::InvalidOutput => "invalid_output",
        StopReason::InternalError => "internal_error",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::domain::{
        AgentAnswer, AnswerStatus, Claim, ClaimStatus, CognitiveMode, Evidence, StopReason,
        TaskRequest, ToolCall, ToolObservation, ToolStatus,
    };
    use crate::journal::Journal;
    use crate::ontology::validate::validate_document;
    use crate::tui_session::TuiSession;

    use super::{export_journal, export_session};

    const RAW_CONTENT: &str = "SENSITIVE_RAW_TOOL_CONTENT";

    fn graph_ids(document: &Value) -> Vec<String> {
        document["@graph"]
            .as_array()
            .expect("graph")
            .iter()
            .filter_map(|node| node["@id"].as_str().map(ToOwned::to_owned))
            .collect()
    }

    fn node<'a>(document: &'a Value, id: &str) -> &'a Value {
        document["@graph"]
            .as_array()
            .expect("graph")
            .iter()
            .find(|node| node["@id"] == id)
            .unwrap_or_else(|| panic!("missing node {id}"))
    }

    fn synthetic_session(
        workspace: &std::path::Path,
        journal_path: &std::path::Path,
    ) -> (String, Uuid, String) {
        let mut session = TuiSession::open(workspace, None).unwrap();
        let session_id = session.id().to_string();
        session.rename("Ontology export test").unwrap();
        let mission_id = session
            .start_mission("Ship the ontology export", workspace.to_path_buf())
            .unwrap()
            .id
            .clone();

        let run_id = Uuid::new_v4();
        let mut journal = Journal::create(journal_path, run_id).unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("Investigate A", workspace.to_path_buf()),
                mode: CognitiveMode::EvidenceResearch,
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::ToolCalled {
                call: ToolCall {
                    id: "call-1".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "secret.txt", "content": RAW_CONTENT}),
                },
            })
            .unwrap();
        let sha256 = "ab".repeat(32);
        journal
            .append(crate::journal::JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "call-1".into(),
                    tool: "read_file".into(),
                    status: ToolStatus::Ok,
                    summary: "read secret.txt".into(),
                    content: RAW_CONTENT.into(),
                    error_code: None,
                    metadata: BTreeMap::from([
                        ("sha256".to_string(), sha256.clone()),
                        ("path".to_string(), "report.md".to_string()),
                        ("bytes".to_string(), "12".to_string()),
                    ]),
                },
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::AnswerProduced {
                answer: AgentAnswer {
                    schema_version: 2,
                    status: AnswerStatus::Completed,
                    conclusion: "The export is deterministic".into(),
                    claims: vec![Claim {
                        id: "c1".into(),
                        text: "Export bytes are stable".into(),
                        status: ClaimStatus::Verified,
                        sources: vec!["tool:call-1".into()],
                        scope: None,
                    }],
                    evidence: vec![Evidence {
                        source: "tool:call-1".into(),
                        statement: "Two exports have the same hash".into(),
                        observed_unix_ms: Some(1_700_000_000_000),
                        expires_unix_ms: Some(1_700_000_600_000),
                        claim_ids: vec!["c1".into()],
                    }],
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                    task_need: None,
                },
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(journal);

        session.record_journal(journal_path).unwrap();
        (session_id, run_id, mission_id)
    }

    #[test]
    fn export_session_is_deterministic_redacted_and_valid() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_path = workspace.path().join("run.jsonl");
        let (session_id, run_id, mission_id) = synthetic_session(workspace.path(), &journal_path);
        let run = run_id.to_string();

        let first = export_session(workspace.path(), &session_id).unwrap();
        let second = export_session(workspace.path(), &session_id).unwrap();
        assert_eq!(first.value, second.value);
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(
            serde_json::to_vec(&first.value).unwrap(),
            serde_json::to_vec(&second.value).unwrap()
        );
        assert_eq!(
            first.sha256,
            super::super::sha256_hex(&serde_json::to_vec(&first.value).unwrap())
        );

        let ids = graph_ids(&first.value);
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "@graph must be sorted by @id");

        for expected in [
            format!("urn:1agent:session:{session_id}"),
            format!("urn:1agent:run:{run}"),
            format!("urn:1agent:task:{run}:main"),
            format!("urn:1agent:tool-call:{run}:call-1"),
            format!("urn:1agent:observation:{run}:call-1"),
            format!("urn:1agent:answer:{run}"),
            format!("urn:1agent:claim:{run}:c1"),
            format!("urn:1agent:evidence:{run}:3:0"),
            format!("urn:1agent:artifact:sha256:{}", "ab".repeat(32)),
            format!("urn:1agent:goal:{mission_id}"),
        ] {
            assert!(ids.contains(&expected), "missing {expected}");
        }

        let evidence = node(&first.value, &format!("urn:1agent:evidence:{run}:3:0"));
        assert_eq!(evidence["sourceKind"], "tool");
        assert_eq!(
            evidence["supportsClaim"][0]["@id"],
            format!("urn:1agent:claim:{run}:c1")
        );
        let claim = node(&first.value, &format!("urn:1agent:claim:{run}:c1"));
        assert_eq!(claim["cites"][0]["@id"], evidence["@id"]);
        let run_node = node(&first.value, &format!("urn:1agent:run:{run}"));
        assert_eq!(run_node["hasOutcome"], "completed");
        let goal = node(&first.value, &format!("urn:1agent:goal:{mission_id}"));
        assert_eq!(goal["hasRun"][0]["@id"], format!("urn:1agent:run:{run}"));

        let text = serde_json::to_string(&first.value).unwrap();
        assert!(!text.contains("\"content\""));
        assert!(!text.contains("\"arguments\""));
        assert!(!text.contains(RAW_CONTENT));
        assert!(!text.contains("Investigate A"));
        assert!(!text.contains("Ship the ontology export"));
        assert!(!text.contains("Ontology export test"));
        assert!(!text.contains("\"generatedAt\""));

        let report = validate_document(&first.value);
        assert!(report.ok, "validator errors: {:?}", report.errors);
    }

    #[test]
    fn validator_catches_broken_claim_and_evidence_in_an_export() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_path = workspace.path().join("run.jsonl");
        let (session_id, run_id, _) = synthetic_session(workspace.path(), &journal_path);
        let run = run_id.to_string();
        let mut document = export_session(workspace.path(), &session_id).unwrap().value;

        let evidence_iri = format!("urn:1agent:evidence:{run}:3:0");
        let claim_iri = format!("urn:1agent:claim:{run}:c1");
        for entry in document["@graph"].as_array_mut().unwrap() {
            let id = entry
                .get("@id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            if id.as_deref() == Some(evidence_iri.as_str()) {
                entry["supportsClaim"] =
                    json!([{"@id": format!("urn:1agent:claim:{run}:missing")}]);
            }
            if id.as_deref() == Some(claim_iri.as_str()) {
                entry.as_object_mut().unwrap().remove("claimStatus");
            }
        }

        let report = validate_document(&document);
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("supportsClaim")),
            "errors: {:?}",
            report.errors
        );
        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("claimStatus")),
            "errors: {:?}",
            report.errors
        );
    }

    #[test]
    fn successful_spawn_observation_exports_squad_lineage() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_path = workspace.path().join("spawn.jsonl");
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let session_id = session.id().to_string();
        let run_id = Uuid::new_v4();
        let mut journal = Journal::create(&journal_path, run_id).unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("parent task", workspace.path().to_path_buf()),
                mode: CognitiveMode::Execute,
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::ToolCalled {
                call: ToolCall {
                    id: "spawn-1".into(),
                    name: "spawn_agent".into(),
                    arguments: json!({"task": "child"}),
                },
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "spawn-1".into(),
                    tool: "spawn_agent".into(),
                    status: ToolStatus::Ok,
                    summary: "队员完成".into(),
                    content: "{}".into(),
                    error_code: None,
                    metadata: BTreeMap::from([
                        ("status".to_string(), "completed".to_string()),
                        ("child_run_id".to_string(), "child-run-1".to_string()),
                    ]),
                },
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(journal);
        session.record_journal(&journal_path).unwrap();

        let document = export_session(workspace.path(), &session_id).unwrap();
        let suffix = format!("urn:1agent:squad-member:{run_id}:spawn-1");
        let squad = node(&document.value, &suffix);
        assert_eq!(squad["@type"], "SquadMember");
        assert_eq!(squad["status"], "completed");
        assert_eq!(squad["childRunId"], "child-run-1");
        assert_eq!(
            squad["spawnedBy"]["@id"],
            format!("urn:1agent:tool-call:{run_id}:spawn-1")
        );
        assert_eq!(
            squad["parentRun"]["@id"],
            format!("urn:1agent:run:{run_id}")
        );
        let report = validate_document(&document.value);
        assert!(report.ok, "validator errors: {:?}", report.errors);
    }

    #[test]
    fn export_journal_is_standalone_without_part_of_session() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_path = workspace.path().join("run.jsonl");
        let (_, run_id, _) = synthetic_session(workspace.path(), &journal_path);

        let export = export_journal(&journal_path).unwrap();
        let run = node(&export.value, &format!("urn:1agent:run:{run_id}"));
        assert!(run.get("partOfSession").is_none());
        assert!(
            graph_ids(&export.value)
                .iter()
                .all(|id| !id.starts_with("urn:1agent:session:"))
        );
        let again = export_journal(&journal_path).unwrap();
        assert_eq!(export.sha256, again.sha256);
        assert!(validate_document(&export.value).ok);
    }

    #[test]
    fn export_session_does_not_create_a_missing_session() {
        let workspace = tempfile::tempdir().unwrap();
        let error = export_session(workspace.path(), "ses_missing").unwrap_err();
        assert!(matches!(error, crate::ontology::OntologyError::Session(_)));
        assert!(
            !workspace
                .path()
                .join(".sessions")
                .join("ses_missing")
                .exists()
        );
    }
}
