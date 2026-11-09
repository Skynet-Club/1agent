use std::time::Instant;

use first_principles_agent::{
    AuditKind, Decision, MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemoryService,
    MemorySource, MemorySourceKind, SkillManifest, SkillRisk, SkillService, TaskGraphError,
    TaskGraphService, Trigger, TriggerSource, audit_store,
};

#[test]
#[ignore = "soak; run manually before a release gate"]
fn a_gate_soak_keeps_task_memory_and_skill_logs_auditable() {
    let started = Instant::now();
    let dir = tempfile::tempdir().unwrap();
    let graph_path = dir.path().join("task-graph.jsonl");
    let mut graph = TaskGraphService::open(&graph_path).unwrap();
    graph
        .with_graph_mut(|graph| {
            let thread_id = graph.open_thread("soak", 1)?;
            for index in 0..300u128 {
                let trigger_id = graph
                    .ingest_trigger(
                        &thread_id,
                        Trigger {
                            trigger_id: format!("trigger-{index}"),
                            source: TriggerSource::Web,
                            source_event_id: format!("event-{index}"),
                            summary: "soak".into(),
                            received_ms: index,
                        },
                        index + 1,
                    )?
                    .trigger_id;
                let eval_id = graph.begin_eval(&thread_id, &trigger_id, index + 2)?;
                graph.decide_eval(&eval_id, Decision::Accept, "soak", index + 3)?;
            }
            Ok::<(), TaskGraphError>(())
        })
        .unwrap();
    let receipt = audit_store(AuditKind::TaskGraph, &graph_path, 10_000).unwrap();
    assert_eq!(receipt.events, 901);
    let reopened = TaskGraphService::open(&graph_path).unwrap();
    assert_eq!(reopened.graph().events().len(), 901);

    let memory_path = dir.path().join("memory-journal.jsonl");
    let mut memory = MemoryService::open(
        &memory_path,
        dir.path().join("payloads"),
        dir.path().join("memory.key"),
    )
    .unwrap();
    for index in 0..200u128 {
        memory
            .remember_at(
                MemoryRecord::new(
                    format!("mem-{index}"),
                    MemoryLayer::Semantic,
                    MemoryKind::Preference,
                    MemoryScope::bot("bot-1"),
                    format!("soak preference {index}"),
                    MemorySource::new(MemorySourceKind::UserMessage, format!("event-{index}")),
                ),
                index,
            )
            .unwrap();
    }
    assert_eq!(
        audit_store(AuditKind::Memory, &memory_path, 20_000)
            .unwrap()
            .events,
        200
    );

    let skill_path = dir.path().join("skill-journal.jsonl");
    let mut skills = SkillService::open(&skill_path).unwrap();
    for index in 0..100 {
        skills
            .register(
                SkillManifest {
                    name: format!("skill-{index}"),
                    description: "soak".into(),
                    version: "1.0.0".into(),
                    platforms: vec!["linux".into()],
                    scope: "repo".into(),
                    requires_tools: Vec::new(),
                    risk: SkillRisk::Low,
                    body: "1. verify".into(),
                    content_sha256: String::new(),
                },
                index,
            )
            .unwrap();
    }
    assert_eq!(
        audit_store(AuditKind::Skill, &skill_path, 30_000)
            .unwrap()
            .events,
        100
    );
    assert!(
        started.elapsed().as_secs() < 30,
        "soak took {:?}",
        started.elapsed()
    );
}
