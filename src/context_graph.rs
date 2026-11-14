//! Rebuildable personal-context view over goals, tasks and memory references.
//!
//! The graph is a projection, not a new source of truth. Nodes carry a stable
//! prefixed id and their source store; edges are derived only from explicit
//! references (goal→commitment, thread→ticket, commitment→ticket). Missing or
//! superseded memory is shown as such rather than silently filled in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::goal::{CommitmentStatus, GoalGraph, GoalStatus};
use crate::memory::{MemoryLayer, MemoryRecord};
use crate::task_graph::{TaskGraphSnapshot, ThreadStatus, TicketStatus};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContextNodeKind {
    Goal,
    Commitment,
    Thread,
    Ticket,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextNode {
    pub id: String,
    pub kind: ContextNodeKind,
    pub label: String,
    pub status: String,
    pub scope: Option<String>,
    pub updated_ms: u128,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextGraph {
    pub nodes: Vec<ContextNode>,
    pub edges: Vec<ContextEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextExplanation {
    pub node: ContextNode,
    pub incoming: Vec<ContextEdge>,
    pub outgoing: Vec<ContextEdge>,
}

impl ContextGraph {
    pub fn node(&self, id: &str) -> Option<&ContextNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn edges_from(&self, id: &str) -> Vec<&ContextEdge> {
        self.edges.iter().filter(|edge| edge.from == id).collect()
    }

    pub fn edges_to(&self, id: &str) -> Vec<&ContextEdge> {
        self.edges.iter().filter(|edge| edge.to == id).collect()
    }

    pub fn explain(&self, id: &str) -> Option<ContextExplanation> {
        let node = self.node(id)?.clone();
        Some(ContextExplanation {
            node,
            incoming: self.edges_to(id).into_iter().cloned().collect(),
            outgoing: self.edges_from(id).into_iter().cloned().collect(),
        })
    }

    pub fn counts(&self) -> BTreeMap<ContextNodeKind, usize> {
        let mut counts = BTreeMap::new();
        for node in &self.nodes {
            *counts.entry(node.kind).or_insert(0) += 1;
        }
        counts
    }
}

pub fn build_context_graph(
    goals: &GoalGraph,
    tasks: &TaskGraphSnapshot,
    memories: &[MemoryRecord],
    now_ms: u128,
) -> ContextGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for goal in &goals.snapshot().goals {
        nodes.push(ContextNode {
            id: format!("goal:{}", goal.goal_id),
            kind: ContextNodeKind::Goal,
            label: truncate(&goal.title),
            status: goal_status(goal.status).into(),
            scope: None,
            updated_ms: goal.updated_ms,
            source: "goal-store".into(),
        });
    }
    for commitment in &goals.snapshot().commitments {
        nodes.push(ContextNode {
            id: format!("commitment:{}", commitment.commitment_id),
            kind: ContextNodeKind::Commitment,
            label: truncate(&commitment.description),
            status: commitment_status(commitment.status).into(),
            scope: None,
            updated_ms: commitment.updated_ms,
            source: "goal-store".into(),
        });
        edges.push(ContextEdge {
            from: format!("goal:{}", commitment.goal_id),
            to: format!("commitment:{}", commitment.commitment_id),
            relation: "has_commitment".into(),
        });
        if let Some(ticket_id) = &commitment.ticket_id {
            let target = format!("ticket:{ticket_id}");
            if tasks
                .tickets
                .iter()
                .any(|ticket| &ticket.ticket_id == ticket_id)
            {
                edges.push(ContextEdge {
                    from: format!("commitment:{}", commitment.commitment_id),
                    to: target,
                    relation: "pursued_by".into(),
                });
            }
        }
    }
    for thread in &tasks.threads {
        nodes.push(ContextNode {
            id: format!("thread:{}", thread.thread_id),
            kind: ContextNodeKind::Thread,
            label: truncate(&thread.topic),
            status: thread_status(thread.status).into(),
            scope: None,
            updated_ms: thread.updated_ms,
            source: "task-graph".into(),
        });
        for ticket_id in &thread.ticket_ids {
            edges.push(ContextEdge {
                from: format!("thread:{}", thread.thread_id),
                to: format!("ticket:{ticket_id}"),
                relation: "has_ticket".into(),
            });
        }
    }
    for ticket in &tasks.tickets {
        let scope = tasks
            .mandates
            .iter()
            .find(|mandate| mandate.mandate_id == ticket.mandate_id)
            .map(|mandate| mandate.scope.clone());
        nodes.push(ContextNode {
            id: format!("ticket:{}", ticket.ticket_id),
            kind: ContextNodeKind::Ticket,
            label: format!("ticket {}", ticket.ticket_id),
            status: ticket_status(ticket.status).into(),
            scope,
            updated_ms: ticket.updated_ms,
            source: "task-graph".into(),
        });
    }
    for memory in memories {
        if !memory_visible(memory, now_ms) {
            continue;
        }
        nodes.push(ContextNode {
            id: format!("memory:{}", memory.memory_id),
            kind: ContextNodeKind::Memory,
            label: truncate(&memory.content),
            status: if memory.superseded_by.is_some() {
                "superseded".into()
            } else {
                "active".into()
            },
            scope: Some(memory.scope.scope_id()),
            updated_ms: memory.valid_from_ms,
            source: format!("memory/{}", memory_layer(memory.layer)),
        });
    }

    nodes.sort_by(|left, right| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.id.cmp(&right.id))
    });
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then_with(|| left.to.cmp(&right.to))
            .then_with(|| left.relation.cmp(&right.relation))
    });
    edges.dedup();
    ContextGraph { nodes, edges }
}

