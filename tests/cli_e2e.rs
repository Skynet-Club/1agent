use std::io::Write;
use std::process::{Command, Stdio};

use first_principles_agent::{
    AgentProfile, AssignmentProposal, AuthenticityStatus, ContentBindingStatus,
    DecisionCompletenessFloorSource, EvalReport, EvalReportSummary, ExecutionObservation,
    ExecutionSubmission, ExecutionTarget, JournalAudit, MasterJournalEventKind, MasterState,
    MasterStore, PlanProposal, ReplayState, RunResult, StopReason, TuiSession, WorkItemStatus,
};
use sha2::{Digest, Sha256};

#[test]
fn cli_run_audit_and_replay_form_a_complete_flywheel() {
    let workspace = tempfile::tempdir().unwrap();
    let evidence_dir = workspace.path().join("evidence dir");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    let evidence = evidence_dir.join("decision.txt");
    std::fs::write(
        &evidence,
        "Verified fact: option A has lower total cost under the stated constraints.",
    )
    .unwrap();
    let journal_dir = workspace.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let output = Command::new(binary)
        .args([
            "run",
            "--task",
            &format!(
                "Compare option A and B using @read-file:\"{}\"",
                evidence.display()
            ),
            "--read-root",
            &workspace.path().display().to_string(),
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result.stop_reason, StopReason::Completed);
    assert_eq!(result.answer.evidence.len(), 1);
    assert!(result.journal_path.exists());

    let audit_output = Command::new(binary)
        .args([
            "audit",
            "--journal",
            &result.journal_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        audit_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&audit_output.stderr)
    );
    let audit: JournalAudit = serde_json::from_slice(&audit_output.stdout).unwrap();
    assert!(audit.ok, "{:?}", audit.issues);
    assert!(audit.findings.is_empty());

    let replay_output = Command::new(binary)
        .args([
            "replay",
            "--journal",
            &result.journal_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        replay_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&replay_output.stderr)
    );
    let replay: ReplayState = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(replay.stop_reason, Some(StopReason::Completed));
    assert_eq!(replay.tool_calls.len(), 1);
    assert_eq!(replay.observations.len(), 1);
}

#[test]
fn cli_tui_keeps_last_instruction_in_topbar() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args([
            "tui",
            "--journal-dir",
            &workspace.path().join("journal").display().to_string(),
        ])
        .env("COLUMNS", "100")
        .env("LINES", "30")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/help\nCompare option A and option B\n/quit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1A · 运行中"));
    assert!(stdout.contains("You > Compare option A and option B"));
    assert!(stdout.contains("1A · 已完成"));
    assert!(stdout.contains("Session ended."));
}

#[test]
fn cli_tui_master_commands_require_workshop_master_context() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("standalone-journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args(["tui", "--journal-dir", &journal_dir.display().to_string()])
        .env_remove("SKYTERM_HOST_CONTEXT")
        .env_remove("SKYTERM_AGENT_ROLE")
        .env_remove("SKYTERM_HOST_CONTEXT_VERSION")
        .env_remove("SKYTERM_EXPERT_INTERACTIVE")
        .env_remove("SKYTERM_AGENT_ID")
        .env_remove("SKYTERM_AGENT_RUNTIME")
        .env_remove("SKYTERM_SESSION_ID")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/status\n/mission new Create and verify Test mission\n/triage\n/quit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("运行环境      独立 1A"));
    assert!(stdout.contains("仅在 Skynet Workshop Master-Agent 上下文可用"));
    let events = read_session_events(&journal_dir);
    assert!(events.contains("\"host_kind\":\"standalone\""));
    assert!(events.contains("\"type\":\"command_rejected\""));
    assert!(events.contains("\"type\":\"closed\""));

    let master_journal = workspace.path().join("master-journal");
    let mut child = Command::new(binary)
        .args([
            "tui",
            "--journal-dir",
            &master_journal.display().to_string(),
        ])
        .env("SKYTERM_HOST_CONTEXT", "skynet-workshop")
        .env("SKYTERM_AGENT_ROLE", "master")
        .env("SKYTERM_HOST_CONTEXT_VERSION", "1")
        .env("SKYTERM_MASTER_CONVERSATION_ID", "main-coordinator")
        .env("SKYTERM_EXPERT_INTERACTIVE", "1")
        .env("SKYTERM_AGENT_ID", "main-coordinator")
        .env("SKYTERM_AGENT_RUNTIME", "1agent")
        .env("SKYTERM_SESSION_ID", "term-1790000000000-1234-1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/status\n/mission new Create and verify Test mission\n/triage\n/quit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("运行环境      Workshop 总控"));
    assert!(stdout.contains("Mission · mission-"), "{stdout}");
    assert!(
        stdout.contains("目标: Create and verify Test mission"),
        "{stdout}"
    );
    assert!(stdout.contains("Triage"), "{stdout}");
    assert!(stdout.contains("route: SingleAgent"), "{stdout}");
    let events = read_session_events(&master_journal);
    assert!(events.contains("\"host_kind\":\"workshop-master\""));
    assert!(events.contains("\"type\":\"mission_started\""));
}

#[test]
fn cli_tui_epistemic_commands_bind_results_to_journal_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args(["tui", "--journal-dir", &journal_dir.display().to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"/frame Compare option A and option B\n\
              /evidence\n\
              /audit\n\
              /report\n\
              /challenge\n\
              /postmortem\n\
              /decide\n\
              /quit\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("证据板"));
    assert!(stdout.contains("Journal 审计"));
    assert!(stdout.contains("任务报告"));
    assert!(stdout.contains("1A · 已完成"));

    let events = read_session_events(&journal_dir);
    assert_eq!(events.matches("\"type\":\"task_submitted\"").count(), 4);
    assert!(events.contains("\"mode\":\"evidence_research\""));
    assert!(events.contains("\"mode\":\"diagnosis\""));
    assert!(events.contains("\"mode\":\"decision_analysis\""));
}

#[test]
fn cli_tui_model_permissions_and_tools_are_inspectable_and_controllable() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args(["tui", "--journal-dir", &journal_dir.display().to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"/model show\n\
              /model objective balanced\n\
              /permissions show\n\
              /permissions network on\n\
              /permissions write on\n\
              /permissions show\n\
              /tools\n\
              /tools inspect read_file\n\
              /quit\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("模型路由"));
    assert!(stdout.contains("质量与成本均衡"));
    assert!(stdout.contains("运行权限（仅影响下一次运行）"));
    assert!(stdout.contains("网络访问：允许"));
    assert!(stdout.contains("写入与执行：允许"));
    assert!(stdout.contains("- read_file"));
    assert!(stdout.contains("\"name\": \"read_file\""));
}

#[test]
fn cli_tui_agents_refresh_uses_configured_profiles() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("journal");
    let agents_path = workspace.path().join("agents.json");
    std::fs::write(
        &agents_path,
        serde_json::to_vec_pretty(&serde_json::json!([{
            "agent_id": "test-worker",
            "runtime": "mock",
            "capabilities": ["general"],
            "tools": ["read_file"],
            "permissions": {
                "read_roots": [workspace.path().display().to_string()],
                "allow_network": false,
                "allow_write": false
            },
            "availability": "idle"
        }]))
        .unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args([
            "tui",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--master-agents",
            &agents_path.display().to_string(),
        ])
        .env("SKYTERM_HOST_CONTEXT", "skynet-workshop")
        .env("SKYTERM_AGENT_ROLE", "master")
        .env("SKYTERM_HOST_CONTEXT_VERSION", "1")
        .env("SKYTERM_MASTER_CONVERSATION_ID", "main-coordinator")
        .env("SKYTERM_EXPERT_INTERACTIVE", "1")
        .env("SKYTERM_AGENT_ID", "main-coordinator")
        .env("SKYTERM_AGENT_RUNTIME", "1agent")
        .env("SKYTERM_SESSION_ID", "term-1790000000000-1234-1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/agents refresh\n/quit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test-worker"));
    assert!(stdout.contains("运行时        mock"));
}

#[test]
fn cli_master_validates_and_assigns_standalone_plan() {
    let workspace = tempfile::tempdir().unwrap();
    let plan_path = workspace.path().join("plan.json");
    let agents_path = workspace.path().join("agents.json");
    let plan = serde_json::json!({
        "schema_version": 1,
        "plan_id": "plan-1",
        "goal_id": "goal-1",
        "version": 1,
        "summary": "Implement one verified task",
        "max_parallel": 1,
        "approved": false,
        "work_items": [{
            "id": "task-1",
            "title": "Implement",
            "objective": "Produce a verified implementation",
            "dependencies": [],
            "required_capabilities": ["rust"],
            "required_tools": ["shell"],
            "permissions": {
                "read_roots": ["/tmp"],
                "allow_network": false,
                "allow_write": false
            },
            "acceptance_criteria": ["cargo test passes"],
            "expected_artifacts": ["result.json"],
            "workspace": "/tmp",
            "budget": {
                "max_steps": 6,
                "timeout_ms": 120000,
                "max_tool_calls": 8
            },
            "deadline_unix_ms": null,
            "risk": "low",
            "status": "pending"
        }]
    });
    let agents = serde_json::json!([{
        "agent_id": "agent-1",
        "runtime": "pi",
        "capabilities": ["rust"],
        "tools": ["shell"],
        "permissions": {
            "read_roots": ["/tmp"],
            "allow_network": false,
            "allow_write": false
        },
        "availability": "idle",
        "current_load": 0,
        "observed_success_permille": 900,
        "cost_micros": 10,
        "max_risk": "low"
    }]);
    std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    std::fs::write(&agents_path, serde_json::to_vec_pretty(&agents).unwrap()).unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let validation = Command::new(binary)
        .args([
            "master",
            "validate",
            "--plan",
            &plan_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        validation.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&validation.stderr)
    );

    let assignment = Command::new(binary)
        .args([
            "master",
            "assign",
            "--plan",
            &plan_path.display().to_string(),
            "--agents",
            &agents_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        assignment.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&assignment.stderr)
    );
    let assignment: AssignmentProposal = serde_json::from_slice(&assignment.stdout).unwrap();
    assert_eq!(assignment.assignments[0].agent_id, "agent-1");
    assert_eq!(assignment.assignments[0].runtime, "pi");
}

#[test]
fn cli_master_runs_verifies_and_replays_a_workshop_work_item() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let state_root = root.path().join("master");
    std::fs::create_dir_all(&workspace).unwrap();
    let prefix = root.path().join("fake-workshop");
    write_executable(
        &root.path().join("fake-workshop-agents"),
        r#"#!/bin/sh
printf '%s\n' '[{"version":1,"agent_id":"agent-1","display_name":"Worker One","provider":"pi","capabilities":["rust"],"sessions":[],"panes":[],"status":"idle","registered_ms":1,"heartbeat_ms":1,"expires_ms":9999999999999,"stale":false}]'
"#,
    );
    write_executable(
        &root.path().join("fake-workshop-agent-run"),
        r#"#!/bin/sh
cwd=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --cwd) cwd=$2; shift 2 ;;
    *) shift ;;
  esac
