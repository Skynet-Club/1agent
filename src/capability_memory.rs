//! Capability-path memory.
//!
//! Records which installed manifest resolved a capability requirement and how
//! that run ended. The resolver uses this as a small, explainable boost, never
//! as a hard override. No task-type labels are stored.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::capability_compiler::{CapabilityRequirement, CapabilityResolution};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityPathRecord {
    pub schema_version: u32,
    pub at_unix_ms: u128,
    pub requirement: CapabilityRequirement,
    pub manifest: String,
    pub status: String,
    pub artifact_verified: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CapabilityMemory {
    path: PathBuf,
    records: Vec<CapabilityPathRecord>,
}

impl CapabilityMemory {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, String> {
        let path = path.into();
        let mut memory = Self {
            path,
            records: Vec::new(),
        };
        if memory.path.exists() {
            memory.records = load_records(&memory.path)?;
        }
        Ok(memory)
    }

    pub fn records(&self) -> &[CapabilityPathRecord] {
        &self.records
    }

    pub fn record(&mut self, record: CapabilityPathRecord) -> Result<(), String> {
        if record.manifest.trim().is_empty() || record.requirement.verb.trim().is_empty() {
            return Err("capability path record is incomplete".into());
        }
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let line = serde_json::to_string(&record).map_err(|error| error.to_string())?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| error.to_string())?;
        writeln!(file, "{line}").map_err(|error| error.to_string())?;
        self.records.push(record);
        Ok(())
    }

    /// Small explainable boost for a manifest that previously completed the
    /// same capability signature.
    pub fn boost(&self, requirement: &CapabilityRequirement, manifest: &str) -> u16 {
        let successes = self
            .records
            .iter()
            .filter(|record| {
                record.manifest == manifest
                    && record.requirement.verb == requirement.verb
                    && record.requirement.outputs == requirement.outputs
                    && matches!(record.status.as_str(), "completed" | "conditional")
            })
            .count();
        (successes.min(3) as u16) * 5
    }

    pub fn apply(&self, resolution: &mut CapabilityResolution) {
        for requirement in &mut resolution.requirements {
            let mut candidates = requirement
                .selected
                .take()
                .into_iter()
                .chain(requirement.alternatives.drain(..))
                .collect::<Vec<_>>();
            for candidate in &mut candidates {
                let boost = self.boost(&requirement.requirement, &candidate.name);
                if boost > 0 {
                    candidate.score = candidate.score.saturating_add(boost);
                    candidate.reasons.push(format!("history +{boost}"));
                }
            }
            candidates.sort_by(|left, right| {
                right
                    .score
                    .cmp(&left.score)
                    .then_with(|| right.installed.cmp(&left.installed))
                    .then_with(|| left.name.cmp(&right.name))
            });
            requirement.selected = candidates
                .iter()
                .find(|candidate| candidate.installed)
                .cloned();
            requirement.alternatives = candidates
                .into_iter()
                .filter(|candidate| {
                    requirement.selected.as_ref().is_none_or(|selected| {
                        selected.manifest_id != candidate.manifest_id
                            || selected.name != candidate.name
                    })
                })
                .take(5)
                .collect();
        }
    }
}

fn load_records(path: &Path) -> Result<Vec<CapabilityPathRecord>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut records = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record: CapabilityPathRecord = serde_json::from_str(line).map_err(|error| {
            format!(
                "invalid capability path record at {}:{}: {error}",
                path.display(),
                line_number + 1
            )
        })?;
        if record.schema_version != 1 {
            return Err(format!(
                "unsupported capability path schema {}",
                record.schema_version
            ));
        }
        records.push(record);
    }
    Ok(records)
}

pub fn record_from_resolution(
    resolution: &CapabilityResolution,
    status: &str,
    artifact_verified: bool,
    now_ms: u128,
) -> Vec<CapabilityPathRecord> {
    resolution
        .requirements
        .iter()
        .filter_map(|requirement| {
            requirement
                .selected
                .as_ref()
                .map(|candidate| CapabilityPathRecord {
                    schema_version: 1,
                    at_unix_ms: now_ms,
                    requirement: requirement.requirement.clone(),
                    manifest: candidate.name.clone(),
                    status: status.into(),
                    artifact_verified,
                })
        })
        .collect()
}

/// Look up a candidate's install proposal from the manifest registry.
pub fn proposal_for(name: &str) -> Result<Option<InstallProposal>, String> {
    let manifests = crate::capability_compiler::manifests()?;
    let Some(manifest) = manifests.manifests.iter().find(|manifest| {
        manifest.name.eq_ignore_ascii_case(name) || manifest.id.eq_ignore_ascii_case(name)
    }) else {
        return Ok(None);
    };
    let Some(install) = manifest.install.as_ref() else {
        return Ok(None);
    };
    Ok(Some(InstallProposal {
        name: manifest.name.clone(),
        source: manifest.source.clone(),
        license: manifest.license.clone(),
        command: install.command.clone(),
        approval: install.approval.clone(),
        notes: install.notes.clone(),
        verification: manifest.verification.clone(),
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallProposal {
    pub name: String,
    pub source: String,
    pub license: String,
    pub command: String,
    pub approval: String,
    pub notes: String,
    pub verification: Vec<String>,
}

pub fn summarize(records: &[CapabilityPathRecord]) -> BTreeMap<String, u16> {
    let mut counts = BTreeMap::new();
    for record in records {
        if matches!(record.status.as_str(), "completed" | "conditional") {
            *counts.entry(record.manifest.clone()).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_boosts_are_path_based() {
        let dir = tempfile::tempdir().unwrap();
        let mut memory = CapabilityMemory::open(dir.path().join("paths.jsonl")).unwrap();
        let requirement = CapabilityRequirement {
            verb: "render".into(),
            inputs: vec!["structured_spec".into()],
            outputs: vec!["image/svg+xml".into()],
            constraints: Vec::new(),
            required: true,
        };
        memory
            .record(CapabilityPathRecord {
                schema_version: 1,
                at_unix_ms: 1,
                requirement: requirement.clone(),
                manifest: "manual-svg".into(),
                status: "completed".into(),
                artifact_verified: true,
            })
            .unwrap();
        assert_eq!(memory.boost(&requirement, "manual-svg"), 5);
        assert_eq!(memory.boost(&requirement, "archify"), 0);
        let reopened = CapabilityMemory::open(dir.path().join("paths.jsonl")).unwrap();
        assert_eq!(reopened.records().len(), 1);
    }
}
