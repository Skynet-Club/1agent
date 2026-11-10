//! Read-only audit receipts and exports for the append-only 1Agent stores.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::memory::MemoryStore;
use crate::memory_journal::{MemoryJournalError, MemoryJournalStore};
use crate::skill_journal::{SkillJournalError, SkillJournalStore};
use crate::task_graph_store::{TaskGraphService, TaskGraphStoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditKind {
    TaskGraph,
    Memory,
    Skill,
}

impl AuditKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TaskGraph => "task-graph",
            Self::Memory => "memory",
            Self::Skill => "skill",
        }
    }
}

impl FromStr for AuditKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "task-graph" => Ok(Self::TaskGraph),
            "memory" => Ok(Self::Memory),
            "skill" => Ok(Self::Skill),
            other => Err(format!(
                "unknown audit kind {other}; expected task-graph, memory or skill"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditReceipt {
    pub kind: AuditKind,
    pub path: String,
    pub events: usize,
    pub head_sha256: Option<String>,
    pub verified: bool,
    pub audited_ms: u128,
}

#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    pub thread_id: Option<String>,
    pub ticket_id: Option<String>,
    pub memory_id: Option<String>,
    pub scope: Option<String>,
    pub skill: Option<String>,
    pub version: Option<String>,
    pub limit: Option<usize>,
    pub after_seq: Option<u64>,
    pub include_records: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditExportResult {
    pub receipt: AuditReceipt,
    pub matched: usize,
    pub exported: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub records: Option<serde_json::Value>,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit io: {0}")]
    Io(#[from] std::io::Error),
    #[error("audit json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audit path is not a regular file: {0}")]
    NotFound(String),
    #[error("audit task graph: {0}")]
    TaskGraph(#[from] TaskGraphStoreError),
    #[error("audit memory journal: {0}")]
    Memory(#[from] MemoryJournalError),
    #[error("audit memory state: {0}")]
    MemoryState(#[from] crate::memory::MemoryError),
    #[error("audit skill journal: {0}")]
    Skill(#[from] SkillJournalError),
}

pub fn audit_store(kind: AuditKind, path: &Path, now_ms: u128) -> Result<AuditReceipt, AuditError> {
    Ok(audit_export(kind, path, &AuditQuery::default(), None, now_ms)?.receipt)
}

pub fn audit_export(
    kind: AuditKind,
    path: &Path,
    query: &AuditQuery,
    export_path: Option<&Path>,
    now_ms: u128,
) -> Result<AuditExportResult, AuditError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AuditError::NotFound(path.display().to_string()));
    }
    let (events, head_sha256, matched, records) = match kind {
        AuditKind::TaskGraph => {
            let service = TaskGraphService::open_read_only(path)?;
            let graph = service.graph();
            let events = graph.events();
            let records = events
                .iter()
                .filter(|event| {
                    query.after_seq.is_none_or(|seq| event.seq > seq)
                        && query.thread_id.as_deref().is_none_or(|thread_id| {
                            graph.event_thread_id(event).as_deref() == Some(thread_id)
                        })
                        && query.ticket_id.as_deref().is_none_or(|ticket_id| {
                            graph.event_ticket_id(event).as_deref() == Some(ticket_id)
                        })
                })
                .take(query.limit.unwrap_or(usize::MAX))
                .cloned()
                .collect::<Vec<_>>();
            if let Some(output) = export_path {
                write_jsonl(output, &records)?;
            }
            let records_json = if query.include_records {
                Some(serde_json::to_value(&records)?)
            } else {
                None
            };
            (
                events.len(),
                events.last().map(|event| event.event_sha256.clone()),
                records.len(),
                records_json,
            )
        }
        AuditKind::Memory => {
            let journal = MemoryJournalStore::open_read_only(path)?;
            let store = MemoryStore::from_events(journal.events())?;
            let events = journal.events();
            let records = events
                .iter()
                .filter(|event| {
                    let memory_id = event.kind.memory_id();
                    let memory_match = query
                        .memory_id
                        .as_deref()
                        .is_none_or(|expected| expected == memory_id);
                    let scope_match = query.scope.as_deref().is_none_or(|expected| {
                        store
                            .get(memory_id)
                            .is_some_and(|record| record.scope.scope_id() == expected)
                    });
                    query.after_seq.is_none_or(|seq| event.seq > seq) && memory_match && scope_match
                })
                .take(query.limit.unwrap_or(usize::MAX))
                .cloned()
                .collect::<Vec<_>>();
            if let Some(output) = export_path {
                write_jsonl(output, &records)?;
            }
            let records_json = if query.include_records {
                Some(serde_json::to_value(&records)?)
            } else {
                None
            };
            (
                events.len(),
                events.last().map(|event| event.event_sha256.clone()),
                records.len(),
                records_json,
            )
        }
        AuditKind::Skill => {
            let journal = SkillJournalStore::open_read_only(path)?;
            let events = journal.events();
            let records = events
                .iter()
                .filter(|event| {
                    query.after_seq.is_none_or(|seq| event.seq > seq)
                        && query
                            .skill
                            .as_deref()
                            .is_none_or(|expected| expected == event.kind.name())
                        && query
                            .version
                            .as_deref()
                            .is_none_or(|expected| expected == event.kind.version())
                })
                .take(query.limit.unwrap_or(usize::MAX))
                .cloned()
                .collect::<Vec<_>>();
            if let Some(output) = export_path {
                write_jsonl(output, &records)?;
            }
            let records_json = if query.include_records {
                Some(serde_json::to_value(&records)?)
            } else {
                None
            };
            (
                events.len(),
                events.last().map(|event| event.event_sha256.clone()),
                records.len(),
                records_json,
            )
        }
    };
    Ok(AuditExportResult {
        receipt: AuditReceipt {
            kind,
            path: path.display().to_string(),
            events,
            head_sha256,
            verified: true,
            audited_ms: now_ms,
        },
        matched,
        exported: export_path.map(|path| path.display().to_string()),
        records,
    })
}

fn write_jsonl<T: Serialize>(path: &Path, records: &[T]) -> Result<(), AuditError> {
    crate::jsonl::ensure_parent_private(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    for record in records {
        file.write_all(serde_json::to_string(record)?.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.sync_data()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{
        MemoryKind, MemoryLayer, MemoryRecord, MemoryScope, MemorySource, MemorySourceKind,
    };
    use crate::memory_service::MemoryService;
    use crate::skill::{SkillManifest, SkillRisk};
    use crate::skill_service::SkillService;

    #[test]
    fn audits_task_graph_memory_and_skill_and_rejects_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let graph_path = dir.path().join("task-graph.jsonl");
        let mut graph = TaskGraphService::open(&graph_path).unwrap();
        graph
            .with_graph_mut(|graph| graph.open_thread("audit me", 10))
            .unwrap();
        let receipt = audit_store(AuditKind::TaskGraph, &graph_path, 20).unwrap();
        assert_eq!(receipt.events, 1);
        assert!(receipt.verified);
        assert!(receipt.head_sha256.is_some());

        let memory_path = dir.path().join("memory-journal.jsonl");
        let mut memory = MemoryService::open(
            &memory_path,
            dir.path().join("payloads"),
            dir.path().join("memory.key"),
        )
        .unwrap();
        memory
            .remember_at(
                MemoryRecord::new(
                    "mem-1",
                    MemoryLayer::Semantic,
                    MemoryKind::Preference,
                    MemoryScope::bot("bot-1"),
                    "preference",
                    MemorySource::new(MemorySourceKind::UserMessage, "event-1"),
                ),
                10,
            )
            .unwrap();
        let receipt = audit_store(AuditKind::Memory, &memory_path, 20).unwrap();
        assert_eq!(receipt.events, 1);

        let skill_path = dir.path().join("skill-journal.jsonl");
        let mut skills = SkillService::open(&skill_path).unwrap();
        skills
            .register(
                SkillManifest {
                    name: "release".into(),
                    description: "release process".into(),
                    version: "1.0.0".into(),
                    platforms: vec!["linux".into()],
                    scope: "repo:1-agent".into(),
                    requires_tools: vec!["shell".into()],
                    risk: SkillRisk::Low,
                    body: "1. verify".into(),
                    content_sha256: String::new(),
                },
                10,
            )
            .unwrap();
        let receipt = audit_store(AuditKind::Skill, &skill_path, 20).unwrap();
        assert_eq!(receipt.events, 1);

        let text = std::fs::read_to_string(&graph_path).unwrap();
        std::fs::write(&graph_path, text.replace("audit me", "tampered")).unwrap();
        assert!(audit_store(AuditKind::TaskGraph, &graph_path, 21).is_err());
    }

    #[test]
    fn query_and_export_filter_task_graph_memory_and_skill() {
        use crate::task_graph::{Decision, MandateStrength, TicketDraft, Trigger, TriggerSource};

        let dir = tempfile::tempdir().unwrap();
        let graph_path = dir.path().join("task-graph.jsonl");
        let mut graph = TaskGraphService::open(&graph_path).unwrap();
        let (thread_a, thread_b, ticket_id) = graph
            .with_graph_mut(|graph| {
                let thread_a = graph.open_thread("thread-a", 10)?;
                let thread_b = graph.open_thread("thread-b", 11)?;
                let trigger = Trigger {
                    trigger_id: "trigger-a".into(),
                    source: TriggerSource::Web,
                    source_event_id: "event-a".into(),
                    summary: "a".into(),
                    received_ms: 12,
                };
                let trigger_id = graph.ingest_trigger(&thread_a, trigger, 12)?.trigger_id;
                let eval_id = graph.begin_eval(&thread_a, &trigger_id, 13)?;
                graph.decide_eval(&eval_id, Decision::Accept, "ok", 14)?;
                let ticket_id = graph.create_ticket_from_eval(
                    &eval_id,
                    TicketDraft {
                        owner_id: "owner".into(),
                        scope: "read".into(),
                        strength: MandateStrength::ReadOnly,
                        policy_version: "v1".into(),
                        requires_approval: false,
                    },
                    15,
                )?;
                Ok((thread_a, thread_b, ticket_id))
            })
            .unwrap();
        let export_path = dir.path().join("task-graph-export.jsonl");
        let result = audit_export(
            AuditKind::TaskGraph,
            &graph_path,
            &AuditQuery {
                thread_id: Some(thread_a.clone()),
                ..Default::default()
            },
            Some(&export_path),
            20,
        )
        .unwrap();
        assert_eq!(result.matched, 5);
        assert_eq!(
            std::fs::read_to_string(&export_path)
                .unwrap()
                .lines()
                .count(),
            5
        );
        let page = audit_export(
            AuditKind::TaskGraph,
            &graph_path,
            &AuditQuery {
                thread_id: Some(thread_a.clone()),
                limit: Some(2),
                include_records: true,
                ..Default::default()
            },
            None,
            20,
        )
        .unwrap();
        let records = page.records.unwrap();
        let records = records.as_array().unwrap();
        assert_eq!(records.len(), 2);
        let last_seq = records[1]["seq"].as_u64().unwrap();
        let next = audit_export(
            AuditKind::TaskGraph,
            &graph_path,
            &AuditQuery {
                thread_id: Some(thread_a),
                after_seq: Some(last_seq),
                limit: Some(2),
                include_records: true,
                ..Default::default()
            },
            None,
            21,
        )
        .unwrap();
        let next = next.records.unwrap();
        let next = next.as_array().unwrap();
        assert!(!next.is_empty());
        assert!(next[0]["seq"].as_u64().unwrap() > last_seq);
        let result = audit_export(
            AuditKind::TaskGraph,
            &graph_path,
            &AuditQuery {
                ticket_id: Some(ticket_id),
                ..Default::default()
            },
            None,
            20,
        )
        .unwrap();
        assert_eq!(result.matched, 1);
        let result = audit_export(
            AuditKind::TaskGraph,
            &graph_path,
            &AuditQuery {
                thread_id: Some(thread_b),
                ..Default::default()
            },
            None,
            20,
        )
        .unwrap();
        assert_eq!(result.matched, 1);

        let memory_path = dir.path().join("memory-journal.jsonl");
        let mut memory = MemoryService::open(
            &memory_path,
            dir.path().join("payloads"),
            dir.path().join("memory.key"),
        )
        .unwrap();
        for (id, scope) in [("mem-1", "bot-1"), ("mem-2", "bot-2")] {
            memory
                .remember_at(
                    MemoryRecord::new(
                        id,
                        MemoryLayer::Semantic,
                        MemoryKind::Preference,
                        MemoryScope::bot(scope),
                        "preference",
                        MemorySource::new(MemorySourceKind::UserMessage, format!("event-{id}")),
                    ),
                    10,
                )
                .unwrap();
        }
        let result = audit_export(
            AuditKind::Memory,
            &memory_path,
            &AuditQuery {
                scope: Some(MemoryScope::bot("bot-1").scope_id()),
                ..Default::default()
            },
            None,
            20,
        )
        .unwrap();
        assert_eq!(result.matched, 1);

        let skill_path = dir.path().join("skill-journal.jsonl");
        let mut skills = SkillService::open(&skill_path).unwrap();
        for (name, version) in [
            ("release", "1.0.0"),
            ("release", "1.1.0"),
            ("other", "1.0.0"),
        ] {
            skills
                .register(
                    SkillManifest {
                        name: name.into(),
                        description: "process".into(),
                        version: version.into(),
                        platforms: vec!["linux".into()],
                        scope: "repo:1-agent".into(),
                        requires_tools: Vec::new(),
                        risk: SkillRisk::Low,
                        body: "1. verify".into(),
                        content_sha256: String::new(),
                    },
                    10,
                )
                .unwrap();
        }
        let result = audit_export(
            AuditKind::Skill,
            &skill_path,
            &AuditQuery {
                skill: Some("release".into()),
                version: Some("1.0.0".into()),
                ..Default::default()
            },
            None,
            20,
        )
        .unwrap();
        assert_eq!(result.matched, 1);
    }

    #[test]
    fn kind_parsing_is_exact() {
        assert_eq!(
            "task-graph".parse::<AuditKind>().unwrap(),
            AuditKind::TaskGraph
        );
        assert_eq!("memory".parse::<AuditKind>().unwrap(), AuditKind::Memory);
        assert_eq!("skill".parse::<AuditKind>().unwrap(), AuditKind::Skill);
        assert!("task_graph".parse::<AuditKind>().is_err());
    }
}