fn truncate(value: &str) -> String {
    value.chars().take(120).collect()
}

fn memory_visible(record: &MemoryRecord, now_ms: u128) -> bool {
    record.hard_forgotten_ms.is_none()
        && record.soft_forgotten_ms.is_none()
        && record.valid_from_ms <= now_ms
        && record.valid_to_ms.is_none_or(|valid_to| valid_to > now_ms)
}

fn goal_status(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Active => "active",
        GoalStatus::Paused => "paused",
        GoalStatus::Achieved => "achieved",
        GoalStatus::Abandoned => "abandoned",
    }
}

fn commitment_status(status: CommitmentStatus) -> &'static str {
    match status {
        CommitmentStatus::Pending => "pending",
        CommitmentStatus::Kept => "kept",
        CommitmentStatus::Missed => "missed",
        CommitmentStatus::Cancelled => "cancelled",
    }
}

fn memory_layer(layer: MemoryLayer) -> &'static str {
    match layer {
        MemoryLayer::Working => "working",
        MemoryLayer::Episodic => "episodic",
        MemoryLayer::Semantic => "semantic",
        MemoryLayer::Procedural => "procedural",
    }
}

fn thread_status(status: ThreadStatus) -> &'static str {
    match status {
        ThreadStatus::Open => "open",
        ThreadStatus::Closed => "closed",
    }
}

fn ticket_status(status: TicketStatus) -> &'static str {
    match status {
        TicketStatus::Authorized => "authorized",
        TicketStatus::Approved => "approved",
        TicketStatus::Denied => "denied",
        TicketStatus::Assigned => "assigned",
        TicketStatus::Running => "running",
        TicketStatus::WaitingInput => "waiting_input",
        TicketStatus::Ready => "ready",
        TicketStatus::Blocked => "blocked",
        TicketStatus::Failed => "failed",
        TicketStatus::Dropped => "dropped",
        TicketStatus::Cancelled => "cancelled",
    }
}

const CACHE_SCHEMA_VERSION: u32 = 1;
const MAX_CACHE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedContextGraph {
    schema_version: u32,
    input_sha256: String,
    graph: ContextGraph,
}

/// File-backed context graph cache keyed by an input fingerprint.
#[derive(Debug, Clone)]
pub struct ContextGraphCache {
    path: PathBuf,
}

