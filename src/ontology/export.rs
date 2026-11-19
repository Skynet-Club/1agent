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
use crate::goal::{CommitmentStatus, GoalEvent, GoalGraph, GoalStatus};
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
        let goal_iri = ids::goal_id(mission.goal_id());
        session_object.insert("hasGoal".into(), ref_iri(&goal_iri));
        nodes.insert(goal_iri, goal_node(mission, &run_iris));
    }
    nodes.insert(session_iri, Value::Object(session_object));

    finish(nodes)
}

/// Project the hash-chained goal log without modifying it. Goal titles and
/// commitment descriptions are exported as SHA plus character count only.
pub fn export_goal_log(path: &Path) -> Result<ExportDocument, OntologyError> {
    const MAX_GOAL_LOG_BYTES: u64 = 16 * 1024 * 1024;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| OntologyError::Io(format!("{}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(OntologyError::Io(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > MAX_GOAL_LOG_BYTES {
        return Err(OntologyError::Io(format!(
            "{} exceeds {MAX_GOAL_LOG_BYTES} bytes",
            path.display()
        )));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|error| OntologyError::Io(format!("{}: {error}", path.display())))?;
    let mut events = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        events.push(serde_json::from_str::<GoalEvent>(line).map_err(|error| {
            OntologyError::Json(format!("goal log {}: {error}", path.display()))
        })?);
    }
    let graph = GoalGraph::replay(&events)
        .map_err(|error| OntologyError::Validation(format!("goal log: {error}")))?;
    let snapshot = graph.snapshot();
    let mut commitments_by_goal: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for commitment in &snapshot.commitments {
        commitments_by_goal
            .entry(commitment.goal_id.clone())
            .or_default()
            .insert(ids::commitment_id(&commitment.commitment_id));
    }

    let mut nodes = BTreeMap::new();
    for goal in &snapshot.goals {
        let goal_iri = ids::goal_id(&goal.goal_id);
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(goal_iri.clone()));
        object.insert("@type".into(), Value::String("Goal".into()));
        object.insert(
            "status".into(),
            Value::String(goal_status_text(goal.status).into()),
        );
        object.insert("ownerId".into(), Value::String(bounded(&goal.owner_id)));
        object.insert(
            "titleSha256".into(),
            Value::String(super::sha256_hex(goal.title.as_bytes())),
        );
        object.insert(
            "titleChars".into(),
            Value::from(goal.title.chars().count() as u64),
        );
        if let Some(due_ms) = goal.due_ms {
            object.insert("dueUnixMs".into(), unix_ms_value(due_ms));
        }
        object.insert("createdUnixMs".into(), unix_ms_value(goal.created_ms));
        object.insert("updatedUnixMs".into(), unix_ms_value(goal.updated_ms));
        if let Some(commitments) = commitments_by_goal.get(&goal.goal_id)
            && !commitments.is_empty()
        {
            object.insert("hasCommitment".into(), refs_array(commitments.clone()));
        }
        if let Some(binding) = snapshot
            .bindings
            .iter()
            .find(|binding| binding.goal_id == goal.goal_id)
        {
            if let Some(session_id) = &binding.session_id {
                let session_iri = ids::session_id(session_id);
                object.insert("partOfSession".into(), ref_iri(&session_iri));
                nodes.entry(session_iri.clone()).or_insert_with(|| {
                    json!({
                        "@id": session_iri,
                        "@type": "Session",
                    })
                });
            }
            if !binding.run_ids.is_empty() {
                object.insert(
                    "runIds".into(),
                    Value::Array(
                        binding
                            .run_ids
                            .iter()
                            .map(|run_id| Value::String(bounded(run_id)))
                            .collect(),
                    ),
                );
            }
        }
        nodes.insert(goal_iri, Value::Object(object));
    }
    for commitment in &snapshot.commitments {
        let commitment_iri = ids::commitment_id(&commitment.commitment_id);
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(commitment_iri.clone()));
        object.insert("@type".into(), Value::String("Commitment".into()));
        object.insert(
            "status".into(),
            Value::String(commitment_status_text(commitment.status).into()),
        );
        object.insert(
            "descriptionSha256".into(),
            Value::String(super::sha256_hex(commitment.description.as_bytes())),
        );
        object.insert(
            "descriptionChars".into(),
            Value::from(commitment.description.chars().count() as u64),
        );
        if let Some(due_ms) = commitment.due_ms {
            object.insert("dueUnixMs".into(), unix_ms_value(due_ms));
        }
        object.insert("createdUnixMs".into(), unix_ms_value(commitment.created_ms));
        object.insert("updatedUnixMs".into(), unix_ms_value(commitment.updated_ms));
        if let Some(closed_ms) = commitment.closed_ms {
            object.insert("closedUnixMs".into(), unix_ms_value(closed_ms));
        }
        object.insert(
            "belongsToGoal".into(),
            ref_iri(&ids::goal_id(&commitment.goal_id)),
        );
        if let Some(ticket_id) = &commitment.ticket_id {
            object.insert("ticketId".into(), Value::String(bounded(ticket_id)));
        }
        nodes.insert(commitment_iri, Value::Object(object));
    }
    finish(nodes)
}