done
printf 'verified artifact\n' > "$cwd/result.txt"
printf '%s\n' '{"schema":"skynet.term.agent-run/v1","agent":{"agent_id":"agent-1"},"session_id":"term-master-1","gui":{"started":false},"exec":{"status":"ok","exit":0,"duration_ms":5,"artifact":{"path":"/tmp/fake.output","sha256":"abc"}}}'
"#,
    );
    write_executable(
        &root.path().join("fake-workshop-lease"),
        "#!/bin/sh\nif [ \"$1\" = request ]; then printf '%s\\n' '{\"ok\":true,\"lease\":{\"lease_id\":\"lease-test\"}}'; else printf '%s\\n' '{\"ok\":true}'; fi\n",
    );
    let plan_path = root.path().join("plan.json");
    let agents_path = root.path().join("agents.json");
    let verification_path = root.path().join("verification.json");
    let plan = serde_json::json!({
        "schema_version": 1,
        "plan_id": "plan-master-e2e",
        "goal_id": "goal-master-e2e",
        "version": 1,
        "summary": "Implement and verify one artifact",
        "max_parallel": 1,
        "approved": false,
        "work_items": [{
            "id": "implement",
            "title": "Implement",
            "objective": "Create result.txt",
            "dependencies": [],
            "required_capabilities": ["rust"],
            "required_tools": ["shell"],
            "permissions": {
                "read_roots": [workspace.display().to_string()],
                "allow_network": false,
                "allow_write": true
            },
            "acceptance_criteria": ["result.txt exists"],
            "expected_artifacts": ["result.txt"],
            "workspace": workspace.display().to_string(),
            "budget": {
                "max_steps": 6,
                "timeout_ms": 120000,
                "max_tool_calls": 8
            },
            "deadline_unix_ms": null,
            "risk": "medium",
            "status": "pending"
        }]
    });
    let agents = serde_json::json!([{
        "agent_id": "agent-1",
        "runtime": "pi",
        "display_name": "Worker One",
        "capabilities": ["rust"],
        "tools": ["shell"],
        "runtime_command": ["true"],
        "permissions": {
            "read_roots": [workspace.display().to_string()],
            "allow_network": false,
            "allow_write": true
        },
        "availability": "idle",
        "current_load": 0,
        "observed_success_permille": 900,
        "observed_runs": 8,
        "cost_micros": 10,
        "max_risk": "medium"
    }]);
    let verification = serde_json::json!({
        "task_id": "implement",
        "criteria_results": [{
            "criterion": "result.txt exists",
            "passed": true,
            "evidence": ["regular file with SHA-256 computed by verifier"]
        }],
        "test_results": [],
        "evidence": ["artifact inspection performed after execution"],
        "verified_by": "cli-e2e-verifier"
    });
    std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    std::fs::write(&agents_path, serde_json::to_vec_pretty(&agents).unwrap()).unwrap();
    std::fs::write(
        &verification_path,
        serde_json::to_vec_pretty(&verification).unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let trace_id = "trace-master-e2e";

    let init = Command::new(binary)
        .args([
            "master",
            "init",
            "--plan",
            &plan_path.display().to_string(),
            "--agents",
            &agents_path.display().to_string(),
            "--workshop-prefix",
            &prefix.display().to_string(),
            "--trace-id",
            trace_id,
            "--state-root",
            &state_root.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let approve = Command::new(binary)
        .args([
            "master",
            "approve",
            "--trace-id",
            trace_id,
            "--state-root",
            &state_root.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        approve.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&approve.stderr)
    );

    let run = Command::new(binary)
        .args([
            "master",
            "run",
            "--trace-id",
            trace_id,
            "--state-root",
            &state_root.display().to_string(),
            "--runtime-profiles",
            &agents_path.display().to_string(),
            "--workshop-prefix",
            &prefix.display().to_string(),
            "--no-gui",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let run: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(
        run["receipts"][0]["execution_target"]["session_id"],
        "term-master-1"
    );
    assert!(workspace.join("result.txt").is_file());

    let verify = Command::new(binary)
        .args([
            "master",
            "verify",
            "--trace-id",
            trace_id,
            "--state-root",
            &state_root.display().to_string(),
            "--input",
            &verification_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let verify: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(verify["verification"]["status"], "verified");

    let audit = Command::new(binary)
        .args([
            "master",
            "audit",
            "--trace-id",
            trace_id,
            "--state-root",
            &state_root.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        audit.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&audit.stderr),
        String::from_utf8_lossy(&audit.stdout)
    );
    let audit: serde_json::Value = serde_json::from_slice(&audit.stdout).unwrap();
    assert_eq!(audit["audit"]["ok"], true);
    assert_eq!(audit["state"]["tasks"]["implement"]["status"], "verified");
}

#[test]
fn cli_master_async_poll_records_running_then_terminal_state() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    let state_root = root.path().join("master");
    std::fs::create_dir_all(&workspace).unwrap();
    let prefix = root.path().join("async-workshop");
    write_executable(
        &root.path().join("async-workshop-agents"),
        "#!/bin/sh\nprintf '%s\\n' '[]'\n",
    );
    write_executable(
        &root.path().join("async-workshop-session"),
        r#"#!/bin/sh
command=$1
shift
case "$command" in
  create)
    printf '%s\n' '{"session_id":"term-async-cli"}'
    ;;
  write)
    session=$1
    shift
    if [ "${1-}" = "--require-idle" ]; then shift; fi
    /bin/bash -c "$*"
    ;;
  status)
    printf '%s\n' '{"session_id":"term-async-cli","running":false}'
    ;;
  kill)
    printf '%s\n' '{"signaled":true}'
    ;;
  *)
    printf '%s\n' '{}'
    ;;
esac
"#,
    );
    write_executable(
        &root.path().join("async-workshop-lease"),
        r#"#!/bin/sh
dir=$(dirname "$0")
printf '%s\n' "$*" >> "$dir/lease-calls.log"
if [ "$1" = request ]; then
  printf '%s\n' '{"ok":true,"lease":{"lease_id":"lease-test"}}'
else
  printf '%s\n' '{"ok":true}'
fi
"#,
    );
    let plan_path = root.path().join("plan.json");
    let agents_path = root.path().join("agents.json");
    let verification_path = root.path().join("verification.json");
    let plan = serde_json::json!({
        "schema_version": 1,
        "plan_id": "plan-async-cli",
        "goal_id": "goal-async-cli",
        "version": 1,
        "summary": "Async one task",
        "max_parallel": 1,
        "approved": false,
        "work_items": [{
            "id": "async-task",
            "title": "Async task",
            "objective": "Create result.txt",
            "required_capabilities": ["shell"],
            "required_tools": ["shell"],
            "acceptance_criteria": ["result.txt exists"],
            "expected_artifacts": ["result.txt"],
            "workspace": workspace.display().to_string(),
            "permissions": {
                "read_roots": [workspace.display().to_string()],
                "allow_write": true
            },
            "status": "pending"
        }]
    });
    let runtime_command = format!(
        "printf async > '{}'",
        workspace.join("result.txt").display()
    );
    let agents = serde_json::json!([{
        "agent_id": "async-worker",
        "runtime": "shell",
        "capabilities": ["shell"],
        "tools": ["shell"],
        "runtime_command": ["sh", "-c", runtime_command],
        "permissions": {"allow_write": true},
        "availability": "idle",
        "max_risk": "high"
    }]);
    let verification = serde_json::json!({
        "task_id": "async-task",
        "criteria_results": [{
            "criterion": "result.txt exists",
            "passed": true,
            "evidence": ["artifact check"]
        }],
        "evidence": ["poll and verifier evidence"],
        "verified_by": "async-cli-verifier"
    });
    std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).unwrap();
    std::fs::write(&agents_path, serde_json::to_vec_pretty(&agents).unwrap()).unwrap();
    std::fs::write(
        &verification_path,
        serde_json::to_vec_pretty(&verification).unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let trace = "trace-async-cli";
    for args in [
        vec![
            "master",
            "init",
            "--plan",
            &plan_path.display().to_string(),
            "--agents",
            &agents_path.display().to_string(),
            "--trace-id",
            trace,
            "--state-root",
            &state_root.display().to_string(),
        ],
        vec![
            "master",
            "approve",
            "--trace-id",
            trace,
            "--state-root",
            &state_root.display().to_string(),
        ],
    ] {
        let output = Command::new(binary).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let start = Command::new(binary)
        .args([
            "master",
            "run",
            "--detach",
            "--trace-id",
            trace,
            "--state-root",
            &state_root.display().to_string(),
            "--runtime-profiles",
            &agents_path.display().to_string(),
            "--workshop-prefix",
            &prefix.display().to_string(),
            "--no-gui",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        start.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&start.stderr)
    );
    let start: serde_json::Value = serde_json::from_slice(&start.stdout).unwrap();
    assert_eq!(start["state"]["tasks"]["async-task"]["status"], "running");
    let poll = Command::new(binary)
        .args([
            "master",
            "poll",
            "--trace-id",
            trace,
            "--state-root",
            &state_root.display().to_string(),
            "--workshop-prefix",
            &prefix.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        poll.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&poll.stderr)
    );
    let poll: serde_json::Value = serde_json::from_slice(&poll.stdout).unwrap();
    assert_eq!(poll["observed"][0]["observation"]["kind"], "finished");
    assert_eq!(
        poll["state"]["tasks"]["async-task"]["status"],
        "artifact_ready"
    );
    let lease_calls = std::fs::read_to_string(root.path().join("lease-calls.log")).unwrap();
    assert!(
        lease_calls.lines().any(|line| line
            .starts_with("release --conversation trace-async-cli --task async-task --attempt 1")),
        "poll must release the task lease with the trace conversation: {lease_calls}"
    );
    let verify = Command::new(binary)
        .args([
            "master",
            "verify",
            "--trace-id",
            trace,
            "--state-root",
            &state_root.display().to_string(),
            "--input",
            &verification_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let verify: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(verify["verification"]["status"], "verified");
}

#[test]
fn cli_tui_master_supervision_closes_the_human_in_the_loop_path() {
    let root = tempfile::tempdir().unwrap();
    let journal_dir = root.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut session = TuiSession::open(&journal_dir, None).unwrap();
    session
        .start_mission(
            "Exercise Master supervision commands",
            root.path().to_path_buf(),
        )
        .unwrap();
    let plan = PlanProposal {
        schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
        plan_id: "plan-supervision-cli".into(),
        goal_id: "goal-supervision-cli".into(),
        version: 1,
        summary: "CLI supervision test".into(),
        work_items: vec![first_principles_agent::WorkItem {
            id: "task-1".into(),
            title: "Supervised task".into(),
            objective: "Wait for explicit operator input".into(),
            dependencies: Vec::new(),
            required_capabilities: vec!["general".into()],
            required_tools: vec!["read_file".into()],
            permissions: first_principles_agent::PermissionSet {
                read_roots: vec![root.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            acceptance_criteria: vec!["result exists".into()],
            expected_artifacts: vec!["result.json".into()],
            workspace: root.path().to_path_buf(),
            budget: first_principles_agent::WorkBudget::default(),
            deadline_unix_ms: None,
            risk: first_principles_agent::RiskLevel::Low,
            status: WorkItemStatus::Pending,
        }],
        max_parallel: 1,
        approved: false,
    };
    let plan_path = session.root().join("plan.json");
    let plan_bytes = serde_json::to_vec_pretty(&plan).unwrap();
    std::fs::write(&plan_path, &plan_bytes).unwrap();
    let digest = hex::encode(Sha256::digest(&plan_bytes));
    session
        .bind_plan(
            plan_path,
            plan.plan_id.clone(),
            plan.goal_id.clone(),
            &digest,
        )
        .unwrap();
    session.approve_plan("tester", &digest, 10).unwrap();
    let assignment = AssignmentProposal {
        schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        assignments: vec![first_principles_agent::AssignmentCandidate {
            task_id: "task-1".into(),
            agent_id: "test-worker".into(),
            runtime: "mock".into(),
            selection_reason: "CLI supervision fixture".into(),
        }],
    };
    session.bind_assignment(assignment.clone()).unwrap();
    let state_root = session.root().join("master-state");
    let trace_id = "trace-supervision-cli";
    session.bind_dispatch(trace_id, state_root.clone()).unwrap();
    let store = MasterStore::new(&state_root, trace_id).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let master = MasterState::initialize(trace_id, plan, assignment, now).unwrap();
    store.initialize(master, now).unwrap();
    let mut master = store.load().unwrap();
    master.approve("tester", now).unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Approved,
            None,
            "tester",
            "approved by CLI supervision fixture",
            now,
        )
        .unwrap();
    let profile = AgentProfile {
        agent_id: "test-worker".into(),
        runtime: "mock".into(),
        display_name: Some("Test Worker".into()),
        capabilities: vec!["general".into()],
        tools: vec!["read_file".into()],
        runtime_command: vec!["true".into()],
        permissions: first_principles_agent::PermissionSet::default(),
        availability: first_principles_agent::AgentAvailability::Idle,
        current_load: 0,
        observed_success_permille: None,
        observed_runs: 0,
        cost_micros: None,
        max_risk: first_principles_agent::RiskLevel::Low,
    };
    let lease = master
        .lease_task(
            "task-1",
            &profile,
            "workshop-session:test-worker".into(),
            600_000,
            now,
        )
        .unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Leased,
            Some("task-1".into()),
            "master",
            "CLI supervision lease",
            now,
        )
        .unwrap();
    master
        .begin_starting("task-1", lease.generation, now)
        .unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Starting,
            Some("task-1".into()),
            "master",
            "CLI supervision start",
            now,
        )
        .unwrap();
    master
        .record_submission(
            ExecutionSubmission {
                task_id: "task-1".into(),
                agent_id: lease.agent_id.clone(),
                lease_generation: lease.generation,
                idempotency_key: lease.idempotency_key,
                execution_target: ExecutionTarget {
                    session_id: Some("term-supervision-cli".into()),
                    pane_id: None,
                    job_id: None,
                },
                submitted_unix_ms: now,
                deadline_unix_ms: now.saturating_add(600_000),
            },
            now,
        )
        .unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Submitted,
            Some("task-1".into()),
            "test-worker",
            "submitted to term-supervision-cli",
            now,
        )
        .unwrap();
    master
        .record_observation(
            "task-1",
            ExecutionObservation::AwaitingInput {
                reason: "approve deployment".into(),
            },
            now,
        )
        .unwrap();
    store
        .append(
            master,
            MasterJournalEventKind::Observed,
            Some("task-1".into()),
            "test-worker",
            "awaiting operator deployment approval",
            now,
        )
        .unwrap();
    std::fs::create_dir_all(store.trace_dir().join("executions")).unwrap();
    std::fs::write(
        store.trace_dir().join("executions/task-1-1.output"),
        "[1A:AWAITING_INPUT] approve deployment\n",
    )
    .unwrap();

    let prefix = root.path().join("skynet-workshop");
    let capture = root.path().join("session-capture.log");
    write_executable(
        &root.path().join("skynet-workshop-session"),
        &format!(
            "#!/bin/sh\n\
             case \"$1\" in\n\
               write|kill) printf '%s\\n' \"$@\" >> '{}' ;;\n\
               status) printf '%s\\n' '{{\"running\":true}}' ;;\n\
             esac\n",
            capture.display()
        ),
    );
    write_executable(
        &root.path().join("skynet-workshop-events"),
        "#!/bin/sh\nprintf '%s\\n' '{\"event\":\"supervision-cli-event\",\"session_id\":\"term-supervision-cli\"}'\n",
    );

    let mut child = Command::new(binary)
        .args([
            "tui",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--session",
            session.id(),
            "--workshop-prefix",
            &prefix.display().to_string(),
        ])
        .env("SKYTERM_HOST_CONTEXT", "skynet-workshop")
        .env("SKYTERM_AGENT_ROLE", "master")
        .env("SKYTERM_HOST_CONTEXT_VERSION", "1")
        .env("SKYTERM_MASTER_CONVERSATION_ID", "main-coordinator")
        .env("SKYTERM_EXPERT_INTERACTIVE", "1")
        .env("SKYTERM_AGENT_ID", "main-coordinator")
        .env("SKYTERM_AGENT_RUNTIME", "1agent")
        .env("SKYTERM_SESSION_ID", "term-supervision-cli")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"/board\n\
              /watch\n\
              /attention\n\
              /respond task-1 approve deployment\n\
              /control pause\n\
              /control resume\n\
              /control takeover task-1 inspect workspace\n\
              /control cancel task-1 operator complete\n\
              /timeline\n\
              /quit\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Board · trace=trace-supervision-cli"),
        "{stdout}"
    );
    assert!(stdout.contains("Attention · trace=trace-supervision-cli"));
    assert!(stdout.contains("已回复 task-1 · session=term-supervision-cli"));
    assert!(stdout.contains("Master run paused"));
    assert!(stdout.contains("Master run resumed"));
    assert!(stdout.contains("任务 task-1 已由人工接管"));
    assert!(stdout.contains("任务 task-1 已取消"));
    assert!(stdout.contains("Timeline · trace=trace-supervision-cli"));
    assert!(stdout.contains("human response routed"), "{stdout}");
    assert!(stdout.contains("supervision-cli-event"));
    assert!(stdout.contains("session=term-supervision-cli"));
    let capture = std::fs::read_to_string(capture).unwrap();
    assert!(capture.contains("write\nterm-supervision-cli\n--require-idle\napprove deployment"));
    assert_eq!(
        store.load().unwrap().tasks["task-1"].status,
        WorkItemStatus::Cancelled
    );
}

