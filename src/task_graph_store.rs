//! Persistent task-graph event log and application service.
//!
//! The store is intentionally append-only and single-writer. Replay verifies the
//! same hash chain as the in-memory graph; a mismatch fails closed.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::task_graph::{TaskGraph, TaskGraphError, TaskGraphEvent, TaskGraphSnapshot};

#[derive(Debug, Error)]
pub enum TaskGraphStoreError {
    #[error("task graph io: {0}")]
    Io(#[from] std::io::Error),
    #[error("task graph json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("task graph replay: {0}")]
    Replay(#[from] TaskGraphError),
    #[error("task graph event sequence is not contiguous")]
    NonContiguousSequence,
    #[error("task graph store was opened read-only")]
    ReadOnly,
    #[error(
        "task graph was modified by another writer (expected {expected_bytes} bytes, found {actual_bytes})"
    )]
    ConcurrentWrite {
        expected_bytes: u64,
        actual_bytes: u64,
    },
}

#[derive(Debug, Clone)]
pub struct TaskGraphStore {
    path: PathBuf,
    next_seq: u64,
    persisted_bytes: u64,
    writable: bool,
}

impl TaskGraphStore {
    /// Open or create an empty graph. The file is created on first persist.
    pub fn open(path: impl Into<PathBuf>) -> Result<(Self, TaskGraph), TaskGraphStoreError> {
        Self::open_with(path.into(), true)
    }

    /// Open without mutating the log; a torn tail is rejected instead of repaired.
    pub fn open_read_only(
        path: impl Into<PathBuf>,
    ) -> Result<(Self, TaskGraph), TaskGraphStoreError> {
        Self::open_with(path.into(), false)
    }