fn goal_status_text(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Active => "active",
        GoalStatus::Paused => "paused",
        GoalStatus::Achieved => "achieved",
        GoalStatus::Abandoned => "abandoned",
    }
}

fn commitment_status_text(status: CommitmentStatus) -> &'static str {
    match status {
        CommitmentStatus::Pending => "pending",
        CommitmentStatus::Kept => "kept",
        CommitmentStatus::Missed => "missed",
        CommitmentStatus::Cancelled => "cancelled",
    }
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
    let mut client_request_id: Option<String> = None;
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
            JournalEventKind::ClientRequestLinked {
                client_request_id: id,
            } => {
                client_request_id = Some(id.clone());
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

    let mut action_iris = BTreeSet::new();
    for (observation, _) in &observations {
        let has_action_metadata = [
            "action_id",
            "policy_outcome",
            "policy_reason",
            "authority_profile_id",
            "approval_card_id",
            "approval_id",
            "risk",
            "reversible",
        ]
        .iter()
        .any(|key| observation.metadata.contains_key(*key));
        if !has_action_metadata {
            continue;
        }
        let observation_iri = ids::observation_id(&run_segment, &observation.call_id);
        let action_iri = ids::action_invocation_id(&run_segment, &observation.call_id);
        action_iris.insert(action_iri.clone());
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(action_iri.clone()));
        object.insert("@type".into(), Value::String("ActionInvocation".into()));
        object.insert(
            "status".into(),
            Value::String(
                crate::runtime::action_status_for(observation)
                    .as_str()
                    .into(),
            ),
        );
        object.insert(
            "actionType".into(),
            Value::String(bounded(&observation.tool)),
        );
        object.insert("partOfRun".into(), ref_iri(&run_iri));
        if called.contains(observation.call_id.as_str()) {
            object.insert(
                "hasToolCall".into(),
                ref_iri(&ids::tool_call_id(&run_segment, &observation.call_id)),
            );
        }
        object.insert("hasObservation".into(), ref_iri(&observation_iri));
        if let Some(risk) = observation.metadata.get("risk") {
            object.insert("risk".into(), Value::String(bounded(risk)));
        }
        if let Some(reversible) =
            observation
                .metadata
                .get("reversible")
                .and_then(|value| match value.as_str() {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                })
        {
            object.insert("reversible".into(), Value::Bool(reversible));
        }
        if let Some(outcome) = observation.metadata.get("policy_outcome") {
            let decision_iri = ids::policy_decision_id(&run_segment, &observation.call_id);
            let mut decision = Map::new();
            decision.insert("@id".into(), Value::String(decision_iri.clone()));
            decision.insert("@type".into(), Value::String("PolicyDecision".into()));
            decision.insert("outcome".into(), Value::String(bounded(outcome)));
            if let Some(reason) = observation.metadata.get("policy_reason") {
                decision.insert("reason".into(), Value::String(bounded(reason)));
            }
            if let Some(profile) = observation.metadata.get("authority_profile_id") {
                decision.insert("authorityProfileId".into(), Value::String(bounded(profile)));
            }
            decision.insert("forAction".into(), ref_iri(&action_iri));
            object.insert("hasPolicyDecision".into(), ref_iri(&decision_iri));
            nodes.insert(decision_iri, Value::Object(decision));
        }
        if let Some(card_id) = observation
            .metadata
            .get("approval_card_id")
            .or_else(|| observation.metadata.get("approval_id"))
            .filter(|card_id| !card_id.trim().is_empty())
        {
            let approval_iri = ids::approval_id(&run_segment, &observation.call_id);
            let mut approval = Map::new();
            approval.insert("@id".into(), Value::String(approval_iri.clone()));
            approval.insert("@type".into(), Value::String("Approval".into()));
            approval.insert("status".into(), Value::String("recorded".into()));
            approval.insert("cardId".into(), Value::String(bounded(card_id)));
            approval.insert("forAction".into(), ref_iri(&action_iri));
            object.insert("hasApproval".into(), ref_iri(&approval_iri));
            nodes.insert(approval_iri, Value::Object(approval));
        }
        if let Some(iris) = observation_artifacts.get(&observation_iri)
            && !iris.is_empty()
        {
            object.insert("hasArtifact".into(), refs_array(iris.clone()));
        }
        nodes.insert(action_iri, Value::Object(object));
    }

    let mut squad_iris = BTreeSet::new();
    for (observation, _) in &observations {
        if observation.tool != "spawn_agent" {
            continue;
        }
        // A spawn refused before any child started (depth/fanout) has no
        // status, no child run and failed before a member existed. A timeout
        // still identifies a member attempt even without metadata.
        let has_child_run = observation
            .metadata
            .get("child_run_id")
            .is_some_and(|child| !child.trim().is_empty());
        let is_member_attempt = observation.status == ToolStatus::Ok
            || observation.status == ToolStatus::TimedOut
            || observation.metadata.contains_key("status")
            || has_child_run;
        if !is_member_attempt {
            continue;
        }
        let status = observation
            .metadata
            .get("status")
            .map(String::as_str)
            .unwrap_or_else(|| match observation.status {
                ToolStatus::Ok => "completed",
                ToolStatus::TimedOut => "timed_out",
                ToolStatus::InvalidArguments
                | ToolStatus::PermissionDenied
                | ToolStatus::Failed => "failed",
            });
        let squad_iri = ids::squad_member_id(&run_segment, &observation.call_id);
        let mut object = Map::new();
        object.insert("@id".into(), Value::String(squad_iri.clone()));
        object.insert("@type".into(), Value::String("SquadMember".into()));
        object.insert("status".into(), Value::String(bounded(status)));
        object.insert("parentRun".into(), ref_iri(&run_iri));
        let session_iri = part_of_session.map(str::to_string).or_else(|| {
            observation
                .metadata
                .get("parent_session_id")
                .filter(|session_id| !session_id.trim().is_empty())
                .map(|session_id| ids::session_id(session_id))
        });
        if let Some(session_iri) = session_iri {
            object.insert("partOfSession".into(), ref_iri(&session_iri));
            if part_of_session.is_none() {
                nodes.entry(session_iri.clone()).or_insert_with(|| {
                    json!({
                        "@id": session_iri,
                        "@type": "Session",
                    })
                });
            }
        }
        object.insert(
            "spawnedBy".into(),
            ref_iri(&ids::tool_call_id(&run_segment, &observation.call_id)),
        );
        if let Some(child_run_id) = observation
            .metadata
            .get("child_run_id")
            .filter(|child| !child.trim().is_empty())
        {
            object.insert("childRunId".into(), Value::String(bounded(child_run_id)));
        }
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

    let mut decision_iri = None;
    if let (Some((answer, seq, timestamp)), Some(request)) = (answer, request) {
        let decision = ids::decision_id(&run_segment);
        let outcome = ids::outcome_id(&run_segment);
        let answer_iri = ids::answer_id(&run_segment);
        let claim_iris = answer
            .claims
            .iter()
            .map(|claim| ids::claim_id(&run_segment, &claim.id))
            .collect::<BTreeSet<_>>();
        let evidence_iris = answer
            .evidence
            .iter()
            .enumerate()
            .map(|(index, _)| ids::evidence_id(&run_segment, seq, index))
            .collect::<BTreeSet<_>>();

        let mut decision_object = Map::new();
        decision_object.insert("@id".into(), Value::String(decision.clone()));
        decision_object.insert("@type".into(), Value::String("Decision".into()));
        decision_object.insert(
            "questionSha256".into(),
            Value::String(super::sha256_hex(request.objective.as_bytes())),
        );
        decision_object.insert(
            "questionChars".into(),
            Value::from(request.objective.chars().count() as u64),
        );
        decision_object.insert(
            "status".into(),
            Value::String(answer_status_text(answer.status).into()),
        );
        decision_object.insert("decidedAtUnixMs".into(), unix_ms_value(timestamp));
        decision_object.insert("partOfRun".into(), ref_iri(&run_iri));
        decision_object.insert("hasAnswer".into(), ref_iri(&answer_iri));
        if !claim_iris.is_empty() {
            decision_object.insert("hasClaim".into(), refs_array(claim_iris));
        }
        if !evidence_iris.is_empty() {
            decision_object.insert("hasEvidence".into(), refs_array(evidence_iris));
        }
        decision_object.insert("hasOutcomeRecord".into(), ref_iri(&outcome));
        nodes.insert(decision.clone(), Value::Object(decision_object));

        let mut outcome_object = Map::new();
        outcome_object.insert("@id".into(), Value::String(outcome.clone()));
        outcome_object.insert("@type".into(), Value::String("Outcome".into()));
        outcome_object.insert(
            "status".into(),
            Value::String(
                stop_reason
                    .map(stop_reason_text)
                    .unwrap_or_else(|| answer_status_text(answer.status))
                    .into(),
            ),
        );
        outcome_object.insert(
            "recordedAtUnixMs".into(),
            unix_ms_value(stopped_at.unwrap_or(timestamp)),
        );
        outcome_object.insert("forDecision".into(), ref_iri(&decision));
        if !artifacts.is_empty() {
            outcome_object.insert("hasArtifact".into(), refs_array(artifacts.keys().cloned()));
        }
        nodes.insert(outcome, Value::Object(outcome_object));
        decision_iri = Some(decision);
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
    if !action_iris.is_empty() {
        run_object.insert("hasActionInvocation".into(), refs_array(action_iris));
    }
    if let Some(decision) = &decision_iri {
        run_object.insert("hasDecision".into(), ref_iri(decision));
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
    if let Some(id) = &client_request_id {
        run_object.insert("clientRequestId".into(), Value::String(bounded(id)));
        if let Some(case_id) = id.strip_prefix("eval:") {
            run_object.insert("evalCaseId".into(), Value::String(bounded(case_id)));
        }
    }
    let audit = crate::journal::audit_events(events);
    let replay_ok = crate::journal::replay_events(events).is_ok();
    run_object.insert("auditOk".into(), Value::Bool(audit.ok));
    run_object.insert("replayOk".into(), Value::Bool(replay_ok));
    let issue_codes = audit
        .findings
        .iter()
        .filter_map(|finding| {
            serde_json::to_value(finding.code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
        })
        .collect::<BTreeSet<_>>();
    if !issue_codes.is_empty() {
        run_object.insert(
            "auditIssueCodes".into(),
            Value::Array(
                issue_codes
                    .into_iter()
                    .take(16)
                    .map(Value::String)
                    .collect(),
            ),
        );
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
        "@id": ids::goal_id(mission.goal_id()),
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

    use super::{export_goal_log, export_journal, export_session};
    use crate::goal::{CommitmentStatus, GoalStatus};

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
            format!("urn:1agent:decision:{run}:main"),
            format!("urn:1agent:outcome:{run}"),
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
        let decision = node(&first.value, &format!("urn:1agent:decision:{run}:main"));
        assert_eq!(decision["@type"], "Decision");
        assert_eq!(decision["status"], "completed");
        assert_eq!(
            decision["questionSha256"],
            super::super::sha256_hex("Investigate A".as_bytes())
        );
        assert_eq!(
            run_node["hasDecision"]["@id"],
            format!("urn:1agent:decision:{run}:main")
        );
        let outcome = node(&first.value, &format!("urn:1agent:outcome:{run}"));
        assert_eq!(outcome["@type"], "Outcome");
        assert_eq!(outcome["status"], "completed");
        assert_eq!(
            outcome["forDecision"]["@id"],
            format!("urn:1agent:decision:{run}:main")
        );
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
    fn gated_actions_and_failed_spawns_export_policy_and_squad_nodes() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_path = workspace.path().join("actions.jsonl");
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let session_id = session.id().to_string();
        let run_id = Uuid::new_v4();
        let artifact_sha = "ab".repeat(32);
        let mut journal = Journal::create(&journal_path, run_id).unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("parent task", workspace.path().to_path_buf()),
                mode: CognitiveMode::Execute,
            })
            .unwrap();
        for (call_id, tool, status, error_code, metadata) in [
            (
                "call-denied",
                "write_file",
                ToolStatus::PermissionDenied,
                Some("authority_denied"),
                BTreeMap::from([
                    ("action_id".into(), format!("{run_id}:call-denied")),
                    ("policy_outcome".into(), "deny".into()),
                    (
                        "policy_reason".into(),
                        "tool write_file is not in the profile allowlist".into(),
                    ),
                    ("authority_profile_id".into(), "tool-profile".into()),
                    ("risk".into(), "medium".into()),
                    ("reversible".into(), "false".into()),
                ]),
            ),
            (
                "call-executed",
                "write_file",
                ToolStatus::Ok,
                None,
                BTreeMap::from([
                    ("action_id".into(), format!("{run_id}:call-executed")),
                    ("policy_outcome".into(), "allow".into()),
                    ("policy_reason".into(), "write within profile".into()),
                    ("authority_profile_id".into(), "tool-profile".into()),
                    ("approval_id".into(), "card-1".into()),
                    ("risk".into(), "medium".into()),
                    ("reversible".into(), "false".into()),
                    ("sha256".into(), artifact_sha.clone()),
                ]),
            ),
            (
                "spawn-failed",
                "spawn_agent",
                ToolStatus::Failed,
                Some("tool_failed"),
                BTreeMap::from([
                    ("status".into(), "failed".into()),
                    ("parent_run_id".into(), run_id.to_string()),
                    ("parent_session_id".into(), "ses-parent".into()),
                    ("spawn_call_id".into(), "spawn-failed".into()),
                    ("stop_reason".into(), "needs_authorization".into()),
                ]),
            ),
        ] {
            journal
                .append(crate::journal::JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: call_id.into(),
                        name: tool.into(),
                        arguments: json!({"path": "/tmp/x"}),
                    },
                })
                .unwrap();
            journal
                .append(crate::journal::JournalEventKind::ToolObserved {
                    observation: ToolObservation {
                        call_id: call_id.into(),
                        tool: tool.into(),
                        status,
                        summary: "synthetic".into(),
                        content: "{}".into(),
                        error_code: error_code.map(str::to_string),
                        metadata,
                    },
                })
                .unwrap();
        }
        journal
            .append(crate::journal::JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(journal);
        session.record_journal(&journal_path).unwrap();

        let document = export_session(workspace.path(), &session_id).unwrap();
        let denied = node(
            &document.value,
            &format!("urn:1agent:action:{run_id}:call-denied"),
        );
        assert_eq!(denied["@type"], "ActionInvocation");
        assert_eq!(denied["status"], "denied");
        assert_eq!(denied["actionType"], "write_file");
        assert_eq!(denied["risk"], "medium");
        assert_eq!(denied["reversible"], json!(false));
        let decision = node(
            &document.value,
            &format!("urn:1agent:policy-decision:{run_id}:call-denied"),
        );
        assert_eq!(decision["outcome"], "deny");
        assert_eq!(decision["authorityProfileId"], "tool-profile");
        assert_eq!(
            decision["forAction"]["@id"],
            format!("urn:1agent:action:{run_id}:call-denied")
        );

        let executed = node(
            &document.value,
            &format!("urn:1agent:action:{run_id}:call-executed"),
        );
        assert_eq!(executed["status"], "executed");
        assert_eq!(
            executed["hasArtifact"][0]["@id"],
            format!("urn:1agent:artifact:sha256:{artifact_sha}")
        );
        let approval = node(
            &document.value,
            &format!("urn:1agent:approval:{run_id}:call-executed"),
        );
        assert_eq!(approval["status"], "recorded");
        assert_eq!(approval["cardId"], "card-1");

        let squad = node(
            &document.value,
            &format!("urn:1agent:squad-member:{run_id}:spawn-failed"),
        );
        assert_eq!(squad["@type"], "SquadMember");
        assert_eq!(squad["status"], "failed");
        assert!(squad.get("childRunId").is_none());
        assert_eq!(
            squad["parentRun"]["@id"],
            format!("urn:1agent:run:{run_id}")
        );
        assert_eq!(
            squad["partOfSession"]["@id"],
            crate::ontology::ids::session_id(&session_id)
        );

        let report = validate_document(&document.value);
        assert!(report.ok, "validator errors: {:?}", report.errors);

        // A standalone journal has no exported session, but the failed spawn
        // carries the parent session id, so it is kept as a Session stub.
        let standalone = export_journal(&journal_path).unwrap();
        let standalone_squad = node(
            &standalone.value,
            &format!("urn:1agent:squad-member:{run_id}:spawn-failed"),
        );
        assert_eq!(
            standalone_squad["partOfSession"]["@id"],
            "urn:1agent:session:ses-parent"
        );
        assert!(validate_document(&standalone.value).ok);
    }

    #[test]
    fn run_exports_eval_case_id_and_audit_status() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_path = workspace.path().join("eval.jsonl");
        let run_id = Uuid::new_v4();
        let mut journal = Journal::create(&journal_path, run_id).unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStarted {
                request: TaskRequest::from_objective("eval case", workspace.path().to_path_buf()),
                mode: CognitiveMode::Execute,
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::ClientRequestLinked {
                client_request_id: "eval:case-11".into(),
            })
            .unwrap();
        for _ in 0..2 {
            journal
                .append(crate::journal::JournalEventKind::ToolCalled {
                    call: ToolCall {
                        id: "call-1".into(),
                        name: "read_file".into(),
                        arguments: json!({"path": "/tmp/x"}),
                    },
                })
                .unwrap();
        }
        journal
            .append(crate::journal::JournalEventKind::ToolObserved {
                observation: ToolObservation {
                    call_id: "call-1".into(),
                    tool: "read_file".into(),
                    status: ToolStatus::Ok,
                    summary: "read".into(),
                    content: "{}".into(),
                    error_code: None,
                    metadata: BTreeMap::new(),
                },
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::AnswerProduced {
                answer: AgentAnswer {
                    task_need: None,
                    schema_version: 2,
                    status: AnswerStatus::Completed,
                    conclusion: "done".into(),
                    claims: vec![Claim {
                        id: "c1".into(),
                        text: "done".into(),
                        status: ClaimStatus::Verified,
                        sources: vec!["tool:call-1".into()],
                        scope: None,
                    }],
                    evidence: vec![Evidence {
                        source: "tool:call-1".into(),
                        statement: "done".into(),
                        observed_unix_ms: None,
                        expires_unix_ms: None,
                        claim_ids: vec!["c1".into()],
                    }],
                    assumptions: Vec::new(),
                    constraints: Vec::new(),
                    unknowns: Vec::new(),
                    conflicts: Vec::new(),
                    flip_conditions: Vec::new(),
                    action_receipts: Vec::new(),
                },
            })
            .unwrap();
        journal
            .append(crate::journal::JournalEventKind::RunStopped {
                reason: StopReason::Completed,
            })
            .unwrap();
        drop(journal);

        let document = export_journal(&journal_path).unwrap();
        let run = node(&document.value, &format!("urn:1agent:run:{run_id}"));
        assert_eq!(run["clientRequestId"], "eval:case-11");
        assert_eq!(run["evalCaseId"], "case-11");
        assert_eq!(run["auditOk"], json!(false));
        assert_eq!(run["replayOk"], json!(true));
        assert!(
            run["auditIssueCodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|code| code == "duplicate_tool_call"),
            "{}",
            run["auditIssueCodes"]
        );
        let report = validate_document(&document.value);
        assert!(report.ok, "{:?}", report.errors);
    }

    #[test]
    fn goal_log_exports_goals_commitments_and_bindings_without_prose() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("goals.jsonl");
        let mut store = crate::goal::GoalStore::open(&path).unwrap();
        store
            .with_graph_mut(|graph| {
                graph.create_goal("goal-1", "owner-1", "SECRET GOAL TITLE", Some(1_000), 10)?;
                graph.add_commitment("commit-1", "goal-1", "SECRET COMMITMENT", Some(500), 11)?;
                graph.bind_session("goal-1", "ses-1", 12)?;
                graph.link_run("goal-1", "run-1", 13)
            })
            .unwrap();
        drop(store);

        let document = export_goal_log(&path).unwrap();
        let text = document.value.to_string();
        assert!(!text.contains("SECRET GOAL TITLE"));
        assert!(!text.contains("SECRET COMMITMENT"));
        let goal = node(&document.value, "urn:1agent:goal:goal-1");
        assert_eq!(goal["status"], "active");
        assert_eq!(goal["ownerId"], "owner-1");
        assert_eq!(goal["titleChars"], 17);
        assert_eq!(goal["dueUnixMs"], 1_000);
        assert_eq!(
            goal["hasCommitment"][0]["@id"],
            "urn:1agent:commitment:commit-1"
        );
        assert_eq!(goal["partOfSession"]["@id"], "urn:1agent:session:ses-1");
        assert_eq!(goal["runIds"], json!(["run-1"]));
        let commitment = node(&document.value, "urn:1agent:commitment:commit-1");
        assert_eq!(commitment["status"], "pending");
        assert_eq!(commitment["belongsToGoal"]["@id"], "urn:1agent:goal:goal-1");
        assert_eq!(commitment["descriptionChars"], 17);
        let report = validate_document(&document.value);
        assert!(report.ok, "{:?}", report.errors);
        assert_eq!(document.sha256, export_goal_log(&path).unwrap().sha256);

        let mut store = crate::goal::GoalStore::open(&path).unwrap();
        store
            .with_graph_mut(|graph| {
                graph.close_commitment("commit-1", CommitmentStatus::Kept, "done", 20)?;
                graph.set_goal_status("goal-1", GoalStatus::Achieved, "done", 21)
            })
            .unwrap();
        drop(store);
        let closed = export_goal_log(&path).unwrap();
        assert_eq!(
            node(&closed.value, "urn:1agent:goal:goal-1")["status"],
            "achieved"
        );
        let commitment = node(&closed.value, "urn:1agent:commitment:commit-1");
        assert_eq!(commitment["status"], "kept");
        assert_eq!(commitment["closedUnixMs"], 20);
        assert!(validate_document(&closed.value).ok);
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