impl ContextGraphCache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self, input_sha256: &str) -> Result<Option<ContextGraph>, String> {
        let metadata = match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("cache stat failed: {error}")),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("context cache must be a regular file".into());
        }
        if metadata.len() > MAX_CACHE_BYTES {
            return Err("context cache is too large".into());
        }
        let bytes =
            std::fs::read(&self.path).map_err(|error| format!("cache read failed: {error}"))?;
        let cached: CachedContextGraph = serde_json::from_slice(&bytes)
            .map_err(|error| format!("cache json failed: {error}"))?;
        if cached.schema_version != CACHE_SCHEMA_VERSION || cached.input_sha256 != input_sha256 {
            return Ok(None);
        }
        Ok(Some(cached.graph))
    }

    pub fn store(&self, input_sha256: &str, graph: &ContextGraph) -> Result<(), String> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cache mkdir failed: {error}"))?;
        }
        let cached = CachedContextGraph {
            schema_version: CACHE_SCHEMA_VERSION,
            input_sha256: input_sha256.into(),
            graph: graph.clone(),
        };
        let bytes =
            serde_json::to_vec(&cached).map_err(|error| format!("cache encode failed: {error}"))?;
        let temporary = self
            .path
            .with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("cache write failed: {error}"))?;
        if let Err(error) = std::fs::rename(&temporary, &self.path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(format!("cache replace failed: {error}"));
        }
        Ok(())
    }

    pub fn get_or_build(
        &self,
        input_sha256: &str,
        build: impl FnOnce() -> ContextGraph,
    ) -> Result<(ContextGraph, bool), String> {
        if let Some(graph) = self.load(input_sha256)? {
            return Ok((graph, true));
        }
        let graph = build();
        self.store(input_sha256, &graph)?;
        Ok((graph, false))
    }
}

