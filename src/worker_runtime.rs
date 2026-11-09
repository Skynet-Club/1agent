//! Deterministic Router/Worker scheduling contract.
//!
//! This layer assigns work to Worker slots and keeps same-Thread dispatch
//! sticky. It deliberately holds no tools and performs no execution; a real
//! Workshop adapter is a later integration step.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const ROUTER_SNAPSHOT_SCHEMA: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkerRuntimeError {
    #[error("worker id is unsafe")]
    UnsafeWorkerId,
    #[error("task id is unsafe")]
    UnsafeTaskId,
    #[error("thread id is unsafe")]
    UnsafeThreadId,
    #[error("worker {0} already exists")]
    WorkerExists(String),
    #[error("worker {0} not found")]
    UnknownWorker(String),
    #[error("worker slots must be 1..=64")]
    InvalidSlots,
    #[error("worker queue is full")]
    Backpressure,
    #[error("task {0} is not queued")]
    UnknownTask(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerTask {
    pub task_id: String,
    pub thread_id: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    pub task_id: String,
    pub thread_id: String,
    pub worker_id: String,
    pub queued: bool,
}

#[derive(Debug, Clone)]
struct WorkerSlot {
    active: usize,
    max_concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerSnapshot {
    pub worker_id: String,
    pub active: usize,
    pub max_concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueuedTaskSnapshot {
    pub task: WorkerTask,
    pub preferred_worker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouterSnapshot {
    pub schema_version: u32,
    pub queue_capacity: usize,
    pub workers: Vec<WorkerSnapshot>,
    pub thread_workers: BTreeMap<String, String>,
    pub queue: Vec<QueuedTaskSnapshot>,
}

#[derive(Debug, Error)]
pub enum WorkerStoreError {
    #[error("worker store io: {0}")]
    Io(#[from] std::io::Error),
    #[error("worker store json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("worker store is corrupt: {0}")]
    Corrupt(String),
}

#[derive(Debug, Default)]
pub struct RouterRuntime {
    workers: BTreeMap<String, WorkerSlot>,
    thread_workers: BTreeMap<String, String>,
    queue: VecDeque<(WorkerTask, Option<String>)>,
    queue_capacity: usize,
}

impl RouterRuntime {
    pub fn new(queue_capacity: usize) -> Self {
        Self {
            queue_capacity: queue_capacity.max(1),
            ..Self::default()
        }
    }

    pub fn register_worker(
        &mut self,
        worker_id: &str,
        max_concurrency: usize,
    ) -> Result<(), WorkerRuntimeError> {
        if !safe_id(worker_id) {
            return Err(WorkerRuntimeError::UnsafeWorkerId);
        }
        if !(1..=64).contains(&max_concurrency) {
            return Err(WorkerRuntimeError::InvalidSlots);
        }
        if self.workers.contains_key(worker_id) {
            return Err(WorkerRuntimeError::WorkerExists(worker_id.into()));
        }
        self.workers.insert(
            worker_id.into(),
            WorkerSlot {
                active: 0,
                max_concurrency,
            },
        );
        Ok(())
    }

    pub fn remove_worker(&mut self, worker_id: &str) -> Result<(), WorkerRuntimeError> {
        if self.workers.remove(worker_id).is_none() {
            return Err(WorkerRuntimeError::UnknownWorker(worker_id.into()));
        }
        self.thread_workers
            .retain(|_, assigned| assigned != worker_id);
        Ok(())
    }

    /// Assign a task to a worker. Same-thread tasks are sticky to the worker
    /// that first served the thread. If no slot is free, the task is queued.
    pub fn dispatch(&mut self, task: WorkerTask) -> Result<Dispatch, WorkerRuntimeError> {
        if !safe_id(&task.task_id) {
            return Err(WorkerRuntimeError::UnsafeTaskId);
        }
        if !safe_id(&task.thread_id) {
            return Err(WorkerRuntimeError::UnsafeThreadId);
        }
        if let Some(worker_id) = self.thread_workers.get(&task.thread_id).cloned() {
            if self.has_capacity(&worker_id) {
                self.occupy(&worker_id);
                return Ok(Dispatch {
                    task_id: task.task_id,
                    thread_id: task.thread_id,
                    worker_id,
                    queued: false,
                });
            }
            return self.enqueue(task, Some(worker_id));
        }
        if let Some(worker_id) = self.pick_worker() {
            self.thread_workers
                .insert(task.thread_id.clone(), worker_id.clone());
            self.occupy(&worker_id);
            return Ok(Dispatch {
                task_id: task.task_id,
                thread_id: task.thread_id,
                worker_id,
                queued: false,
            });
        }
        self.enqueue(task, None)
    }

    /// Mark a task complete and immediately drain one queued task.
    pub fn complete(&mut self, worker_id: &str) -> Result<Option<Dispatch>, WorkerRuntimeError> {
        let Some(slot) = self.workers.get_mut(worker_id) else {
            return Err(WorkerRuntimeError::UnknownWorker(worker_id.into()));
        };
        slot.active = slot.active.saturating_sub(1);
        self.drain_queue()
    }

    /// Release a slot without draining the queue. Used when graph assignment
    /// fails after scheduling so the worker capacity is not leaked.
    pub fn release(&mut self, worker_id: &str) -> Result<(), WorkerRuntimeError> {
        let Some(slot) = self.workers.get_mut(worker_id) else {
            return Err(WorkerRuntimeError::UnknownWorker(worker_id.into()));
        };
        slot.active = slot.active.saturating_sub(1);
        Ok(())
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    pub fn active(&self, worker_id: &str) -> Option<usize> {
        self.workers.get(worker_id).map(|slot| slot.active)
    }

    fn has_capacity(&self, worker_id: &str) -> bool {
        self.workers
            .get(worker_id)
            .is_some_and(|slot| slot.active < slot.max_concurrency)
    }

    fn occupy(&mut self, worker_id: &str) {
        if let Some(slot) = self.workers.get_mut(worker_id) {
            slot.active += 1;
        }
    }

    fn pick_worker(&self) -> Option<String> {
        self.workers
            .iter()
            .find(|(_, slot)| slot.active < slot.max_concurrency)
            .map(|(worker_id, _)| worker_id.clone())
    }

    fn enqueue(
        &mut self,
        task: WorkerTask,
        worker_id: Option<String>,
    ) -> Result<Dispatch, WorkerRuntimeError> {
        if self.queue.len() >= self.queue_capacity {
            return Err(WorkerRuntimeError::Backpressure);
        }
        let dispatch = Dispatch {
            task_id: task.task_id.clone(),
            thread_id: task.thread_id.clone(),
            worker_id: worker_id.clone().unwrap_or_default(),
            queued: true,
        };
        self.queue.push_back((task, worker_id));
        Ok(dispatch)
    }

    fn drain_queue(&mut self) -> Result<Option<Dispatch>, WorkerRuntimeError> {
        let position = self.queue.iter().position(|(_, preferred)| {
            preferred
                .as_deref()
                .is_none_or(|worker_id| self.has_capacity(worker_id))
        });
        let Some(position) = position else {
            return Ok(None);
        };
        let Some((task, preferred)) = self.queue.remove(position) else {
            return Ok(None);
        };
        let Some(worker_id) = preferred
            .filter(|worker_id| self.has_capacity(worker_id))
            .or_else(|| self.pick_worker())
        else {
            return Ok(None);
        };
        self.thread_workers
            .entry(task.thread_id.clone())
            .or_insert_with(|| worker_id.clone());
        self.occupy(&worker_id);
        Ok(Some(Dispatch {
            task_id: task.task_id,
            thread_id: task.thread_id,
            worker_id,
            queued: false,
        }))
    }
}

impl RouterRuntime {
    pub fn snapshot(&self) -> RouterSnapshot {
        RouterSnapshot {
            schema_version: ROUTER_SNAPSHOT_SCHEMA,
            queue_capacity: self.queue_capacity,
            workers: self
                .workers
                .iter()
                .map(|(worker_id, slot)| WorkerSnapshot {
                    worker_id: worker_id.clone(),
                    active: slot.active,
                    max_concurrency: slot.max_concurrency,
                })
                .collect(),
            thread_workers: self.thread_workers.clone(),
            queue: self
                .queue
                .iter()
                .map(|(task, preferred_worker)| QueuedTaskSnapshot {
                    task: task.clone(),
                    preferred_worker: preferred_worker.clone(),
                })
                .collect(),
        }
    }

    pub fn from_snapshot(snapshot: RouterSnapshot) -> Result<Self, WorkerStoreError> {
        if snapshot.schema_version != ROUTER_SNAPSHOT_SCHEMA {
            return Err(WorkerStoreError::Corrupt(
                "unsupported router snapshot schema".into(),
            ));
        }
        if snapshot.queue_capacity == 0 || snapshot.queue.len() > snapshot.queue_capacity {
            return Err(WorkerStoreError::Corrupt(
                "router queue capacity is invalid".into(),
            ));
        }
        let mut runtime = Self::new(snapshot.queue_capacity);
        for worker in snapshot.workers {
            if !safe_id(&worker.worker_id)
                || !(1..=64).contains(&worker.max_concurrency)
                || worker.active > worker.max_concurrency
            {
                return Err(WorkerStoreError::Corrupt(
                    "router worker snapshot is invalid".into(),
                ));
            }
            if runtime
                .workers
                .insert(
                    worker.worker_id,
                    WorkerSlot {
                        active: worker.active,
                        max_concurrency: worker.max_concurrency,
                    },
                )
                .is_some()
            {
                return Err(WorkerStoreError::Corrupt("duplicate router worker".into()));
            }
        }
        for (thread_id, worker_id) in snapshot.thread_workers {
            if !safe_id(&thread_id) || !runtime.workers.contains_key(&worker_id) {
                return Err(WorkerStoreError::Corrupt(
                    "router thread binding is invalid".into(),
                ));
            }
            runtime.thread_workers.insert(thread_id, worker_id);
        }
        for queued in snapshot.queue {
            if !safe_id(&queued.task.task_id) || !safe_id(&queued.task.thread_id) {
                return Err(WorkerStoreError::Corrupt(
                    "router queued task is invalid".into(),
                ));
            }
            if let Some(preferred) = &queued.preferred_worker
                && !runtime.workers.contains_key(preferred)
            {
                return Err(WorkerStoreError::Corrupt(
                    "router queued task has unknown worker".into(),
                ));
            }
            runtime
                .queue
                .push_back((queued.task, queued.preferred_worker));
        }
        Ok(runtime)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), WorkerStoreError> {
        let bytes = serde_json::to_vec_pretty(&self.snapshot())?;
        write_private_atomic(path.as_ref(), &bytes)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, WorkerStoreError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(WorkerStoreError::Corrupt(
                "router store must be a regular file".into(),
            ));
        }
        let bytes = std::fs::read(path)?;
        let snapshot: RouterSnapshot = serde_json::from_slice(&bytes)?;
        Self::from_snapshot(snapshot)
    }
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), WorkerStoreError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        crate::jsonl::ensure_parent_private(path)?;
    }
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("router.json");
    let temp = path.with_file_name(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<(), WorkerStoreError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_data()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        if let Some(parent) = path.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !value.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, thread: &str) -> WorkerTask {
        WorkerTask {
            task_id: id.into(),
            thread_id: thread.into(),
            summary: "work".into(),
        }
    }

    #[test]
    fn sticky_thread_dispatch_and_queue_backpressure() {
        let mut runtime = RouterRuntime::new(2);
        runtime.register_worker("worker-1", 1).unwrap();
        assert_eq!(
            runtime
                .dispatch(task("task-1", "thread-1"))
                .unwrap()
                .worker_id,
            "worker-1"
        );
        let queued = runtime.dispatch(task("task-2", "thread-1")).unwrap();
        assert!(queued.queued);
        assert_eq!(runtime.queued(), 1);
        let next = runtime.complete("worker-1").unwrap().unwrap();
        assert_eq!(next.task_id, "task-2");
        assert_eq!(next.worker_id, "worker-1");
        assert!(!next.queued);

        assert!(runtime.dispatch(task("task-3", "thread-1")).unwrap().queued);
        assert!(runtime.dispatch(task("task-4", "thread-2")).unwrap().queued);
        assert_eq!(runtime.queued(), 2);
        assert!(matches!(
            runtime.dispatch(task("task-5", "thread-3")),
            Err(WorkerRuntimeError::Backpressure)
        ));
    }

    #[test]
    fn worker_selection_is_deterministic_and_recovery_unbinds_threads() {
        let mut runtime = RouterRuntime::new(4);
        runtime.register_worker("worker-b", 1).unwrap();
        runtime.register_worker("worker-a", 1).unwrap();
        let first = runtime.dispatch(task("task-1", "thread-1")).unwrap();
        assert_eq!(first.worker_id, "worker-a");
        let second = runtime.dispatch(task("task-2", "thread-2")).unwrap();
        assert_eq!(second.worker_id, "worker-b");
        let third = runtime.dispatch(task("task-3", "thread-3")).unwrap();
        assert!(third.queued);

        runtime.remove_worker("worker-b").unwrap();
        assert_eq!(runtime.active("worker-b"), None);
        let next = runtime.complete("worker-a").unwrap().unwrap();
        assert_eq!(next.worker_id, "worker-a");
        assert_eq!(next.task_id, "task-3");
    }

    #[test]
    fn snapshot_round_trips_active_and_queued_work() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("router.json");
        let mut runtime = RouterRuntime::new(2);
        runtime.register_worker("worker-1", 1).unwrap();
        runtime.dispatch(task("task-1", "thread-1")).unwrap();
        runtime.dispatch(task("task-2", "thread-1")).unwrap();
        runtime.save(&path).unwrap();

        let mut loaded = RouterRuntime::load(&path).unwrap();
        assert_eq!(loaded.active("worker-1"), Some(1));
        assert_eq!(loaded.queued(), 1);
        let next = loaded.complete("worker-1").unwrap().unwrap();
        assert_eq!(next.task_id, "task-2");
        assert_eq!(next.worker_id, "worker-1");
    }

    #[test]
    fn invalid_inputs_fail_closed() {
        let mut runtime = RouterRuntime::new(1);
        assert_eq!(
            runtime.register_worker("bad/id", 1).unwrap_err(),
            WorkerRuntimeError::UnsafeWorkerId
        );
        assert_eq!(
            runtime.register_worker("worker-1", 0).unwrap_err(),
            WorkerRuntimeError::InvalidSlots
        );
        runtime.register_worker("worker-1", 1).unwrap();
        assert_eq!(
            runtime.register_worker("worker-1", 1).unwrap_err(),
            WorkerRuntimeError::WorkerExists("worker-1".into())
        );
        assert_eq!(
            runtime.complete("missing").unwrap_err(),
            WorkerRuntimeError::UnknownWorker("missing".into())
        );
    }
}