#[test]
fn cli_tui_master_verification_commands_require_independent_artifact_evidence() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let journal_dir = root.path().join("journal");
    let mut session = TuiSession::open(&journal_dir, None).unwrap();
    session
        .start_mission("Exercise Master verification commands", workspace.clone())
        .unwrap();
    let plan = PlanProposal {
        schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
        plan_id: "plan-verification-cli".into(),
        goal_id: "goal-verification-cli".into(),
        version: 1,
        summary: "CLI verification test".into(),
        work_items: vec![first_principles_agent::WorkItem {
            id: "task-verify".into(),
            title: "Verified delivery".into(),
            objective: "Deliver a verifiable result".into(),
            dependencies: Vec::new(),
            required_capabilities: vec!["general".into()],
            required_tools: vec!["read_file".into()],
            permissions: first_principles_agent::PermissionSet {
                read_roots: vec![workspace.clone()],
                allow_network: false,
                allow_write: false,
            },
            acceptance_criteria: vec!["result exists".into()],
            expected_artifacts: vec!["result.json".into()],
            workspace: workspace.clone(),
            budget: first_principles_agent::WorkBudget::default(),
            deadline_unix_ms: None,
            risk: first_principles_agent::RiskLevel::Low,
            status: WorkItemStatus::Pending,
        }],
        max_parallel: 1,
        approved: false,
    };
    let plan_path = session.root().join("plan.json");
    let plan_bytes = serde_json::to_vec_pretty(&plan).unwrap();
    std::fs::write(&plan_path, &plan_bytes).unwrap();
    let digest = hex::encode(Sha256::digest(&plan_bytes));
    session
        .bind_plan(
            plan_path,
            plan.plan_id.clone(),
            plan.goal_id.clone(),
            &digest,
        )
        .unwrap();
    session.approve_plan("tester", &digest, 10).unwrap();
    let assignment = AssignmentProposal {
        schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        assignments: vec![first_principles_agent::AssignmentCandidate {
            task_id: "task-verify".into(),
            agent_id: "test-worker".into(),
            runtime: "mock".into(),
            selection_reason: "CLI verification fixture".into(),
        }],
    };
    session.bind_assignment(assignment.clone()).unwrap();
    let state_root = session.root().join("master-state");
    let trace_id = "trace-verification-cli";
    session.bind_dispatch(trace_id, state_root.clone()).unwrap();
    let store = MasterStore::new(&state_root, trace_id).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let master = MasterState::initialize(trace_id, plan, assignment, now).unwrap();
    store.initialize(master, now).unwrap();
    let mut master = store.load().unwrap();
    master.approve("tester", now).unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Approved,
            None,
            "tester",
            "approved by CLI verification fixture",
            now,
        )
        .unwrap();
    let profile = AgentProfile {
        agent_id: "test-worker".into(),
        runtime: "mock".into(),
        display_name: Some("Test Worker".into()),
        capabilities: vec!["general".into()],
        tools: vec!["read_file".into()],
        runtime_command: vec!["true".into()],
        permissions: first_principles_agent::PermissionSet::default(),
        availability: first_principles_agent::AgentAvailability::Idle,
        current_load: 0,
        observed_success_permille: None,
        observed_runs: 0,
        cost_micros: None,
        max_risk: first_principles_agent::RiskLevel::Low,
    };
    let lease = master
        .lease_task(
            "task-verify",
            &profile,
            "workshop-session:test-worker".into(),
            600_000,
            now,
        )
        .unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Leased,
            Some("task-verify".into()),
            "master",
            "verification fixture lease",
            now,
        )
        .unwrap();
    master
        .begin_starting("task-verify", lease.generation, now)
        .unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Starting,
            Some("task-verify".into()),
            "master",
            "verification fixture start",
            now,
        )
        .unwrap();
    master
        .record_submission(
            ExecutionSubmission {
                task_id: "task-verify".into(),
                agent_id: lease.agent_id.clone(),
                lease_generation: lease.generation,
                idempotency_key: lease.idempotency_key.clone(),
                execution_target: ExecutionTarget {
                    session_id: Some("term-verification-cli".into()),
                    pane_id: None,
                    job_id: None,
                },
                submitted_unix_ms: now,
                deadline_unix_ms: now.saturating_add(600_000),
            },
            now,
        )
        .unwrap();
    master = store
        .append(
            master,
            MasterJournalEventKind::Submitted,
            Some("task-verify".into()),
            "test-worker",
            "submitted for verification",
            now,
        )
        .unwrap();
    let artifact = workspace.join("result.json");
    std::fs::write(&artifact, b"verified delivery").unwrap();
    let artifact_digest = hex::encode(Sha256::digest(b"verified delivery"));
    master
        .record_receipt(
            first_principles_agent::ExecutionReceipt {
                task_id: "task-verify".into(),
                agent_id: lease.agent_id,
                lease_generation: lease.generation,
                idempotency_key: lease.idempotency_key,
                execution_target: ExecutionTarget {
                    session_id: Some("term-verification-cli".into()),
                    pane_id: None,
                    job_id: None,
                },
                status: first_principles_agent::ExecutionStatus::Completed,
                exit_code: Some(0),
                artifacts: vec![artifact.display().to_string()],
                result_digest: Some(artifact_digest),
                error: None,
                started_unix_ms: now,
                finished_unix_ms: now,
            },
            now,
        )
        .unwrap();
    store
        .append(
            master,
            MasterJournalEventKind::ReceiptRecorded,
            Some("task-verify".into()),
            "test-worker",
            "artifact ready for independent verification",
            now,
        )
        .unwrap();
    let verification_path = workspace.join("verification.json");
    std::fs::write(
        &verification_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "task_id": "task-verify",
            "criteria_results": [{
                "criterion": "result exists",
                "passed": true,
                "evidence": ["independent CLI verifier checked result.json"]
            }],
            "test_results": [{
                "name": "delivery check",
                "passed": true,
                "evidence": "artifact exists and digest was inspected"
            }],
            "evidence": ["CLI verifier inspected the delivered Artifact"],
            "verified_by": "cli-independent-verifier"
        }))
        .unwrap(),
    )
    .unwrap();
    let prefix = root.path().join("skynet-workshop");
    write_executable(
        &root.path().join("skynet-workshop-events"),
        "#!/bin/sh\nprintf '%s\\n' '{\"event\":\"verification-cli-event\",\"session_id\":\"term-verification-cli\"}'\n",
    );
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args([
            "tui",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--session",
            session.id(),
            "--workshop-prefix",
            &prefix.display().to_string(),
        ])
        .env("SKYTERM_HOST_CONTEXT", "skynet-workshop")
        .env("SKYTERM_AGENT_ROLE", "master")
        .env("SKYTERM_HOST_CONTEXT_VERSION", "1")
        .env("SKYTERM_MASTER_CONVERSATION_ID", "main-coordinator")
        .env("SKYTERM_EXPERT_INTERACTIVE", "1")
        .env("SKYTERM_AGENT_ID", "main-coordinator")
        .env("SKYTERM_AGENT_RUNTIME", "1agent")
        .env("SKYTERM_SESSION_ID", "term-verification-cli")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!(
                "/artifacts task-verify\n\
                 /verify task-verify {}\n\
                 /evidence master\n\
                 /audit\n\
                 /report\n\
                 /quit\n",
                verification_path.display()
            )
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Artifacts · trace=trace-verification-cli"));
    assert!(stdout.contains("verdict=ok"), "{stdout}");
    assert!(stdout.contains("验收：Verified"), "{stdout}");
    assert!(stdout.contains("Master 证据板"), "{stdout}");
    assert!(stdout.contains("claim [pass] result exists"), "{stdout}");
    assert!(stdout.contains("Master Journal 审计 · 通过"), "{stdout}");
    assert!(stdout.contains("生产线报告 · trace=trace-verification-cli"));
    assert!(stdout.contains("verified: Verified"), "{stdout}");
    assert_eq!(
        store.load().unwrap().tasks["task-verify"].status,
        WorkItemStatus::Verified
    );
}

