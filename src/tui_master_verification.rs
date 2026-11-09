use std::path::Path;

use crate::master_coordination::{
    MasterAudit, MasterState, artifact_expectation, inspect_artifacts,
};

pub fn artifacts_summary(master: &MasterState, filter: &str) -> String {
    let filter = filter.trim();
    let mut output = format!(
        "Artifacts · trace={} · plan={}",
        master.trace_id, master.plan.plan_id
    );
    let mut matched = 0;
    for item in &master.plan.work_items {
        if !filter.is_empty()
            && filter != "all"
            && item.id != filter
            && !item.title.contains(filter)
        {
            continue;
        }
        matched += 1;
        let task = master.tasks.get(&item.id);
        output.push_str(&format!(
            "\n- {} · {} · status={:?}",
            item.id,
            bounded(&item.title, 120),
            task.map(|task| task.status).unwrap_or_default()
        ));
        if let Some(receipt) = task.and_then(|task| task.receipt.as_ref()) {
            output.push_str(&format!(
                "\n  receipt · status={:?} · exit={:?} · artifacts={} · digest={} · error={}",
                receipt.status,
                receipt.exit_code,
                if receipt.artifacts.is_empty() {
                    "-".into()
                } else {
                    receipt
                        .artifacts
                        .iter()
                        .map(|path| bounded(path, 240))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                receipt.result_digest.as_deref().unwrap_or("-"),
                receipt
                    .error
                    .as_deref()
                    .map(|error| bounded(error, 240))
                    .unwrap_or_else(|| "-".into())
            ));
        }
        if item.expected_artifacts.is_empty() {
            output.push_str("\n  expected: none");
            continue;
        }
        let expectations = item
            .expected_artifacts
            .iter()
            .map(|spec| artifact_expectation(spec))
            .collect::<Vec<_>>();
        match inspect_artifacts(&item.workspace, &expectations) {
            Ok(checks) => {
                for check in checks {
                    let verdict = if !check.exists {
                        "missing"
                    } else if !check.regular_file {
                        "not_regular_file"
                    } else if check
                        .expected_sha256
                        .as_deref()
                        .is_some_and(|expected| check.sha256.as_deref() != Some(expected))
                    {
                        "sha256_mismatch"
                    } else {
                        "ok"
                    };
                    output.push_str(&format!(
                        "\n  - {} · actual={} · regular={} · bytes={} · sha256={} · expected_sha256={} · verdict={verdict}",
                        check.path,
                        if check.exists { "present" } else { "missing" },
                        check.regular_file,
                        check.bytes,
                        check.sha256.as_deref().unwrap_or("-"),
                        check.expected_sha256.as_deref().unwrap_or("unpinned")
                    ));
                    if let Some(error) = check.error {
                        output.push_str(&format!(" · error={}", bounded(&error, 240)));
                    }
                }
            }
            Err(error) => output.push_str(&format!(
                "\n  - inspection failed: {}",
                bounded(&error.to_string(), 400)
            )),
        }
    }
    if matched == 0 {
        output.push_str("\n没有匹配的 Work Item。");
    }
    output
}

pub fn master_evidence_summary(master: &MasterState) -> String {
    let mut output = format!(
        "Master 证据板 · trace={} · plan={} · approved={}",
        master.trace_id, master.plan.plan_id, master.plan.approved
    );
    for item in &master.plan.work_items {
        let Some(task) = master.tasks.get(&item.id) else {
            continue;
        };
        output.push_str(&format!(
            "\n- {} · {} · status={:?}",
            item.id,
            bounded(&item.title, 160),
            task.status
        ));
        if let Some(receipt) = &task.receipt {
            output.push_str(&format!(
                "\n  receipt: {:?} · exit={:?} · artifacts={} · digest={}",
                receipt.status,
                receipt.exit_code,
                if receipt.artifacts.is_empty() {
                    "-".into()
                } else {
                    receipt
                        .artifacts
                        .iter()
                        .map(|path| bounded(path, 240))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                receipt.result_digest.as_deref().unwrap_or("-")
            ));
        } else {
            output.push_str("\n  receipt: none");
        }
        if let Some(verification) = &task.verification {
            output.push_str(&format!(
                "\n  verification: {:?} · verifier={} · checked_artifacts={} · tests={}",
                verification.status,
                bounded(&verification.verified_by, 160),
                verification.checked_artifacts.len(),
                verification.test_results.len()
            ));
            for criterion in &verification.criteria_results {
                output.push_str(&format!(
                    "\n  claim [{}] {} ← {}",
                    if criterion.passed { "pass" } else { "fail" },
                    bounded(&criterion.criterion, 300),
                    if criterion.evidence.is_empty() {
                        "no evidence".into()
                    } else {
                        criterion
                            .evidence
                            .iter()
                            .map(|evidence| bounded(evidence, 240))
                            .collect::<Vec<_>>()
                            .join("; ")
                    }
                ));
            }
            for artifact in &verification.checked_artifacts {
                output.push_str(&format!(
                    "\n  artifact [{}] {} · bytes={} · sha256={} · expected_sha256={}",
                    if artifact.exists
                        && artifact.regular_file
                        && artifact
                            .expected_sha256
                            .as_deref()
                            .is_none_or(|expected| { artifact.sha256.as_deref() == Some(expected) })
                    {
                        "pass"
                    } else {
                        "fail"
                    },
                    artifact.path,
                    artifact.bytes,
                    artifact.sha256.as_deref().unwrap_or("-"),
                    artifact.expected_sha256.as_deref().unwrap_or("unpinned")
                ));
            }
            for evidence in &verification.evidence {
                output.push_str(&format!("\n  evidence: {}", bounded(evidence, 400)));
            }
            if let Some(reason) = &verification.failure_reason {
                output.push_str(&format!("\n  failure: {}", bounded(reason, 500)));
            }
        } else {
            output.push_str("\n  verification: pending");
        }
        if let Some(error) = &task.last_error {
            output.push_str(&format!("\n  last_error: {}", bounded(error, 400)));
        }
    }
    output
}

pub fn master_report(master: &MasterState) -> String {
    let executed = master
        .tasks
        .values()
        .filter(|task| task.receipt.is_some())
        .count();
    let verified = master
        .tasks
        .values()
        .filter(|task| task.status == crate::master::WorkItemStatus::Verified)
        .count();
    let mut output = format!(
        "生产线报告 · trace={} · plan={} · planned={} · executed={} · verified={} · decisions={} · paused={}",
        master.trace_id,
        master.plan.plan_id,
        master.plan.work_items.len(),
        executed,
        verified,
        master.decisions.len(),
        master.paused
    );
    for item in &master.plan.work_items {
        let task = master.tasks.get(&item.id);
        output.push_str(&format!(
            "\n- {} · {} · {:?}",
            item.id,
            bounded(&item.title, 160),
            task.map(|task| task.status).unwrap_or_default()
        ));
        output.push_str(&format!(
            "\n  planned: criteria={} · artifacts={} · risk={:?}",
            item.acceptance_criteria.len(),
            item.expected_artifacts.join(", "),
            item.risk
        ));
        if let Some(task) = task {
            output.push_str(&format!(
                "\n  executed: agent={} · session={} · generation={}",
                task.active_lease
                    .as_ref()
                    .map(|lease| lease.agent_id.as_str())
                    .unwrap_or("-"),
                task.execution_target
                    .as_ref()
                    .and_then(|target| target.session_id.as_deref())
                    .unwrap_or("-"),
                task.generation
            ));
            if let Some(receipt) = &task.receipt {
                output.push_str(&format!(
                    "\n  receipt: {:?} · exit={:?} · artifacts={}",
                    receipt.status,
                    receipt.exit_code,
                    receipt.artifacts.len()
                ));
            }
            if let Some(verification) = &task.verification {
                output.push_str(&format!(
                    "\n  verified: {:?} · by={} · evidence={} · failure={}",
                    verification.status,
                    bounded(&verification.verified_by, 160),
                    verification.evidence.len(),
                    verification
                        .failure_reason
                        .as_deref()
                        .map(|reason| bounded(reason, 400))
                        .unwrap_or_else(|| "-".into())
                ));
            } else {
                output.push_str("\n  verified: pending");
            }
        }
        let decisions = master
            .decisions
            .iter()
            .filter(|decision| decision.task_id.as_deref() == Some(item.id.as_str()))
            .collect::<Vec<_>>();
        if !decisions.is_empty() {
            output.push_str("\n  decided:");
            for decision in decisions {
                output.push_str(&format!(
                    "\n  - {:?} · {} · {}",
                    decision.kind,
                    bounded(&decision.actor, 120),
                    bounded(&decision.reason, 300)
                ));
            }
        }
    }
    output
}

pub fn master_audit_summary(master: &MasterState, audit: &MasterAudit, journal: &Path) -> String {
    let mut issues = audit.issues.clone();
    let now = now_unix_ms();
    for item in &master.plan.work_items {
        let Some(task) = master.tasks.get(&item.id) else {
            issues.push(format!("{}: missing task runtime state", item.id));
            continue;
        };
        if task
            .active_lease
            .as_ref()
            .is_some_and(|lease| !lease.is_active(now))
        {
            issues.push(format!("{}: active lease has expired", item.id));
        }
        if matches!(
            task.status,
            crate::master::WorkItemStatus::Running
                | crate::master::WorkItemStatus::AwaitingInput
                | crate::master::WorkItemStatus::Blocked
        ) && task
            .execution_target
            .as_ref()
            .is_none_or(|target| !target.is_connected())
        {
            issues.push(format!("{}: running task has no connected target", item.id));
        }
        if task
            .receipt
            .as_ref()
            .is_some_and(|receipt| receipt.lease_generation != task.generation)
        {
            issues.push(format!(
                "{}: receipt generation does not match task",
                item.id
            ));
        }
        if task.status == crate::master::WorkItemStatus::Verified && task.verification.is_none() {
            issues.push(format!(
                "{}: verified task has no verification evidence",
                item.id
            ));
        }
        if matches!(
            task.status,
            crate::master::WorkItemStatus::ArtifactReady | crate::master::WorkItemStatus::Verified
        ) {
            let expectations = item
                .expected_artifacts
                .iter()
                .map(|spec| artifact_expectation(spec))
                .collect::<Vec<_>>();
            match inspect_artifacts(&item.workspace, &expectations) {
                Ok(checks) => {
                    for check in checks {
                        if !check.exists || !check.regular_file {
                            issues.push(format!("{}: artifact missing: {}", item.id, check.path));
                        } else if check
                            .expected_sha256
                            .as_deref()
                            .is_some_and(|expected| check.sha256.as_deref() != Some(expected))
                        {
                            issues.push(format!(
                                "{}: artifact SHA-256 drift: {}",
                                item.id, check.path
                            ));
                        }
                    }
                }
                Err(error) => issues.push(format!("{}: artifact audit failed: {error}", item.id)),
            }
        }
    }
    let mut output = format!(
        "Master Journal 审计 · {} · {} events · trace={} · journal={}",
        if issues.is_empty() {
            "通过"
        } else {
            "失败"
        },
        audit.event_count,
        master.trace_id,
        journal.display()
    );
    if issues.is_empty() {
        output.push_str("\n未发现结构性问题。");
    } else {
        for issue in issues {
            output.push_str(&format!("\n- {}", bounded(&issue, 600)));
        }
    }
    output
}

fn bounded(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut output = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        output.push('…');
    }
    output.replace(['\r', '\n'], " ")
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_text_never_emits_multiline_payloads() {
        assert_eq!(bounded("a\nb", 10), "a b");
        assert_eq!(bounded("abcd", 3), "abc…");
    }
}
