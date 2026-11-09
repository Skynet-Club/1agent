use first_principles_agent::{
    ActionKind, ActionLog, ApprovalDecision, Decision, MandateStrength, MemoryKind, MemoryLayer,
    MemoryRecord, MemoryScope, MemoryService, MemorySource, MemorySourceKind, ReportStatus,
    SideEffectDraft, SideEffectStatus, SkillCandidateStore, SkillEvalCase, SkillEvalSuite,
    SkillManifest, SkillRisk, SkillService, TaskGraph, TaskGraphError, TicketDraft, Trigger,
    TriggerSource,
};

fn trigger() -> Trigger {
    Trigger {
        trigger_id: "heldout-trigger".into(),
        source: TriggerSource::Web,
        source_event_id: "heldout-event".into(),
        summary: "held out".into(),
        received_ms: 1,
    }
}

fn ticket(graph: &mut TaskGraph, requires_approval: bool) -> String {
    let thread_id = graph.open_thread("held out", 10).unwrap();
    let trigger_id = graph
        .ingest_trigger(&thread_id, trigger(), 11)
        .unwrap()
        .trigger_id;
    let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12).unwrap();
    graph
        .decide_eval(&eval_id, Decision::Accept, "accepted", 13)
        .unwrap();
    graph
        .create_ticket_from_eval(
            &eval_id,
            TicketDraft {
                owner_id: "owner-1".into(),
                scope: "read held-out".into(),
                strength: MandateStrength::ReadOnly,
                policy_version: "v1".into(),
                requires_approval,
            },
            14,
        )
        .unwrap()
}

#[test]
fn heldout_task_lifecycle_survives_retry_takeover_and_compensation() {
    let mut graph = TaskGraph::default();
    let ticket_id = ticket(&mut graph, true);
    graph
        .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
        .unwrap();
    graph
        .assign_ticket(&ticket_id, "worker-1", None, 16)
        .unwrap();
    graph
        .takeover_ticket(&ticket_id, "owner-1", "human", 17)
        .unwrap();
    assert_eq!(
        graph.start_ticket(&ticket_id, "worker-1", 18).unwrap_err(),
        TaskGraphError::HumanTakeover(ticket_id.clone())
    );
    graph
        .release_takeover(&ticket_id, "owner-1", "hand back", 19)
        .unwrap();
    graph.start_ticket(&ticket_id, "worker-1", 20).unwrap();
    let effect = graph
        .record_side_effect(
            &ticket_id,
            SideEffectDraft {
                action: "write".into(),
                target: "/workspace/out".into(),
                idempotency_key: "effect-1".into(),
                reversible: true,
                compensation: Some("delete".into()),
                status: SideEffectStatus::Planned,
            },
            21,
        )
        .unwrap();
    graph
        .set_side_effect_status(
            &ticket_id,
            &effect,
            SideEffectStatus::Applied,
            "applied",
            22,
        )
        .unwrap();
    graph
        .compensate_side_effect(&ticket_id, &effect, "rolled back", 23)
        .unwrap();
    graph
        .report_ticket(
            &ticket_id,
            "worker-1",
            ReportStatus::Failed,
            "failed",
            None,
            24,
        )
        .unwrap();
    graph
        .retry_ticket(&ticket_id, "owner-1", "retry", 25)
        .unwrap();
    let replayed = TaskGraph::replay(graph.events()).unwrap();
    assert_eq!(replayed.ticket(&ticket_id).unwrap().generation, 1);
    assert_eq!(replayed.reconcile_side_effects(&ticket_id).compensated, 1);
}

#[test]
fn heldout_memory_and_user_model_keep_scope_and_sources() {
    let dir = tempfile::tempdir().unwrap();
    let mut memory = MemoryService::open(
        dir.path().join("memory.jsonl"),
        dir.path().join("payloads"),
        dir.path().join("memory.key"),
    )
    .unwrap();
    for (id, kind, content) in [
        ("mem-1", MemoryKind::Preference, "editor=pnpm"),
        (
            "mem-2",
            MemoryKind::Feedback,
            "boundary: never touch production",
        ),
        ("mem-3", MemoryKind::Preference, "editor=npm"),
    ] {
        let mut record = MemoryRecord::new(
            id,
            MemoryLayer::Semantic,
            kind,
            MemoryScope::bot("bot-1"),
            content,
            MemorySource::new(MemorySourceKind::UserMessage, format!("event-{id}")),
        );
        record.valid_from_ms = match id {
            "mem-1" => 10,
            "mem-2" => 11,
            _ => 20,
        };
        let at = record.valid_from_ms;
        memory.remember_at(record, at).unwrap();
    }
    let model = memory.build_user_model("user-1", &MemoryScope::bot("bot-1"));
    assert_eq!(model.preference("editor").unwrap().value, "npm");
    assert_eq!(
        model.preference("editor").unwrap().source_memory_id,
        "mem-3"
    );
    assert!(model.has_boundary("never touch production"));
    assert!(
        memory
            .recall("editor", &MemoryScope::bot("bot-2"), 30, 5)
            .is_empty()
    );
    memory
        .forget(
            "mem-1",
            &MemoryScope::bot("bot-1"),
            first_principles_agent::ForgetMode::Hard,
            31,
        )
        .unwrap();
    assert!(
        !memory
            .payload_contains("mem-1", &MemoryScope::bot("bot-1"))
            .unwrap()
    );
}

#[test]
fn heldout_skill_gate_and_candidate_capture() {
    let dir = tempfile::tempdir().unwrap();
    let mut skills = SkillService::open(dir.path().join("skills.jsonl")).unwrap();
    let mut manifest = SkillManifest {
        name: "release-heldout".into(),
        description: "release held out".into(),
        version: "1.0.0".into(),
        platforms: vec!["linux".into()],
        scope: "repo".into(),
        requires_tools: Vec::new(),
        risk: SkillRisk::Low,
        body: "1. verify\n2. build".into(),
        content_sha256: String::new(),
    };
    skills.register(manifest.clone(), 10).unwrap();
    let suite = SkillEvalSuite {
        suite_id: "heldout-suite".into(),
        min_score_bps: 10_000,
        cases: vec![SkillEvalCase {
            case_id: "verify".into(),
            required_markers: vec!["verify".into()],
            forbidden_markers: vec!["rm -rf".into()],
        }],
    };
    skills
        .activate_if_passed("release-heldout", "1.0.0", &suite, 11)
        .unwrap();
    assert_eq!(skills.active("release-heldout").unwrap().version, "1.0.0");

    manifest.version = "1.1.0".into();
    manifest.body = "rm -rf /".into();
    let mut candidates = SkillCandidateStore::default();
    let candidate = candidates.propose(manifest, 12);
    let demo = candidates
        .capture(
            &candidate.candidate_id,
            "held-out input",
            "held-out output",
            true,
            13,
        )
        .unwrap();
    assert!(!format!("{demo:?}").contains("held-out input"));
    assert!(
        !candidates
            .evaluate(&candidate.candidate_id, &suite)
            .unwrap()
            .passed
    );

    let mut actions = ActionLog::default();
    actions
        .record(
            "owner-1",
            ActionKind::Approve,
            "ticket-1",
            "allowed",
            "ok",
            1,
        )
        .unwrap();
    actions.verify().unwrap();
}