#[test]
fn cli_tui_phase_six_session_workspace_context_preset_export_and_copy_work() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("journal");
    let agents_path = workspace.path().join("AGENTS.md");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let mut child = Command::new(binary)
        .args(["tui", "--journal-dir", &journal_dir.display().to_string()])
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            format!(
                "/status\n\
                 /workspace show\n\
                 /workspace init\n\
                 /workspace mention {}\n\
                 /workspace diff\n\
                 /context show\n\
                 /config set context 20000\n\
                 /config set compact 70\n\
                 /status\n\
                 Compare option A and option B\n\
                 /context recap\n\
                 /context compact\n\
                 /copy\n\
                 /export report\n\
                 /export evidence\n\
                 /preset save review Review the current result\n\
                 /preset list\n\
                 /preset run review\n\
                 /cost\n\
                 /session rename Phase six CLI\n\
                 /session fork Phase six fork\n\
                 /session list all\n\
                 /export session\n\
                 /quit\n",
                agents_path.display()
            )
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("推理步骤      最多 6 步"), "{stdout}");
    assert!(stdout.contains("工作区概览"), "{stdout}");
    assert!(stdout.contains("已创建"), "{stdout}");
    assert!(stdout.contains("已记录引用"), "{stdout}");
    assert!(stdout.contains("无法读取 git diff"), "{stdout}");
    assert!(stdout.contains("剩余空间"), "{stdout}");
    assert!(stdout.contains("推理步骤      最多 6 步"), "{stdout}");
    assert!(stdout.contains("上下文上限    20000 字符"), "{stdout}");
    assert!(stdout.contains("达到 70% 时触发"), "{stdout}");
    assert!(stdout.contains("上下文回顾"), "{stdout}");
    assert!(stdout.contains("上下文已压缩"), "{stdout}");
    assert!(stdout.contains("最近回答已复制"), "{stdout}");
    assert!(stdout.contains("已导出："), "{stdout}");
    assert!(stdout.contains("预设已保存：review"), "{stdout}");
    assert!(stdout.contains("Review the current result"), "{stdout}");
    assert!(stdout.contains("Token 与成本汇总"), "{stdout}");
    assert!(stdout.contains("模型调用      2 次"), "{stdout}");
    assert!(stdout.contains("会话已重命名为：Phase six CLI"), "{stdout}");
    assert!(stdout.contains("Phase six fork"), "{stdout}");
    assert!(stdout.contains("当前会话已切换"), "{stdout}");
    assert!(agents_path.is_file());
    let sessions = TuiSession::list(&journal_dir).unwrap();
    assert!(sessions.len() >= 2);
    assert!(
        sessions
            .iter()
            .any(|session| session.title == "Phase six fork")
    );
}