    fn open_with(path: PathBuf, repair: bool) -> Result<(Self, TaskGraph), TaskGraphStoreError> {
        let _lock = if repair {
            Some(crate::jsonl::FileLock::exclusive(&path)?)
        } else {
            None
        };
        if repair {
            crate::jsonl::repair_torn_tail(&path)?;
        }
        let mut events = Vec::new();
        if path.exists() {
            let file = File::open(&path)?;
            for line in BufReader::new(file).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                events.push(serde_json::from_str::<TaskGraphEvent>(&line)?);
            }
        }
        let graph = TaskGraph::replay(&events)?;
        let next_seq = events.last().map(|event| event.seq).unwrap_or(0);
        let persisted_bytes = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        drop(_lock);
        Ok((
            Self {
                path,
                next_seq,
                persisted_bytes,
                writable: repair,
            },
            graph,
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append events that are newer than the last persisted sequence.
    pub fn persist(&mut self, graph: &TaskGraph) -> Result<usize, TaskGraphStoreError> {
        if !self.writable {
            return Err(TaskGraphStoreError::ReadOnly);
        }
        let new_events: Vec<&TaskGraphEvent> = graph
            .events()
            .iter()
            .filter(|event| event.seq > self.next_seq)
            .collect();
        for (index, event) in new_events.iter().enumerate() {
            let expected = self.next_seq + 1 + index as u64;
            if event.seq != expected {
                return Err(TaskGraphStoreError::NonContiguousSequence);
            }
        }
        if new_events.is_empty() {
            return Ok(0);
        }
        let mut buffer = String::new();
        for event in &new_events {
            buffer.push_str(&serde_json::to_string(event)?);
            buffer.push('\n');
        }
        self.persisted_bytes =
            crate::jsonl::append_batch_locked(&self.path, self.persisted_bytes, &buffer).map_err(
                |error| match error {
                    crate::jsonl::LockedAppendError::Io(error) => TaskGraphStoreError::Io(error),
                    crate::jsonl::LockedAppendError::ConcurrentWrite {
                        expected_bytes,
                        actual_bytes,
                    } => TaskGraphStoreError::ConcurrentWrite {
                        expected_bytes,
                        actual_bytes,
                    },
                },
            )?;
        if let Some(last) = new_events.last() {
            self.next_seq = last.seq;
        }
        Ok(new_events.len())
    }
}

#[derive(Debug, Clone)]
pub struct TaskGraphService {
    graph: TaskGraph,
    store: TaskGraphStore,
}

impl TaskGraphService {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, TaskGraphStoreError> {
        let (store, graph) = TaskGraphStore::open(path)?;
        Ok(Self { graph, store })
    }

    pub fn open_read_only(path: impl Into<PathBuf>) -> Result<Self, TaskGraphStoreError> {
        let (store, graph) = TaskGraphStore::open_read_only(path)?;
        Ok(Self { graph, store })
    }

    pub fn graph(&self) -> &TaskGraph {
        &self.graph
    }

    pub fn snapshot(&self) -> TaskGraphSnapshot {
        self.graph.snapshot()
    }

    pub fn store_path(&self) -> &Path {
        self.store.path()
    }

    /// Replay the log again after another writer appended. Governance state
    /// (authority, standing approvals, action log) is preserved; only the
    /// graph snapshot is replaced.
    fn reload(&mut self) -> Result<(), TaskGraphStoreError> {
        let (store, graph) = TaskGraphStore::open(self.store.path())?;
        self.store = store;
        self.graph = graph;
        Ok(())
    }

    /// Mutate a candidate graph and persist any newly appended events. On
    /// failure the candidate is discarded, so the in-memory graph never carries
    /// events that are absent from the log; the caller may retry the operation.
    pub fn with_graph_mut<R>(
        &mut self,
        operation: impl FnOnce(&mut TaskGraph) -> Result<R, TaskGraphError>,
    ) -> Result<R, TaskGraphStoreError> {
        let mut candidate = self.graph.clone();
        let result = operation(&mut candidate)?;
        match self.store.persist(&candidate) {
            Ok(_) => {
                self.graph = candidate;
                Ok(result)
            }
            Err(error @ TaskGraphStoreError::ConcurrentWrite { .. }) => {
                let _ = self.reload();
                Err(error)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_graph::{
        ApprovalDecision, Decision, MandateStrength, ReportStatus, TicketDraft, TicketStatus,
        Trigger, TriggerSource,
    };

    fn trigger() -> Trigger {
        Trigger {
            trigger_id: "trigger-1".into(),
            source: TriggerSource::Web,
            source_event_id: "event-1".into(),
            summary: "persist me".into(),
            received_ms: 1,
        }
    }

    #[test]
    fn persists_and_replays_ticket_flow() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        let (thread_id, ticket_id) = service
            .with_graph_mut(|graph| {
                let thread_id = graph.open_thread("persisted", 10)?;
                let trigger_id = graph.ingest_trigger(&thread_id, trigger(), 11)?.trigger_id;
                let eval_id = graph.begin_eval(&thread_id, &trigger_id, 12)?;
                graph.decide_eval(&eval_id, Decision::Accept, "ok", 13)?;
                let ticket_id = graph.create_ticket_from_eval(
                    &eval_id,
                    TicketDraft {
                        owner_id: "owner-1".into(),
                        scope: "read".into(),
                        strength: MandateStrength::ReadOnly,
                        policy_version: "v1".into(),
                        requires_approval: true,
                    },
                    14,
                )?;
                graph.approve_ticket(
                    &ticket_id,
                    "owner-1",
                    ApprovalDecision::AllowOnce,
                    None,
                    15,
                )?;
                graph.assign_ticket(&ticket_id, "worker-1", None, 16)?;
                graph.start_ticket(&ticket_id, "worker-1", 17)?;
                graph.report_ticket(
                    &ticket_id,
                    "worker-1",
                    ReportStatus::Ready,
                    "ready",
                    Some("sha256:1".into()),
                    18,
                )?;
                Ok((thread_id, ticket_id))
            })
            .unwrap();

        assert!(path.exists());
        let reopened = TaskGraphService::open(&path).unwrap();
        assert_eq!(
            reopened.graph().ticket(&ticket_id).unwrap().status,
            TicketStatus::Ready
        );
        assert_eq!(
            reopened.graph().thread(&thread_id).unwrap().ticket_ids,
            vec![ticket_id]
        );
        assert_eq!(reopened.snapshot().approvals.len(), 1);
        assert_eq!(reopened.snapshot().reports.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn read_only_service_cannot_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open_read_only(&path).unwrap();
        let error = service
            .with_graph_mut(|graph| graph.open_thread("blocked", 1))
            .unwrap_err();
        assert!(matches!(error, TaskGraphStoreError::ReadOnly));
    }

    #[test]
    fn opening_a_store_does_not_change_parent_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o775)).unwrap();
        let path = dir.path().join("task-graph.jsonl");
        TaskGraphService::open(&path).unwrap();
        let mode = std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o775);
    }

    #[test]
    fn stale_writer_fails_closed_after_another_writer_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut first = TaskGraphService::open(&path).unwrap();
        let mut second = TaskGraphService::open(&path).unwrap();
        first
            .with_graph_mut(|graph| graph.open_thread("first", 10))
            .unwrap();
        let error = second
            .with_graph_mut(|graph| graph.open_thread("second", 11))
            .unwrap_err();
        assert!(matches!(
            error,
            TaskGraphStoreError::ConcurrentWrite {
                expected_bytes: 0,
                actual_bytes: _
            }
        ));
        second
            .with_graph_mut(|graph| graph.open_thread("second", 11))
            .unwrap();
        let reopened = TaskGraphService::open(&path).unwrap();
        assert_eq!(reopened.snapshot().threads.len(), 2);
    }

