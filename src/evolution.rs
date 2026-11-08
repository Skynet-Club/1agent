//! Reflection, lessons and a human-gated evolution ledger.
//!
//! The ledger records proposals and decisions, not silent self-modification.
//! Proposals can only target skills, routines or strategy notes; authority,
//! policy, audit and persona changes are not representable here. Approval must
//! come from a different actor than the proposer. Every transition is a
//! hash-chained receipt.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::RiskLevel;

const MAX_TEXT_CHARS: usize = 2_000;

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
    #[error("receipt sequence is not contiguous")]
    NonContiguousSequence,
    #[error("evolution json: {0}")]
    Json(#[from] serde_json::Error),
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
        self.proposals
            .insert(proposal.proposal_id.clone(), proposal);
        self.receipt(
            receipt_proposal_id,
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
        self.receipt(proposal_id.into(), action, actor_id.into(), now_ms)
    }

    fn receipt(
        &mut self,
        proposal_id: String,
        action: EvolutionAction,
        actor_id: String,
        at_ms: u128,
    ) -> Result<(), EvolutionError> {
        let seq = self.receipts.len() as u64 + 1;
        let mut receipt = EvolutionReceipt {
            seq,
            receipt_id: format!("receipt-{seq}"),
            proposal_id,
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

fn hash_receipt(receipt: &EvolutionReceipt) -> Result<String, EvolutionError> {
    #[derive(Serialize)]
    struct Payload<'a> {
        seq: u64,
        receipt_id: &'a str,
        proposal_id: &'a str,
        action: EvolutionAction,
        actor_id: &'a str,
        at_ms: u128,
        previous_sha256: &'a Option<String>,
    }
    let payload = Payload {
        seq: receipt.seq,
        receipt_id: &receipt.receipt_id,
        proposal_id: &receipt.proposal_id,
        action: receipt.action,
        actor_id: &receipt.actor_id,
        at_ms: receipt.at_ms,
        previous_sha256: &receipt.previous_sha256,
    };
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&payload)?)))
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