#[cfg(unix)]
#[test]
fn cli_master_capability_probe_preserves_bounded_blocked_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let manifest = workspace.path().join("capability.json");
    let report_path = workspace.path().join("capability-report.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "suite_id": "cli-probe",
            "tasks": [{
                "id": "exact",
                "prompt": "OK",
                "timeout_seconds": 5,
                "success_rule": {"kind": "exact", "expected": "OK"}
            }],
            "runtimes": [
                {
                    "runtime": "passing",
                    "command": ["/bin/echo"],
                    "prompt_delivery": "arg",
                    "blocked_patterns": []
                },
                {
                    "runtime": "blocked",
                    "command": ["/bin/sh", "-c", "echo authentication required >&2; exit 1"],
                    "prompt_delivery": "none",
                    "blocked_patterns": ["authentication required"]
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "master",
            "capability-probe",
            "--manifest",
            &manifest.display().to_string(),
            "--output",
            &report_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["results"][0]["status"], "passed");
    assert_eq!(stdout["results"][1]["status"], "blocked");
    assert_eq!(
        stdout["results"][1]["failure_kind"],
        "blocked_authentication"
    );
    let persisted = std::fs::read_to_string(report_path).unwrap();
    assert!(persisted.contains("\"status\": \"blocked\""));
    assert!(!persisted.contains("authentication required"));
}

fn write_executable(path: &std::path::Path, body: &str) {
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}

fn read_session_events(journal_dir: &std::path::Path) -> String {
    let sessions = journal_dir.join(".sessions");
    std::fs::read_dir(sessions)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("events.jsonl"))
        .find(|path| path.exists())
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default()
}

#[test]
fn cli_eval_reports_versioned_completeness_policy() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary)
        .args(["eval", "--suite", &suite, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: EvalReport = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report.decision_completeness_policy.version, 1);
    assert_eq!(
        report.decision_completeness_policy.floor_source,
        DecisionCompletenessFloorSource::Default
    );

    let report_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(report_path.path(), &output.stdout).unwrap();
    let inspect_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        inspect_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect_output.stderr)
    );
    let inspected: EvalReport = serde_json::from_slice(&inspect_output.stdout).unwrap();
    assert_eq!(inspected, report);

    let summary_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--summary",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        summary_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&summary_output.stderr)
    );
    let summary: EvalReportSummary = serde_json::from_slice(&summary_output.stdout).unwrap();
    assert_eq!(summary.schema_version, 4);
    assert_eq!(summary.total, report.total);
    assert_eq!(summary.passed, report.passed);
    assert_eq!(summary.report_content_sha256, report.content_sha256);
    assert_eq!(
        summary.report_content_sha256.as_deref().map(str::len),
        Some(64)
    );
    assert!(summary.verification.integrity_checked);
    assert!(!summary.verification.signature_present);
    assert_eq!(
        summary.verification.content_binding,
        ContentBindingStatus::Bound
    );
    assert_eq!(
        summary.verification.authenticity,
        AuthenticityStatus::Unauthenticated
    );
    let summary_json: serde_json::Value = serde_json::from_slice(&summary_output.stdout).unwrap();
    assert!(summary_json.get("cases").is_none());

    let binding_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--require-content-binding",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        binding_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&binding_output.stderr)
    );

    let authenticity_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--require-authenticity",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!authenticity_output.status.success());
    assert!(
        String::from_utf8_lossy(&authenticity_output.stderr).contains(
            "report authenticity is required but no verified report signature was provided"
        )
    );
}

#[test]
fn cli_eval_inspect_rejects_unsupported_policy_version() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary)
        .args(["eval", "--suite", &suite, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["decision_completeness_policy"]["version"] = serde_json::json!(2);
    let report_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(report_path.path(), serde_json::to_vec(&report).unwrap()).unwrap();

    let inspect_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!inspect_output.status.success());
    assert!(
        String::from_utf8_lossy(&inspect_output.stderr)
            .contains("unsupported decision-completeness policy version: 2")
    );

    report["decision_completeness_policy"]["version"] = serde_json::json!(1);
    report["total"] = serde_json::json!(99);
    std::fs::write(report_path.path(), serde_json::to_vec(&report).unwrap()).unwrap();
    let inspect_output = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &report_path.path().display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(!inspect_output.status.success());
    assert!(
        String::from_utf8_lossy(&inspect_output.stderr).contains("case count does not match total")
    );
}

