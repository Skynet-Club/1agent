//! Source adapters normalize external events into task-graph triggers.
//!
//! Transport parsing stays outside this layer: an adapter receives a typed
//! event (adapter id, external id, summary) and produces a deterministic
//! `Trigger`. The registry owns enablement, per-adapter limits and delivery
//! health so a silent source can be escalated by maintenance.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const SOURCE_SNAPSHOT_SCHEMA: u32 = 1;

use crate::governed_task_graph::GovernedTaskGraph;
use crate::task_graph::{Trigger, TriggerIngest, TriggerSource};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceAdapterError {
    #[error("source adapter id must not be empty")]
    EmptyAdapterId,
    #[error("source adapter {0} already exists")]
    AdapterExists(String),
    #[error("source adapter {0} is not registered")]
    UnknownAdapter(String),
    #[error("source adapter {0} is disabled")]
    AdapterDisabled(String),
    #[error("source adapter summary limit must be greater than zero")]
    InvalidSummaryLimit,
    #[error("source event external id must not be empty")]
    EmptyExternalId,
    #[error("source event summary must not be empty")]
    EmptySummary,
    #[error("source event summary exceeds {0} characters")]
    SummaryTooLong(usize),
    #[error("source adapter ingest: {0}")]
    Graph(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceAdapterSpec {
    pub adapter_id: String,
    pub source: TriggerSource,
    pub enabled: bool,
    pub max_summary_chars: usize,
    /// Emit a maintenance warning after this much silence, if set.
    pub stale_after_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceEvent {
    pub adapter_id: String,
    pub external_id: String,
    pub summary: String,
    pub received_ms: u128,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceHealth {
    pub deliveries: u64,
    pub duplicates: u64,
    pub rejected: u64,
    pub last_delivery_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceWarning {
    NeverDelivered {
        adapter_id: String,
        at_ms: u128,
    },
    Stale {
        adapter_id: String,
        idle_ms: u128,
        at_ms: u128,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceAdapterSnapshot {
    pub schema_version: u32,
    pub specs: Vec<SourceAdapterSpec>,
    pub health: Vec<(String, SourceHealth)>,
}

#[derive(Debug, Error)]
pub enum SourceStoreError {
    #[error("source store io: {0}")]
    Io(#[from] std::io::Error),
    #[error("source store json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("source adapter: {0}")]
    Adapter(#[from] SourceAdapterError),
    #[error("source store is corrupt: {0}")]
    Corrupt(String),
}

#[derive(Debug, Default)]
pub struct SourceAdapterRegistry {
    specs: BTreeMap<String, SourceAdapterSpec>,
    health: BTreeMap<String, SourceHealth>,
}

impl SourceAdapterRegistry {
    pub fn register(&mut self, spec: SourceAdapterSpec) -> Result<(), SourceAdapterError> {
        if spec.adapter_id.trim().is_empty() {
            return Err(SourceAdapterError::EmptyAdapterId);
        }
        if spec.max_summary_chars == 0 {
            return Err(SourceAdapterError::InvalidSummaryLimit);
        }
        if self.specs.contains_key(&spec.adapter_id) {
            return Err(SourceAdapterError::AdapterExists(spec.adapter_id));
        }
        let adapter_id = spec.adapter_id.clone();
        self.specs.insert(adapter_id.clone(), spec);
        self.health.insert(adapter_id, SourceHealth::default());
        Ok(())
    }

    pub fn spec(&self, adapter_id: &str) -> Option<&SourceAdapterSpec> {
        self.specs.get(adapter_id)
    }

    pub fn health(&self, adapter_id: &str) -> Option<&SourceHealth> {
        self.health.get(adapter_id)
    }

    /// Normalize without ingesting; this is the unit-testable adapter seam and
    /// does not count as a delivery.
    pub fn normalize(&self, event: SourceEvent) -> Result<Trigger, SourceAdapterError> {
        self.build_trigger(&event)
    }

    /// Normalize and append to a thread. Duplicate external events still count
    /// as deliveries because the graph reports them explicitly.
    pub fn ingest(
        &mut self,
        graph: &mut GovernedTaskGraph,
        thread_id: &str,
        event: SourceEvent,
        now_ms: u128,
    ) -> Result<TriggerIngest, SourceAdapterError> {
        let trigger = self.build_trigger(&event)?;
        let health = self
            .health
            .get_mut(&event.adapter_id)
            .ok_or_else(|| SourceAdapterError::UnknownAdapter(event.adapter_id.clone()))?;
        let ingest = match graph.ingest_trigger(thread_id, trigger, now_ms) {
            Ok(ingest) => ingest,
            Err(error) => {
                health.rejected = health.rejected.saturating_add(1);
                return Err(SourceAdapterError::Graph(error.to_string()));
            }
        };
        health.deliveries = health.deliveries.saturating_add(1);
        health.last_delivery_ms = Some(now_ms);
        if ingest.duplicate {
            health.duplicates = health.duplicates.saturating_add(1);
        }
        Ok(ingest)
    }

    pub fn maintain(&self, now_ms: u128) -> Vec<SourceWarning> {
        let mut warnings = Vec::new();
        for spec in self.specs.values() {
            if !spec.enabled {
                continue;
            }
            let Some(stale_after_ms) = spec.stale_after_ms else {
                continue;
            };
            match self
                .health
                .get(&spec.adapter_id)
                .and_then(|health| health.last_delivery_ms)
            {
                None => warnings.push(SourceWarning::NeverDelivered {
                    adapter_id: spec.adapter_id.clone(),
                    at_ms: now_ms,
                }),
                Some(last) => {
                    let idle_ms = now_ms.saturating_sub(last);
                    if idle_ms > stale_after_ms {
                        warnings.push(SourceWarning::Stale {
                            adapter_id: spec.adapter_id.clone(),
                            idle_ms,
                            at_ms: now_ms,
                        });
                    }
                }
            }
        }
        warnings
    }

    pub fn snapshot(&self) -> SourceAdapterSnapshot {
        SourceAdapterSnapshot {
            schema_version: SOURCE_SNAPSHOT_SCHEMA,
            specs: self.specs.values().cloned().collect(),
            health: self
                .health
                .iter()
                .map(|(id, h)| (id.clone(), h.clone()))
                .collect(),
        }
    }

    pub fn from_snapshot(snapshot: SourceAdapterSnapshot) -> Result<Self, SourceStoreError> {
        if snapshot.schema_version != SOURCE_SNAPSHOT_SCHEMA {
            return Err(SourceStoreError::Corrupt(
                "unsupported source snapshot schema".into(),
            ));
        }
        let mut registry = Self::default();
        for spec in snapshot.specs {
            registry.register(spec)?;
        }
        for (adapter_id, health) in snapshot.health {
            if !registry.health.contains_key(&adapter_id) {
                return Err(SourceStoreError::Corrupt(
                    "health entry has no adapter spec".into(),
                ));
            }
            registry.health.insert(adapter_id, health);
        }
        Ok(registry)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SourceStoreError> {
        let bytes = serde_json::to_vec_pretty(&self.snapshot())?;
        write_private_atomic(path.as_ref(), &bytes)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, SourceStoreError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(SourceStoreError::Corrupt(
                "source store must be a regular file".into(),
            ));
        }
        let bytes = std::fs::read(path)?;
        let snapshot: SourceAdapterSnapshot = serde_json::from_slice(&bytes)?;
        Self::from_snapshot(snapshot)
    }

    fn build_trigger(&self, event: &SourceEvent) -> Result<Trigger, SourceAdapterError> {
        let spec = self
            .specs
            .get(&event.adapter_id)
            .ok_or_else(|| SourceAdapterError::UnknownAdapter(event.adapter_id.clone()))?;
        if !spec.enabled {
            return Err(SourceAdapterError::AdapterDisabled(
                event.adapter_id.clone(),
            ));
        }
        if event.external_id.trim().is_empty() {
            return Err(SourceAdapterError::EmptyExternalId);
        }
        if event.summary.trim().is_empty() {
            return Err(SourceAdapterError::EmptySummary);
        }
        if event.summary.chars().count() > spec.max_summary_chars {
            return Err(SourceAdapterError::SummaryTooLong(spec.max_summary_chars));
        }
        Ok(Trigger {
            trigger_id: format!("trigger:{}:{}", spec.adapter_id, event.external_id),
            source: spec.source,
            source_event_id: format!("{}:{}", spec.adapter_id, event.external_id),
            summary: event.summary.clone(),
            received_ms: event.received_ms,
        })
    }
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), SourceStoreError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        crate::jsonl::ensure_parent_private(path)?;
    }
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("sources.json");
    let temp = path.with_file_name(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<(), SourceStoreError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, enabled: bool, stale_after_ms: Option<u128>) -> SourceAdapterSpec {
        SourceAdapterSpec {
            adapter_id: id.into(),
            source: TriggerSource::Webhook,
            enabled,
            max_summary_chars: 32,
            stale_after_ms,
        }
    }

    fn event(adapter_id: &str, external_id: &str, summary: &str) -> SourceEvent {
        SourceEvent {
            adapter_id: adapter_id.into(),
            external_id: external_id.into(),
            summary: summary.into(),
            received_ms: 10,
        }
    }

    #[test]
    fn normalizes_and_ingests_duplicate_events_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let mut graph = GovernedTaskGraph::open(dir.path().join("graph.jsonl")).unwrap();
        let thread_id = graph.open_thread("source", 1).unwrap();
        let mut registry = SourceAdapterRegistry::default();
        registry.register(spec("webhook", true, None)).unwrap();

        let trigger = registry
            .normalize(event("webhook", "e-1", "hello"))
            .unwrap();
        assert_eq!(trigger.trigger_id, "trigger:webhook:e-1");
        assert_eq!(trigger.source, TriggerSource::Webhook);
        assert_eq!(trigger.source_event_id, "webhook:e-1");

        let first = registry
            .ingest(&mut graph, &thread_id, event("webhook", "e-1", "hello"), 11)
            .unwrap();
        let second = registry
            .ingest(&mut graph, &thread_id, event("webhook", "e-1", "hello"), 12)
            .unwrap();
        assert!(!first.duplicate);
        assert!(second.duplicate);
        assert_eq!(second.trigger_id, first.trigger_id);
        let health = registry.health("webhook").unwrap();
        assert_eq!(health.deliveries, 2);
        assert_eq!(health.duplicates, 1);
        assert_eq!(health.last_delivery_ms, Some(12));
    }

    #[test]
    fn rejects_disabled_unknown_and_oversized_events() {
        let mut registry = SourceAdapterRegistry::default();
        registry.register(spec("webhook", true, None)).unwrap();
        registry.register(spec("off", false, None)).unwrap();

        assert_eq!(
            registry.normalize(event("missing", "e", "x")).unwrap_err(),
            SourceAdapterError::UnknownAdapter("missing".into())
        );
        assert_eq!(
            registry.normalize(event("off", "e", "x")).unwrap_err(),
            SourceAdapterError::AdapterDisabled("off".into())
        );
        assert_eq!(
            registry.normalize(event("webhook", " ", "x")).unwrap_err(),
            SourceAdapterError::EmptyExternalId
        );
        assert_eq!(
            registry.normalize(event("webhook", "e", " ")).unwrap_err(),
            SourceAdapterError::EmptySummary
        );
        let long = "x".repeat(33);
        assert_eq!(
            registry
                .normalize(event("webhook", "e", &long))
                .unwrap_err(),
            SourceAdapterError::SummaryTooLong(32)
        );
        assert_eq!(registry.health("webhook").unwrap().deliveries, 0);
    }

    #[test]
    fn registry_persists_specs_and_health() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sources.json");
        let mut registry = SourceAdapterRegistry::default();
        registry.register(spec("webhook", true, Some(100))).unwrap();
        let mut graph = GovernedTaskGraph::open(dir.path().join("graph.jsonl")).unwrap();
        let thread_id = graph.open_thread("persist", 1).unwrap();
        registry
            .ingest(&mut graph, &thread_id, event("webhook", "e-1", "x"), 11)
            .unwrap();
        registry.save(&path).unwrap();

        let loaded = SourceAdapterRegistry::load(&path).unwrap();
        assert!(loaded.spec("webhook").is_some());
        assert_eq!(loaded.health("webhook").unwrap().deliveries, 1);
        assert_eq!(loaded.health("webhook").unwrap().last_delivery_ms, Some(11));
        assert_eq!(
            loaded.maintain(200),
            vec![SourceWarning::Stale {
                adapter_id: "webhook".into(),
                idle_ms: 189,
                at_ms: 200
            }]
        );
    }

    #[test]
    fn maintenance_warns_about_never_delivered_and_stale_adapters() {
        let mut registry = SourceAdapterRegistry::default();
        registry.register(spec("fresh", true, Some(100))).unwrap();
        registry.register(spec("silent", true, Some(100))).unwrap();
        registry
            .register(spec("ignored", false, Some(100)))
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut graph = GovernedTaskGraph::open(dir.path().join("graph.jsonl")).unwrap();
        let thread_id = graph.open_thread("maintenance", 1).unwrap();
        registry
            .ingest(&mut graph, &thread_id, event("fresh", "e", "x"), 11)
            .unwrap();

        let warnings = registry.maintain(200);
        assert_eq!(
            warnings,
            vec![
                SourceWarning::Stale {
                    adapter_id: "fresh".into(),
                    idle_ms: 189,
                    at_ms: 200
                },
                SourceWarning::NeverDelivered {
                    adapter_id: "silent".into(),
                    at_ms: 200
                }
            ]
        );
        let warnings = registry.maintain(50);
        assert_eq!(
            warnings,
            vec![SourceWarning::NeverDelivered {
                adapter_id: "silent".into(),
                at_ms: 50
            }]
        );
    }
}
