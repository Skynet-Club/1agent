use first_principles_agent::{
    CommitmentStatus, DataClass, DelegationMode, DelegationProfile, DelegationStore, DoctorStatus,
    EvolutionKind, EvolutionProposalDraft, EvolutionStatus, EvolutionStore, GoalStatus, GoalStore,
    MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemoryService, MemorySource,
    MemorySourceKind, MemoryTaint, RiskLevel, TaskGraphService, build_context_graph, runtime_home,
};

fn record() -> MemoryRecord {
    let mut record = MemoryRecord::new(
        "mem-1",
        MemoryLayer::Semantic,
        MemoryKind::Preference,
        MemoryScope::bot("bot-1"),
        "prefers pnpm and a short release checklist",
        MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
    );
    record.taint = MemoryTaint::Trusted;
    record.data_class = DataClass::Personal;
    record
}

fn delegation_profile() -> DelegationProfile {
    DelegationProfile {
        profile_id: "delegation-1".into(),
        actor_id: "owner-1".into(),
        domain: "repo:1-agent".into(),
        mode: DelegationMode::Suggest,
        max_risk: RiskLevel::Medium,
        allowed_tools: vec!["write_file".into()],
        expires_ms: None,
    }
}

fn evolution_draft() -> EvolutionProposalDraft {
    EvolutionProposalDraft {
        proposal_id: "proposal-1".into(),
        proposer: "owner-1".into(),
        kind: EvolutionKind::SkillDraft,
        summary: "tighten release checklist".into(),
        evidence: vec!["skill-eval:42".into()],
        rollback_plan: "revert to previous skill version".into(),
        risk: RiskLevel::Low,
    }
}

#[test]
fn goals_memory_delegation_and_evolution_survive_restart() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    runtime_home::init(&home, 1).unwrap();

    {
        let mut memory = MemoryService::open(
            home.join("memory-journal.jsonl"),
            home.join("payloads"),
            home.join("memory.key"),
        )
        .unwrap();
        memory.remember_at(record(), 10).unwrap();
    }
    {
        let mut goals = GoalStore::open(home.join("goal-journal.jsonl")).unwrap();
        goals
            .with_graph_mut(|graph| {
                graph.create_goal("goal-1", "owner-1", "ship 1Agent", None, 10)?;
                graph.add_commitment("commit-1", "goal-1", "finish release gate", None, 11)
            })
            .unwrap();
    }
    {
        let mut delegation = DelegationStore::open(home.join("delegation.json")).unwrap();
        delegation
            .with_registry_mut(|registry| registry.register(delegation_profile()))
            .unwrap();
    }
    {
        let mut evolution = EvolutionStore::open(home.join("evolution.json")).unwrap();
        evolution
            .with_registry_mut(|registry| registry.propose(evolution_draft(), 10))
            .unwrap();
    }

    // Reopen every store as a fresh process would.
    let memory = MemoryService::open(
        home.join("memory-journal.jsonl"),
        home.join("payloads"),
        home.join("memory.key"),
    )
    .unwrap();
    let recalled = memory.get("mem-1", &MemoryScope::bot("bot-1")).unwrap();
    assert_eq!(
        recalled.content,
        "prefers pnpm and a short release checklist"
    );

    let goals = GoalStore::open(home.join("goal-journal.jsonl")).unwrap();
    assert_eq!(
        goals.graph().goal("goal-1").unwrap().status,
        GoalStatus::Active
    );
    assert_eq!(
        goals.graph().commitment("commit-1").unwrap().status,
        CommitmentStatus::Pending
    );

    let delegation = DelegationStore::open(home.join("delegation.json")).unwrap();
    assert!(
        delegation
            .registry()
            .profile("owner-1", "repo:1-agent")
            .is_some()
    );

    let evolution = EvolutionStore::open(home.join("evolution.json")).unwrap();
    assert_eq!(
        evolution.registry().proposal("proposal-1").unwrap().status,
        EvolutionStatus::Proposed
    );

    let tasks = TaskGraphService::open(home.join("task-graph.jsonl")).unwrap();
    let graph = build_context_graph(
        goals.graph(),
        &tasks.snapshot(),
        std::slice::from_ref(recalled),
        20,
    );
    assert!(graph.node("goal:goal-1").is_some());
    assert!(graph.node("commitment:commit-1").is_some());
    assert!(graph.node("memory:mem-1").is_some());
    let explanation = graph.explain("commitment:commit-1").unwrap();
    assert!(
        explanation
            .incoming
            .iter()
            .any(|edge| { edge.from == "goal:goal-1" && edge.relation == "has_commitment" })
    );

    let report = runtime_home::doctor(&home).unwrap();
    assert_eq!(report.status, DoctorStatus::Ok, "{report:?}");
    assert!(report.root.is_dir());
}