/// Deterministic fingerprint of the graph inputs.
pub fn fingerprint(
    goals: &GoalGraph,
    tasks: &TaskGraphSnapshot,
    memories: &[MemoryRecord],
) -> String {
    let memory_refs = memories
        .iter()
        .map(|memory| {
            serde_json::json!({
                "id": memory.memory_id,
                "sha256": memory.content_sha256,
                "valid_from_ms": memory.valid_from_ms,
                "valid_to_ms": memory.valid_to_ms,
                "soft_forgotten_ms": memory.soft_forgotten_ms,
                "hard_forgotten_ms": memory.hard_forgotten_ms,
                "superseded_by": memory.superseded_by
            })
        })
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "goals": goals.snapshot(),
        "tasks": tasks,
        "memories": memory_refs
    });
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal::GoalGraph;
    use crate::memory::{MemoryKind, MemoryLayer, MemoryScope, MemorySource, MemorySourceKind};
    use crate::task_graph::{
        ApprovalDecision, Decision, MandateStrength, TaskGraph, TicketDraft, Trigger, TriggerSource,
    };

    fn goal_graph(ticket_id: &str) -> GoalGraph {
        let mut goals = GoalGraph::default();
        goals
            .create_goal("goal-1", "owner-1", "ship", None, 1)
            .unwrap();
        goals
            .add_commitment("commit-1", "goal-1", "write notes", None, 2)
            .unwrap();
        goals
            .link_commitment_ticket("commit-1", ticket_id, 3)
            .unwrap();
        goals
    }

    fn task_graph() -> (TaskGraph, String, String) {
        let mut graph = TaskGraph::default();
        let thread_id = graph.open_thread("release", 1).unwrap();
        let trigger_id = graph
            .ingest_trigger(
                &thread_id,
                Trigger {
                    trigger_id: "trigger".into(),
                    source: TriggerSource::Web,
                    source_event_id: "event".into(),
                    summary: "release".into(),
                    received_ms: 1,
                },
                2,
            )
            .unwrap()
            .trigger_id;
        let eval_id = graph.begin_eval(&thread_id, &trigger_id, 3).unwrap();
        graph
            .decide_eval(&eval_id, Decision::Accept, "ok", 4)
            .unwrap();
        let ticket_id = graph
            .create_ticket_from_eval(
                &eval_id,
                TicketDraft {
                    owner_id: "owner-1".into(),
                    scope: "read repo".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "v1".into(),
                    requires_approval: false,
                },
                5,
            )
            .unwrap();
        graph
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 6)
            .unwrap();
        (graph, thread_id, ticket_id)
    }

    fn memory(id: &str, content: &str) -> MemoryRecord {
        MemoryRecord::new(
            id,
            MemoryLayer::Semantic,
            MemoryKind::Preference,
            MemoryScope::bot("bot-1"),
            content,
            MemorySource::new(MemorySourceKind::UserMessage, "event"),
        )
    }

    #[test]
    fn builds_deterministic_goal_task_memory_view() {
        let (tasks, _, ticket_id) = task_graph();
        let goals = goal_graph(&ticket_id);
        let memories = vec![memory("mem-1", "prefers pnpm")];
        let view = build_context_graph(&goals, &tasks.snapshot(), &memories, 100);
        assert_eq!(view.counts().get(&ContextNodeKind::Goal), Some(&1usize));
        assert_eq!(
            view.counts().get(&ContextNodeKind::Commitment),
            Some(&1usize)
        );
        assert_eq!(view.counts().get(&ContextNodeKind::Ticket), Some(&1usize));
        assert_eq!(view.counts().get(&ContextNodeKind::Memory), Some(&1usize));
        assert_eq!(view.edges.len(), 3);
        assert!(view.node("memory:mem-1").is_some());
        assert_eq!(view.edges_from("commitment:commit-1").len(), 1);
        let rebuilt = build_context_graph(&goals, &tasks.snapshot(), &memories, 100);
        assert_eq!(view, rebuilt);
    }

    #[test]
    fn hard_forgotten_memory_is_absent_and_superseded_is_marked() {
        let (tasks, _, ticket_id) = task_graph();
        let goals = goal_graph(&ticket_id);
        let mut old = memory("mem-old", "old");
        old.hard_forgotten_ms = Some(10);
        let mut superseded = memory("mem-new", "new");
        superseded.superseded_by = Some("mem-old".into());
        let mut soft = memory("mem-soft", "soft forgotten");
        soft.soft_forgotten_ms = Some(20);
        let mut future = memory("mem-future", "future");
        future.valid_from_ms = 200;
        let mut expired = memory("mem-expired", "expired");
        expired.valid_to_ms = Some(50);
        let view = build_context_graph(
            &goals,
            &tasks.snapshot(),
            &[old, superseded, soft, future, expired],
            100,
        );
        assert!(view.node("memory:mem-old").is_none());
        assert!(view.node("memory:mem-soft").is_none());
        assert!(view.node("memory:mem-future").is_none());
        assert!(view.node("memory:mem-expired").is_none());
        assert_eq!(view.node("memory:mem-new").unwrap().status, "superseded");
        assert_eq!(
            view.node(&format!("ticket:{ticket_id}"))
                .unwrap()
                .scope
                .as_deref(),
            Some("read repo")
        );
    }

    #[test]
    fn cache_hits_and_invalidates_on_fingerprint_change() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ContextGraphCache::new(dir.path().join("context-cache.json"));
        let graph = ContextGraph::default();
        let (_, hit) = cache.get_or_build("first", || graph.clone()).unwrap();
        assert!(!hit);
        let (cached, hit) = cache
            .get_or_build("first", || panic!("cache should hit"))
            .unwrap();
        assert!(hit);
        assert_eq!(cached, graph);
        let (_, hit) = cache.get_or_build("second", || graph.clone()).unwrap();
        assert!(!hit);

        let goals = GoalGraph::default();
        let tasks = crate::task_graph::TaskGraph::default().snapshot();
        assert_eq!(
            fingerprint(&goals, &tasks, &[]),
            fingerprint(&goals, &tasks, &[])
        );
    }
}