#[test]
fn cli_eval_inspect_verifies_signed_report() {
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let workspace = tempfile::tempdir().unwrap();
    let suite = format!("{}/evals/core.json", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(binary)
        .args(["eval", "--suite", &suite, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report_path = workspace.path().join("report.json");
    std::fs::write(&report_path, &output.stdout).unwrap();

    let registry_path = workspace.path().join("approval-registry.json");
    let key_add = Command::new(binary)
        .args([
            "approval",
            "key-add",
            "--registry",
            &registry_path.display().to_string(),
            "--key-id",
            "report-key",
        ])
        .output()
        .unwrap();
    assert!(
        key_add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&key_add.stderr)
    );

    let approval_path = workspace.path().join("report.approval.json");
    let sign = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "approval",
            "sign",
            "--artifact",
            &report_path.display().to_string(),
            "--artifact-kind",
            "evaluation_report",
            "--approver",
            "qa",
            "--purpose",
            "report-inspection",
            "--key-id",
            "report-key",
            "--key-env",
            "REPORT_APPROVAL_KEY",
            "--registry",
            &registry_path.display().to_string(),
            "--output",
            &approval_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        sign.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sign.stderr)
    );

    let inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--summary",
            "--json",
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let summary: EvalReportSummary = serde_json::from_slice(&inspect.stdout).unwrap();
    assert!(summary.verification.signature_present);
    assert_eq!(
        summary.verification.authenticity,
        AuthenticityStatus::Verified
    );

    let text_inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--summary",
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        text_inspect.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&text_inspect.stderr)
    );
    let text_summary = String::from_utf8_lossy(&text_inspect.stdout);
    assert!(text_summary.contains("floor=0 bps:passed"));
    assert!(text_summary.contains(
        "verification=integrity:checked,binding:bound,signature:present,authenticity:verified"
    ));

    let partial = Command::new(binary)
        .args([
            "eval-inspect",
            "--report",
            &workspace
                .path()
                .join("missing-report.json")
                .display()
                .to_string(),
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!partial.status.success());
    assert!(String::from_utf8_lossy(&partial.stderr).contains("must be supplied together"));

    let wrong_kind_path = workspace.path().join("wrong-kind.approval.json");
    let wrong_kind = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "approval",
            "sign",
            "--artifact",
            &report_path.display().to_string(),
            "--artifact-kind",
            "gold_set",
            "--approver",
            "qa",
            "--purpose",
            "wrong-kind",
            "--key-id",
            "report-key",
            "--key-env",
            "REPORT_APPROVAL_KEY",
            "--registry",
            &registry_path.display().to_string(),
            "--output",
            &wrong_kind_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(wrong_kind.status.success());
    let wrong_kind_inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--require-authenticity",
            "--approval",
            &wrong_kind_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!wrong_kind_inspect.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_kind_inspect.stderr)
            .contains("approval artifact kind does not match the required use")
    );

    let revoke = Command::new(binary)
        .args([
            "approval",
            "key-revoke",
            "--registry",
            &registry_path.display().to_string(),
            "--key-id",
            "report-key",
            "--reason",
            "test rotation",
        ])
        .output()
        .unwrap();
    assert!(revoke.status.success());
    let revoked_inspect = Command::new(binary)
        .env("REPORT_APPROVAL_KEY", "test-only-report-key-32-bytes!!!")
        .args([
            "eval-inspect",
            "--report",
            &report_path.display().to_string(),
            "--require-authenticity",
            "--approval",
            &approval_path.display().to_string(),
            "--approval-key-env",
            "REPORT_APPROVAL_KEY",
            "--approval-registry",
            &registry_path.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!revoked_inspect.status.success());
    assert!(String::from_utf8_lossy(&revoked_inspect.stderr).contains("approval key is revoked"));
}