    #[test]
    fn torn_tail_is_repaired_and_append_continues() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        service
            .with_graph_mut(|graph| graph.open_thread("before crash", 10))
            .unwrap();
        drop(service);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{\"seq\":2,\"kind\":").unwrap();
        file.sync_all().unwrap();
        drop(file);

        let mut reopened = TaskGraphService::open(&path).unwrap();
        assert_eq!(reopened.graph().events().len(), 1);
        reopened
            .with_graph_mut(|graph| graph.open_thread("after crash", 20))
            .unwrap();
        drop(reopened);

        let verified = TaskGraphService::open(&path).unwrap();
        assert_eq!(verified.graph().events().len(), 2);
        assert_eq!(verified.snapshot().threads.len(), 2);
    }

    #[test]
    fn read_only_open_does_not_rewrite_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        service
            .with_graph_mut(|graph| graph.open_thread("persisted", 10))
            .unwrap();
        drop(service);
        let text = std::fs::read_to_string(&path).unwrap();
        let without_newline = text.trim_end_matches('\n').to_string();
        std::fs::write(&path, &without_newline).unwrap();

        let inspected = TaskGraphService::open_read_only(&path).unwrap();
        assert_eq!(inspected.graph().events().len(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), without_newline);
    }

    #[test]
    fn replay_rejects_a_tampered_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        service
            .with_graph_mut(|graph| graph.open_thread("persisted", 10))
            .unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        text = text.replacen("\"topic\":\"persisted\"", "\"topic\":\"tampered\"", 1);
        std::fs::write(&path, text).unwrap();
        assert!(TaskGraphService::open(&path).is_err());
    }

    #[test]
    fn cross_thread_duplicate_trigger_keeps_the_sequence_contiguous() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        let (thread_a, trigger_id) = service
            .with_graph_mut(|graph| {
                let thread_a = graph.open_thread("thread-a", 10)?;
                let thread_b = graph.open_thread("thread-b", 11)?;
                let trigger = trigger();
                let trigger_id = trigger.trigger_id.clone();
                graph.ingest_trigger(&thread_a, trigger.clone(), 12)?;
                graph.ingest_trigger(&thread_b, trigger, 13)?;
                Ok((thread_a, trigger_id))
            })
            .unwrap();
        let eval_id = service
            .with_graph_mut(|graph| graph.begin_eval(&thread_a, &trigger_id, 14))
            .unwrap();
        service
            .with_graph_mut(|graph| graph.decide_eval(&eval_id, Decision::Accept, "ok", 15))
            .unwrap();

        let reopened = TaskGraphService::open(&path).unwrap();
        assert_eq!(reopened.graph().events().len(), 6);
        assert_eq!(
            reopened.graph().eval(&eval_id).unwrap().decision,
            Some(Decision::Accept)
        );
    }

    #[test]
    fn failed_operation_does_not_persist_phantom_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        let result: Result<(), TaskGraphStoreError> = service.with_graph_mut(|graph| {
            graph.open_thread("phantom", 10)?;
            Err(TaskGraphError::UnknownTicket("missing".into()))
        });
        assert!(result.is_err());
        assert!(service.graph().events().is_empty());
        assert!(!path.exists());
        let reopened = TaskGraphService::open(&path).unwrap();
        assert!(reopened.graph().events().is_empty());
    }

    #[test]
    fn persist_is_idempotent_when_no_new_events_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("task-graph.jsonl");
        let mut service = TaskGraphService::open(&path).unwrap();
        service
            .with_graph_mut(|graph| graph.open_thread("persisted", 10))
            .unwrap();
        assert_eq!(service.store.persist(&service.graph).unwrap(), 0);
    }
}
