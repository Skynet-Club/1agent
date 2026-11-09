//! Reflection, lessons and a human-gated evolution ledger.
//!
//! The ledger records proposals and decisions, not silent self-modification.
//! Proposals can only target skills, routines or strategy notes; authority,
//! policy, audit and persona changes are not representable here. Approval must
//! come from a different actor than the proposer. Every transition is a
//! hash-chained receipt.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::RiskLevel;

const MAX_TEXT_CHARS: usize = 2_000;
const EVOLUTION_SNAPSHOT_SCHEMA: u32 = 1;

#[derive(Debug, Error)]
pub enum EvolutionError {
    #[error("proposal id must not be empty")]
    EmptyProposalId,
    #[error("proposal {0} already exists")]
    ProposalExists(String),
    #[error("proposal {0} is not found")]
    UnknownProposal(String),
    #[error("proposer must not be empty")]
    EmptyProposer,
    #[error("summary must be 1..={MAX_TEXT_CHARS} characters")]
    InvalidSummary,
    #[error("evidence is required")]
    MissingEvidence,
    #[error("evidence reference must not be empty")]
    EmptyEvidence,
    #[error("rollback plan is required")]
    MissingRollback,
    #[error("proposal {0} is not in state {1:?}")]
    InvalidState(String, EvolutionStatus),
    #[error("proposer cannot approve their own proposal")]
    SelfApproval,
    #[error("receipt hash mismatch at {0}")]
    HashMismatch(u64),
    #[error("proposal digest mismatch for {0}")]
    ProposalDigestMismatch(String),
    #[error("receipt sequence is not contiguous")]
    NonContiguousSequence,
    #[error("evolution json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("evolution ledger is corrupt: {0}")]
    Corrupt(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionKind {
    SkillDraft,
    RoutineDraft,
    StrategyNote,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionStatus {
    Proposed,
    Approved,
    Applied,
    Rejected,
    RolledBack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionAction {
    Proposed,
    Approved,
    Applied,
    Rejected,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvolutionProposalDraft {
    pub proposal_id: String,
    pub proposer: String,
    pub kind: EvolutionKind,
    pub summary: String,
    pub evidence: Vec<String>,
    pub rollback_plan: String,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvolutionProposal {
    pub proposal_id: String,
    pub proposer: String,
    pub kind: EvolutionKind,
    pub summary: String,
    pub evidence: Vec<String>,
    pub rollback_plan: String,
    pub risk: RiskLevel,
    pub status: EvolutionStatus,
    pub created_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvolutionReceipt {
    pub seq: u64,
    pub receipt_id: String,
    pub proposal_id: String,
    pub proposal_sha256: String,
    pub action: EvolutionAction,
    pub actor_id: String,
    pub at_ms: u128,
    pub previous_sha256: Option<String>,
    pub receipt_sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvolutionLedger {
    pub proposals: Vec<EvolutionProposal>,
    pub receipts: Vec<EvolutionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EvolutionSnapshot {
    pub schema_version: u32,
    pub ledger: EvolutionLedger,
}

#[derive(Debug, Error)]
pub enum EvolutionStoreError {
    #[error("evolution store io: {0}")]
    Io(#[from] std::io::Error),
    #[error("evolution store json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("evolution: {0}")]
    Evolution(#[from] EvolutionError),
    #[error("evolution store is corrupt: {0}")]
    Corrupt(String),
    #[error("evolution store was modified by another writer (expected {expected}, found {actual})")]
    ConcurrentWrite { expected: String, actual: String },
}

#[derive(Debug)]
pub struct EvolutionStore {
    path: PathBuf,
    registry: EvolutionRegistry,
    loaded_sha256: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EvolutionRegistry {
    proposals: BTreeMap<String, EvolutionProposal>,
    receipts: Vec<EvolutionReceipt>,
    last_sha256: Option<String>,
}

impl EvolutionRegistry {
    pub fn propose(
        &mut self,
        draft: EvolutionProposalDraft,
        now_ms: u128,
    ) -> Result<(), EvolutionError> {
        if draft.proposal_id.trim().is_empty() {
            return Err(EvolutionError::EmptyProposalId);
        }
        if draft.proposer.trim().is_empty() {
            return Err(EvolutionError::EmptyProposer);
        }
        if self.proposals.contains_key(&draft.proposal_id) {
            return Err(EvolutionError::ProposalExists(draft.proposal_id));
        }
        validate_text(&draft.summary, EvolutionError::InvalidSummary)?;
        validate_text(&draft.rollback_plan, EvolutionError::MissingRollback)?;
        if draft.evidence.is_empty() {
            return Err(EvolutionError::MissingEvidence);
        }
        if draft.evidence.iter().any(|item| item.trim().is_empty()) {
            return Err(EvolutionError::EmptyEvidence);
        }
        let proposal = EvolutionProposal {
            proposal_id: draft.proposal_id,
            proposer: draft.proposer,
            kind: draft.kind,
            summary: draft.summary,
            evidence: draft.evidence,
            rollback_plan: draft.rollback_plan,
            risk: draft.risk,
            status: EvolutionStatus::Proposed,
            created_ms: now_ms,
        };
        let receipt_proposal_id = proposal.proposal_id.clone();
        let receipt_actor = proposal.proposer.clone();
        let proposal_sha256 = proposal_digest(&proposal)?;
        self.proposals
            .insert(proposal.proposal_id.clone(), proposal);
        self.receipt(
            receipt_proposal_id,
            proposal_sha256,
            EvolutionAction::Proposed,
            receipt_actor,
            now_ms,
        )
    }

    pub fn approve(
        &mut self,
        proposal_id: &str,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<(), EvolutionError> {
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or_else(|| EvolutionError::UnknownProposal(proposal_id.into()))?;
        if proposal.status != EvolutionStatus::Proposed {
            return Err(EvolutionError::InvalidState(
                proposal_id.into(),
                proposal.status,
            ));
        }
        if proposal.proposer == actor_id {
            return Err(EvolutionError::SelfApproval);
        }
        self.transition(
            proposal_id,
            EvolutionStatus::Approved,
            EvolutionAction::Approved,
            actor_id,
            now_ms,
        )
    }

    pub fn reject(
        &mut self,
        proposal_id: &str,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<(), EvolutionError> {
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or_else(|| EvolutionError::UnknownProposal(proposal_id.into()))?;
        if proposal.status != EvolutionStatus::Proposed {
            return Err(EvolutionError::InvalidState(
                proposal_id.into(),
                proposal.status,
            ));
        }
        if proposal.proposer == actor_id {
            return Err(EvolutionError::SelfApproval);
        }
        self.transition(
            proposal_id,
            EvolutionStatus::Rejected,
            EvolutionAction::Rejected,
            actor_id,
            now_ms,
        )
    }

    pub fn apply(
        &mut self,
        proposal_id: &str,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<(), EvolutionError> {
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or_else(|| EvolutionError::UnknownProposal(proposal_id.into()))?;
        if proposal.status != EvolutionStatus::Approved {
            return Err(EvolutionError::InvalidState(
                proposal_id.into(),
                proposal.status,
            ));
        }
        self.transition(
            proposal_id,
            EvolutionStatus::Applied,
            EvolutionAction::Applied,
            actor_id,
            now_ms,
        )
    }

    pub fn rollback(
        &mut self,
        proposal_id: &str,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<(), EvolutionError> {
        let proposal = self
            .proposals
            .get(proposal_id)
            .ok_or_else(|| EvolutionError::UnknownProposal(proposal_id.into()))?;
        if proposal.status != EvolutionStatus::Applied {
            return Err(EvolutionError::InvalidState(
                proposal_id.into(),
                proposal.status,
            ));
        }
        self.transition(
            proposal_id,
            EvolutionStatus::RolledBack,
            EvolutionAction::RolledBack,
            actor_id,
            now_ms,
        )
    }

    pub fn proposal(&self, proposal_id: &str) -> Option<&EvolutionProposal> {
        self.proposals.get(proposal_id)
    }

    pub fn ledger(&self) -> EvolutionLedger {
        EvolutionLedger {
            proposals: self.proposals.values().cloned().collect(),
            receipts: self.receipts.clone(),
        }
    }

    pub fn from_ledger(ledger: EvolutionLedger) -> Result<Self, EvolutionError> {
        let mut proposals = BTreeMap::new();
        for proposal in ledger.proposals {
            if proposals
                .insert(proposal.proposal_id.clone(), proposal)
                .is_some()
            {
                return Err(EvolutionError::Corrupt(
                    "duplicate proposal in ledger".into(),
                ));
            }
        }
        let mut registry = Self {
            proposals,
            receipts: ledger.receipts,
            last_sha256: None,
        };
        registry.verify()?;
        let mut last_action = BTreeMap::new();
        for receipt in &registry.receipts {
            last_action.insert(receipt.proposal_id.clone(), receipt.action);
        }
        for (proposal_id, proposal) in &registry.proposals {
            let action = last_action.get(proposal_id).ok_or_else(|| {
                EvolutionError::Corrupt(format!("proposal {proposal_id} has no receipt"))
            })?;
            let expected = match action {
                EvolutionAction::Proposed => EvolutionStatus::Proposed,
                EvolutionAction::Approved => EvolutionStatus::Approved,
                EvolutionAction::Applied => EvolutionStatus::Applied,
                EvolutionAction::Rejected => EvolutionStatus::Rejected,
                EvolutionAction::RolledBack => EvolutionStatus::RolledBack,
            };
            if proposal.status != expected {
                return Err(EvolutionError::InvalidState(
                    proposal_id.clone(),
                    proposal.status,
                ));
            }
        }
        registry.last_sha256 = registry
            .receipts
            .last()
            .map(|receipt| receipt.receipt_sha256.clone());
        Ok(registry)
    }

    pub fn verify(&self) -> Result<(), EvolutionError> {
        let mut previous = None;
        for (index, receipt) in self.receipts.iter().enumerate() {
            if receipt.seq != index as u64 + 1 {
                return Err(EvolutionError::NonContiguousSequence);
            }
            if receipt.previous_sha256 != previous {
                return Err(EvolutionError::HashMismatch(receipt.seq));
            }
            if receipt.receipt_sha256 != hash_receipt(receipt)? {
                return Err(EvolutionError::HashMismatch(receipt.seq));
            }
            let proposal = self.proposals.get(&receipt.proposal_id).ok_or_else(|| {
                EvolutionError::ProposalDigestMismatch(receipt.proposal_id.clone())
            })?;
            if proposal_digest(proposal)? != receipt.proposal_sha256 {
                return Err(EvolutionError::ProposalDigestMismatch(
                    receipt.proposal_id.clone(),
                ));
            }
            previous = Some(receipt.receipt_sha256.clone());
        }
        Ok(())
    }

    fn transition(
        &mut self,
        proposal_id: &str,
        status: EvolutionStatus,
        action: EvolutionAction,
        actor_id: &str,
        now_ms: u128,
    ) -> Result<(), EvolutionError> {
        let proposal = self
            .proposals
            .get_mut(proposal_id)
            .ok_or_else(|| EvolutionError::UnknownProposal(proposal_id.into()))?;
        proposal.status = status;
        let proposal_sha256 = proposal_digest(proposal)?;
        let proposal_id = proposal.proposal_id.clone();
        self.receipt(
            proposal_id,
            proposal_sha256,
            action,
            actor_id.into(),
            now_ms,
        )
    }

    fn receipt(
        &mut self,
        proposal_id: String,
        proposal_sha256: String,
        action: EvolutionAction,
        actor_id: String,
        at_ms: u128,
    ) -> Result<(), EvolutionError> {
        let seq = self.receipts.len() as u64 + 1;
        let mut receipt = EvolutionReceipt {
            seq,
            receipt_id: format!("receipt-{seq}"),
            proposal_id,
            proposal_sha256,
            action,
            actor_id,
            at_ms,
            previous_sha256: self.last_sha256.clone(),
            receipt_sha256: String::new(),
        };
        receipt.receipt_sha256 = hash_receipt(&receipt)?;
        self.last_sha256 = Some(receipt.receipt_sha256.clone());
        self.receipts.push(receipt);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LessonKind {
    Fact,
    Process,
    Boundary,
    Style,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reflection {
    pub reflection_id: String,
    pub actor_id: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub created_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub lesson_id: String,
    pub kind: LessonKind,
    pub statement: String,
    pub evidence: Vec<String>,
    /// 0..=100
    pub confidence: u8,
    pub created_ms: u128,
}

pub fn new_reflection(
    reflection_id: impl Into<String>,
    actor_id: impl Into<String>,
    summary: impl Into<String>,
    evidence: Vec<String>,
    created_ms: u128,
) -> Result<Reflection, EvolutionError> {
    let reflection_id = reflection_id.into();
    if reflection_id.trim().is_empty() {
        return Err(EvolutionError::EmptyProposalId);
    }
    let actor_id = actor_id.into();
    if actor_id.trim().is_empty() {
        return Err(EvolutionError::EmptyProposer);
    }
    let summary = summary.into();
    validate_text(&summary, EvolutionError::InvalidSummary)?;
    if evidence.is_empty() {
        return Err(EvolutionError::MissingEvidence);
    }
    if evidence.iter().any(|item| item.trim().is_empty()) {
        return Err(EvolutionError::EmptyEvidence);
    }
    Ok(Reflection {
        reflection_id,
        actor_id,
        summary,
        evidence,
        created_ms,
    })
}

pub fn new_lesson(
    lesson_id: impl Into<String>,
    kind: LessonKind,
    statement: impl Into<String>,
    evidence: Vec<String>,
    confidence: u8,
    created_ms: u128,
) -> Result<Lesson, EvolutionError> {
    let lesson_id = lesson_id.into();
    if lesson_id.trim().is_empty() {
        return Err(EvolutionError::EmptyProposalId);
    }
    let statement = statement.into();
    validate_text(&statement, EvolutionError::InvalidSummary)?;
    if evidence.is_empty() {
        return Err(EvolutionError::MissingEvidence);
    }
    if evidence.iter().any(|item| item.trim().is_empty()) {
        return Err(EvolutionError::EmptyEvidence);
    }
    Ok(Lesson {
        lesson_id,
        kind,
        statement,
        evidence,
        confidence: confidence.min(100),
        created_ms,
    })
}

fn validate_text(value: &str, error: EvolutionError) -> Result<(), EvolutionError> {
    if value.trim().is_empty() || value.chars().count() > MAX_TEXT_CHARS {
        Err(error)
    } else {
        Ok(())
    }
}

fn proposal_digest(proposal: &EvolutionProposal) -> Result<String, EvolutionError> {
    #[derive(Serialize)]
    struct Payload<'a> {
        proposal_id: &'a str,
        proposer: &'a str,
        kind: EvolutionKind,
        summary: &'a str,
        evidence: &'a [String],
        rollback_plan: &'a str,
        risk: RiskLevel,
        created_ms: u128,
    }
    let payload = Payload {
        proposal_id: &proposal.proposal_id,
        proposer: &proposal.proposer,
        kind: proposal.kind,
        summary: &proposal.summary,
        evidence: &proposal.evidence,
        rollback_plan: &proposal.rollback_plan,
        risk: proposal.risk,
        created_ms: proposal.created_ms,
    };
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&payload)?)))
}

fn hash_receipt(receipt: &EvolutionReceipt) -> Result<String, EvolutionError> {
    #[derive(Serialize)]
    struct Payload<'a> {
        seq: u64,
        receipt_id: &'a str,
        proposal_id: &'a str,
        proposal_sha256: &'a str,
        action: EvolutionAction,
        actor_id: &'a str,
        at_ms: u128,
        previous_sha256: &'a Option<String>,
    }
    let payload = Payload {
        seq: receipt.seq,
        receipt_id: &receipt.receipt_id,
        proposal_id: &receipt.proposal_id,
        proposal_sha256: &receipt.proposal_sha256,
        action: receipt.action,
        actor_id: &receipt.actor_id,
        at_ms: receipt.at_ms,
        previous_sha256: &receipt.previous_sha256,
    };
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&payload)?)))
}

impl EvolutionStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, EvolutionStoreError> {
        let path = path.into();
        let _lock = crate::jsonl::FileLock::exclusive(&path)?;
        let (loaded_sha256, registry) = read_snapshot(&path)?;
        Ok(Self {
            path,
            registry,
            loaded_sha256,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn registry(&self) -> &EvolutionRegistry {
        &self.registry
    }

    pub fn with_registry_mut<R>(
        &mut self,
        operation: impl FnOnce(&mut EvolutionRegistry) -> Result<R, EvolutionError>,
    ) -> Result<R, EvolutionStoreError> {
        let _lock = crate::jsonl::FileLock::exclusive(&self.path)?;
        let expected = self.loaded_sha256.clone();
        let current = current_sha256(&self.path)?;
        if current != expected {
            let (sha, registry) = read_snapshot(&self.path)?;
            self.loaded_sha256 = sha;
            self.registry = registry;
            return Err(EvolutionStoreError::ConcurrentWrite {
                expected: expected_sha(&expected),
                actual: expected_sha(&current),
            });
        }
        let mut candidate = self.registry.clone();
        let result = operation(&mut candidate)?;
        let snapshot = EvolutionSnapshot {
            schema_version: EVOLUTION_SNAPSHOT_SCHEMA,
            ledger: candidate.ledger(),
        };
        let bytes = serde_json::to_vec_pretty(&snapshot)?;
        write_private_atomic(&self.path, &bytes)?;
        self.loaded_sha256 = Some(sha256_hex(&bytes));
        self.registry = candidate;
        Ok(result)
    }
}

fn read_snapshot(path: &Path) -> Result<(Option<String>, EvolutionRegistry), EvolutionStoreError> {
    let Some(bytes) = read_bytes(path)? else {
        return Ok((None, EvolutionRegistry::default()));
    };
    let snapshot: EvolutionSnapshot = serde_json::from_slice(&bytes)?;
    if snapshot.schema_version != EVOLUTION_SNAPSHOT_SCHEMA {
        return Err(EvolutionStoreError::Corrupt(
            "unsupported evolution snapshot schema".into(),
        ));
    }
    Ok((
        Some(sha256_hex(&bytes)),
        EvolutionRegistry::from_ledger(snapshot.ledger)?,
    ))
}

fn current_sha256(path: &Path) -> Result<Option<String>, EvolutionStoreError> {
    Ok(read_bytes(path)?.map(|bytes| sha256_hex(&bytes)))
}

fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>, EvolutionStoreError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(EvolutionStoreError::Corrupt(
                    "evolution store must be a regular file".into(),
                ));
            }
            Ok(Some(std::fs::read(path)?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn expected_sha(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "none".into())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), EvolutionStoreError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        crate::jsonl::ensure_parent_private(path)?;
    }
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("evolution.json");
    let temp = path.with_file_name(format!(".{name}.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<(), EvolutionStoreError> {
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

    fn draft(id: &str) -> EvolutionProposalDraft {
        EvolutionProposalDraft {
            proposal_id: id.into(),
            proposer: "owner-1".into(),
            kind: EvolutionKind::SkillDraft,
            summary: "tighten release checklist".into(),
            evidence: vec!["skill-eval:42".into()],
            rollback_plan: "revert to previous skill version".into(),
            risk: RiskLevel::Low,
        }
    }

    fn propose(registry: &mut EvolutionRegistry) {
        registry.propose(draft("proposal-1"), 10).unwrap();
    }

    #[test]
    fn proposal_moves_through_human_gated_receipts() {
        let mut registry = EvolutionRegistry::default();
        propose(&mut registry);
        assert_eq!(
            registry.proposal("proposal-1").unwrap().status,
            EvolutionStatus::Proposed
        );
        registry.approve("proposal-1", "owner-2", 20).unwrap();
        registry.apply("proposal-1", "owner-2", 30).unwrap();
        registry.rollback("proposal-1", "owner-2", 40).unwrap();
        registry.verify().unwrap();
        assert_eq!(registry.ledger().receipts.len(), 4);
        assert_eq!(
            registry.proposal("proposal-1").unwrap().status,
            EvolutionStatus::RolledBack
        );
    }

    #[test]
    fn self_approval_evidence_and_rollback_fail_closed() {
        let mut registry = EvolutionRegistry::default();
        let mut missing = draft("p");
        missing.evidence = Vec::new();
        assert!(matches!(
            registry.propose(missing, 1).unwrap_err(),
            EvolutionError::MissingEvidence
        ));
        propose(&mut registry);
        assert!(matches!(
            registry.approve("proposal-1", "owner-1", 20).unwrap_err(),
            EvolutionError::SelfApproval
        ));
        assert!(matches!(
            registry.apply("proposal-1", "owner-2", 20).unwrap_err(),
            EvolutionError::InvalidState(_, _)
        ));
    }

    #[test]
    fn stale_writer_reloads_and_retry_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evolution.json");
        let mut first = EvolutionStore::open(&path).unwrap();
        let mut second = EvolutionStore::open(&path).unwrap();
        first
            .with_registry_mut(|registry| registry.propose(draft("proposal-1"), 10))
            .unwrap();
        let error = second
            .with_registry_mut(|registry| registry.propose(draft("proposal-2"), 11))
            .unwrap_err();
        assert!(matches!(error, EvolutionStoreError::ConcurrentWrite { .. }));
        second
            .with_registry_mut(|registry| registry.propose(draft("proposal-2"), 12))
            .unwrap();
        let reopened = EvolutionStore::open(&path).unwrap();
        assert!(reopened.registry().proposal("proposal-1").is_some());
        assert!(reopened.registry().proposal("proposal-2").is_some());
    }

    #[test]
    fn receipt_tampering_is_detected() {
        let mut registry = EvolutionRegistry::default();
        propose(&mut registry);
        let mut ledger = registry.ledger();
        ledger.receipts[0].actor_id = "attacker".into();
        assert!(hash_receipt(&ledger.receipts[0]).is_ok());
        let tampered = EvolutionRegistry {
            proposals: registry.proposals.clone(),
            receipts: ledger.receipts,
            last_sha256: registry.last_sha256.clone(),
        };
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn store_persists_and_detects_body_or_status_tampering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evolution.json");
        let mut store = EvolutionStore::open(&path).unwrap();
        store
            .with_registry_mut(|registry| {
                registry.propose(draft("proposal-1"), 10)?;
                registry.approve("proposal-1", "owner-2", 20)?;
                registry.apply("proposal-1", "owner-2", 30)
            })
            .unwrap();
        drop(store);

        let reopened = EvolutionStore::open(&path).unwrap();
        assert_eq!(
            reopened.registry().proposal("proposal-1").unwrap().status,
            EvolutionStatus::Applied
        );
        reopened.registry().verify().unwrap();

        let mut snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        snapshot["ledger"]["proposals"][0]["summary"] = "tampered".into();
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        assert!(matches!(
            EvolutionStore::open(&path).unwrap_err(),
            EvolutionStoreError::Evolution(EvolutionError::ProposalDigestMismatch(_))
        ));

        let mut snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        snapshot["ledger"]["proposals"][0]["status"] = "proposed".into();
        std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        assert!(matches!(
            EvolutionStore::open(&path).unwrap_err(),
            EvolutionStoreError::Evolution(EvolutionError::ProposalDigestMismatch(_))
                | EvolutionStoreError::Evolution(EvolutionError::InvalidState(_, _))
        ));
    }

    #[test]
    fn reflections_and_lessons_require_evidence() {
        assert!(matches!(
            new_reflection("r", "owner-1", "summary", Vec::new(), 1).unwrap_err(),
            EvolutionError::MissingEvidence
        ));
        let reflection =
            new_reflection("r", "owner-1", "summary", vec!["journal:1".into()], 1).unwrap();
        assert_eq!(reflection.evidence.len(), 1);
        let lesson = new_lesson(
            "l",
            LessonKind::Process,
            "run tests before release",
            vec!["ci:1".into()],
            150,
            2,
        )
        .unwrap();
        assert_eq!(lesson.confidence, 100);
    }
}
