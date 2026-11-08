//! Bridge from a TaskGraph ticket to the existing Workshop CLI adapter.
//!
//! The adapter performs the actual execution; this layer only builds the
//! bounded WorkItem, dispatches it, and maps the receipt back into a ticket
//! report. It never grants authority itself.

use std::path::PathBuf;

use thiserror::Error;

use crate::domain::{PermissionSet, RiskLevel};
use crate::governed_task_graph::{GovernedTaskGraph, GovernedTaskGraphError};
use crate::master::{AgentProfile, ExecutionStatus, WorkBudget, WorkItem, WorkItemStatus};
use crate::master_coordination::ExecutionReceipt;
use crate::master_workshop::{WorkshopAdapterError, WorkshopCliAdapter};
use crate::task_graph::{ReportStatus, TaskGraphError};

#[derive(Debug, Error)]
pub enum WorkshopWorkerError {
    #[error("workshop adapter: {0}")]
    Adapter(#[from] WorkshopAdapterError),
    #[error("governed task graph: {0}")]
    Governed(#[from] GovernedTaskGraphError),
    #[error("task graph: {0}")]
    Graph(#[from] TaskGraphError),
    #[error("workspace is not a directory: {0}")]
    BadWorkspace(String),
}

#[derive(Debug)]
pub struct WorkshopWorker {
    adapter: WorkshopCliAdapter,
    profile: AgentProfile,
    workspace: PathBuf,
    lease_generation: u64,
}

impl WorkshopWorker {
    pub fn new(
        adapter: WorkshopCliAdapter,
        profile: AgentProfile,
        workspace: impl Into<PathBuf>,
    ) -> Result<Self, WorkshopWorkerError> {
        let workspace = workspace.into();
        if !workspace.is_dir() {
            return Err(WorkshopWorkerError::BadWorkspace(
                workspace.display().to_string(),
            ));
        }
        if profile.runtime_command.is_empty() {
            return Err(WorkshopWorkerError::Adapter(
                WorkshopAdapterError::MissingRuntimeCommand(profile.agent_id.clone()),
            ));
        }
        Ok(Self {
            adapter,
            profile,
            workspace,
            lease_generation: 1,
        })
    }

    pub fn with_lease_generation(mut self, generation: u64) -> Self {
        self.lease_generation = generation;
        self
    }

    pub fn execute_task(
        &self,
        task_id: &str,
        title: &str,
        objective: &str,
        idempotency_key: &str,
        deadline_unix_ms: Option<u128>,
        now_ms: u128,
    ) -> Result<ExecutionReceipt, WorkshopWorkerError> {
        let item = WorkItem {
            id: task_id.into(),
            title: title.into(),
            objective: objective.into(),
            dependencies: Vec::new(),
            required_capabilities: self.profile.capabilities.clone(),
            required_tools: self.profile.tools.clone(),
            permissions: PermissionSet {
                read_roots: vec![self.workspace.clone()],
                allow_network: self.profile.permissions.allow_network,
                allow_write: self.profile.permissions.allow_write,
            },
            acceptance_criteria: Vec::new(),
            expected_artifacts: Vec::new(),
            workspace: self.workspace.clone(),
            budget: WorkBudget::default(),
            deadline_unix_ms,
            risk: RiskLevel::Low,
            status: WorkItemStatus::Pending,
        };
        Ok(self.adapter.dispatch(
            &item,
            &self.profile,
            self.lease_generation,
            idempotency_key,
            now_ms,
        )?)
    }

    pub fn execute_and_report(
        &self,
        service: &mut GovernedTaskGraph,
        ticket_id: &str,
        worker_id: &str,
        now_ms: u128,
    ) -> Result<ExecutionReceipt, WorkshopWorkerError> {
        let receipt = self.execute_task(
            ticket_id,
            ticket_id,
            ticket_id,
            &format!("ticket:{ticket_id}:{}", self.lease_generation),
            None,
            now_ms,
        )?;
        let status = match receipt.status {
            ExecutionStatus::Completed => ReportStatus::Ready,
            ExecutionStatus::Interrupted | ExecutionStatus::Cancelled => ReportStatus::Progress,
            ExecutionStatus::Failed | ExecutionStatus::TimedOut => ReportStatus::Failed,
        };
        let summary = match &receipt.error {
            Some(error) => format!("workshop {:?}: {error}", receipt.status),
            None => format!(
                "workshop {:?} target={:?}",
                receipt.status, receipt.execution_target
            ),
        };
        service.report_ticket(
            ticket_id,
            worker_id,
            status,
            summary,
            receipt.result_digest.clone(),
            now_ms,
        )?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::AuthorityProfile;
    use crate::domain::PermissionSet;
    use crate::master::AgentAvailability;
    use crate::task_graph::{
        ApprovalDecision, Decision, MandateStrength, TicketDraft, Trigger, TriggerSource,
    };
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    fn profile() -> AgentProfile {
        AgentProfile {
            agent_id: "worker-1".into(),
            runtime: "pi".into(),
            display_name: None,
            capabilities: Vec::new(),
            tools: Vec::new(),
            runtime_command: vec!["pi".into(), "--print".into()],
            permissions: PermissionSet::default(),
            availability: AgentAvailability::Idle,
            current_load: 0,
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: RiskLevel::Low,
        }
    }

    fn ticket(service: &mut GovernedTaskGraph) -> String {
        service
            .register_profile(AuthorityProfile {
                profile_id: "owner-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["read*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        service.bind_actor("owner-1", "owner-profile").unwrap();
        service
            .register_profile(AuthorityProfile {
                profile_id: "worker-profile".into(),
                actor_id: "worker-1".into(),
                scopes: vec!["read*".into()],
                max_strength: MandateStrength::ReadOnly,
                allowed_tools: Vec::new(),
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Medium,
                expires_ms: None,
            })
            .unwrap();
        service.bind_actor("worker-1", "worker-profile").unwrap();
        let thread_id = service.open_thread("workshop", 10).unwrap();
        let trigger_id = service
            .ingest_trigger(
                &thread_id,
                Trigger {
                    trigger_id: "trigger-1".into(),
                    source: TriggerSource::Web,
                    source_event_id: "event-1".into(),
                    summary: "workshop".into(),
                    received_ms: 1,
                },
                11,
            )
            .unwrap()
            .trigger_id;
        let eval_id = service.begin_eval(&thread_id, &trigger_id, 12).unwrap();
        service
            .decide_eval(&eval_id, Decision::Accept, "ok", 13)
            .unwrap();
        let ticket_id = service
            .create_ticket_from_eval(
                &eval_id,
                "owner-1",
                TicketDraft {
                    owner_id: "owner-1".into(),
                    scope: "read".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "v1".into(),
                    requires_approval: true,
                },
                14,
            )
            .unwrap();
        service
            .approve_ticket(&ticket_id, "owner-1", ApprovalDecision::AllowOnce, None, 15)
            .unwrap();
        service
            .assign_ticket(&ticket_id, "owner-1", "worker-1", None, 16)
            .unwrap();
        service.start_ticket(&ticket_id, "worker-1", 17).unwrap();
        ticket_id
    }

    #[test]
    fn workshop_receipt_is_reported_back_to_the_ticket() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("fake");
        let script = dir.path().join("fake-agent-run");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '%s\n' '{"schema":"skynet.term.agent-run/v1","agent":{"agent_id":"worker-1"},"session_id":"term-test","gui":{"started":false},"exec":{"status":"ok","exit":0,"duration_ms":12,"artifact":{"path":"/tmp/out","sha256":"abc"}}}'
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();

        let adapter =
            WorkshopCliAdapter::new(prefix.display().to_string(), Duration::from_secs(2), false)
                .unwrap();
        let worker = WorkshopWorker::new(adapter, profile(), dir.path()).unwrap();
        let mut service = GovernedTaskGraph::open(dir.path().join("task-graph.jsonl")).unwrap();
        let ticket_id = ticket(&mut service);
        let receipt = worker
            .execute_and_report(&mut service, &ticket_id, "worker-1", 20)
            .unwrap();
        assert_eq!(receipt.status, ExecutionStatus::Completed);
        assert_eq!(
            service.graph().ticket(&ticket_id).unwrap().status,
            crate::task_graph::TicketStatus::Ready
        );
    }
}