#[test]
fn schedule_store_and_daemon_once_run_a_job() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("schedules.json");
    let journals = workspace.path().join("daemon-journals");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let add = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "test-job",
            "--task",
            "用一句话回答 1+1",
            "--every-seconds",
            "3600",
            "--start-now",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let job: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();

    let daemon = Command::new(binary)
        .env_remove("AGENT_BASE_URL")
        .env_remove("AGENT_MODEL")
        .env_remove("AGENT_API_KEY")
        .args([
            "daemon",
            "--store",
            &store.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--once",
        ])
        .output()
        .unwrap();
    assert!(
        daemon.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&daemon.stderr)
    );

    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    let updated = state["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|job| job["id"] == job_id.as_str())
        .unwrap();
    assert!(updated["last_status"].is_string());
    assert!(updated["next_run_ms"].as_u64().unwrap() > 0);

    let remove = Command::new(binary)
        .args([
            "schedule",
            "remove",
            &job_id,
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(remove.status.success());
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    assert!(state["jobs"].as_array().unwrap().is_empty());
}

#[test]
fn schedule_trigger_file_runs_once() {
    let workspace = tempfile::tempdir().unwrap();
    let store = workspace.path().join("schedules.json");
    let journals = workspace.path().join("daemon-journals");
    let trigger = workspace.path().join("trigger.txt");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let add = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "trigger-job",
            "--task",
            "用一句话回答 1+1",
            "--every-seconds",
            "3600",
            "--trigger-file",
            &trigger.display().to_string(),
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(add.status.success());
    let job: serde_json::Value = serde_json::from_slice(&add.stdout).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();

    std::fs::write(&trigger, b"first").unwrap();
    let first = Command::new(binary)
        .env_remove("AGENT_BASE_URL")
        .env_remove("AGENT_MODEL")
        .env_remove("AGENT_API_KEY")
        .args([
            "daemon",
            "--store",
            &store.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--once",
        ])
        .output()
        .unwrap();
    assert!(first.status.success());
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    let first_run = state["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|job| job["id"] == job_id.as_str())
        .unwrap()["last_run_ms"]
        .as_u64();

    let second = Command::new(binary)
        .env_remove("AGENT_BASE_URL")
        .env_remove("AGENT_MODEL")
        .env_remove("AGENT_API_KEY")
        .args([
            "daemon",
            "--store",
            &store.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--once",
        ])
        .output()
        .unwrap();
    assert!(second.status.success());
    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    let second_run = state["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|job| job["id"] == job_id.as_str())
        .unwrap()["last_run_ms"]
        .as_u64();
    assert_eq!(first_run, second_run);
}

#[test]
fn cost_reports_usage_and_latency_percentiles() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("journal");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let run = Command::new(binary)
        .args([
            "run",
            "--task",
            "用一句话回答 1+1",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let cost = Command::new(binary)
        .args([
            "cost",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        cost.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&cost.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&cost.stdout).unwrap();
    assert!(value["usage"]["totals"]["calls"].as_u64().unwrap() >= 1);
    let latency = value["latency"].as_array().unwrap();
    assert!(!latency.is_empty());
    assert!(latency[0]["p50_ms"].is_u64());
    assert!(latency[0]["p95_ms"].is_u64());
}

#[test]
fn installer_places_a_runnable_binary_in_a_prefix() {
    let workspace = tempfile::tempdir().unwrap();
    let prefix = workspace.path().join("prefix");
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/install.sh");
    let output = Command::new("bash")
        .arg(script)
        .args([
            "--prefix",
            &prefix.display().to_string(),
            "--binary",
            env!("CARGO_BIN_EXE_first-principles-agent"),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let installed = prefix.join("bin/1agent");
    assert!(installed.is_file());
    let version = Command::new(&installed).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("1agent"));
}

#[test]
fn digest_reports_recent_runs_and_schedules() {
    let workspace = tempfile::tempdir().unwrap();
    let journal_dir = workspace.path().join("journal");
    let store = workspace.path().join("schedules.json");
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let run = Command::new(binary)
        .args([
            "run",
            "--task",
            "用一句话回答 1+1",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(run.status.success());
    let add = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "digest-job",
            "--task",
            "用一句话回答 2+2",
            "--every-seconds",
            "60",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(add.status.success());

    let alert_log = workspace.path().join("alerts.jsonl");
    let alert = serde_json::json!({
        "alert_id": "alert-1",
        "at_ms": 1,
        "kind": "provider_latency",
        "provider_id": "p",
        "model": "m",
        "observed": 10,
        "threshold": 5,
        "message": "slow",
    });
    std::fs::write(&alert_log, format!("{alert}\n")).unwrap();

    let digest = Command::new(binary)
        .args([
            "digest",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--schedule-store",
            &store.display().to_string(),
            "--alert-log",
            &alert_log.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        digest.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&digest.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&digest.stdout).unwrap();
    assert!(!value["runs"].as_array().unwrap().is_empty());
    assert_eq!(value["schedules"]["jobs"].as_array().unwrap().len(), 1);
    assert!(
        value["stop_reasons"]
            .as_object()
            .unwrap()
            .values()
            .any(|count| count.as_u64().unwrap() >= 1)
    );
    assert!(value["next_run_ms"].as_u64().is_some());
    assert_eq!(value["schedule_health"][0]["state"], "ok");
    assert_eq!(value["alerts"]["total"], 1);
    assert_eq!(value["alerts"]["by_kind"]["provider_latency"], 1);
}

#[test]
fn retention_dry_run_and_apply() {
    let workspace = tempfile::tempdir().unwrap();
    let old = workspace.path().join("old.jsonl");
    let new = workspace.path().join("new.jsonl");
    std::fs::write(&old, b"old").unwrap();
    std::fs::write(&new, b"new").unwrap();
    let old_file = std::fs::File::options().write(true).open(&old).unwrap();
    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
    old_file
        .set_times(std::fs::FileTimes::new().set_modified(old_time))
        .unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let dry = Command::new(binary)
        .args([
            "retention",
            "--journal-dir",
            &workspace.path().display().to_string(),
            "--older-than-days",
            "30",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(dry.status.success());
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(report["matched"].as_u64(), Some(1));
    assert!(old.exists());

    let apply = Command::new(binary)
        .args([
            "retention",
            "--journal-dir",
            &workspace.path().display().to_string(),
            "--older-than-days",
            "30",
            "--apply",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(apply.status.success());
    assert!(!old.exists());
    assert!(new.exists());
    assert!(workspace.path().join("retention-receipt.jsonl").exists());
}

#[test]
fn provider_alerts_summarizes_the_alert_log() {
    let workspace = tempfile::tempdir().unwrap();
    let log = workspace.path().join("alerts.jsonl");
    let records = [
        serde_json::json!({
            "alert_id": "a1", "at_ms": 1, "kind": "provider_health",
            "provider_id": "deepseek", "model": "flash",
            "observed": 2, "threshold": 2, "message": "down"
        }),
        serde_json::json!({
            "alert_id": "a2", "at_ms": 2, "kind": "provider_health",
            "provider_id": "deepseek", "model": "flash",
            "observed": 3, "threshold": 2, "message": "still down"
        }),
    ];
    let mut text = String::new();
    for record in records {
        text.push_str(&serde_json::to_string(&record).unwrap());
        text.push('\n');
    }
    std::fs::write(&log, text).unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "provider-alerts",
            "--alert-log",
            &log.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["alerts"].as_u64(), Some(2));
    assert_eq!(value["groups"][0]["count"].as_u64(), Some(2));
}

#[test]
fn notify_sends_a_loopback_webhook() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 8192];
        let bytes = stream.read(&mut buffer).unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .unwrap();
        String::from_utf8_lossy(&buffer[..bytes]).to_string()
    });

    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(binary)
        .args([
            "notify",
            "--webhook",
            &format!("http://{address}/"),
            "--message",
            "hello-notify",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let request = server.join().unwrap();
    assert!(request.contains("hello-notify"));
}

#[test]
fn default_run_allows_execution_and_read_only_blocks_it() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");

    let default_journal = workspace.path().join("default-journal");
    let output = Command::new(binary)
        .args([
            "run",
            "--task",
            "执行写入文件操作",
            "--journal-dir",
            &default_journal.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_ne!(run.stop_reason, StopReason::NeedsAuthorization);

    let read_only_journal = workspace.path().join("read-only-journal");
    let output = Command::new(binary)
        .args([
            "run",
            "--read-only",
            "--task",
            "执行写入文件操作",
            "--journal-dir",
            &read_only_journal.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(run.stop_reason, StopReason::NeedsAuthorization);
}

#[test]
fn skill_bundles_export_and_import_only_with_a_matching_sha256() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let store = workspace.path().join("skills.jsonl");
    let bundle_path = workspace.path().join("bundle.json");

    let body = "step one\nstep two";
    let body_sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    let bundle = serde_json::json!({
        "schema_version": 1,
        "skills": [{
            "name": "release-check",
            "description": "check release",
            "version": "1.0.0",
            "platforms": ["linux"],
            "scope": "project",
            "requires_tools": [],
            "risk": "low",
            "body": body,
            "content_sha256": body_sha,
        }]
    });
    let bundle_bytes = serde_json::to_vec_pretty(&bundle).unwrap();
    std::fs::write(&bundle_path, &bundle_bytes).unwrap();
    let bundle_sha = format!("{:x}", Sha256::digest(&bundle_bytes));

    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &store.display().to_string(),
            "--bundle",
            &bundle_path.display().to_string(),
            "--sha256",
            &bundle_sha,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["imported"], 1);
    assert_eq!(report["skipped"], 0);

    let exported_path = workspace.path().join("exported.json");
    let output = Command::new(binary)
        .args([
            "skill",
            "export",
            "--store",
            &store.display().to_string(),
            "--out",
            &exported_path.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(exported["skills"], 1);
    let exported_bytes = std::fs::read(&exported_path).unwrap();
    assert_eq!(
        exported["sha256"],
        format!("{:x}", Sha256::digest(&exported_bytes))
    );

    let tampered_store = workspace.path().join("tampered.jsonl");
    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &tampered_store.display().to_string(),
            "--bundle",
            &bundle_path.display().to_string(),
            "--sha256",
            &"0".repeat(64),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("sha256 mismatch"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!tampered_store.exists());
}

#[test]
fn routine_skill_is_gated_by_activation_and_composed_by_the_daemon() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let store = workspace.path().join("skills.jsonl");
    let bundle_path = workspace.path().join("routine.json");
    let schedules = workspace.path().join("schedules.json");
    let journals = workspace.path().join("routine-journals");

    let body = "ROUTINE_MARKER: always report the check result.";
    let bundle = serde_json::json!({
        "schema_version": 1,
        "skills": [{
            "name": "routine-demo",
            "description": "routine demo",
            "version": "1.0.0",
            "platforms": ["linux"],
            "scope": "project",
            "requires_tools": [],
            "risk": "low",
            "body": body,
            "content_sha256": format!("{:x}", Sha256::digest(body.as_bytes())),
        }]
    });
    let bundle_bytes = serde_json::to_vec_pretty(&bundle).unwrap();
    std::fs::write(&bundle_path, &bundle_bytes).unwrap();
    let bundle_sha = format!("{:x}", Sha256::digest(&bundle_bytes));

    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &store.display().to_string(),
            "--bundle",
            &bundle_path.display().to_string(),
            "--sha256",
            &bundle_sha,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(binary)
        .args([
            "run",
            "--routine-skill",
            "routine-demo@1.0.0",
            "--task",
            "check the build",
            "--journal-dir",
            &workspace
                .path()
                .join("inactive-journal")
                .display()
                .to_string(),
            "--json",
        ])
        .env("ONEAGENT_SKILL_JOURNAL", &store)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("has no active version"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(binary)
        .args([
            "skill",
            "activate",
            "--store",
            &store.display().to_string(),
            "--name",
            "routine-demo",
            "--version",
            "1.0.0",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "routine-job",
            "--task",
            "check the build",
            "--every-seconds",
            "3600",
            "--start-now",
            "--routine-skill",
            "routine-demo@1.0.0",
            "--store",
            &schedules.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let job: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(job["routine_skill"], "routine-demo@1.0.0");
    let job_id = job["id"].as_str().unwrap().to_string();

    let output = Command::new(binary)
        .env_remove("AGENT_BASE_URL")
        .env_remove("AGENT_MODEL")
        .env_remove("AGENT_API_KEY")
        .env("ONEAGENT_SKILL_JOURNAL", &store)
        .args([
            "daemon",
            "--store",
            &schedules.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--once",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut journal_files = Vec::new();
    for entry in std::fs::read_dir(&journals).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            for child in std::fs::read_dir(&path).unwrap() {
                let child = child.unwrap().path();
                if child.extension().and_then(|value| value.to_str()) == Some("jsonl") {
                    journal_files.push(child);
                }
            }
        }
    }
    assert_eq!(journal_files.len(), 1, "{journal_files:?}");
    let first_line = std::fs::read_to_string(&journal_files[0])
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    let event: serde_json::Value = serde_json::from_str(&first_line).unwrap();
    assert_eq!(event["event"]["type"], "run_started");
    let objective = event["event"]["data"]["request"]["objective"]
        .as_str()
        .unwrap();
    assert!(objective.contains("## Routine: routine-demo 1.0.0"));
    assert!(objective.contains(body));
    assert!(objective.contains("## Task\ncheck the build"));

    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&schedules).unwrap()).unwrap();
    let updated = state["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|job| job["id"] == job_id.as_str())
        .unwrap();
    assert!(updated["last_status"].is_string());
}

#[test]
fn daemon_catches_up_an_overdue_job_after_restart_and_health_reports_it() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let store = workspace.path().join("schedules.json");
    let journals = workspace.path().join("restart-journals");

    let output = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "restart-job",
            "--task",
            "用一句话回答 1+1",
            "--every-seconds",
            "3600",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let job: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let job_id = job["id"].as_str().unwrap().to_string();
    assert!(job["last_run_ms"].is_null());

    // Simulate downtime: the persisted next run passed while no daemon ran.
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    state["jobs"][0]["next_run_ms"] = serde_json::json!(0);
    std::fs::write(&store, serde_json::to_vec_pretty(&state).unwrap()).unwrap();

    let output = Command::new(binary)
        .args([
            "schedule",
            "health",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let health: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(health[0]["state"], "overdue");

    let output = Command::new(binary)
        .env_remove("AGENT_BASE_URL")
        .env_remove("AGENT_MODEL")
        .env_remove("AGENT_API_KEY")
        .args([
            "daemon",
            "--store",
            &store.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--once",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    let updated = state["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|job| job["id"] == job_id.as_str())
        .unwrap();
    assert!(!updated["last_run_ms"].is_null());
    assert!(updated["next_run_ms"].as_u64().unwrap() > 0);

    let output = Command::new(binary)
        .args([
            "schedule",
            "health",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let health: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_ne!(health[0]["state"], "overdue");
}

#[test]
fn notify_sends_slack_and_discord_payload_shapes() {
    use std::io::{Read, Write};

    fn capture(listener: std::net::TcpListener) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 8192];
            let bytes = stream.read(&mut buffer).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
            String::from_utf8_lossy(&buffer[..bytes]).to_string()
        })
    }

    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    for (format, marker) in [("slack", "\"text\""), ("discord", "\"content\"")] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = capture(listener);
        let output = Command::new(binary)
            .args([
                "notify",
                "--webhook",
                &format!("http://{address}/"),
                "--message",
                "format-check",
                "--format",
                format,
                "--json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let request = server.join().unwrap();
        assert!(request.contains(marker), "request: {request}");
        assert!(request.contains("format-check"));
        assert!(!request.contains("alert_id"), "request: {request}");
    }
}

#[test]
fn daemon_alerts_when_a_scheduled_run_fails() {
    use std::io::{Read, Write};

    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let store = workspace.path().join("schedules.json");
    let journals = workspace.path().join("alert-journals");
    let alerts = workspace.path().join("alerts.jsonl");
    let missing_workspace = workspace.path().join("missing-workspace");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for _ in 0..500 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 8192];
                    let bytes = stream.read(&mut buffer).unwrap();
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .unwrap();
                    return String::from_utf8_lossy(&buffer[..bytes]).to_string();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("accept failed: {error}"),
            }
        }
        panic!("daemon did not send a webhook alert");
    });

    let output = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "broken-job",
            "--task",
            "should never run",
            "--every-seconds",
            "3600",
            "--workspace",
            &missing_workspace.display().to_string(),
            "--start-now",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(binary)
        .env_remove("AGENT_BASE_URL")
        .env_remove("AGENT_MODEL")
        .env_remove("AGENT_API_KEY")
        .args([
            "daemon",
            "--store",
            &store.display().to_string(),
            "--journal-dir",
            &journals.display().to_string(),
            "--once",
            "--alert-webhook",
            &format!("http://{address}/"),
            "--alert-log",
            &alerts.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let request = server.join().unwrap();
    assert!(
        request.contains("schedule_run_failed"),
        "request: {request}"
    );
    assert!(request.contains("broken-job"), "request: {request}");

    let alert_log = std::fs::read_to_string(&alerts).unwrap();
    assert!(alert_log.contains("schedule_run_failed"));
    assert!(alert_log.contains("broken-job"));

    let state: serde_json::Value = serde_json::from_slice(&std::fs::read(&store).unwrap()).unwrap();
    assert_eq!(state["jobs"][0]["last_status"], "failed");
}

#[test]
fn dashboard_renders_usage_health_and_escaped_alerts() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let journal_dir = workspace.path().join("journals");
    let store = workspace.path().join("schedules.json");
    let alert_log = workspace.path().join("alerts.jsonl");
    let out = workspace.path().join("dashboard.html");

    let run = Command::new(binary)
        .args([
            "run",
            "--task",
            "用一句话回答 1+1",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(run.status.success());

    let add = Command::new(binary)
        .args([
            "schedule",
            "add",
            "--name",
            "dashboard-job",
            "--task",
            "用一句话回答 2+2",
            "--every-seconds",
            "60",
            "--store",
            &store.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(add.status.success());

    let alert = serde_json::json!({
        "alert_id": "alert-1",
        "at_ms": 1,
        "kind": "provider_latency",
        "provider_id": "p",
        "model": "m",
        "observed": 10,
        "threshold": 5,
        "message": "<script>alert(1)</script>",
    });
    std::fs::write(&alert_log, format!("{alert}\n")).unwrap();

    let output = Command::new(binary)
        .args([
            "dashboard",
            "--journal-dir",
            &journal_dir.display().to_string(),
            "--schedule-store",
            &store.display().to_string(),
            "--alert-log",
            &alert_log.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["jobs"], 1);
    assert_eq!(summary["alerts"], 1);

    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("1Agent 运行看板"));
    assert!(html.contains("按任务"));
    assert!(html.contains("计划健康"));
    assert!(html.contains("dashboard-job"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("Content-Security-Policy"));
}

#[test]
fn signed_skill_bundles_verify_against_a_trusted_public_key() {
    let workspace = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_first-principles-agent");
    let source_store = workspace.path().join("source.jsonl");
    let bundle_path = workspace.path().join("unsigned.json");
    let signed_path = workspace.path().join("signed.json");
    let private_key = workspace.path().join("signing.key");
    let public_key = workspace.path().join("signing.pub");

    let body = "signed body";
    let bundle = serde_json::json!({
        "schema_version": 1,
        "skills": [{
            "name": "signed-skill",
            "description": "signed demo",
            "version": "1.0.0",
            "platforms": ["linux"],
            "scope": "project",
            "requires_tools": [],
            "risk": "low",
            "body": body,
            "content_sha256": format!("{:x}", Sha256::digest(body.as_bytes())),
        }]
    });
    let bytes = serde_json::to_vec_pretty(&bundle).unwrap();
    std::fs::write(&bundle_path, &bytes).unwrap();
    let sha = format!("{:x}", Sha256::digest(&bytes));
    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &source_store.display().to_string(),
            "--bundle",
            &bundle_path.display().to_string(),
            "--sha256",
            &sha,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(binary)
        .args([
            "skill",
            "keygen",
            "--private-key",
            &private_key.display().to_string(),
            "--public-key",
            &public_key.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let keys: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(keys["public_key"].as_str().unwrap().len(), 64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&private_key)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    let output = Command::new(binary)
        .args([
            "skill",
            "export",
            "--store",
            &source_store.display().to_string(),
            "--out",
            &signed_path.display().to_string(),
            "--signing-key",
            &private_key.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let signed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&signed_path).unwrap()).unwrap();
    assert_eq!(signed["signature"]["algorithm"], "ed25519");
    assert_eq!(signed["signature"]["public_key"], keys["public_key"]);

    let target_store = workspace.path().join("target.jsonl");
    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &target_store.display().to_string(),
            "--bundle",
            &signed_path.display().to_string(),
            "--sha256",
            exported["sha256"].as_str().unwrap(),
            "--public-key",
            &public_key.display().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["imported"], 1);

    let no_key_store = workspace.path().join("no-key.jsonl");
    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &no_key_store.display().to_string(),
            "--bundle",
            &signed_path.display().to_string(),
            "--sha256",
            exported["sha256"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("provide the trusted public key"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!no_key_store.exists());

    let mut tampered = signed.clone();
    let signature = tampered["signature"]["signature"]
        .as_str()
        .unwrap()
        .to_string();
    let first = if signature.starts_with('0') { '1' } else { '0' };
    tampered["signature"]["signature"] = format!("{first}{}", &signature[1..]).into();
    let tampered_bytes = serde_json::to_vec_pretty(&tampered).unwrap();
    let tampered_path = workspace.path().join("tampered.json");
    std::fs::write(&tampered_path, &tampered_bytes).unwrap();
    let tampered_sha = format!("{:x}", Sha256::digest(&tampered_bytes));
    let tampered_store = workspace.path().join("tampered.jsonl");
    let output = Command::new(binary)
        .args([
            "skill",
            "import",
            "--store",
            &tampered_store.display().to_string(),
            "--bundle",
            &tampered_path.display().to_string(),
            "--sha256",
            &tampered_sha,
            "--public-key",
            &public_key.display().to_string(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("signature is invalid"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
