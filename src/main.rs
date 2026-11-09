use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use first_principles_agent::{
    AgentAnswer, AgentConfig, AgentProfile, AgentRuntime, AnnotationLabel, AnnotationRecord,
    AnnotationStore, AnswerStatus, ApprovalEnvelope, ApprovalKeyRegistry, ApprovalKeyStatus,
    ArtifactKind, AuthenticityStatus, CalibrationPrediction, CapabilityProbeManifest,
    CognitiveMode, CommandMenuItem, CommandStage, ConversationQueue, DeterministicProvider,
    DoctorStatus, EvalReportSummary, EvalReportTrustRequirements, ExecutionObservation,
    ExecutionReceipt, ExecutionStatus, ExecutionSubmission, ExecutionTarget,
    ExperienceCandidateSpec, ExperienceStore, ExperienceVerificationInput, GoldSetManifest,
    HostBinding, HostCommand, HostError, HostRegistry, HostRequestState, HostResponse, Journal,
    JournalEventKind, MasterJournalEventKind, MasterState, MasterStore, ModelPlanGenerator,
    ModelProductionPlanGenerator, ModelProvider, OpenAiCompatibleProvider, PlanGenerator,
    PlanProposal, PlanningContext, PolicyValueSource, ProductionPlan, ProductionPlanningContext,
    ProviderAdapter, ProviderCandidate, ProviderProbeReport, ProviderToolProbeReport,
    REFRAME_SYSTEM_PROMPT, ReframeChoice, ReframeProposal, RegistrySnapshot, RejectedReason,
    ReleasePolicy, ReleasePolicyMode, RequestRole, RiskLevel, RouteObjective, RoutePolicySources,
    RouteRequest, Router, RunBudget, RuntimeError, SchedulerLimits, SessionEvent,
    SessionPreferences, StopReason, TaskRequest, TerminalSession, ToolObservation, ToolRegistry,
    ToolStatus, TriageSignals, TrustAnchorManifest, TuiCommandId, TuiInput, TuiInputResolution,
    TuiPreset, TuiSession, TuiState, TuiStatus, TuiTone, VerificationInput, WorkItemStatus,
    WorkshopCliAdapter, add_git_worktree, artifact_expectation, artifacts_summary, audit_events,
    audit_summary, build_release_verification_report, build_usage_ledger, build_user_message,
    collect_provenance, collect_release_manifest, commands_for_surface, copy_to_clipboard,
    create_goldset_manifest, create_trust_anchor, current_time_ms, default_home, default_registry,
    doctor, epistemic_command_prompt, evaluate_experience_suite, evidence_summary, export,
    filter_capability_manifest_by_runtime, find_resumable_prefix, git_diff, git_worktrees,
    home_or_legacy, infer_mode, init, init_agents_file, inspect_artifacts,
    interactive_terminal_available, is_destructive_input, key_escrow::escrow as escrow_keys,
    key_escrow::restore as restore_keys, list_presets, load_agent_profiles, load_eval_report,
    load_preset, load_suite, local_manifest, master_audit_summary, master_evidence_summary,
    master_report, next_actions, parse_model_output, print_session_banner, probe_provider,
    probe_provider_tools, prompt_sha256, read_clipboard, read_events, read_master_journal_events,
    rebuild_and_compare, remote_manifest, render_capability_report, render_receipt,
    render_usage_ledger, replay_events, replay_master_journal, resolve_input, restore,
    risk_for_mode, run_capability_suite, run_report, run_suite_with_providers, save_preset,
    scan_history, schedule_ready, spawn_input_reader, stage_next_actions, summarize_route_trace,
    terminal_dimensions, trash_preset, triage_goal, uninstall, upgrade, verify_goldset_manifest,
    verify_provenance, verify_release_approval, verify_release_manifest, verify_snapshot_chain,
    verify_snapshot_chain_with_anchor, write_capability_report,
};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "1agent",
    version,
    about = "Evidence-accountable decision agent"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long, conflicts_with = "input")]
        task: Option<String>,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(
            long,
            conflicts_with_all = ["provider", "model", "base_url", "api_key"]
        )]
        config: Option<PathBuf>,
        #[arg(long, default_value = "mock")]
        provider: String,
        #[arg(long, env = "AGENT_MODEL")]
        model: Option<String>,
        #[arg(long, env = "AGENT_BASE_URL")]
        base_url: Option<String>,
        #[arg(long, env = "AGENT_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long)]
        read_root: Vec<PathBuf>,
        #[arg(long, default_value_os_t = home_or_legacy("journals", ".journal"))]
        journal_dir: PathBuf,
        #[arg(long)]
        experience_dir: Option<PathBuf>,
        #[arg(long)]
        experience_scope: Option<String>,
        #[arg(long, default_value_t = 6)]
        max_steps: usize,
        #[arg(long, default_value_t = 8)]
        max_tool_calls: usize,
        #[arg(long, default_value_t = 4)]
        max_provider_attempts: usize,
        #[arg(long, default_value_t = 50)]
        provider_retry_backoff_ms: u64,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 100_000)]
        max_context_chars: usize,
        #[arg(long)]
        max_provider_p95_latency_ms: Option<u64>,
        #[arg(long)]
        max_unit_cost_micros_per_k: Option<u64>,
        #[arg(long)]
        evidence_max_age_ms: Option<u64>,
        #[arg(long)]
        route_objective: Option<String>,
        #[arg(long)]
        authority: Option<PathBuf>,
        #[arg(long, default_value = "operator")]
        actor_id: String,
        #[arg(long, default_value = "workspace")]
        authority_scope: String,
        #[arg(long)]
        provider_health: bool,
        #[arg(long)]
        alert_log: Option<PathBuf>,
        #[arg(long)]
        alert_webhook: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Tui {
        #[arg(
            long,
            conflicts_with_all = ["provider", "model", "base_url", "api_key"]
        )]
        config: Option<PathBuf>,
        #[arg(long, default_value = "mock")]
        provider: String,
        #[arg(long, env = "AGENT_MODEL")]
        model: Option<String>,
        #[arg(long, env = "AGENT_BASE_URL")]
        base_url: Option<String>,
        #[arg(long, env = "AGENT_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long)]
        allow_write: bool,
        #[arg(long)]
        read_root: Vec<PathBuf>,
        #[arg(long, default_value_os_t = home_or_legacy("journals", ".journal"))]
        journal_dir: PathBuf,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        experience_dir: Option<PathBuf>,
        #[arg(long)]
        experience_scope: Option<String>,
        #[arg(long, default_value_t = 6)]
        max_steps: usize,
        #[arg(long, default_value_t = 8)]
        max_tool_calls: usize,
        #[arg(long)]
        max_provider_attempts: Option<usize>,
        #[arg(long)]
        master_agents: Option<PathBuf>,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 100_000)]
        max_context_chars: usize,
        #[arg(long)]
        route_objective: Option<String>,
    },
    Master {
        #[command(subcommand)]
        command: MasterCommand,
    },
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
    Audit {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        json: bool,
    },
    AuditStore {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        thread: Option<String>,
        #[arg(long)]
        ticket: Option<String>,
        #[arg(long)]
        memory: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long)]
        skill: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        export: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    TaskGraph {
        #[command(subcommand)]
        command: TaskGraphCommand,
    },
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    Surface {
        #[command(subcommand)]
        command: SurfaceCommand,
    },
    Replay {
        #[arg(long)]
        journal: PathBuf,
    },
    Eval {
        #[arg(long, default_value = "evals/core.json")]
        suite: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long)]
        json: bool,
    },
    EvalInspect {
        #[arg(long)]
        report: PathBuf,
        #[arg(long)]
        summary: bool,
        #[arg(long)]
        require_content_binding: bool,
        #[arg(long)]
        require_authenticity: bool,
        #[arg(long)]
        approval: Option<PathBuf>,
        #[arg(long)]
        approval_key_env: Option<String>,
        #[arg(long)]
        approval_registry: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Serve {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long, default_value = "mock")]
        provider: String,
        #[arg(long, env = "AGENT_MODEL")]
        model: Option<String>,
        #[arg(long, env = "AGENT_BASE_URL")]
        base_url: Option<String>,
        #[arg(long, env = "AGENT_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long, default_value_os_t = home_or_legacy("journals", ".journal"))]
        journal_dir: PathBuf,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 4)]
        max_concurrency: usize,
        #[arg(long)]
        provider_health: bool,
        #[arg(long)]
        alert_log: Option<PathBuf>,
        #[arg(long)]
        alert_webhook: Option<String>,
    },
    Providers {
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        probe: bool,
        #[arg(long, conflicts_with = "probe")]
        probe_tools: bool,
        #[arg(long)]
        allow_network: bool,
        #[arg(long, default_value_t = 30)]
        probe_timeout_seconds: u64,
        #[arg(long)]
        json: bool,
    },
    Tools {
        #[arg(long)]
        json: bool,
    },
    RoutePlan {
        #[arg(long, conflicts_with = "input")]
        task: Option<String>,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        allow_network: bool,
        #[arg(long)]
        max_provider_p95_latency_ms: Option<u64>,
        #[arg(long)]
        max_unit_cost_micros_per_k: Option<u64>,
        #[arg(long)]
        route_objective: Option<String>,
        #[arg(long)]
        json: bool,
    },
    History {
        #[arg(long, default_value_os_t = home_or_legacy("journals", ".journal"))]
        journal_dir: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    Annotate {
        #[command(subcommand)]
        command: AnnotateCommand,
    },
    Approval {
        #[command(subcommand)]
        command: ApprovalCommand,
    },
    Provenance {
        #[command(subcommand)]
        command: ProvenanceCommand,
    },
    Release {
        #[command(subcommand)]
        command: ReleaseCommand,
    },
    Resume {
        #[arg(long)]
        journal: PathBuf,
        #[arg(
            long,
            conflicts_with_all = ["provider", "model", "base_url", "api_key"]
        )]
        config: Option<PathBuf>,
        #[arg(long, default_value = "mock")]
        provider: String,
        #[arg(long, env = "AGENT_MODEL")]
        model: Option<String>,
        #[arg(long, env = "AGENT_BASE_URL")]
        base_url: Option<String>,
        #[arg(long, env = "AGENT_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long, default_value_os_t = home_or_legacy("journals", ".journal"))]
        journal_dir: PathBuf,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long)]
        route_objective: Option<String>,
        #[arg(long)]
        json: bool,
    },
    ResumeStatus {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Experience {
        #[command(subcommand)]
        command: ExperienceCommand,
    },
    Reframe {
        #[arg(long, conflicts_with = "input")]
        task: Option<String>,
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        constraint: Vec<String>,
        #[arg(long, env = "AGENT_BASE_URL")]
        base_url: Option<String>,
        #[arg(long, env = "AGENT_MODEL")]
        model: Option<String>,
        #[arg(long, env = "AGENT_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long, default_value_t = 60)]
        timeout_seconds: u64,
        #[arg(long)]
        accept: Option<usize>,
        #[arg(long)]
        substitute: Option<usize>,
        #[arg(long)]
        reject: Option<String>,
        #[arg(long)]
        defer: bool,
        #[arg(long, default_value_os_t = home_or_legacy("journals", ".journal"))]
        journal_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Receipt {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Init {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Doctor {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Export {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Restore {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Upgrade {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        rollback: bool,
        #[arg(long)]
        json: bool,
    },
    Uninstall {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        purge: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum MasterCommand {
    Validate {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        json: bool,
    },
    ProductionTaskList {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Schedule {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 6)]
        global_active: usize,
        #[arg(long, default_value_t = 3)]
        per_conversation: usize,
        #[arg(long)]
        json: bool,
    },
    Assign {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        agents: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Triage {
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "low")]
        risk: String,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        external_action: bool,
        #[arg(long)]
        requested_items: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    CapabilityProbe {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long = "runtime")]
        runtime: Vec<String>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    Plan {
        #[arg(long)]
        task: String,
        #[arg(
            long,
            conflicts_with_all = ["provider", "model", "base_url", "api_key"]
        )]
        config: Option<PathBuf>,
        #[arg(long, default_value = "mock")]
        provider: String,
        #[arg(long, env = "AGENT_MODEL")]
        model: Option<String>,
        #[arg(long, env = "AGENT_BASE_URL")]
        base_url: Option<String>,
        #[arg(long, env = "AGENT_API_KEY", hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long)]
        risk: Option<String>,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        external_action: bool,
        #[arg(long)]
        requested_items: Option<usize>,
        #[arg(long)]
        agents: Option<PathBuf>,
        #[arg(long)]
        workshop_prefix: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_os_t = home_or_legacy("master-plans", ".master-plans"))]
        planner_journal_dir: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Init {
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        agents: PathBuf,
        #[arg(long)]
        workshop_prefix: Option<String>,
        #[arg(long)]
        trace_id: Option<String>,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Status {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Approve {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        json: bool,
    },
    Reject {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Revise {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        agents: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Run {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        runtime_profiles: PathBuf,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        #[arg(long)]
        no_gui: bool,
        #[arg(long)]
        detach: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Poll {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        #[arg(long)]
        json: bool,
    },
    Verify {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Retry {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "master")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        #[arg(long)]
        json: bool,
    },
    ReplanRequest {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "master")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        #[arg(long)]
        json: bool,
    },
    ReplanApply {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        agents: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Pause {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        json: bool,
    },
    Resume {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        json: bool,
    },
    Takeover {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Accept {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Return {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        json: bool,
    },
    Reclaim {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        /// Assert the old execution is already gone; skip the cancel call.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    Cancel {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        task: String,
        #[arg(long, default_value = "user")]
        actor: String,
        #[arg(long)]
        reason: String,
        #[arg(long, default_value = "skynet-workshop")]
        workshop_prefix: String,
        #[arg(long)]
        json: bool,
    },
    Audit {
        #[arg(long)]
        trace_id: String,
        #[arg(long, default_value_os_t = home_or_legacy("master", ".master"))]
        state_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Agents {
        #[arg(long, default_value = "skynet-workshop")]
        prefix: String,
        #[arg(long)]
        json: bool,
    },
    Jobs {
        #[arg(long, default_value = "skynet-workshop")]
        prefix: String,
        #[arg(long)]
        json: bool,
    },
    Events {
        #[arg(long, default_value = "skynet-workshop")]
        prefix: String,
        #[arg(long)]
        since_ms: Option<u128>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum TaskGraphCommand {
    Snapshot {
        #[arg(long, default_value_os_t = home_or_legacy("task-graph.jsonl", ".1a/task-graph.jsonl"))]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Demo {
        #[arg(long, default_value_os_t = home_or_legacy("task-graph.jsonl", ".1a/task-graph.jsonl"))]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum MemoryCommand {
    Export {
        #[arg(long, default_value_os_t = home_or_legacy("memory-journal.jsonl", ".1a/memory-journal.jsonl"))]
        journal: PathBuf,
        #[arg(long, default_value_os_t = home_or_legacy("payloads", ".1a/payloads"))]
        payloads: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Verify {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        json: bool,
    },
    RotateKey {
        #[arg(long, default_value_os_t = home_or_legacy("payloads", ".1a/payloads"))]
        payloads: PathBuf,
        #[arg(long, default_value_os_t = home_or_legacy("keys/memory.key", ".1a/memory.key"))]
        key: PathBuf,
        #[arg(long, conflicts_with = "bot_id")]
        scope: Option<String>,
        #[arg(long, conflicts_with = "scope")]
        bot_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    RotateMasterKey {
        #[arg(long, default_value_os_t = home_or_legacy("payloads", ".1a/payloads"))]
        payloads: PathBuf,
        #[arg(long, default_value_os_t = home_or_legacy("keys/memory.key", ".1a/memory.key"))]
        key: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum KeyCommand {
    Escrow {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        recovery_key: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Restore {
        #[arg(long)]
        root: Option<PathBuf>,
        #[arg(long)]
        escrow: PathBuf,
        #[arg(long)]
        recovery_key: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum SurfaceCommand {
    Openapi,
    Validate {
        #[arg(long)]
        request: PathBuf,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1:8788")]
        addr: String,
        #[arg(long, default_value_os_t = home_or_legacy("task-graph.jsonl", ".1a/task-graph.jsonl"))]
        store: PathBuf,
        #[arg(long)]
        token: Option<String>,
        #[arg(long, default_value = "operator")]
        actor_id: String,
        #[arg(long = "scope", default_value = "*")]
        scopes: Vec<String>,
        #[arg(long)]
        authority: Option<PathBuf>,
        #[arg(long)]
        memory_journal: Option<PathBuf>,
        #[arg(long)]
        memory_payloads: Option<PathBuf>,
        #[arg(long)]
        memory_key: Option<PathBuf>,
        #[arg(long)]
        skill_journal: Option<PathBuf>,
        #[arg(long)]
        goal_journal: Option<PathBuf>,
        #[arg(long)]
        delegation_store: Option<PathBuf>,
        #[arg(long)]
        evolution_store: Option<PathBuf>,
        #[arg(long)]
        audit_task_graph: Option<PathBuf>,
        #[arg(long)]
        audit_memory: Option<PathBuf>,
        #[arg(long)]
        audit_skill: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ExperienceCommand {
    Propose {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        input: PathBuf,
    },
    Review {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        journal: Option<PathBuf>,
    },
    Verify {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        input: PathBuf,
    },
    Evaluate {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        suite: PathBuf,
        #[arg(long)]
        training_group: String,
        #[arg(long)]
        evaluation_group: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Approve {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        journal: Option<PathBuf>,
    },
    Reject {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        reason: String,
    },
    Activate {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        journal: Option<PathBuf>,
    },
    Degrade {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        reason: String,
    },
    Rollback {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        reason: String,
    },
    List {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
    },
    Audit {
        #[arg(long, default_value_os_t = home_or_legacy("experience", ".experience"))]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AnnotateCommand {
    Add {
        #[arg(long, default_value_os_t = home_or_legacy("annotations", ".annotations"))]
        store: PathBuf,
        #[arg(long)]
        case_id: String,
        #[arg(long)]
        annotator: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        evidence_source: Vec<String>,
        #[arg(long, default_value = "")]
        note: String,
    },
    Agreement {
        #[arg(long, default_value_os_t = home_or_legacy("annotations", ".annotations"))]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long, default_value_os_t = home_or_legacy("annotations", ".annotations"))]
        store: PathBuf,
    },
    Calibrate {
        #[arg(long, default_value_os_t = home_or_legacy("annotations", ".annotations"))]
        store: PathBuf,
        #[arg(long)]
        predictions: PathBuf,
        #[arg(long)]
        goldset: PathBuf,
        #[arg(long)]
        suite: PathBuf,
        #[arg(long)]
        approval: PathBuf,
        #[arg(long)]
        approval_key_env: String,
        #[arg(long)]
        approval_registry: PathBuf,
        #[arg(long)]
        registry_snapshot: Vec<PathBuf>,
        #[arg(long)]
        registry_key_env: String,
        #[arg(long, default_value_t = 1)]
        min_registry_version: u64,
        #[arg(long)]
        trust_anchor: PathBuf,
        #[arg(long, default_value_t = 600)]
        min_kappa_millis: i32,
        #[arg(long, default_value_t = 20)]
        min_samples: usize,
        #[arg(long)]
        json: bool,
    },
    CreateGoldset {
        #[arg(long)]
        suite: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        version: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        reviewer: Vec<String>,
        #[arg(long)]
        redaction_policy: String,
        #[arg(long, default_value_t = 25)]
        batch_size: usize,
    },
    VerifyGoldset {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        suite: PathBuf,
    },
}

#[derive(Subcommand)]
enum ApprovalCommand {
    Sign {
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        artifact_kind: String,
        #[arg(long)]
        approver: String,
        #[arg(long)]
        purpose: String,
        #[arg(long)]
        key_id: String,
        #[arg(long)]
        key_env: String,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    KeyAdd {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        key_id: String,
    },
    KeyRetire {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        key_id: String,
        #[arg(long)]
        reason: String,
    },
    KeyRevoke {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        key_id: String,
        #[arg(long)]
        reason: String,
    },
    Snapshot {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        approver: String,
        #[arg(long)]
        key_id: String,
        #[arg(long)]
        key_env: String,
        #[arg(long)]
        previous: Option<PathBuf>,
    },
    VerifySnapshot {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        key_env: String,
        #[arg(long, default_value_t = 1)]
        min_version: u64,
    },
    VerifyChain {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        snapshot: Vec<PathBuf>,
        #[arg(long)]
        key_env: String,
        #[arg(long, default_value_t = 1)]
        min_version: u64,
        #[arg(long)]
        anchor: Option<PathBuf>,
    },
    Anchor {
        #[arg(long)]
        snapshot: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        operator: String,
        #[arg(long)]
        key_id: Vec<String>,
    },
    Verify {
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        envelope: PathBuf,
        #[arg(long)]
        key_env: String,
        #[arg(long)]
        registry: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ProvenanceCommand {
    Create {
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        key_env: Option<String>,
        #[arg(long)]
        approver: Option<String>,
        #[arg(long)]
        approval_output: Option<PathBuf>,
    },
    Verify {
        #[arg(long)]
        provenance: PathBuf,
        #[arg(long)]
        artifact: PathBuf,
    },
    Rebuild {
        #[arg(long)]
        artifact: PathBuf,
        #[arg(long)]
        target: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ReleaseCommand {
    Manifest {
        #[arg(long)]
        package_root: PathBuf,
        #[arg(long)]
        linux_archive: PathBuf,
        #[arg(long)]
        windows_archive: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        allow_dirty: bool,
    },
    Verify {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        approval: Option<PathBuf>,
        #[arg(long)]
        key_env: Option<String>,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        production: bool,
        #[arg(long)]
        registry_key_env: Option<String>,
        #[arg(long)]
        registry_snapshot: Vec<PathBuf>,
        #[arg(long)]
        trust_anchor: Option<PathBuf>,
        #[arg(long, default_value_t = 1)]
        min_registry_version: u64,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

fn run_task_graph_demo(store: &std::path::Path, json: bool) -> Result<()> {
    use first_principles_agent::{
        ApprovalDecision, MandateStrength, ReportStatus, SideEffectDraft, SideEffectStatus,
        TaskGraphService, TicketDraft, Trigger, TriggerSource,
    };

    let mut service = TaskGraphService::open(store)
        .with_context(|| format!("cannot open {}", store.display()))?;
    if !service.snapshot().threads.is_empty() {
        bail!(
            "task graph store {} already contains data; use a fresh path",
            store.display()
        );
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let (thread_id, ticket_id, report_id) = service
        .with_graph_mut(|graph| {
            let thread_id = graph.open_thread("A1 demo", now)?;
            let trigger_id = graph
                .ingest_trigger(
                    &thread_id,
                    Trigger {
                        trigger_id: format!("trigger-demo-{now}"),
                        source: TriggerSource::Web,
                        source_event_id: format!("event-demo-{now}"),
                        summary: "A1 acceptance demo".into(),
                        received_ms: now,
                    },
                    now + 1,
                )?
                .trigger_id;
            let eval_id = graph.begin_eval(&thread_id, &trigger_id, now + 2)?;
            graph.decide_eval(
                &eval_id,
                first_principles_agent::Decision::Accept,
                "accepted",
                now + 3,
            )?;
            let ticket_id = graph.create_ticket_from_eval(
                &eval_id,
                TicketDraft {
                    owner_id: "demo-owner".into(),
                    scope: "read demo input".into(),
                    strength: MandateStrength::ReadOnly,
                    policy_version: "policy-v1".into(),
                    requires_approval: true,
                },
                now + 4,
            )?;
            graph.approve_ticket(
                &ticket_id,
                "demo-owner",
                ApprovalDecision::AllowOnce,
                None,
                now + 5,
            )?;
            graph.assign_ticket(&ticket_id, "fake-worker", None, now + 6)?;
            graph.start_ticket(&ticket_id, "fake-worker", now + 7)?;
            graph.record_side_effect(
                &ticket_id,
                SideEffectDraft {
                    action: "write".into(),
                    target: "demo/result.txt".into(),
                    idempotency_key: format!("demo-effect-{now}"),
                    reversible: true,
                    compensation: Some("remove demo/result.txt".into()),
                    status: SideEffectStatus::Applied,
                },
                now + 8,
            )?;
            let report_id = graph.report_ticket(
                &ticket_id,
                "fake-worker",
                ReportStatus::Ready,
                "demo artifact ready",
                Some("sha256:demo".into()),
                now + 9,
            )?;
            Ok((thread_id, ticket_id, report_id))
        })
        .map_err(anyhow::Error::msg)?;
    let snapshot = service.snapshot();
    if json {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
    } else {
        println!(
            "A1 demo: thread={thread_id} ticket={ticket_id} report={report_id} tickets={} reports={}",
            snapshot.tickets.len(),
            snapshot.reports.len()
        );
    }
    drop(service);
    let reopened = TaskGraphService::open(store)
        .with_context(|| format!("cannot reopen {}", store.display()))?;
    if reopened.snapshot().tickets.is_empty() {
        bail!("task graph demo did not survive replay");
    }
    if json {
        eprintln!("replay: ok ({})", reopened.store_path().display());
    } else {
        println!("replay: ok ({})", reopened.store_path().display());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Run {
            task,
            input,
            config,
            provider,
            model,
            base_url,
            api_key,
            allow_network,
            read_root,
            journal_dir,
            experience_dir,
            experience_scope,
            max_steps,
            max_tool_calls,
            max_provider_attempts,
            provider_retry_backoff_ms,
            timeout_seconds,
            max_context_chars,
            max_provider_p95_latency_ms,
            max_unit_cost_micros_per_k,
            evidence_max_age_ms,
            route_objective,
            authority,
            actor_id,
            authority_scope,
            provider_health,
            alert_log,
            alert_webhook,
            json,
        } => {
            let mut request = load_request(task, input)?;
            let task_latency_budget = request.budget.max_provider_p95_latency_ms;
            let task_unit_cost_budget = request.budget.max_unit_cost_micros_per_k;
            let effective_latency_budget = max_provider_p95_latency_ms.or(task_latency_budget);
            let effective_unit_cost_budget = max_unit_cost_micros_per_k.or(task_unit_cost_budget);
            let latency_budget_source = if max_provider_p95_latency_ms.is_some() {
                PolicyValueSource::CliOverride
            } else if task_latency_budget.is_some() {
                PolicyValueSource::TaskRequest
            } else {
                PolicyValueSource::Unset
            };
            let unit_cost_budget_source = if max_unit_cost_micros_per_k.is_some() {
                PolicyValueSource::CliOverride
            } else if task_unit_cost_budget.is_some() {
                PolicyValueSource::TaskRequest
            } else {
                PolicyValueSource::Unset
            };
            request.budget = RunBudget {
                max_steps,
                max_tool_calls,
                max_provider_attempts,
                provider_retry_backoff_ms,
                timeout_ms: timeout_seconds.saturating_mul(1_000),
                max_context_chars,
                auto_compact_percent: 80,
                max_provider_p95_latency_ms: effective_latency_budget,
                max_unit_cost_micros_per_k: effective_unit_cost_budget,
            };
            request.permissions.allow_network = allow_network;
            if evidence_max_age_ms.is_some() {
                request.evidence_max_age_ms = evidence_max_age_ms;
            }
            if experience_scope.is_some() {
                request.experience_scope = experience_scope;
            }
            if !read_root.is_empty() {
                request.permissions.read_roots =
                    read_root.iter().map(canonical_or_original).collect();
            }
            if request.permissions.read_roots.is_empty() {
                request.permissions.read_roots =
                    vec![std::env::current_dir().context("cannot determine current directory")?];
            }

            let timeout = Duration::from_secs(timeout_seconds.max(1));
            let (candidates, tools) = if let Some(config_path) = config {
                let config = AgentConfig::load(&config_path)?;
                (config.build_providers(timeout)?, default_registry())
            } else {
                build_runtime_parts(&provider, model, base_url, api_key, allow_network, timeout)?
            };
            let (objective, objective_source) =
                resolve_route_objective(route_objective.as_deref())?;
            let gate = if let Some(path) = authority {
                let bytes = std::fs::read(&path)
                    .with_context(|| format!("cannot read {}", path.display()))?;
                let config: first_principles_agent::AuthorityConfig =
                    serde_json::from_slice(&bytes).context("invalid authority config")?;
                let mut registry = first_principles_agent::AuthorityRegistry::default();
                registry.apply_config(&config)?;
                Some(first_principles_agent::ExecutionGate::new(registry, &tools))
            } else {
                None
            };
            let health = if provider_health || alert_log.is_some() || alert_webhook.is_some() {
                Some(std::sync::Arc::new(std::sync::Mutex::new(
                    first_principles_agent::ProviderHealth::new(
                        first_principles_agent::ProviderHealthPolicy::default(),
                    )
                    .map_err(anyhow::Error::msg)?,
                )))
            } else {
                None
            };
            let alerts = match alert_log {
                Some(path) => Some(std::sync::Arc::new(std::sync::Mutex::new(
                    first_principles_agent::AlertLog::open(path)?,
                ))),
                None => None,
            };
            let mut runtime = AgentRuntime::new(candidates, tools, journal_dir)
                .with_route_objective(objective)
                .with_route_policy_sources(RoutePolicySources {
                    objective: objective_source,
                    latency_budget: latency_budget_source,
                    unit_cost_budget: unit_cost_budget_source,
                });
            if let Some(gate) = gate {
                runtime = runtime.with_execution_gate(
                    gate,
                    actor_id,
                    authority_scope,
                    first_principles_agent::MandateStrength::ReadOnly,
                    now_unix_ms(),
                );
            }
            if let Some(health) = health {
                runtime = runtime.with_provider_health(health);
            }
            if let Some(alerts) = alerts {
                runtime = runtime.with_alert_log(alerts);
            }
            if let Some(url) = alert_webhook {
                runtime = runtime
                    .with_alert_webhook(Arc::new(first_principles_agent::AlertWebhook::new(url)?));
            }
            if let Some(experience_dir) = experience_dir {
                runtime = runtime.with_experience_dir(experience_dir);
            }
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_cancellation.cancel();
                }
            });
            let result = runtime.run(request, cancellation).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{}", render_answer(&result));
                print_route_trace(&result.journal_path)?;
                println!("\n运行日志：{}", result.journal_path.display());
            }
        }
        Command::Tui {
            config,
            provider,
            model,
            base_url,
            api_key,
            allow_network,
            allow_write,
            read_root,
            journal_dir,
            session,
            experience_dir,
            experience_scope,
            max_steps,
            max_tool_calls,
            max_provider_attempts,
            master_agents,
            workshop_prefix,
            timeout_seconds,
            max_context_chars,
            route_objective,
        } => {
            run_tui(
                config,
                provider,
                model,
                base_url,
                api_key,
                allow_network,
                allow_write,
                read_root,
                journal_dir,
                session,
                experience_dir,
                experience_scope,
                max_steps,
                max_tool_calls,
                max_provider_attempts,
                master_agents,
                workshop_prefix,
                timeout_seconds,
                max_context_chars,
                route_objective,
            )
            .await?;
        }
        Command::Master { command } => match command {
            MasterCommand::Validate { plan, json } => {
                let proposal: PlanProposal = serde_json::from_slice(
                    &std::fs::read(&plan)
                        .with_context(|| format!("cannot read {}", plan.display()))?,
                )
                .with_context(|| format!("invalid plan {}", plan.display()))?;
                proposal.validate()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&proposal)?);
                } else {
                    println!("{}", proposal.to_markdown());
                }
            }
            MasterCommand::ProductionTaskList {
                plan,
                state_root,
                json,
            } => {
                let production: ProductionPlan = read_json_file(&plan, "production-line plan")?;
                production.validate()?;
                let snapshot = production
                    .write_task_list(&state_root)
                    .map_err(anyhow::Error::msg)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&snapshot)?);
                } else {
                    println!(
                        "任务清单：{}\n计划 SHA-256：{}\n清单 SHA-256：{}",
                        snapshot.path.display(),
                        snapshot.plan_sha256,
                        snapshot.markdown_sha256
                    );
                }
            }
            MasterCommand::Schedule {
                input,
                global_active,
                per_conversation,
                json,
            } => {
                let conversations: Vec<ConversationQueue> =
                    read_json_file(&input, "scheduler conversations")?;
                let decision = schedule_ready(
                    &conversations,
                    SchedulerLimits {
                        global_active,
                        per_conversation,
                    },
                )
                .map_err(anyhow::Error::msg)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&decision)?);
                } else {
                    println!("Starts:");
                    for item in decision.starts {
                        println!("  {} · {}", item.stable_id, item.task_id);
                    }
                    println!("Waiting:");
                    for item in decision.waiting {
                        println!("  {} · {:?}", item.stable_id, item.reason);
                    }
                }
            }
            MasterCommand::Assign { plan, agents, json } => {
                let proposal: PlanProposal = serde_json::from_slice(
                    &std::fs::read(&plan)
                        .with_context(|| format!("cannot read {}", plan.display()))?,
                )
                .with_context(|| format!("invalid plan {}", plan.display()))?;
                let agents: Vec<AgentProfile> = serde_json::from_slice(
                    &std::fs::read(&agents)
                        .with_context(|| format!("cannot read {}", agents.display()))?,
                )
                .with_context(|| format!("invalid Agent profiles {}", agents.display()))?;
                let assignment = proposal.assign(&agents)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&assignment)?);
                } else {
                    for item in assignment.assignments {
                        println!(
                            "{} -> {} ({})",
                            item.agent_id, item.runtime, item.selection_reason
                        );
                    }
                }
            }
            MasterCommand::Triage {
                task,
                risk,
                write,
                external_action,
                requested_items,
                json,
            } => {
                let signals = TriageSignals {
                    requested_items,
                    requires_write: write,
                    requires_external_action: external_action,
                    risk: parse_risk(&risk)?,
                };
                let decision = triage_goal(&task, &signals);
                if json {
                    println!("{}", serde_json::to_string_pretty(&decision)?);
                } else {
                    println!("分诊：{:?}", decision.route);
                    println!("理由：{}", decision.reason);
                    println!("需要计划：{}", yes_no(decision.requires_plan));
                    println!("需要审批：{}", yes_no(decision.requires_user_approval));
                }
            }
            MasterCommand::CapabilityProbe {
                manifest,
                runtime,
                output,
                json,
            } => {
                let manifest: CapabilityProbeManifest =
                    read_json_file(&manifest, "capability probe manifest")?;
                let manifest = filter_capability_manifest_by_runtime(manifest, &runtime)
                    .map_err(anyhow::Error::msg)?;
                let report = run_capability_suite(&manifest).map_err(anyhow::Error::msg)?;
                if let Some(output) = output.as_ref() {
                    write_capability_report(&report, output).map_err(anyhow::Error::msg)?;
                }
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("{}", render_capability_report(&report));
                    if let Some(output) = output {
                        println!("report: {}", output.display());
                    }
                }
            }
            MasterCommand::Plan {
                task,
                config,
                provider,
                model,
                base_url,
                api_key,
                allow_network,
                workspace,
                risk,
                write,
                external_action,
                requested_items,
                agents,
                workshop_prefix,
                output,
                planner_journal_dir,
                json,
            } => {
                run_master_plan(
                    task,
                    config,
                    provider,
                    model,
                    base_url,
                    api_key,
                    allow_network,
                    workspace,
                    risk,
                    write,
                    external_action,
                    requested_items,
                    agents,
                    workshop_prefix,
                    output,
                    planner_journal_dir,
                    json,
                )
                .await?;
            }
            MasterCommand::Init {
                plan,
                agents,
                workshop_prefix,
                trace_id,
                state_root,
                json,
            } => {
                let mut profile = load_agent_profiles(&agents)?;
                if let Some(prefix) = workshop_prefix {
                    profile = WorkshopCliAdapter::new(prefix, Duration::from_secs(30), true)?
                        .merge_profiles(&profile)?;
                }
                let proposal: PlanProposal = read_json_file(&plan, "master plan")?;
                let assignment = proposal.assign(&profile)?;
                let trace_id = trace_id.unwrap_or_else(|| proposal.plan_id.clone());
                let state =
                    MasterState::initialize(trace_id.clone(), proposal, assignment, now_unix_ms())?;
                let store = MasterStore::new(&state_root, &trace_id)?;
                store.initialize(state.clone(), now_unix_ms())?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Status {
                trace_id,
                state_root,
                json,
            } => {
                let state = MasterStore::new(&state_root, &trace_id)?.load()?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Approve {
                trace_id,
                state_root,
                actor,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.approve(actor, now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::Approved,
                    None,
                    "user",
                    "plan approved for controlled execution",
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Reject {
                trace_id,
                state_root,
                actor,
                reason,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.reject_plan(actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::PlanRejected,
                    None,
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Revise {
                trace_id,
                state_root,
                plan,
                agents,
                actor,
                reason,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                let proposal: PlanProposal = read_json_file(&plan, "revised plan")?;
                let profiles = load_agent_profiles(&agents)?;
                let assignment = proposal.assign(&profiles)?;
                state.revise_plan(proposal, assignment, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::PlanRevised,
                    None,
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Run {
                trace_id,
                state_root,
                runtime_profiles,
                workshop_prefix,
                no_gui,
                detach,
                dry_run,
                json,
            } => {
                if detach {
                    run_master_start(
                        trace_id,
                        state_root,
                        runtime_profiles,
                        workshop_prefix,
                        no_gui,
                        dry_run,
                        json,
                    )?;
                } else {
                    run_master_ready(
                        trace_id,
                        state_root,
                        runtime_profiles,
                        workshop_prefix,
                        no_gui,
                        dry_run,
                        json,
                    )?;
                }
            }
            MasterCommand::Poll {
                trace_id,
                state_root,
                workshop_prefix,
                json,
            } => {
                run_master_poll(trace_id, state_root, workshop_prefix, json)?;
            }
            MasterCommand::Verify {
                trace_id,
                state_root,
                input,
                json,
            } => {
                run_master_verify(trace_id, state_root, input, json)?;
            }
            MasterCommand::Retry {
                trace_id,
                state_root,
                task,
                actor,
                reason,
                workshop_prefix,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                cancel_active_session_if_any(&state, &task, &workshop_prefix)?;
                state.request_retry(&task, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::RetryRequested,
                    Some(task),
                    "master",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::ReplanRequest {
                trace_id,
                state_root,
                task,
                actor,
                reason,
                workshop_prefix,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                cancel_active_session_if_any(&state, &task, &workshop_prefix)?;
                let request = state.request_replan(&task, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::ReplanRequested,
                    Some(task),
                    "master",
                    reason,
                    now_unix_ms(),
                )?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "request": request,
                            "state": state,
                        }))?
                    );
                } else {
                    println!("重规划范围：{:?}", request.affected_tasks);
                    println!("保留已验证任务：{:?}", request.preserved_tasks);
                }
            }
            MasterCommand::ReplanApply {
                trace_id,
                state_root,
                plan,
                agents,
                actor,
                reason,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                let proposal: PlanProposal = read_json_file(&plan, "replacement plan")?;
                let profiles = load_agent_profiles(&agents)?;
                let assignment = proposal.assign(&profiles)?;
                state.apply_replan(proposal, assignment, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::ReplanApplied,
                    None,
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Pause {
                trace_id,
                state_root,
                actor,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.set_paused(true, actor, now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::Paused,
                    None,
                    "user",
                    "run paused",
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Resume {
                trace_id,
                state_root,
                actor,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.set_paused(false, actor, now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::Resumed,
                    None,
                    "user",
                    "run resumed",
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Takeover {
                trace_id,
                state_root,
                task,
                actor,
                reason,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.take_over(&task, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::TakenOver,
                    Some(task),
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Accept {
                trace_id,
                state_root,
                actor,
                reason,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.accept_goal(actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::GoalAccepted,
                    None,
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Return {
                trace_id,
                state_root,
                task,
                actor,
                reason,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                state.end_takeover(&task, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::TakeoverEnded,
                    Some(task),
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Reclaim {
                trace_id,
                state_root,
                task,
                actor,
                reason,
                workshop_prefix,
                force,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                if !force {
                    terminate_expired_session_if_any(&state, &task, &workshop_prefix)?;
                }
                state.reclaim_expired_lease(&task, actor, reason.clone(), true, now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::LeaseReclaimed,
                    Some(task),
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Cancel {
                trace_id,
                state_root,
                task,
                actor,
                reason,
                workshop_prefix,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let mut state = store.load()?;
                let session_id = state
                    .tasks
                    .get(&task)
                    .and_then(|task| task.execution_target.as_ref())
                    .and_then(|target| target.session_id.clone());
                if let Some(session_id) = session_id {
                    WorkshopCliAdapter::new(workshop_prefix, Duration::from_secs(30), true)?
                        .cancel_session(&session_id)?;
                }
                state.cancel_task(&task, actor, reason.clone(), now_unix_ms())?;
                let state = store.append(
                    state,
                    MasterJournalEventKind::Cancelled,
                    Some(task),
                    "user",
                    reason,
                    now_unix_ms(),
                )?;
                print_master_state(&state, json)?;
            }
            MasterCommand::Audit {
                trace_id,
                state_root,
                json,
            } => {
                let store = MasterStore::new(&state_root, &trace_id)?;
                let (state, audit) =
                    replay_master_journal(&store.trace_dir().join("events.jsonl"))?;
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "audit": audit,
                            "state": state,
                        }))?
                    );
                } else {
                    println!(
                        "审计{}：{} 个事件",
                        if audit.ok { "通过" } else { "失败" },
                        audit.event_count
                    );
                    for issue in &audit.issues {
                        println!("- {issue}");
                    }
                }
                if !audit.ok {
                    bail!("master journal audit failed");
                }
            }
            MasterCommand::Agents { prefix, json } => {
                let agents = WorkshopCliAdapter::new(prefix, Duration::from_secs(30), true)?
                    .list_agents()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&agents)?);
                } else {
                    for agent in agents {
                        println!(
                            "{} · {} · {} · {}{}",
                            agent.agent_id,
                            agent.provider,
                            agent.status,
                            if agent.stale { "stale" } else { "live" },
                            agent
                                .sessions
                                .first()
                                .map(|session| format!(" · {session}"))
                                .unwrap_or_default()
                        );
                    }
                }
            }
            MasterCommand::Jobs { prefix, json } => {
                let jobs =
                    WorkshopCliAdapter::new(prefix, Duration::from_secs(30), true)?.list_jobs()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&jobs)?);
                } else {
                    for job in jobs {
                        println!("{} · {} · {} tasks", job.job_id, job.status, job.task_count);
                    }
                }
            }
            MasterCommand::Events {
                prefix,
                since_ms,
                limit,
                json,
            } => {
                let events = WorkshopCliAdapter::new(prefix, Duration::from_secs(30), true)?
                    .bounded_events(since_ms, limit)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&events)?);
                } else {
                    for event in events {
                        println!("{}", serde_json::to_string(&event)?);
                    }
                }
            }
        },
        Command::Audit { journal, json } => {
            let events = read_events(&journal)
                .with_context(|| format!("cannot read {}", journal.display()))?;
            let audit = audit_events(&events);
            if json {
                println!("{}", serde_json::to_string_pretty(&audit)?);
            } else if audit.ok {
                println!(
                    "审计通过：{} 个事件，run_id={}，终止原因={:?}，答案契约={:?}",
                    audit.event_count,
                    audit.run_id.map(|id| id.to_string()).unwrap_or_default(),
                    audit.terminal_reason,
                    audit.answer_schema_version
                );
            } else {
                println!("审计失败：");
                for issue in audit.issues {
                    println!("- {issue}");
                }
            }
            if !audit.ok {
                bail!("journal audit failed");
            }
        }
        Command::AuditStore {
            kind,
            path,
            thread,
            ticket,
            memory,
            scope,
            skill,
            version,
            limit,
            export,
            json,
        } => {
            let kind: first_principles_agent::AuditKind =
                kind.parse().map_err(anyhow::Error::msg)?;
            let query = first_principles_agent::AuditQuery {
                thread_id: thread,
                ticket_id: ticket,
                memory_id: memory,
                scope,
                skill,
                version,
                limit,
            };
            let result = first_principles_agent::audit_export(
                kind,
                &path,
                &query,
                export.as_deref(),
                now_unix_ms(),
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "kind: {}\npath: {}\nevents: {}\nmatched: {}\nverified: {}\nhead: {}\nexported: {}",
                    result.receipt.kind.as_str(),
                    result.receipt.path,
                    result.receipt.events,
                    result.matched,
                    result.receipt.verified,
                    result.receipt.head_sha256.as_deref().unwrap_or("-"),
                    result.exported.as_deref().unwrap_or("-")
                );
            }
        }
        Command::TaskGraph { command } => match command {
            TaskGraphCommand::Snapshot { store, json } => {
                let service = first_principles_agent::TaskGraphService::open_read_only(&store)
                    .with_context(|| format!("cannot open {}", store.display()))?;
                let snapshot = service.snapshot();
                if json {
                    println!("{}", serde_json::to_string_pretty(&snapshot)?);
                } else {
                    println!(
                        "store: {}\nthreads={} evals={} tickets={} reports={} side_effects={}",
                        service.store_path().display(),
                        snapshot.threads.len(),
                        snapshot.evals.len(),
                        snapshot.tickets.len(),
                        snapshot.reports.len(),
                        snapshot.side_effects.len()
                    );
                }
            }
            TaskGraphCommand::Demo { store, json } => {
                run_task_graph_demo(&store, json)?;
            }
        },
        Command::Memory { command } => match command {
            MemoryCommand::Export {
                journal,
                payloads,
                out,
                json,
            } => {
                let manifest = first_principles_agent::export_memory_backup(
                    &journal,
                    &payloads,
                    &out,
                    now_unix_ms(),
                )?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&manifest)?);
                } else {
                    println!(
                        "backup: {}\nfiles={}\nkey_included=false\nhard_forget_covers_backup=false",
                        out.display(),
                        manifest.files.len()
                    );
                }
            }
            MemoryCommand::Verify { backup, json } => {
                let manifest = first_principles_agent::verify_memory_backup(&backup)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&manifest)?);
                } else {
                    println!(
                        "verified: {}\nfiles={}\nkey_included=false\nhard_forget_covers_backup=false",
                        backup.display(),
                        manifest.files.len()
                    );
                }
            }
            MemoryCommand::RotateKey {
                payloads,
                key,
                scope,
                bot_id,
                json,
            } => {
                let scope = match (scope, bot_id) {
                    (Some(scope), None) => scope,
                    (None, Some(bot_id)) => {
                        first_principles_agent::MemoryScope::bot(bot_id).scope_id()
                    }
                    _ => anyhow::bail!("provide exactly one of --scope or --bot-id"),
                };
                let store = first_principles_agent::MemoryPayloadStore::open(&payloads, &key)
                    .map_err(anyhow::Error::msg)?;
                let rotation = store.rotate_scope_key(&scope).map_err(anyhow::Error::msg)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&rotation)?);
                } else {
                    println!(
                        "rotated scope {}\nnew_version={}\npayloads={}\nretired_versions={:?}\nlegacy_migrated={}",
                        rotation.scope_id,
                        rotation.new_version,
                        rotation.rotated,
                        rotation.retired_versions,
                        rotation.legacy_migrated
                    );
                }
            }
            MemoryCommand::RotateMasterKey {
                payloads,
                key,
                json,
            } => {
                let store = first_principles_agent::MemoryPayloadStore::open(&payloads, &key)
                    .map_err(anyhow::Error::msg)?;
                let rotation = store.rotate_master_key().map_err(anyhow::Error::msg)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&rotation)?);
                } else {
                    println!(
                        "rotated master\nnew_master_id={}\nrewrapped_keks={}\nretired_master_ids={:?}",
                        rotation.new_master_id,
                        rotation.rewrapped_keks,
                        rotation.retired_master_ids
                    );
                }
            }
        },
        Command::Surface { command } => match command {
            SurfaceCommand::Openapi => {
                let document = first_principles_agent::openapi_document();
                first_principles_agent::validate_openapi(&document).map_err(anyhow::Error::msg)?;
                println!("{}", serde_json::to_string_pretty(&document)?);
            }
            SurfaceCommand::Validate { request } => {
                let bytes = std::fs::read(&request)
                    .with_context(|| format!("cannot read {}", request.display()))?;
                let request: first_principles_agent::SurfaceRequest =
                    serde_json::from_slice(&bytes).context("invalid surface request")?;
                request.validate().map_err(anyhow::Error::msg)?;
                println!(
                    "ok: request_id={} trace_id={} fingerprint={}",
                    request.request_id,
                    request.trace_id,
                    request.fingerprint().map_err(anyhow::Error::msg)?
                );
            }
            SurfaceCommand::Serve {
                addr,
                store,
                token,
                actor_id,
                scopes,
                authority,
                memory_journal,
                memory_payloads,
                memory_key,
                skill_journal,
                goal_journal,
                delegation_store,
                evolution_store,
                audit_task_graph,
                audit_memory,
                audit_skill,
            } => {
                let token = token
                    .or_else(|| std::env::var("SURFACE_TOKEN").ok())
                    .filter(|token| !token.is_empty())
                    .context("surface serve requires --token or SURFACE_TOKEN")?;
                let mut service = first_principles_agent::SurfaceService::open(&store)
                    .with_context(|| format!("cannot open {}", store.display()))?;
                if let Some(path) = authority {
                    let bytes = std::fs::read(&path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    let config: first_principles_agent::AuthorityConfig =
                        serde_json::from_slice(&bytes).context("invalid authority config")?;
                    service
                        .graph_mut()
                        .authority_mut()
                        .apply_config(&config)
                        .map_err(anyhow::Error::msg)?;
                }
                let mut resources = first_principles_agent::SurfaceResources::default();
                if let Some(path) = goal_journal {
                    resources.goals = Some(
                        first_principles_agent::GoalStore::open(path)
                            .map_err(anyhow::Error::msg)?,
                    );
                }
                if let Some(path) = delegation_store {
                    resources.delegation =
                        Some(first_principles_agent::DelegationStore::open(path)?);
                }
                if let Some(path) = evolution_store {
                    resources.evolution = Some(first_principles_agent::EvolutionStore::open(path)?);
                }
                match (memory_journal, memory_payloads, memory_key) {
                    (Some(journal), Some(payloads), Some(key)) => {
                        resources.memory = Some(
                            first_principles_agent::MemoryService::open(journal, payloads, key)
                                .map_err(anyhow::Error::msg)?,
                        );
                    }
                    (None, None, None) => {}
                    _ => bail!(
                        "surface serve requires --memory-journal, --memory-payloads and --memory-key together"
                    ),
                }
                if let Some(path) = skill_journal {
                    resources.skills = Some(
                        first_principles_agent::SkillService::open(path)
                            .map_err(anyhow::Error::msg)?,
                    );
                }
                resources.audit_paths.insert(
                    first_principles_agent::AuditKind::TaskGraph,
                    audit_task_graph.unwrap_or_else(|| store.clone()),
                );
                if let Some(path) = audit_memory {
                    resources
                        .audit_paths
                        .insert(first_principles_agent::AuditKind::Memory, path);
                }
                if let Some(path) = audit_skill {
                    resources
                        .audit_paths
                        .insert(first_principles_agent::AuditKind::Skill, path);
                }
                service = service.with_resources(resources);
                let service = std::sync::Arc::new(std::sync::Mutex::new(service));
                let events = std::sync::Arc::new(std::sync::Mutex::new(
                    first_principles_agent::SurfaceEventBus::default(),
                ));
                let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                first_principles_agent::serve_surface_http(
                    &addr,
                    token,
                    first_principles_agent::AuthContext { actor_id, scopes },
                    service,
                    events,
                    shutdown,
                )?;
            }
        },
        Command::Replay { journal } => {
            let events = read_events(&journal)
                .with_context(|| format!("cannot read {}", journal.display()))?;
            let state = replay_events(&events).map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        Command::Eval {
            suite,
            config,
            allow_network,
            timeout_seconds,
            json,
        } => {
            let suite = load_suite(&suite)?;
            let providers = if let Some(config_path) = config {
                let config = AgentConfig::load(&config_path)?;
                config.build_providers(Duration::from_secs(timeout_seconds.max(1)))?
            } else {
                vec![ProviderCandidate {
                    provider: Arc::new(DeterministicProvider::new()),
                    manifest: local_manifest(),
                }]
            };
            let report = run_suite_with_providers(suite, providers, allow_network).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "评测：{}（{:?}），通过 {}/{}，硬门禁失败 {}，rubric {}/{}",
                    report.suite,
                    report.kind,
                    report.passed,
                    report.total,
                    report.hard_gate_failures,
                    report.rubric_passed,
                    report.rubric_checks
                );
                println!(
                    "decision completeness: {}/{} cases ({} bps), floor={} bps, policy=v{} {}",
                    report.decision_completeness.ok_cases,
                    report.decision_completeness.cases,
                    report.decision_completeness.completion_rate_bps,
                    report.min_decision_completeness_bps,
                    report.decision_completeness_policy.version,
                    report.decision_completeness_policy.floor_source.as_str()
                );
                for case in &report.cases {
                    println!(
                        "- {}: {}",
                        case.id,
                        if case.passed { "PASS" } else { "FAIL" }
                    );
                    println!(
                        "  decision_completeness={} conditional={} bound_flips={} unbound_flips={} high_unknown_without_reduction={} completed_with_high_assumption={}",
                        if case.decision_completeness.ok {
                            "ok"
                        } else {
                            "failed"
                        },
                        case.decision_completeness.conditional_answers,
                        case.decision_completeness.bound_flip_conditions,
                        case.decision_completeness.unbound_flip_conditions,
                        case.decision_completeness
                            .high_impact_unknowns_without_reduction,
                        case.decision_completeness
                            .completed_with_high_impact_assumptions
                    );
                    for issue in &case.issues {
                        println!("  - {issue}");
                    }
                }
            }
            if report.failed > 0 {
                bail!("evaluation failed");
            }
            if !report.decision_completeness_floor_passed {
                bail!(
                    "evaluation decision-completeness floor failed: {} < {} bps",
                    report.decision_completeness.completion_rate_bps,
                    report.min_decision_completeness_bps
                );
            }
        }
        Command::EvalInspect {
            report,
            summary,
            require_content_binding,
            require_authenticity,
            approval,
            approval_key_env,
            approval_registry,
            json,
        } => {
            let report_path = report;
            let proof_parts = [
                approval.is_some(),
                approval_key_env.is_some(),
                approval_registry.is_some(),
            ]
            .into_iter()
            .filter(|present| *present)
            .count();
            if proof_parts != 0 && proof_parts != 3 {
                bail!(
                    "approval, approval key environment and approval registry must be supplied together"
                );
            }
            let report = load_eval_report(&report_path)?;
            let authenticity_verified = match (approval, approval_key_env, approval_registry) {
                (None, None, None) => false,
                (Some(approval), Some(key_env), Some(registry)) => {
                    let envelope: ApprovalEnvelope = serde_json::from_str(
                        &std::fs::read_to_string(&approval)
                            .with_context(|| format!("cannot read {}", approval.display()))?,
                    )
                    .with_context(|| format!("invalid approval {}", approval.display()))?;
                    envelope.require_kind(ArtifactKind::EvaluationReport)?;
                    let key = std::env::var(&key_env)
                        .with_context(|| format!("missing approval key environment {key_env}"))?;
                    ApprovalKeyRegistry::load(registry)?.verify_approval(
                        &envelope,
                        &report_path,
                        key.as_bytes(),
                    )?;
                    true
                }
                _ => unreachable!("partial approval proof was rejected before report inspection"),
            };
            EvalReportTrustRequirements {
                require_content_binding,
                require_authenticity,
            }
            .validate(&report, authenticity_verified)?;
            if summary {
                let mut summary = EvalReportSummary::from(&report);
                if authenticity_verified {
                    summary.verification.signature_present = true;
                    summary.verification.authenticity = AuthenticityStatus::Verified;
                }
                summary.validate()?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                } else {
                    println!(
                        "{}: {}/{} passed, hard-gate failures={}, completeness={}/{} ({} bps), floor={} bps:{}, policy=v{} {}, verification=integrity:{},binding:{},signature:{},authenticity:{}",
                        summary.suite,
                        summary.passed,
                        summary.total,
                        summary.hard_gate_failures,
                        summary.decision_completeness_ok_cases,
                        summary.decision_completeness_cases,
                        summary.decision_completeness_rate_bps,
                        summary.min_decision_completeness_bps,
                        if summary.decision_completeness_floor_passed {
                            "passed"
                        } else {
                            "failed"
                        },
                        summary.decision_completeness_policy.version,
                        summary.decision_completeness_policy.floor_source.as_str(),
                        if summary.verification.integrity_checked {
                            "checked"
                        } else {
                            "unchecked"
                        },
                        summary.verification.content_binding.as_str(),
                        if summary.verification.signature_present {
                            "present"
                        } else {
                            "absent"
                        },
                        summary.verification.authenticity.as_str()
                    );
                }
            } else if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "评测报告：{}（{:?}），通过 {}/{}，硬门禁失败 {}，policy=v{} {}",
                    report.suite,
                    report.kind,
                    report.passed,
                    report.total,
                    report.hard_gate_failures,
                    report.decision_completeness_policy.version,
                    report.decision_completeness_policy.floor_source.as_str()
                );
            }
        }
        Command::Serve {
            config,
            provider,
            model,
            base_url,
            api_key,
            allow_network,
            journal_dir,
            timeout_seconds,
            max_concurrency,
            provider_health,
            alert_log,
            alert_webhook,
        } => {
            let timeout = Duration::from_secs(timeout_seconds.max(1));
            let (candidates, tools) = if let Some(config_path) = config {
                let config = AgentConfig::load(&config_path)?;
                (config.build_providers(timeout)?, default_registry())
            } else {
                build_runtime_parts(&provider, model, base_url, api_key, allow_network, timeout)?
            };
            if max_concurrency == 0 || max_concurrency > 32 {
                bail!("--max-concurrency must be between 1 and 32");
            }
            let mut runtime = AgentRuntime::new(candidates, tools, journal_dir);
            if provider_health || alert_log.is_some() || alert_webhook.is_some() {
                runtime = runtime.with_provider_health(Arc::new(std::sync::Mutex::new(
                    first_principles_agent::ProviderHealth::new(
                        first_principles_agent::ProviderHealthPolicy::default(),
                    )
                    .map_err(anyhow::Error::msg)?,
                )));
            }
            if let Some(path) = alert_log {
                runtime = runtime.with_alert_log(Arc::new(std::sync::Mutex::new(
                    first_principles_agent::AlertLog::open(path)?,
                )));
            }
            if let Some(url) = alert_webhook {
                runtime = runtime
                    .with_alert_webhook(Arc::new(first_principles_agent::AlertWebhook::new(url)?));
            }
            let runtime = Arc::new(runtime);
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_cancellation.cancel();
                }
            });
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            let stdout = Arc::new(tokio::sync::Mutex::new(tokio::io::stdout()));
            let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
            let registry = Arc::new(tokio::sync::Mutex::new(HostRegistry::default()));
            let mut tasks = tokio::task::JoinSet::new();
            while let Some(line) = lines.next_line().await? {
                if line.trim().is_empty() {
                    continue;
                }
                match HostCommand::parse(&line) {
                    Ok(HostCommand::Cancel(cancel)) => {
                        let response = if registry.lock().await.cancel(&cancel.request_id) {
                            HostResponse::cancelled(cancel.request_id)
                        } else {
                            HostResponse::control_failure(
                                cancel.request_id,
                                HostError {
                                    code: "request_not_found".into(),
                                    message: "no queued or active request has that id".into(),
                                },
                            )
                        };
                        write_host_response(&stdout, &response).await?;
                    }
                    Ok(HostCommand::Status(status)) => {
                        let snapshot = registry
                            .lock()
                            .await
                            .snapshot(status.target_request_id.as_deref());
                        write_host_response(
                            &stdout,
                            &HostResponse::status(status.request_id, snapshot),
                        )
                        .await?;
                    }
                    Ok(HostCommand::Run(request)) => {
                        let request = *request;
                        let permit = match semaphore.clone().try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                write_host_response(
                                    &stdout,
                                    &HostResponse::failure(
                                        request.request_id,
                                        HostError {
                                            code: "queue_full".into(),
                                            message: "maximum concurrent runs reached".into(),
                                        },
                                    ),
                                )
                                .await?;
                                continue;
                            }
                        };
                        let request_cancellation = cancellation.child_token();
                        if let Err(error) = registry
                            .lock()
                            .await
                            .reserve(request.request_id.clone(), request_cancellation.clone())
                        {
                            write_host_response(
                                &stdout,
                                &HostResponse::failure(request.request_id, error),
                            )
                            .await?;
                            continue;
                        }
                        let runtime = runtime.clone();
                        let stdout = stdout.clone();
                        let registry = registry.clone();
                        tasks.spawn(async move {
                            let _permit = permit;
                            let request_id = request.request_id.clone();
                            let (response, journal_path) = match runtime
                                .run_with_client_request_id(
                                    request.task,
                                    request_id.clone(),
                                    request_cancellation,
                                )
                                .await
                            {
                                Ok(result) => {
                                    let path = result.journal_path.clone();
                                    (
                                        HostResponse::success(request_id.clone(), result),
                                        Some(path),
                                    )
                                }
                                Err(RuntimeError::Cancelled { path }) => (
                                    HostResponse::failure(
                                        request_id.clone(),
                                        HostError {
                                            code: "cancelled".into(),
                                            message: "request was cancelled".into(),
                                        },
                                    ),
                                    Some(path),
                                ),
                                Err(error) => {
                                    let path = error.journal_path();
                                    let journal_path =
                                        (!path.as_os_str().is_empty()).then(|| path.to_path_buf());
                                    (
                                        HostResponse::failure(
                                            request_id.clone(),
                                            HostError {
                                                code: "run_failed".into(),
                                                message: error.to_string(),
                                            },
                                        ),
                                        journal_path,
                                    )
                                }
                            };
                            let state = if response.ok {
                                HostRequestState::Completed
                            } else if response
                                .error
                                .as_ref()
                                .is_some_and(|error| error.code == "cancelled")
                            {
                                HostRequestState::Cancelled
                            } else {
                                HostRequestState::Failed
                            };
                            registry
                                .lock()
                                .await
                                .finish(&request_id, state, journal_path);
                            let _ = write_host_response(&stdout, &response).await;
                        });
                    }
                    Err(error) => {
                        write_host_response(&stdout, &HostResponse::failure("<unknown>", error))
                            .await?;
                    }
                }
                if cancellation.is_cancelled() {
                    break;
                }
            }
            while tasks.join_next().await.is_some() {}
        }
        Command::Providers {
            config,
            check,
            probe,
            probe_tools,
            allow_network,
            probe_timeout_seconds,
            json,
        } => {
            if probe || probe_tools {
                if !allow_network {
                    bail!("provider probe requires --allow-network");
                }
                let config_path = config.context("provider probe requires --config")?;
                let config = AgentConfig::load(config_path)?;
                let candidates =
                    config.build_providers(Duration::from_secs(probe_timeout_seconds.max(1)))?;
                let cancellation = CancellationToken::new();
                if probe_tools {
                    let mut results = Vec::new();
                    for (provider_config, candidate) in
                        config.providers.iter().zip(candidates.iter())
                    {
                        if provider_config.adapter != ProviderAdapter::OpenAiCompatible {
                            continue;
                        }
                        results.push(
                            probe_provider_tools(
                                candidate.provider.as_ref(),
                                &candidate.manifest,
                                &cancellation,
                            )
                            .await,
                        );
                    }
                    if results.is_empty() {
                        bail!("provider tool probe found no OpenAI-compatible providers");
                    }
                    let report = ProviderToolProbeReport {
                        schema_version: 1,
                        checked: results.len(),
                        failed: results.iter().filter(|result| !result.ok).count(),
                        results,
                    };
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        for result in &report.results {
                            println!(
                                "{} | {} | ok={} | call={} observation={} final={} | latency={}ms | tokens={}/{} | error={:?}",
                                result.provider_id,
                                result.model,
                                result.ok,
                                result.tool_call_ok,
                                result.observation_ok,
                                result.final_answer_ok,
                                result.latency_ms,
                                result.input_tokens,
                                result.output_tokens,
                                result.error_code
                            );
                        }
                    }
                    if report.failed > 0 {
                        bail!(
                            "{} provider tool conformance probe(s) failed",
                            report.failed
                        );
                    }
                } else {
                    let mut results = Vec::new();
                    for (provider_config, candidate) in
                        config.providers.iter().zip(candidates.iter())
                    {
                        if provider_config.adapter != ProviderAdapter::OpenAiCompatible {
                            continue;
                        }
                        results.push(
                            probe_provider(
                                candidate.provider.as_ref(),
                                &candidate.manifest,
                                &cancellation,
                            )
                            .await,
                        );
                    }
                    if results.is_empty() {
                        bail!("provider probe found no OpenAI-compatible providers");
                    }
                    let report = ProviderProbeReport {
                        schema_version: 1,
                        checked: results.len(),
                        failed: results.iter().filter(|result| !result.ok).count(),
                        results,
                    };
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        for result in &report.results {
                            println!(
                                "{} | {} | ok={} | latency={}ms | tokens={}/{} | error={:?}",
                                result.provider_id,
                                result.model,
                                result.ok,
                                result.latency_ms,
                                result.input_tokens,
                                result.output_tokens,
                                result.error_code
                            );
                        }
                    }
                    if report.failed > 0 {
                        bail!("{} provider conformance probe(s) failed", report.failed);
                    }
                }
            } else {
                let value = if let Some(config_path) = config {
                    let config = AgentConfig::load(config_path)?;
                    if check {
                        let candidates = config.build_providers(Duration::from_secs(1))?;
                        serde_json::json!({
                            "ready": true,
                            "providers": candidates
                                .iter()
                                .map(|candidate| &candidate.manifest)
                                .collect::<Vec<_>>()
                        })
                    } else {
                        serde_json::to_value(config)?
                    }
                } else if check {
                    serde_json::json!({
                        "ready": true,
                        "providers": [local_manifest()]
                    })
                } else {
                    serde_json::json!({
                        "providers": [{
                            "provider_id": local_manifest().provider_id,
                            "model": local_manifest().model,
                            "privacy": "local",
                            "quality_score": 1,
                            "supports_tools": true,
                            "supports_structured_output": true
                        }]
                    })
                };
                if json {
                    println!("{}", serde_json::to_string_pretty(&value)?);
                } else {
                    println!("Provider configuration (secret values are never loaded):");
                    println!("{}", serde_json::to_string_pretty(&value)?);
                }
            }
        }
        Command::RoutePlan {
            task,
            input,
            config,
            allow_network,
            max_provider_p95_latency_ms,
            max_unit_cost_micros_per_k,
            route_objective,
            json,
        } => {
            let mut request = load_request(task, input)?;
            let latency_budget_source = if max_provider_p95_latency_ms.is_some() {
                PolicyValueSource::CliOverride
            } else if request.budget.max_provider_p95_latency_ms.is_some() {
                PolicyValueSource::TaskRequest
            } else {
                PolicyValueSource::Unset
            };
            let unit_cost_budget_source = if max_unit_cost_micros_per_k.is_some() {
                PolicyValueSource::CliOverride
            } else if request.budget.max_unit_cost_micros_per_k.is_some() {
                PolicyValueSource::TaskRequest
            } else {
                PolicyValueSource::Unset
            };
            if max_provider_p95_latency_ms.is_some() {
                request.budget.max_provider_p95_latency_ms = max_provider_p95_latency_ms;
            }
            if max_unit_cost_micros_per_k.is_some() {
                request.budget.max_unit_cost_micros_per_k = max_unit_cost_micros_per_k;
            }
            let config = AgentConfig::load(config)?;
            let manifests = config.manifests()?;
            let readiness = config.inspect_readiness()?;
            let mode = infer_mode(&request);
            let (objective, objective_source) =
                resolve_route_objective(route_objective.as_deref())?;
            let policy_sources = RoutePolicySources {
                objective: objective_source,
                latency_budget: latency_budget_source,
                unit_cost_budget: unit_cost_budget_source,
            };
            let plan = Router::new(manifests).plan(&RouteRequest {
                mode,
                risk: risk_for_mode(mode),
                requires_tools: true,
                requires_structured_output: true,
                allow_remote: allow_network,
                objective,
                max_provider_p95_latency_ms: request.budget.max_provider_p95_latency_ms,
                max_unit_cost_micros_per_k: request.budget.max_unit_cost_micros_per_k,
            });
            let value = serde_json::json!({
                "schema_version": 1,
                "mode": mode,
                "route_objective": objective,
                "objective_source": objective_source,
                "policy_sources": policy_sources,
                "plan": plan,
                "readiness": readiness,
            });
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!("mode={mode:?} objective={objective:?}");
                println!("ranked:");
                for candidate in &plan.ranked {
                    println!(
                        "- {}/{}: {}",
                        candidate.manifest.provider_id, candidate.manifest.model, candidate.reason
                    );
                }
                println!("rejected:");
                for candidate in &plan.rejected {
                    println!(
                        "- {}/{}: {:?}",
                        candidate.provider_id, candidate.model, candidate.reasons
                    );
                }
                println!("readiness:");
                for provider in &readiness {
                    println!(
                        "- {}/{}: ready={} credential={:?} error={:?}",
                        provider.provider_id,
                        provider.model,
                        provider.ready,
                        provider.credential_status,
                        provider.configuration_error
                    );
                }
            }
        }
        Command::Tools { json } => {
            let capabilities = default_registry().capabilities();
            if json {
                println!("{}", serde_json::to_string_pretty(&capabilities)?);
            } else {
                for capability in capabilities {
                    println!(
                        "{} | {:?} | risk={:?} | write={} | timeout={}ms",
                        capability.name,
                        capability.status,
                        capability.risk,
                        capability.requires_write,
                        capability.timeout_ms
                    );
                    println!("- {}", capability.description);
                }
            }
        }
        Command::History {
            journal_dir,
            limit,
            json,
        } => {
            if limit == 0 || limit > 200 {
                bail!("--limit must be between 1 and 200");
            }
            let history = scan_history(&journal_dir, limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else if history.is_empty() {
                println!("no runs found in {}", journal_dir.display());
            } else {
                for entry in history {
                    println!(
                        "{} | {:?} | {} bytes | {}",
                        entry
                            .run_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "<unknown>".into()),
                        entry.stop_reason,
                        entry.size_bytes,
                        entry.path.display()
                    );
                }
            }
        }
        Command::Reframe {
            task,
            input,
            constraint,
            base_url,
            model,
            api_key,
            allow_network,
            timeout_seconds,
            accept,
            substitute,
            reject,
            defer,
            journal_dir,
            json,
        } => {
            let (proposal, provider_id, model_label, prompt_sha) = match (task, input) {
                (Some(task), None) => {
                    if !allow_network {
                        bail!("--allow-network is required to call a provider");
                    }
                    let base_url = base_url
                        .ok_or_else(|| anyhow::anyhow!("--base-url is required with --task"))?;
                    let model = model.unwrap_or_else(|| "deepseek-flash".to_string());
                    let provider = OpenAiCompatibleProvider::new(
                        "reframe",
                        model.clone(),
                        &base_url,
                        api_key,
                        Duration::from_secs(timeout_seconds),
                    )?;
                    let text = provider
                        .complete_text(
                            REFRAME_SYSTEM_PROMPT,
                            &build_user_message(&task, &constraint),
                            &CancellationToken::new(),
                        )
                        .await?;
                    let proposal = parse_model_output(&text)?.into_proposal(&task)?;
                    proposal.validate()?;
                    (
                        proposal,
                        "openai-compatible".to_string(),
                        model,
                        prompt_sha256(),
                    )
                }
                (None, Some(path)) => {
                    let bytes = std::fs::read(&path)?;
                    let proposal: ReframeProposal = serde_json::from_slice(&bytes)
                        .with_context(|| format!("invalid reframe proposal: {}", path.display()))?;
                    proposal.validate()?;
                    (
                        proposal,
                        "file".to_string(),
                        path.display().to_string(),
                        hex::encode(Sha256::digest(&bytes)),
                    )
                }
                _ => bail!("exactly one of --task or --input is required"),
            };

            let decided =
                usize::from(accept.is_some()) + usize::from(reject.is_some()) + usize::from(defer);
            if decided > 1 {
                bail!("exactly one of --accept, --reject or --defer is allowed");
            }
            if substitute.is_some() && accept.is_none() {
                bail!("--substitute only applies with --accept");
            }
            if accept == Some(0) || substitute == Some(0) {
                bail!("choice numbers start at 1");
            }
            if decided == 0 {
                // An end has no fork, so it gets no menu and no pending file.
                if proposal.classification == RequestRole::End {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "pending": false,
                                "proposal": proposal,
                            }))?
                        );
                    } else {
                        println!("这是目的类请求，按原请求执行。");
                    }
                    return Ok(());
                }
                // Nothing is decided, so no journal is written. The proposal is
                // persisted anyway, because a menu whose numbers cannot be
                // resolved later is not actionable: re-calling a model would
                // produce different options and different indices.
                let pending = persist_pending(&proposal, &journal_dir)?;
                let menu = render_choice_menu(&proposal, &format!("--input {}", pending.display()));
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "pending": true,
                            "proposal": proposal,
                            "menu": menu,
                        }))?
                    );
                } else {
                    print!("{menu}");
                }
                return Ok(());
            }

            let choice = match (accept, reject.as_deref(), defer) {
                (Some(choice_number), _, _) => ReframeChoice::Accepted {
                    option_index: choice_number - 1,
                    substitute_index: substitute.map(|number| number - 1),
                },
                (_, Some(reason), _) => ReframeChoice::Rejected {
                    reason: parse_rejected_reason(reason)?,
                },
                (_, _, true) => ReframeChoice::Deferred,
                _ => unreachable!("exactly one decision is required"),
            };
            choice.validate_against(&proposal)?;

            let decided_at = now_unix_ms();
            let (run_id, journal_path) = record_reframe_decision(
                &proposal,
                &choice,
                &provider_id,
                &model_label,
                &prompt_sha,
                &journal_dir,
                decided_at,
            )?;

            let receipt = render_receipt(&proposal, &choice, "user", decided_at);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "run_id": run_id,
                        "journal": journal_path,
                        "choice": choice,
                        "receipt": receipt,
                    }))?
                );
            } else {
                print!("{receipt}");
                println!("journal   {}", journal_path.display());
            }
        }
        Command::Receipt { journal, json } => {
            let events = read_events(&journal)?;
            let state = replay_events(&events).map_err(|error| anyhow::anyhow!(error))?;
            let proposal = state
                .reframe_proposal
                .ok_or_else(|| anyhow::anyhow!("journal contains no reframe proposal"))?;
            let choice = state
                .reframe_choice
                .ok_or_else(|| anyhow::anyhow!("journal contains no reframe decision"))?;
            let actor = state.reframe_actor.unwrap_or_default();
            let decided_at = state.reframe_decided_at_unix_ms.unwrap_or(0);
            let receipt = render_receipt(&proposal, &choice, &actor, decided_at);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "run_id": state.run_id,
                        "actor": actor,
                        "decided_at_unix_ms": decided_at,
                        "choice": choice,
                        "receipt": receipt,
                    }))?
                );
            } else {
                print!("{receipt}");
            }
        }
        Command::Annotate { command } => {
            handle_annotations(command)?;
        }
        Command::Approval { command } => {
            handle_approval(command)?;
        }
        Command::Provenance { command } => {
            handle_provenance(command)?;
        }
        Command::Init { root, json } => {
            let root = root.unwrap_or_else(default_home);
            let outcome = init(&root, current_time_ms())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else if outcome.created {
                println!("initialized 1Agent home at {}", outcome.root.display());
            } else {
                println!(
                    "1Agent home already initialized at {}",
                    outcome.root.display()
                );
            }
        }
        Command::Doctor { root, json } => {
            let root = root.unwrap_or_else(default_home);
            let report = doctor(&root)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                for check in &report.checks {
                    println!("{:?} {}: {}", check.status, check.id, check.detail);
                }
                println!("overall: {:?}", report.status);
            }
            if report.status == DoctorStatus::Fail {
                std::process::exit(1);
            }
        }
        Command::Export { root, output, json } => {
            let root = root.unwrap_or_else(default_home);
            let outcome = export(&root, &output, current_time_ms())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!(
                    "exported {} files to {} ({} key/credential files excluded)",
                    outcome.manifest.files.len(),
                    outcome.output.display(),
                    outcome.manifest.excluded.len()
                );
            }
        }
        Command::Restore { bundle, root, json } => {
            let root = root.unwrap_or_else(default_home);
            let outcome = restore(&bundle, &root, current_time_ms())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!(
                    "restored {} files into {}",
                    outcome.files,
                    outcome.root.display()
                );
            }
        }
        Command::Upgrade {
            root,
            check,
            rollback,
            json,
        } => {
            let root = root.unwrap_or_else(default_home);
            let outcome = upgrade(&root, check, rollback, current_time_ms())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else if outcome.repaired.is_empty() {
                println!(
                    "runtime home is up to date (version {})",
                    outcome.to_version
                );
            } else if check {
                println!("pending changes: {}", outcome.repaired.join(", "));
            } else if outcome.rolled_back {
                println!(
                    "rolled back to version {}: {}",
                    outcome.to_version,
                    outcome.repaired.join(", ")
                );
            } else {
                println!(
                    "upgraded to version {}: {}",
                    outcome.to_version,
                    outcome.repaired.join(", ")
                );
            }
        }
        Command::Uninstall {
            root,
            purge,
            yes,
            json,
        } => {
            let root = root.unwrap_or_else(default_home);
            let outcome = uninstall(&root, purge, yes)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!("{}", outcome.message);
            }
        }
        Command::Key { command } => match command {
            KeyCommand::Escrow {
                root,
                output,
                recovery_key,
                json,
            } => {
                let root = root.unwrap_or_else(default_home);
                let outcome = escrow_keys(&root, &output, &recovery_key, current_time_ms())?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&outcome)?);
                } else {
                    println!(
                        "escrowed {} key files to {} (recovery key: {})",
                        outcome.files.len(),
                        outcome.output.display(),
                        outcome.recovery_key.display()
                    );
                }
            }
            KeyCommand::Restore {
                root,
                escrow,
                recovery_key,
                json,
            } => {
                let root = root.unwrap_or_else(default_home);
                let outcome = restore_keys(&root, &escrow, &recovery_key)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&outcome)?);
                } else {
                    println!(
                        "restored key files ({}) into {}",
                        outcome.files.join(", "),
                        outcome.root.display()
                    );
                }
            }
        },
        Command::Release { command } => {
            handle_release(command)?;
        }
        Command::Resume {
            journal,
            config,
            provider,
            model,
            base_url,
            api_key,
            allow_network,
            journal_dir,
            timeout_seconds,
            route_objective,
            json,
        } => {
            let timeout = Duration::from_secs(timeout_seconds.max(1));
            let (candidates, tools) = if let Some(config_path) = config {
                let config = AgentConfig::load(&config_path)?;
                (config.build_providers(timeout)?, default_registry())
            } else {
                build_runtime_parts(&provider, model, base_url, api_key, allow_network, timeout)?
            };
            let mut runtime = AgentRuntime::new(candidates, tools, journal_dir);
            if let Some(route_objective) = route_objective {
                runtime = runtime
                    .with_route_objective(parse_route_objective(&route_objective)?)
                    .with_route_policy_sources(RoutePolicySources {
                        objective: PolicyValueSource::CliOverride,
                        latency_budget: PolicyValueSource::Unset,
                        unit_cost_budget: PolicyValueSource::Unset,
                    });
            }
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_cancellation.cancel();
                }
            });
            let result = runtime.resume(&journal, cancellation).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("{}", render_answer(&result));
                print_route_trace(&result.journal_path)?;
                println!("\n运行日志：{}", result.journal_path.display());
            }
        }
        Command::ResumeStatus { journal, json } => {
            let events = read_events(&journal)?;
            let prefix = find_resumable_prefix(&events)?;
            let checkpoint = prefix.checkpoint;
            let route_aware = checkpoint.route_plan_sha256.is_some()
                && checkpoint.route_policy_sources.is_some()
                && checkpoint.route_objective.is_some();
            let value = serde_json::json!({
                "route_aware": route_aware,
                "legacy": !route_aware,
                "mode": checkpoint.mode,
                "next_step": checkpoint.next_step,
                "tool_calls_used": checkpoint.tool_calls_used,
                "elapsed_ms": checkpoint.elapsed_ms,
                "route_plan_sha256": checkpoint.route_plan_sha256,
                "route_policy_sources": checkpoint.route_policy_sources,
                "route_objective": checkpoint.route_objective,
                "tool_capabilities_sha256": checkpoint.tool_capabilities_sha256,
            });
            if json {
                println!("{}", serde_json::to_string_pretty(&value)?);
            } else {
                println!(
                    "route_aware={} mode={:?} next_step={} tool_calls={} elapsed_ms={}",
                    route_aware,
                    checkpoint.mode,
                    checkpoint.next_step,
                    checkpoint.tool_calls_used,
                    checkpoint.elapsed_ms
                );
            }
        }
        Command::Experience { command } => {
            handle_experience(command).await?;
        }
    }
    Ok(())
}

async fn handle_experience(command: ExperienceCommand) -> Result<()> {
    match command {
        ExperienceCommand::Propose {
            store,
            journal,
            input,
        } => {
            let source_events = read_events(&journal)
                .with_context(|| format!("cannot read {}", journal.display()))?;
            let content = std::fs::read_to_string(&input)
                .with_context(|| format!("cannot read {}", input.display()))?;
            let spec: ExperienceCandidateSpec = serde_json::from_str(&content)
                .with_context(|| format!("invalid candidate JSON in {}", input.display()))?;
            let mut experience = ExperienceStore::open(store)?;
            let candidate = experience.propose(spec, &source_events)?;
            println!("{}", serde_json::to_string_pretty(&candidate)?);
        }
        ExperienceCommand::Review {
            store,
            id,
            actor,
            journal,
        } => {
            let source_events = journal
                .as_ref()
                .map(read_events)
                .transpose()
                .with_context(|| "cannot read experience source journal")?;
            ExperienceStore::open(store)?.review_with_source(
                id,
                &actor,
                source_events.as_deref(),
            )?;
            println!("candidate {id}: reviewed");
        }
        ExperienceCommand::Verify { store, id, input } => {
            let content = std::fs::read_to_string(&input)
                .with_context(|| format!("cannot read {}", input.display()))?;
            let verification: ExperienceVerificationInput = serde_json::from_str(&content)
                .with_context(|| format!("invalid verification JSON in {}", input.display()))?;
            ExperienceStore::open(store)?.verify(id, verification)?;
            println!("candidate {id}: verified");
        }
        ExperienceCommand::Evaluate {
            store,
            id,
            suite,
            training_group,
            evaluation_group,
            output,
        } => {
            let mut experience = ExperienceStore::open(&store)?;
            let candidate = experience.validate_for_shadow(id)?;
            let providers = vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }];
            let report = evaluate_experience_suite(
                &suite,
                candidate,
                providers,
                false,
                training_group,
                evaluation_group,
            )
            .await?;
            let json = serde_json::to_string_pretty(&report)?;
            if let Some(output) = output {
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&output, format!("{json}\n"))?;
            }
            experience.verify(id, report.verification.clone())?;
            println!(
                "candidate {id}: verified automatically\nbaseline {}/{}; learned {}/{}; hard gates {} -> {}",
                report.baseline.passed,
                report.baseline.total,
                report.learned.passed,
                report.learned.total,
                report.baseline.hard_gate_failures,
                report.learned.hard_gate_failures
            );
        }
        ExperienceCommand::Approve {
            store,
            id,
            actor,
            journal,
        } => {
            let source_events = journal
                .as_ref()
                .map(read_events)
                .transpose()
                .with_context(|| "cannot read experience source journal")?;
            ExperienceStore::open(store)?.approve_with_source(
                id,
                &actor,
                source_events.as_deref(),
            )?;
            println!("candidate {id}: approved");
        }
        ExperienceCommand::Reject {
            store,
            id,
            actor,
            reason,
        } => {
            ExperienceStore::open(store)?.reject(id, &actor, &reason)?;
            println!("candidate {id}: rejected");
        }
        ExperienceCommand::Activate { store, id, journal } => {
            let source_events = journal
                .as_ref()
                .map(read_events)
                .transpose()
                .with_context(|| "cannot read experience source journal")?;
            ExperienceStore::open(store)?.activate_with_source(id, source_events.as_deref())?;
            println!("candidate {id}: active");
        }
        ExperienceCommand::Degrade { store, id, reason } => {
            ExperienceStore::open(store)?.degrade(id, &reason)?;
            println!("candidate {id}: degraded");
        }
        ExperienceCommand::Rollback { store, id, reason } => {
            ExperienceStore::open(store)?.rollback(id, &reason)?;
            println!("candidate {id}: rolled_back");
        }
        ExperienceCommand::List { store } => {
            let candidates = ExperienceStore::open(store)?.candidates()?;
            println!("{}", serde_json::to_string_pretty(&candidates)?);
        }
        ExperienceCommand::Audit { store, json } => {
            let report = ExperienceStore::open(store)?.audit();
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "experience audit {}: events={} candidates={} active={} superseded={} chains={} depth={} expired_active={}",
                    if report.ok { "passed" } else { "failed" },
                    report.event_count,
                    report.candidate_count,
                    report.status_counts.active,
                    report.status_counts.superseded,
                    report.supersession_chains,
                    report.max_supersession_depth,
                    report.expired_active
                );
                for finding in &report.findings {
                    println!("- {:?}: {}", finding.code, finding.message);
                }
            }
            if !report.ok {
                bail!("experience audit failed");
            }
        }
    }
    Ok(())
}

fn handle_annotations(command: AnnotateCommand) -> Result<()> {
    match command {
        AnnotateCommand::Add {
            store,
            case_id,
            annotator,
            label,
            evidence_source,
            note,
        } => {
            let label = parse_annotation_label(&label)?;
            let mut annotations = AnnotationStore::open(store)?;
            annotations.add(AnnotationRecord {
                case_id: case_id.clone(),
                annotator_id: annotator,
                label,
                evidence_sources: evidence_source,
                note,
                created_unix_ms: 0,
            })?;
            println!("annotation saved for {case_id}");
        }
        AnnotateCommand::Agreement { store, json } => {
            let report = AnnotationStore::open(store)?.agreement()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "annotations={} eligible_cases={} observed={}bps kappa={:.3} enough_data={}",
                    report.annotations,
                    report.eligible_cases,
                    report.observed_agreement_bps,
                    report.kappa_millis as f64 / 1_000.0,
                    report.enough_data
                );
            }
        }
        AnnotateCommand::List { store } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&AnnotationStore::open(store)?.records()?)?
            );
        }
        AnnotateCommand::Calibrate {
            store,
            predictions,
            goldset,
            suite,
            approval,
            approval_key_env,
            approval_registry,
            registry_snapshot,
            registry_key_env,
            min_registry_version,
            trust_anchor,
            min_kappa_millis,
            min_samples,
            json,
        } => {
            let envelope: ApprovalEnvelope = serde_json::from_str(
                &std::fs::read_to_string(&approval)
                    .with_context(|| format!("cannot read {}", approval.display()))?,
            )
            .with_context(|| format!("invalid approval envelope {}", approval.display()))?;
            let approval_key = read_secret_env(&approval_key_env)?;
            let registry = ApprovalKeyRegistry::load(&approval_registry)?;
            let snapshots = registry_snapshot
                .iter()
                .map(|path| {
                    let content = std::fs::read_to_string(path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    serde_json::from_str::<RegistrySnapshot>(&content)
                        .with_context(|| format!("invalid registry snapshot {}", path.display()))
                })
                .collect::<Result<Vec<_>>>()?;
            let registry_key = read_secret_env(&registry_key_env)?;
            let anchor: TrustAnchorManifest = serde_json::from_str(
                &std::fs::read_to_string(&trust_anchor)
                    .with_context(|| format!("cannot read {}", trust_anchor.display()))?,
            )?;
            verify_snapshot_chain_with_anchor(
                &snapshots,
                &anchor,
                &registry,
                &approval_registry,
                &registry_key,
                min_registry_version,
            )?;
            registry.verify_approval(&envelope, &goldset, &approval_key)?;
            let manifest: GoldSetManifest = serde_json::from_str(
                &std::fs::read_to_string(&goldset)
                    .with_context(|| format!("cannot read {}", goldset.display()))?,
            )
            .with_context(|| format!("invalid gold-set manifest {}", goldset.display()))?;
            verify_goldset_manifest(&manifest, suite)?;
            let content = std::fs::read_to_string(&predictions)
                .with_context(|| format!("cannot read {}", predictions.display()))?;
            let predictions = content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(serde_json::from_str::<CalibrationPrediction>)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let allowed_cases: std::collections::BTreeSet<_> = manifest.case_ids.iter().collect();
            if predictions
                .iter()
                .any(|prediction| !allowed_cases.contains(&prediction.case_id))
            {
                bail!("prediction references a case outside the frozen gold set");
            }
            let report = AnnotationStore::open(store)?.calibrate(
                predictions,
                min_kappa_millis,
                min_samples,
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "samples={} kappa={:.3} ECE={}bps Brier={}",
                    report.samples,
                    report.kappa_millis as f64 / 1_000.0,
                    report.ece_bps,
                    report.brier_micros
                );
            }
        }
        AnnotateCommand::CreateGoldset {
            suite,
            output,
            version,
            owner,
            reviewer,
            redaction_policy,
            batch_size,
        } => {
            let manifest = create_goldset_manifest(
                suite,
                version,
                owner,
                reviewer,
                redaction_policy,
                batch_size,
            )?;
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_vec_pretty(&manifest)?)?;
            println!("{}", output.display());
        }
        AnnotateCommand::VerifyGoldset { manifest, suite } => {
            let manifest: GoldSetManifest = serde_json::from_str(
                &std::fs::read_to_string(&manifest)
                    .with_context(|| format!("cannot read {}", manifest.display()))?,
            )?;
            verify_goldset_manifest(&manifest, suite)?;
            println!("gold-set manifest verified");
        }
    }
    Ok(())
}

fn handle_approval(command: ApprovalCommand) -> Result<()> {
    match command {
        ApprovalCommand::Sign {
            artifact,
            artifact_kind,
            approver,
            purpose,
            key_id,
            key_env,
            registry,
            output,
        } => {
            let artifact_kind = match artifact_kind.as_str() {
                "gold_set" | "gold-set" => ArtifactKind::GoldSet,
                "experience" => ArtifactKind::Experience,
                "evaluation_report" | "evaluation-report" => ArtifactKind::EvaluationReport,
                "provenance" => ArtifactKind::Provenance,
                "release_manifest" | "release-manifest" => ArtifactKind::ReleaseManifest,
                other => bail!("unsupported artifact kind '{other}'"),
            };
            let key = read_secret_env(&key_env)?;
            if let Some(registry) = registry {
                ApprovalKeyRegistry::load(registry)?.require_active(&key_id)?;
            }
            let envelope =
                ApprovalEnvelope::sign(&artifact, artifact_kind, approver, purpose, key_id, &key)?;
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_vec_pretty(&envelope)?)?;
            println!("{}", output.display());
        }
        ApprovalCommand::Verify {
            artifact,
            envelope,
            key_env,
            registry,
        } => {
            let envelope: ApprovalEnvelope = serde_json::from_str(
                &std::fs::read_to_string(&envelope)
                    .with_context(|| format!("cannot read {}", envelope.display()))?,
            )?;
            let key = read_secret_env(&key_env)?;
            if let Some(registry) = registry {
                ApprovalKeyRegistry::load(registry)?.verify_approval(&envelope, &artifact, &key)?;
            } else {
                envelope.verify(&artifact, &key)?;
            }
            println!("approval verified");
        }
        ApprovalCommand::KeyAdd { registry, key_id } => {
            let path = registry;
            let mut registry = ApprovalKeyRegistry::load(&path).unwrap_or_default();
            registry.add_active(key_id)?;
            registry.save(&path)?;
            println!("key registered");
        }
        ApprovalCommand::KeyRetire {
            registry,
            key_id,
            reason,
        } => {
            let path = registry;
            let mut registry = ApprovalKeyRegistry::load(&path)?;
            registry.transition(&key_id, ApprovalKeyStatus::Retired, reason)?;
            registry.save(&path)?;
            println!("key retired");
        }
        ApprovalCommand::KeyRevoke {
            registry,
            key_id,
            reason,
        } => {
            let path = registry;
            let mut registry = ApprovalKeyRegistry::load(&path)?;
            registry.transition(&key_id, ApprovalKeyStatus::Revoked, reason)?;
            registry.save(&path)?;
            println!("key revoked");
        }
        ApprovalCommand::Snapshot {
            registry,
            output,
            approver,
            key_id,
            key_env,
            previous,
        } => {
            let registry_value = ApprovalKeyRegistry::load(&registry)?;
            registry_value.require_active(&key_id)?;
            let previous = previous
                .map(|path| {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    serde_json::from_str::<RegistrySnapshot>(&content)
                        .with_context(|| format!("invalid snapshot {}", path.display()))
                })
                .transpose()?;
            let snapshot = registry_value.sign_snapshot(
                &registry,
                approver,
                key_id,
                &read_secret_env(&key_env)?,
                previous.as_ref(),
            )?;
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_vec_pretty(&snapshot)?)?;
            println!("{}", output.display());
        }
        ApprovalCommand::VerifySnapshot {
            registry,
            snapshot,
            key_env,
            min_version,
        } => {
            let registry_value = ApprovalKeyRegistry::load(&registry)?;
            let snapshot: RegistrySnapshot = serde_json::from_str(
                &std::fs::read_to_string(&snapshot)
                    .with_context(|| format!("cannot read {}", snapshot.display()))?,
            )?;
            snapshot.verify(
                &registry_value,
                &registry,
                &read_secret_env(&key_env)?,
                min_version,
            )?;
            println!("registry snapshot verified");
        }
        ApprovalCommand::VerifyChain {
            registry,
            snapshot,
            key_env,
            min_version,
            anchor,
        } => {
            let registry_value = ApprovalKeyRegistry::load(&registry)?;
            let snapshots = snapshot
                .iter()
                .map(|path| {
                    let content = std::fs::read_to_string(path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    serde_json::from_str::<RegistrySnapshot>(&content)
                        .with_context(|| format!("invalid snapshot {}", path.display()))
                })
                .collect::<Result<Vec<_>>>()?;
            let key = read_secret_env(&key_env)?;
            if let Some(anchor) = anchor {
                let anchor: TrustAnchorManifest = serde_json::from_str(
                    &std::fs::read_to_string(&anchor)
                        .with_context(|| format!("cannot read {}", anchor.display()))?,
                )?;
                verify_snapshot_chain_with_anchor(
                    &snapshots,
                    &anchor,
                    &registry_value,
                    &registry,
                    &key,
                    min_version,
                )?;
            } else {
                verify_snapshot_chain(&snapshots, &registry_value, &registry, &key, min_version)?;
            }
            println!("registry snapshot chain verified");
        }
        ApprovalCommand::Anchor {
            snapshot,
            output,
            operator,
            key_id,
        } => {
            let snapshot: RegistrySnapshot = serde_json::from_str(
                &std::fs::read_to_string(&snapshot)
                    .with_context(|| format!("cannot read {}", snapshot.display()))?,
            )?;
            let anchor = create_trust_anchor(&snapshot, operator, key_id)?;
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_vec_pretty(&anchor)?)?;
            println!("{}", output.display());
        }
    }
    Ok(())
}

fn handle_provenance(command: ProvenanceCommand) -> Result<()> {
    match command {
        ProvenanceCommand::Create {
            artifact,
            target,
            output,
            key_env,
            approver,
            approval_output,
        } => {
            let workspace = std::env::current_dir()?;
            let provenance = collect_provenance(&workspace, &artifact, target)?;
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output, serde_json::to_vec_pretty(&provenance)?)?;
            match (key_env, approver, approval_output) {
                (Some(key_env), Some(approver), Some(approval_output)) => {
                    let envelope = ApprovalEnvelope::sign(
                        &output,
                        ArtifactKind::Provenance,
                        approver,
                        "release provenance".into(),
                        "provenance-key".into(),
                        &read_secret_env(&key_env)?,
                    )?;
                    if let Some(parent) = approval_output.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&approval_output, serde_json::to_vec_pretty(&envelope)?)?;
                }
                (None, None, None) => {}
                _ => bail!("--key-env, --approver and --approval-output must be provided together"),
            }
            println!("{}", output.display());
        }
        ProvenanceCommand::Verify {
            provenance,
            artifact,
        } => {
            let provenance = serde_json::from_str(
                &std::fs::read_to_string(&provenance)
                    .with_context(|| format!("cannot read {}", provenance.display()))?,
            )?;
            verify_provenance(&provenance, artifact)?;
            println!("provenance verified");
        }
        ProvenanceCommand::Rebuild {
            artifact,
            target,
            json,
        } => {
            let report = rebuild_and_compare(std::env::current_dir()?, &artifact, &target)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "target={} equal={} original={} rebuilt={} duration_ms={}",
                    report.target_triple,
                    report.hashes_equal,
                    report.original_sha256,
                    report.rebuilt_sha256,
                    report.duration_ms
                );
            }
            if !report.hashes_equal || !report.sizes_equal {
                bail!("rebuild is not byte-for-byte reproducible");
            }
        }
    }
    Ok(())
}

fn handle_release(command: ReleaseCommand) -> Result<()> {
    match command {
        ReleaseCommand::Manifest {
            package_root,
            linux_archive,
            windows_archive,
            output,
            allow_dirty,
        } => {
            let parent = output
                .parent()
                .context("release manifest output must have a parent directory")?;
            std::fs::create_dir_all(parent)?;
            let manifest = collect_release_manifest(
                package_root,
                linux_archive,
                windows_archive,
                parent,
                allow_dirty,
            )?;
            std::fs::write(&output, serde_json::to_vec_pretty(&manifest)?)?;
            println!("{}", output.display());
        }
        ReleaseCommand::Verify {
            manifest,
            approval,
            key_env,
            registry,
            production,
            registry_key_env,
            registry_snapshot,
            trust_anchor,
            min_registry_version,
            policy,
            json,
        } => {
            let policy = policy
                .map(|path| {
                    let content = std::fs::read_to_string(&path)
                        .with_context(|| format!("cannot read {}", path.display()))?;
                    serde_json::from_str::<ReleasePolicy>(&content)
                        .with_context(|| format!("invalid release policy {}", path.display()))
                })
                .transpose()?;
            if let Some(policy) = &policy {
                policy.validate()?;
            }
            if production
                && policy.as_ref().map(|policy| policy.mode) == Some(ReleasePolicyMode::Development)
            {
                bail!("--production conflicts with a development release policy");
            }
            let policy_mode = policy
                .as_ref()
                .map(|policy| policy.mode)
                .unwrap_or(if production {
                    ReleasePolicyMode::Production
                } else {
                    ReleasePolicyMode::Development
                });
            let production = policy_mode == ReleasePolicyMode::Production;
            let effective_min_registry_version = policy
                .as_ref()
                .map(|policy| policy.minimum_registry_version)
                .unwrap_or(min_registry_version);
            let content = std::fs::read_to_string(&manifest)
                .with_context(|| format!("cannot read {}", manifest.display()))?;
            let manifest_value = serde_json::from_str(&content)
                .with_context(|| format!("invalid release manifest {}", manifest.display()))?;
            let parent = manifest
                .parent()
                .context("release manifest must have a parent directory")?;
            verify_release_manifest(&manifest_value, parent)?;
            if !production
                && (registry_key_env.is_some()
                    || !registry_snapshot.is_empty()
                    || trust_anchor.is_some())
            {
                bail!("registry chain options require --production");
            }
            if production
                && (approval.is_none()
                    || key_env.is_none()
                    || registry.is_none()
                    || registry_key_env.is_none()
                    || registry_snapshot.is_empty()
                    || trust_anchor.is_none())
            {
                bail!(
                    "production release requires --approval, --key-env, --registry, \
                     --registry-key-env, at least one --registry-snapshot and --trust-anchor"
                );
            }
            let mut signed = false;
            let mut registry_chain_verified = false;
            match (approval, key_env, registry) {
                (Some(approval), Some(key_env), Some(registry)) => {
                    let envelope: ApprovalEnvelope = serde_json::from_str(
                        &std::fs::read_to_string(&approval)
                            .with_context(|| format!("cannot read {}", approval.display()))?,
                    )?;
                    let registry_path = registry;
                    let registry = ApprovalKeyRegistry::load(&registry_path)?;
                    if let Some(policy) = &policy {
                        policy.authorize_manifest_approval(&envelope)?;
                    }
                    verify_release_approval(
                        &manifest,
                        &envelope,
                        &read_secret_env(&key_env)?,
                        &registry,
                    )?;
                    signed = true;
                    if production {
                        let snapshots = registry_snapshot
                            .iter()
                            .map(|path| {
                                let content = std::fs::read_to_string(path)
                                    .with_context(|| format!("cannot read {}", path.display()))?;
                                serde_json::from_str::<RegistrySnapshot>(&content).with_context(
                                    || format!("invalid registry snapshot {}", path.display()),
                                )
                            })
                            .collect::<Result<Vec<_>>>()?;
                        let anchor: TrustAnchorManifest = serde_json::from_str(
                            &std::fs::read_to_string(
                                trust_anchor.as_ref().expect("validated above"),
                            )
                            .with_context(|| {
                                format!(
                                    "cannot read {}",
                                    trust_anchor.as_ref().expect("validated above").display()
                                )
                            })?,
                        )?;
                        if let Some(policy) = &policy {
                            policy.authorize_registry_signers(&snapshots, &anchor)?;
                        }
                        verify_snapshot_chain_with_anchor(
                            &snapshots,
                            &anchor,
                            &registry,
                            &registry_path,
                            &read_secret_env(
                                registry_key_env.as_deref().expect("validated above"),
                            )?,
                            effective_min_registry_version,
                        )?;
                        registry_chain_verified = true;
                    }
                }
                (None, None, None) => {}
                _ => bail!("--approval, --key-env and --registry must be provided together"),
            }
            if content != std::fs::read_to_string(&manifest)? {
                bail!("release manifest changed during verification");
            }
            verify_release_manifest(&manifest_value, parent)?;
            let report = build_release_verification_report(
                &manifest_value,
                &manifest,
                policy_mode,
                signed,
                registry_chain_verified,
            )?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                let mode = match report.policy_mode {
                    ReleasePolicyMode::Development => "development",
                    ReleasePolicyMode::Production => "production",
                };
                println!("release manifest verified ({mode} policy)");
            }
        }
    }
    Ok(())
}

fn read_secret_env(name: &str) -> Result<Vec<u8>> {
    let value = std::env::var(name)
        .with_context(|| format!("required approval key environment variable {name} is missing"))?;
    Ok(value.into_bytes())
}

fn parse_annotation_label(value: &str) -> Result<AnnotationLabel> {
    match value {
        "supported" => Ok(AnnotationLabel::Supported),
        "unsupported" => Ok(AnnotationLabel::Unsupported),
        "needs_evidence" | "needs-evidence" => Ok(AnnotationLabel::NeedsEvidence),
        other => bail!(
            "invalid annotation label '{other}'; use supported, unsupported or needs_evidence"
        ),
    }
}

fn parse_route_objective(value: &str) -> Result<RouteObjective> {
    match value {
        "quality" => Ok(RouteObjective::Quality),
        "balanced" => Ok(RouteObjective::Balanced),
        "latency" => Ok(RouteObjective::Latency),
        "cost" => Ok(RouteObjective::Cost),
        other => {
            bail!("unsupported route objective '{other}'; use quality, balanced, latency or cost")
        }
    }
}

fn resolve_route_objective(value: Option<&str>) -> Result<(RouteObjective, PolicyValueSource)> {
    match value {
        Some(value) => Ok((
            parse_route_objective(value)?,
            PolicyValueSource::CliOverride,
        )),
        None => Ok((RouteObjective::Quality, PolicyValueSource::RuntimeDefault)),
    }
}

fn load_request(task: Option<String>, input: Option<PathBuf>) -> Result<TaskRequest> {
    if let Some(path) = input {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        return serde_json::from_str(&content)
            .with_context(|| format!("invalid task JSON in {}", path.display()));
    }
    let objective = task.context("provide --task or --input")?;
    let cwd = std::env::current_dir().context("cannot determine current directory")?;
    Ok(TaskRequest::from_objective(objective, cwd))
}

fn build_runtime_parts(
    provider: &str,
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    allow_network: bool,
    timeout: Duration,
) -> Result<(Vec<ProviderCandidate>, ToolRegistry)> {
    let tools = default_registry();
    let candidate = match provider {
        "mock" | "deterministic" => ProviderCandidate {
            provider: Arc::new(DeterministicProvider::new()),
            manifest: local_manifest(),
        },
        "openai-compatible" => {
            if !allow_network {
                bail!("openai-compatible provider requires --allow-network");
            }
            let model = model.context("--model or AGENT_MODEL is required")?;
            let base_url = base_url.context("--base-url or AGENT_BASE_URL is required")?;
            let provider_id = "openai-compatible";
            let provider = OpenAiCompatibleProvider::new(
                provider_id,
                model.clone(),
                &base_url,
                api_key,
                timeout,
            )?;
            ProviderCandidate {
                provider: Arc::new(provider),
                manifest: remote_manifest(provider_id, model),
            }
        }
        other => bail!("unsupported provider '{other}'"),
    };
    Ok((vec![candidate], tools))
}

fn canonical_or_original(path: &PathBuf) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.clone())
}

#[allow(clippy::too_many_arguments)]
async fn run_master_plan(
    task: String,
    config: Option<PathBuf>,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    allow_network: bool,
    workspace: Option<PathBuf>,
    risk: Option<String>,
    write: bool,
    external_action: bool,
    requested_items: Option<usize>,
    agents: Option<PathBuf>,
    workshop_prefix: Option<String>,
    output: Option<PathBuf>,
    planner_journal_dir: PathBuf,
    json: bool,
) -> Result<()> {
    let signals = TriageSignals {
        requested_items,
        requires_write: write,
        requires_external_action: external_action,
        risk: parse_risk(risk.as_deref().unwrap_or("low"))?,
    };
    let triage = triage_goal(&task, &signals);
    if !triage.requires_plan {
        if json {
            println!("{}", serde_json::to_string_pretty(&triage)?);
        } else {
            println!("无需任务计划：{}", triage.reason);
        }
        return Ok(());
    }
    let workspace = workspace.unwrap_or(std::env::current_dir()?);
    let workspace = canonical_or_original(&workspace);
    let mut profiles = if let Some(agents) = agents {
        load_agent_profiles(&agents)?
    } else {
        Vec::new()
    };
    if let Some(prefix) = workshop_prefix {
        profiles = WorkshopCliAdapter::new(prefix, Duration::from_secs(30), true)?
            .merge_profiles(&profiles)?;
    }
    let mut context = PlanningContext::new(task, workspace, &triage)?;
    context.risk = if signals.requires_write
        || signals.requires_external_action
        || signals.risk == RiskLevel::High
    {
        RiskLevel::High
    } else {
        signals.risk
    };
    context.agents = profiles;
    let timeout = Duration::from_secs(120);
    let provider: Arc<dyn ModelProvider> = if let Some(config) = config {
        let candidates = AgentConfig::load(&config)?.build_providers(timeout)?;
        candidates
            .first()
            .map(|candidate| candidate.provider.clone())
            .context("configured provider list is empty")?
    } else {
        build_runtime_parts(&provider, model, base_url, api_key, allow_network, timeout)?
            .0
            .first()
            .map(|candidate| candidate.provider.clone())
            .context("provider list is empty")?
    };
    let generator = ModelPlanGenerator::new(provider).with_journal_dir(&planner_journal_dir);
    let cancellation = CancellationToken::new();
    let plan = generator.generate(&context, &cancellation).await?;
    if !json {
        println!(
            "规划 Journal：{}",
            first_principles_agent::planning_journal_path(&planner_journal_dir, &context.plan_id)
                .display()
        );
    }
    if let Some(path) = output {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut bytes = serde_json::to_vec_pretty(&plan)?;
        bytes.push(b'\n');
        std::fs::write(&path, bytes).with_context(|| format!("cannot write {}", path.display()))?;
        if !json {
            println!("计划已写入：{}", path.display());
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        println!("{}", plan.to_markdown());
    }
    Ok(())
}

fn run_master_ready(
    trace_id: String,
    state_root: PathBuf,
    runtime_profiles: PathBuf,
    workshop_prefix: String,
    no_gui: bool,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let store = MasterStore::new(&state_root, &trace_id)?;
    let mut state = store.load()?;
    if !state.plan.approved {
        bail!("master plan is not approved; run `master approve` first");
    }
    if state.paused {
        bail!("master run is paused");
    }
    let profiles = load_agent_profiles(&runtime_profiles)?;
    let adapter = WorkshopCliAdapter::new(workshop_prefix, Duration::from_secs(30), !no_gui)?
        .with_goal_context(state.trace_id.clone(), state.plan.goal_id.clone());
    let profiles = if dry_run {
        profiles
    } else {
        adapter.merge_profiles(&profiles)?
    };
    state.advance_ready(now_unix_ms());
    let ready = state
        .plan
        .work_items
        .iter()
        .filter(|item| {
            state.tasks.get(&item.id).is_some_and(|task| {
                matches!(
                    task.status,
                    WorkItemStatus::Ready | WorkItemStatus::Retrying
                ) && !task.human_takeover
            })
        })
        .take(state.plan.max_parallel.max(1))
        .cloned()
        .collect::<Vec<_>>();
    if ready.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "dispatched": [],
                    "state": state,
                }))?
            );
        } else {
            println!("没有可派发的 Ready 任务。");
        }
        return Ok(());
    }
    if dry_run {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "dry_run": true,
                    "ready": ready,
                    "state": state,
                }))?
            );
        } else {
            for item in ready {
                let assignment = state
                    .assignment
                    .assignments
                    .iter()
                    .find(|assignment| assignment.task_id == item.id)
                    .context("ready task has no assignment")?;
                println!(
                    "{} -> {} ({})",
                    item.id, assignment.agent_id, assignment.selection_reason
                );
            }
        }
        return Ok(());
    }

    let mut leased = Vec::new();
    for item in ready {
        let assignment = state
            .assignment
            .assignments
            .iter()
            .find(|assignment| assignment.task_id == item.id)
            .context("ready task has no assignment")?
            .clone();
        let profile = profiles
            .iter()
            .find(|profile| profile.agent_id == assignment.agent_id)
            .with_context(|| format!("missing runtime profile for {}", assignment.agent_id))?
            .clone();
        let lease = state.lease_task(
            &item.id,
            &profile,
            format!("workshop:{}", assignment.agent_id),
            item.budget.timeout_ms.saturating_add(30_000),
            now_unix_ms(),
        )?;
        state = store.append(
            state,
            MasterJournalEventKind::Leased,
            Some(item.id.clone()),
            "master",
            format!("lease generation {}", lease.generation),
            now_unix_ms(),
        )?;
        state.begin_starting(&item.id, lease.generation, now_unix_ms())?;
        state = store.append(
            state,
            MasterJournalEventKind::Starting,
            Some(item.id.clone()),
            "master",
            format!("lease generation {}", lease.generation),
            now_unix_ms(),
        )?;
        leased.push((item, profile, lease));
    }

    let adapter_for_threads = adapter.clone();
    let dispatched = std::thread::scope(|scope| {
        let handles = leased
            .into_iter()
            .map(|(item, profile, lease)| {
                let adapter = adapter_for_threads.clone();
                scope.spawn(move || {
                    let dispatched_at = now_unix_ms();
                    (
                        item.clone(),
                        profile.clone(),
                        lease.clone(),
                        adapter.dispatch(
                            &item,
                            &profile,
                            lease.generation,
                            &lease.idempotency_key,
                            dispatched_at,
                        ),
                    )
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("a Workshop dispatch worker panicked"))
            })
            .collect::<Result<Vec<_>>>()
    })?;

    let mut receipts = Vec::new();
    for (item, agent, lease, result) in dispatched {
        let receipt = match result {
            Ok(receipt) => receipt,
            Err(error) => ExecutionReceipt {
                task_id: item.id.clone(),
                agent_id: agent.agent_id.clone(),
                lease_generation: lease.generation,
                idempotency_key: lease.idempotency_key.clone(),
                execution_target: Default::default(),
                status: ExecutionStatus::Failed,
                exit_code: None,
                artifacts: Vec::new(),
                result_digest: None,
                error: Some(error.to_string()),
                started_unix_ms: now_unix_ms(),
                finished_unix_ms: now_unix_ms(),
            },
        };
        state.record_receipt(receipt.clone(), now_unix_ms())?;
        state = store.append(
            state,
            MasterJournalEventKind::ReceiptRecorded,
            Some(item.id.clone()),
            "workshop",
            format!(
                "execution {:?}, session {:?}",
                receipt.status, receipt.execution_target.session_id
            ),
            now_unix_ms(),
        )?;
        receipts.push(receipt);
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "receipts": receipts,
                "state": state,
            }))?
        );
    } else {
        for receipt in receipts {
            println!(
                "{} · {:?} · agent={} · session={}",
                receipt.task_id,
                receipt.status,
                receipt.agent_id,
                receipt
                    .execution_target
                    .session_id
                    .as_deref()
                    .unwrap_or("-")
            );
            if let Some(error) = receipt.error {
                println!("  错误：{error}");
            }
        }
        println!("下一步：运行 `master verify` 提交独立验收证据。");
    }
    Ok(())
}

fn run_master_start(
    trace_id: String,
    state_root: PathBuf,
    runtime_profiles: PathBuf,
    workshop_prefix: String,
    no_gui: bool,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let store = MasterStore::new(&state_root, &trace_id)?;
    let mut state = store.load()?;
    if !state.plan.approved {
        bail!("master plan is not approved; run `master approve` first");
    }
    if state.paused {
        bail!("master run is paused");
    }
    let profiles = load_agent_profiles(&runtime_profiles)?;
    let adapter = WorkshopCliAdapter::new(workshop_prefix, Duration::from_secs(30), !no_gui)?
        .with_artifact_dir(store.trace_dir().join("executions"))
        .with_goal_context(state.trace_id.clone(), state.plan.goal_id.clone());
    let profiles = adapter.merge_profiles(&profiles)?;
    state.advance_ready(now_unix_ms());
    let ready = state
        .plan
        .work_items
        .iter()
        .filter(|item| {
            state.tasks.get(&item.id).is_some_and(|task| {
                matches!(
                    task.status,
                    WorkItemStatus::Ready | WorkItemStatus::Retrying
                ) && !task.human_takeover
            })
        })
        .take(state.plan.max_parallel.max(1))
        .cloned()
        .collect::<Vec<_>>();
    if ready.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "submitted": [],
                    "state": state,
                }))?
            );
        } else {
            println!("没有可启动的 Ready 任务。");
        }
        return Ok(());
    }
    if dry_run {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "dry_run": true,
                    "ready": ready,
                    "state": state,
                }))?
            );
        } else {
            for item in ready {
                println!("{} · {}", item.id, item.title);
            }
        }
        return Ok(());
    }

    let mut submitted = Vec::new();
    for item in ready {
        let assignment = state
            .assignment
            .assignments
            .iter()
            .find(|assignment| assignment.task_id == item.id)
            .context("ready task has no assignment")?
            .clone();
        let profile = profiles
            .iter()
            .find(|profile| profile.agent_id == assignment.agent_id)
            .with_context(|| format!("missing runtime profile for {}", assignment.agent_id))?
            .clone();
        let lease = state.lease_task(
            &item.id,
            &profile,
            format!("workshop-session:{}", assignment.agent_id),
            item.budget.timeout_ms.saturating_add(30_000),
            now_unix_ms(),
        )?;
        state = store.append(
            state,
            MasterJournalEventKind::Leased,
            Some(item.id.clone()),
            "master",
            format!("lease generation {}", lease.generation),
            now_unix_ms(),
        )?;
        state.begin_starting(&item.id, lease.generation, now_unix_ms())?;
        state = store.append(
            state,
            MasterJournalEventKind::Starting,
            Some(item.id.clone()),
            "master",
            "starting Workshop session",
            now_unix_ms(),
        )?;
        match adapter.start(
            &item,
            &profile,
            lease.generation,
            &lease.idempotency_key,
            now_unix_ms(),
        ) {
            Ok(submission) => {
                state.record_submission(submission.clone(), now_unix_ms())?;
                state = store.append(
                    state,
                    MasterJournalEventKind::Submitted,
                    Some(item.id.clone()),
                    &profile.agent_id,
                    format!(
                        "submitted to session {}",
                        submission
                            .execution_target
                            .session_id
                            .as_deref()
                            .unwrap_or("-")
                    ),
                    now_unix_ms(),
                )?;
                submitted.push(submission);
            }
            Err(error) => {
                let receipt = failed_receipt(&item, &profile, &lease, error.to_string());
                state.record_receipt(receipt.clone(), now_unix_ms())?;
                state = store.append(
                    state,
                    MasterJournalEventKind::ReceiptRecorded,
                    Some(item.id),
                    "workshop",
                    receipt.error.unwrap_or_else(|| "start failed".into()),
                    now_unix_ms(),
                )?;
            }
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "submitted": submitted,
                "state": state,
            }))?
        );
    } else {
        for submission in submitted {
            println!(
                "{} · submitted · agent={} · session={}",
                submission.task_id,
                submission.agent_id,
                submission
                    .execution_target
                    .session_id
                    .as_deref()
                    .unwrap_or("-")
            );
        }
        println!("下一步：运行 `master poll` 推进督战状态。");
    }
    Ok(())
}

fn run_master_poll(
    trace_id: String,
    state_root: PathBuf,
    workshop_prefix: String,
    json: bool,
) -> Result<()> {
    let store = MasterStore::new(&state_root, &trace_id)?;
    let mut state = store.load()?;
    let adapter = WorkshopCliAdapter::new(workshop_prefix, Duration::from_secs(30), false)?
        .with_artifact_dir(store.trace_dir().join("executions"))
        .with_goal_context(state.trace_id.clone(), state.plan.goal_id.clone());
    let submissions = state
        .plan
        .work_items
        .iter()
        .filter_map(|item| {
            let task = state.tasks.get(&item.id)?;
            if task.human_takeover
                || !matches!(
                    task.status,
                    WorkItemStatus::Running
                        | WorkItemStatus::AwaitingInput
                        | WorkItemStatus::Blocked
                )
            {
                return None;
            }
            let lease = task.active_lease.as_ref()?;
            let target = task.execution_target.clone()?;
            Some(ExecutionSubmission {
                task_id: item.id.clone(),
                agent_id: lease.agent_id.clone(),
                lease_generation: lease.generation,
                idempotency_key: lease.idempotency_key.clone(),
                execution_target: target,
                submitted_unix_ms: task.submitted_unix_ms.unwrap_or_default(),
                deadline_unix_ms: task
                    .submitted_unix_ms
                    .unwrap_or_default()
                    .saturating_add(u128::from(item.budget.timeout_ms)),
            })
        })
        .collect::<Vec<_>>();
    let mut observed = Vec::new();
    for submission in submissions {
        let previous_status = state.tasks.get(&submission.task_id).map(|task| task.status);
        let observation = adapter
            .observe_submission(&submission, now_unix_ms())
            .unwrap_or_else(|error| ExecutionObservation::Blocked {
                reason: error.to_string(),
            });
        let terminal = matches!(observation, ExecutionObservation::Finished { .. });
        let summary = format!("{observation:?}");
        state.record_observation(&submission.task_id, observation.clone(), now_unix_ms())?;
        let next_status = state.tasks.get(&submission.task_id).map(|task| task.status);
        if terminal || previous_status != next_status {
            state = store.append(
                state,
                if terminal {
                    MasterJournalEventKind::ReceiptRecorded
                } else {
                    MasterJournalEventKind::Observed
                },
                Some(submission.task_id.clone()),
                "workshop",
                summary.clone(),
                now_unix_ms(),
            )?;
        }
        observed.push(serde_json::json!({
            "task_id": submission.task_id,
            "session_id": submission.execution_target.session_id,
            "observation": observation,
        }));
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "observed": observed,
                "state": state,
            }))?
        );
    } else if observed.is_empty() {
        println!("没有需要观察的运行中任务。");
    } else {
        for item in observed {
            println!(
                "{} · {}",
                item["task_id"].as_str().unwrap_or("-"),
                item["observation"]["kind"].as_str().unwrap_or("unknown")
            );
        }
    }
    Ok(())
}

fn failed_receipt(
    item: &first_principles_agent::WorkItem,
    agent: &AgentProfile,
    lease: &first_principles_agent::TaskLease,
    error: String,
) -> ExecutionReceipt {
    ExecutionReceipt {
        task_id: item.id.clone(),
        agent_id: agent.agent_id.clone(),
        lease_generation: lease.generation,
        idempotency_key: lease.idempotency_key.clone(),
        execution_target: ExecutionTarget::default(),
        status: ExecutionStatus::Failed,
        exit_code: None,
        artifacts: Vec::new(),
        result_digest: None,
        error: Some(error),
        started_unix_ms: now_unix_ms(),
        finished_unix_ms: now_unix_ms(),
    }
}

fn cancel_active_session_if_any(
    state: &MasterState,
    task_id: &str,
    workshop_prefix: &str,
) -> Result<()> {
    let target = state.tasks.get(task_id).and_then(|task| {
        task.active_lease
            .as_ref()
            .and(task.execution_target.as_ref())
    });
    if let Some(session_id) = target.and_then(|target| target.session_id.as_deref()) {
        WorkshopCliAdapter::new(workshop_prefix, Duration::from_secs(30), false)?
            .cancel_session(session_id)?;
    }
    Ok(())
}

fn terminate_expired_session_if_any(
    state: &MasterState,
    task_id: &str,
    workshop_prefix: &str,
) -> Result<()> {
    let session_id = state
        .tasks
        .get(task_id)
        .and_then(|task| {
            task.active_lease
                .as_ref()
                .and(task.execution_target.as_ref())
        })
        .and_then(|target| target.session_id.clone());
    let Some(session_id) = session_id else {
        return Ok(());
    };
    match WorkshopCliAdapter::new(workshop_prefix, Duration::from_secs(30), false)?
        .cancel_session(&session_id)
    {
        Ok(()) => Ok(()),
        Err(error)
            if error.to_string().contains("session_not_found")
                || error.to_string().contains("session_exited") =>
        {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationSubmission {
    task_id: String,
    criteria_results: Vec<first_principles_agent::CriterionResult>,
    #[serde(default)]
    test_results: Vec<first_principles_agent::TestResult>,
    #[serde(default)]
    evidence: Vec<String>,
    verified_by: String,
}

fn run_master_verify(
    trace_id: String,
    state_root: PathBuf,
    input: PathBuf,
    json: bool,
) -> Result<()> {
    let store = MasterStore::new(&state_root, &trace_id)?;
    let (result, state) = verify_master_task(&store, &input, None).map_err(anyhow::Error::msg)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "verification": result,
                "state": state,
            }))?
        );
    } else {
        println!("验收：{:?}", result.status);
        if let Some(reason) = result.failure_reason {
            println!("原因：{reason}");
        }
    }
    Ok(())
}

fn verify_master_task(
    store: &MasterStore,
    input: &std::path::Path,
    expected_task_id: Option<&str>,
) -> std::result::Result<(first_principles_agent::VerificationResult, MasterState), String> {
    let mut state = store.load().map_err(|error| error.to_string())?;
    let submission: VerificationSubmission =
        read_json_file(input, "verification evidence").map_err(|error| error.to_string())?;
    if expected_task_id.is_some_and(|task_id| task_id != submission.task_id) {
        return Err(format!(
            "验收文件属于任务 {}，不是指定任务 {}",
            submission.task_id,
            expected_task_id.unwrap_or("-")
        ));
    }
    let item = state
        .plan
        .work_items
        .iter()
        .find(|item| item.id == submission.task_id)
        .ok_or_else(|| format!("未知任务：{}", submission.task_id))?
        .clone();
    let assigned_agent = state
        .assignment
        .assignments
        .iter()
        .find(|assignment| assignment.task_id == submission.task_id)
        .map(|assignment| assignment.agent_id.as_str())
        .ok_or_else(|| format!("任务 {} 没有 assignment", submission.task_id))?;
    if submission.verified_by.trim() == assigned_agent {
        return Err("验收人不能与执行任务的 Agent 相同。".into());
    }
    let expectations = item
        .expected_artifacts
        .iter()
        .map(|artifact| artifact_expectation(artifact))
        .collect::<Vec<_>>();
    let artifact_checks =
        inspect_artifacts(&item.workspace, &expectations).map_err(|error| error.to_string())?;
    let task = state
        .tasks
        .get(&submission.task_id)
        .ok_or_else(|| "任务状态缺失。".to_string())?;
    let verified_by = submission.verified_by;
    let now = now_unix_ms();
    let result = state
        .verify(
            VerificationInput {
                task_id: submission.task_id.clone(),
                lease_generation: task.generation,
                criteria_results: submission.criteria_results,
                artifact_checks,
                test_results: submission.test_results,
                evidence: submission.evidence,
                verified_by: verified_by.clone(),
                verified_at_unix_ms: now,
            },
            now,
        )
        .map_err(|error| error.to_string())?;
    let kind = if result.status == WorkItemStatus::Verified {
        MasterJournalEventKind::Verified
    } else {
        MasterJournalEventKind::Rejected
    };
    state = store
        .append(
            state,
            kind,
            Some(submission.task_id.clone()),
            &verified_by,
            result
                .failure_reason
                .clone()
                .unwrap_or_else(|| "independent verification passed".into()),
            now,
        )
        .map_err(|error| error.to_string())?;
    Ok((result, state))
}

fn print_master_state(state: &MasterState, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(state)?);
        return Ok(());
    }
    println!(
        "{} · approved={} · rejected={} · paused={} · retries={}/1 · replans={}/1",
        state.trace_id,
        yes_no(state.plan.approved),
        yes_no(state.plan_rejected),
        yes_no(state.paused),
        state.retry_count,
        state.replan_count
    );
    for item in &state.plan.work_items {
        let task = state.tasks.get(&item.id).context("task state is missing")?;
        let assignment = state
            .assignment
            .assignments
            .iter()
            .find(|assignment| assignment.task_id == item.id);
        println!(
            "{} · {:?} · agent={} · attempts={}",
            item.id,
            task.status,
            assignment
                .map(|value| value.agent_id.as_str())
                .unwrap_or("-"),
            task.attempts
        );
    }
    Ok(())
}

fn read_json_file<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    label: &str,
) -> Result<T> {
    serde_json::from_slice(
        &std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?,
    )
    .with_context(|| format!("invalid {label} JSON in {}", path.display()))
}

fn parse_risk(value: &str) -> Result<RiskLevel> {
    match value {
        "low" => Ok(RiskLevel::Low),
        "medium" => Ok(RiskLevel::Medium),
        "high" => Ok(RiskLevel::High),
        other => bail!("invalid risk '{other}'; expected low, medium or high"),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "是" } else { "否" }
}

fn record_reframe_decision(
    proposal: &ReframeProposal,
    choice: &ReframeChoice,
    provider_id: &str,
    model: &str,
    prompt_sha256: &str,
    journal_dir: &std::path::Path,
    decided_at_unix_ms: u128,
) -> Result<(Uuid, PathBuf)> {
    let run_id = Uuid::new_v4();
    let journal_path = journal_dir.join(format!("{run_id}.jsonl"));
    let mut journal = Journal::create(&journal_path, run_id)?;
    journal.append(JournalEventKind::RunStarted {
        request: TaskRequest::from_objective(proposal.stated_request.clone(), PathBuf::from(".")),
        mode: CognitiveMode::DirectAnswer,
    })?;
    journal.append(JournalEventKind::ReframeProposed {
        proposal: proposal.clone(),
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        prompt_sha256: prompt_sha256.to_string(),
    })?;
    journal.append(JournalEventKind::ReframeDecided {
        choice: choice.clone(),
        actor: "user".into(),
        decided_at_unix_ms,
    })?;
    journal.append(JournalEventKind::AnswerProduced {
        answer: AgentAnswer {
            schema_version: first_principles_agent::ANSWER_SCHEMA_VERSION,
            status: AnswerStatus::Completed,
            conclusion: format!("已记录：{}", choice.label_zh()),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            conflicts: Vec::new(),
            flip_conditions: Vec::new(),
            action_receipts: Vec::new(),
        },
    })?;
    journal.append(JournalEventKind::RunStopped {
        reason: StopReason::Completed,
    })?;
    Ok((run_id, journal_path))
}

fn persist_pending(proposal: &ReframeProposal, journal_dir: &std::path::Path) -> Result<PathBuf> {
    let dir = journal_dir.join("pending");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", Uuid::new_v4()));
    std::fs::write(&path, serde_json::to_vec_pretty(proposal)?)?;
    Ok(path)
}

fn render_choice_menu(proposal: &ReframeProposal, source: &str) -> String {
    let mut out = String::new();
    out.push_str("需要你选一个\n");
    out.push_str(&format!("  原请求   {}\n", proposal.stated_request));
    if let Some(question) = &proposal.question {
        out.push_str(&format!("  追问     {question}\n"));
    }
    out.push('\n');
    for (index, option) in proposal.options.iter().enumerate() {
        let number = index + 1;
        out.push_str(&format!(
            "  [{number}] 采纳「{option}」  → reframe {source} --accept {number}\n"
        ));
    }
    out.push_str(&format!(
        "  [{}] 都不是，按原方案  → reframe {source} --reject prefer_literal\n",
        proposal.options.len() + 1
    ));
    out
}

fn parse_rejected_reason(value: &str) -> Result<RejectedReason> {
    match value {
        "cost_transferred" => Ok(RejectedReason::CostTransferred),
        "constraint_unmet" => Ok(RejectedReason::ConstraintUnmet),
        "prefer_literal" => Ok(RejectedReason::PreferLiteral),
        "unverifiable" => Ok(RejectedReason::Unverifiable),
        "other" => Ok(RejectedReason::Other),
        _ => bail!(
            "unknown rejection reason '{value}'; use cost_transferred, constraint_unmet, \
             prefer_literal, unverifiable or other"
        ),
    }
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn tui_provider_attempt_budget(configured: Option<usize>, max_steps: usize) -> usize {
    configured.unwrap_or(max_steps).clamp(1, 16)
}

struct TuiRunSettings {
    cwd: PathBuf,
    roots: Vec<PathBuf>,
    allow_network: bool,
    allow_write: bool,
    experience_scope: Option<String>,
    max_steps: usize,
    max_tool_calls: usize,
    max_provider_attempts: usize,
    master_agents: Option<PathBuf>,
    workshop_prefix: String,
    timeout_seconds: u64,
    max_context_chars: usize,
    auto_compact_percent: u8,
    default_preferences: SessionPreferences,
    mentions: Vec<PathBuf>,
    planning_provider: Arc<dyn ModelProvider>,
}

impl TuiRunSettings {
    fn request(&self, instruction: &str) -> TaskRequest {
        self.request_with_known_facts(instruction, Vec::new())
    }

    fn request_with_known_facts(
        &self,
        instruction: &str,
        mut known_facts: Vec<String>,
    ) -> TaskRequest {
        let mut request = TaskRequest::from_objective(instruction, self.cwd.clone());
        known_facts.extend(
            self.mentions
                .iter()
                .map(|path| format!("Mentioned workspace path: {}", path.display())),
        );
        request.known_facts = known_facts;
        request.permissions.allow_network = self.allow_network;
        request.permissions.allow_write = self.allow_write;
        request.permissions.read_roots = self.roots.clone();
        request.experience_scope = self.experience_scope.clone();
        request.budget = RunBudget {
            max_steps: self.max_steps,
            max_tool_calls: self.max_tool_calls,
            max_context_chars: self.max_context_chars,
            auto_compact_percent: self.auto_compact_percent,
            max_provider_attempts: self.max_provider_attempts,
            timeout_ms: self.timeout_seconds.saturating_mul(1_000),
            ..RunBudget::default()
        };
        request
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_tui(
    config_path: Option<PathBuf>,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    allow_network: bool,
    allow_write: bool,
    read_root: Vec<PathBuf>,
    journal_dir: PathBuf,
    session: Option<String>,
    experience_dir: Option<PathBuf>,
    experience_scope: Option<String>,
    max_steps: usize,
    max_tool_calls: usize,
    max_provider_attempts: Option<usize>,
    master_agents: Option<PathBuf>,
    workshop_prefix: String,
    timeout_seconds: u64,
    max_context_chars: usize,
    route_objective: Option<String>,
) -> Result<()> {
    let host_binding = HostBinding::from_env();
    let mut session = TuiSession::open(&journal_dir, session.as_deref())
        .context("cannot initialize TUI session")?;
    session.record_event(SessionEvent::Opened {
        host_kind: host_binding.kind().as_str().into(),
        agent_id: host_binding.agent_id().map(str::to_string),
    })?;
    let cli_preferences = SessionPreferences {
        max_steps: max_steps.max(1),
        max_tool_calls: max_tool_calls.max(1),
        max_provider_attempts: tui_provider_attempt_budget(max_provider_attempts, max_steps),
        max_context_chars,
        auto_compact_percent: 80,
        timeout_seconds,
    };
    let preferences = session.preferences().cloned();
    let max_steps = preferences
        .as_ref()
        .map(|preferences| preferences.max_steps)
        .unwrap_or(max_steps);
    let max_tool_calls = preferences
        .as_ref()
        .map(|preferences| preferences.max_tool_calls)
        .unwrap_or(max_tool_calls);
    let max_provider_attempts = preferences
        .as_ref()
        .map(|preferences| Some(preferences.max_provider_attempts))
        .unwrap_or(max_provider_attempts);
    let max_context_chars = preferences
        .as_ref()
        .map(|preferences| preferences.max_context_chars)
        .unwrap_or(max_context_chars);
    let auto_compact_percent = preferences
        .as_ref()
        .map(|preferences| preferences.auto_compact_percent)
        .unwrap_or(80);
    let timeout_seconds = preferences
        .as_ref()
        .map(|preferences| preferences.timeout_seconds)
        .unwrap_or(timeout_seconds);
    let timeout = Duration::from_secs(timeout_seconds.max(1));
    let (provider_label, candidates, tools) = if let Some(config_path) = config_path {
        let config = AgentConfig::load(&config_path)?;
        let label = config
            .manifests()?
            .first()
            .map(|manifest| format!("{}/{}", manifest.provider_id, manifest.model))
            .unwrap_or_else(|| "configured".to_string());
        (label, config.build_providers(timeout)?, default_registry())
    } else {
        let label = format!("{provider}/{}", model.as_deref().unwrap_or("default"));
        let (candidates, tools) =
            build_runtime_parts(&provider, model, base_url, api_key, allow_network, timeout)?;
        (label, candidates, tools)
    };
    let planning_provider = candidates
        .first()
        .map(|candidate| candidate.provider.clone())
        .context("provider list is empty")?;
    let (objective, objective_source) = resolve_route_objective(route_objective.as_deref())?;
    let mut runtime = AgentRuntime::new(candidates, tools, journal_dir)
        .with_route_objective(objective)
        .with_route_policy_sources(RoutePolicySources {
            objective: objective_source,
            latency_budget: PolicyValueSource::Unset,
            unit_cost_budget: PolicyValueSource::Unset,
        });
    if let Some(experience_dir) = experience_dir {
        runtime = runtime.with_experience_dir(experience_dir);
    }

    let cwd = session
        .workspace()
        .and_then(|workspace| std::fs::canonicalize(workspace).ok())
        .filter(|workspace| workspace.is_dir())
        .map(Ok)
        .unwrap_or_else(|| std::env::current_dir().context("cannot determine current directory"))?;
    let roots = if read_root.is_empty() {
        vec![cwd.clone()]
    } else {
        read_root
            .iter()
            .map(canonical_or_original)
            .collect::<Vec<_>>()
    };
    let mut state = TuiState::new(provider_label, max_steps, timeout_seconds.max(1));
    state.set_context_budget(max_context_chars);
    state.set_host_binding(host_binding);
    sync_tui_stage(&mut state, &session);
    let mut settings = TuiRunSettings {
        cwd,
        roots,
        allow_network,
        allow_write,
        experience_scope,
        max_steps,
        max_tool_calls,
        max_provider_attempts: tui_provider_attempt_budget(max_provider_attempts, max_steps),
        master_agents,
        workshop_prefix,
        timeout_seconds,
        max_context_chars,
        auto_compact_percent,
        default_preferences: cli_preferences,
        mentions: session.mentions().to_vec(),
        planning_provider,
    };
    if interactive_terminal_available() {
        run_tui_interactive(runtime, state, settings, session).await
    } else {
        run_tui_line_mode(&mut runtime, &mut state, &mut settings, &mut session).await
    }
}

async fn run_tui_line_mode(
    runtime: &mut AgentRuntime,
    state: &mut TuiState,
    settings: &mut TuiRunSettings,
    session: &mut TuiSession,
) -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    state.info("Type /help for commands. Ctrl+C cancels the active run.");

    loop {
        let (columns, rows) = terminal_dimensions();
        stdout
            .write_all(state.render_screen(columns, rows).as_bytes())
            .await?;
        stdout.flush().await?;
        let Some(line) = lines.next_line().await? else {
            break;
        };
        let instruction = line.trim();
        if instruction.is_empty() {
            continue;
        }
        let resolution = resolve_input(
            instruction,
            state.host_binding(),
            state.command_stage(),
            false,
        );
        match resolution {
            TuiInputResolution::Command { spec, args } => {
                if let Err(error) = session.record_event(SessionEvent::CommandAccepted {
                    name: spec.name.into(),
                    scope: spec.scope.as_str().into(),
                }) {
                    state.info(format!("无法写入 session event log：{error}"));
                    continue;
                }
                if spec.id.uses_command_panel() {
                    state.begin_command_panel(spec.name);
                    state.set_command_panel_menu(command_menu_actions(spec.id));
                } else {
                    state.close_command_panel();
                }
                match spec.id {
                    TuiCommandId::Quit => {
                        state.info("Session ended.");
                        let (columns, rows) = terminal_dimensions();
                        stdout
                            .write_all(state.render_screen(columns, rows).as_bytes())
                            .await?;
                        stdout.flush().await?;
                        break;
                    }
                    TuiCommandId::Help => {
                        display_tui_help_menu(state);
                        continue;
                    }
                    TuiCommandId::Reframe => {
                        run_tui_reframe(runtime, state, args).await?;
                        continue;
                    }
                    TuiCommandId::Status => {
                        display_tui_status(state, session, settings);
                        continue;
                    }
                    TuiCommandId::Clear => {
                        state.clear();
                        state.info("Conversation cleared.");
                        continue;
                    }
                    TuiCommandId::Details => {
                        state.open_details();
                        continue;
                    }
                    TuiCommandId::Journal => {
                        display_journal_paths(state, session);
                        continue;
                    }
                    TuiCommandId::Evidence => {
                        display_evidence(state, session, args);
                        continue;
                    }
                    TuiCommandId::Audit => {
                        display_audit(state, session, args);
                        continue;
                    }
                    TuiCommandId::Report => {
                        display_report(state, session, args);
                        continue;
                    }
                    TuiCommandId::Frame
                    | TuiCommandId::Challenge
                    | TuiCommandId::Decide
                    | TuiCommandId::Postmortem => {
                        run_tui_line_epistemic(runtime, state, settings, session, spec.id, args)
                            .await?;
                        continue;
                    }
                    TuiCommandId::Mission => {
                        handle_mission_command(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Triage => {
                        display_triage(state, session, args);
                        continue;
                    }
                    TuiCommandId::Plan => {
                        run_tui_master_plan(state, session, settings, args).await;
                        continue;
                    }
                    TuiCommandId::Approve => {
                        approve_tui_plan(state, session, args);
                        continue;
                    }
                    TuiCommandId::Mode => {
                        state.info("对话模式：自动 / 直接回答 / 证据研究 / 决策分析 / 执行任务 / 诊断分析。交互 TUI 使用 Ctrl+T 或 Ctrl+Tab 切换。");
                        continue;
                    }
                    TuiCommandId::Model => {
                        handle_model_command(state, runtime, settings, args);
                        continue;
                    }
                    TuiCommandId::Permissions => {
                        handle_permissions_command(settings, state, args);
                        continue;
                    }
                    TuiCommandId::Tools => {
                        display_tools(runtime, state, args);
                        continue;
                    }
                    TuiCommandId::Agents => {
                        display_agents(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Assign => {
                        assign_tui_plan(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Dispatch => {
                        run_tui_dispatch(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Board => {
                        display_master_board(state, session, args);
                        continue;
                    }
                    TuiCommandId::Watch => {
                        watch_tui_master(state, session, settings);
                        continue;
                    }
                    TuiCommandId::Attention => {
                        display_attention(state, session);
                        continue;
                    }
                    TuiCommandId::Respond => {
                        respond_tui_task(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Control => {
                        control_tui_master(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Timeline => {
                        display_master_timeline(state, session, settings);
                        continue;
                    }
                    TuiCommandId::Artifacts => {
                        display_master_artifacts(state, session, args);
                        continue;
                    }
                    TuiCommandId::Verify => {
                        verify_tui_task(state, session, args);
                        continue;
                    }
                    TuiCommandId::Session => {
                        handle_session_command(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Workspace => {
                        handle_workspace_command(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Context => {
                        handle_context_command(state, session, settings, args);
                        continue;
                    }
                    TuiCommandId::Config => {
                        handle_config_command(state, settings, session, args);
                        continue;
                    }
                    TuiCommandId::Export => {
                        handle_export_command(state, session, args);
                        continue;
                    }
                    TuiCommandId::Copy => {
                        handle_copy_command(state);
                        continue;
                    }
                    TuiCommandId::Cost => {
                        handle_cost_command(state, session);
                        continue;
                    }
                    TuiCommandId::Preset => {
                        if let Some(request) = handle_preset_command(state, session, settings, args)
                        {
                            run_tui_line_request(runtime, state, session, &mut stdout, request)
                                .await?;
                        }
                        continue;
                    }
                }
            }
            TuiInputResolution::UnavailableMaster(spec) => {
                record_rejected_command(
                    state,
                    session,
                    spec.name,
                    "requires Workshop Master context",
                );
                state.info(format!(
                    "{} 仅在 Skynet Workshop Master-Agent 上下文可用。",
                    spec.name
                ));
                continue;
            }
            TuiInputResolution::UnavailableInStage(spec) => {
                record_rejected_command(
                    state,
                    session,
                    spec.name,
                    "unavailable in current lifecycle stage",
                );
                state.info(format!("{} 不在当前生产线阶段可用。", spec.name));
                continue;
            }
            TuiInputResolution::UnavailableWhileRunning(spec) => {
                record_rejected_command(
                    state,
                    session,
                    spec.name,
                    "unavailable while a run is active",
                );
                state.info(format!("{} 无法在运行中执行。", spec.name));
                continue;
            }
            TuiInputResolution::UnknownCommand(name) => {
                record_rejected_command(state, session, name, "unknown command");
                state.info(format!("Unknown command: {name}"));
                continue;
            }
            TuiInputResolution::Task(task) => {
                if handle_master_task_intake(state, session, settings, task).await {
                    continue;
                }
            }
        }

        state.close_command_panel();
        let request = settings.request(instruction);
        run_tui_line_request(runtime, state, session, &mut stdout, request).await?;
    }
    session.record_event(SessionEvent::Closed)?;
    Ok(())
}

async fn run_tui_line_request(
    runtime: &mut AgentRuntime,
    state: &mut TuiState,
    session: &mut TuiSession,
    stdout: &mut tokio::io::Stdout,
    request: TaskRequest,
) -> Result<()> {
    let display = request.objective.clone();
    let mode = infer_mode(&request);
    let risk = risk_for_mode(mode);
    if let Err(error) = session.record_event(SessionEvent::TaskSubmitted {
        mode: mode.as_str().into(),
    }) {
        state.info(format!("无法写入 session event log：{error}"));
        return Ok(());
    }
    state.begin(&display, mode, risk);
    let (columns, rows) = terminal_dimensions();
    stdout
        .write_all(state.render_screen(columns, rows).as_bytes())
        .await?;
    stdout.flush().await?;
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancellation.cancel();
        }
    });
    let result = runtime.run(request, cancellation).await;
    signal.abort();
    match result {
        Ok(result) => {
            session.set_title_from_prompt(&state.last_instruction);
            session.record_journal(&result.journal_path)?;
            state.set_last_journal(&result.journal_path);
            let output = format!(
                "{}\n\nRun journal: {}",
                render_answer(&result),
                result.journal_path.display()
            );
            state.finish(TuiStatus::Completed, output);
        }
        Err(RuntimeError::Cancelled { .. }) => {
            state.finish(TuiStatus::Cancelled, "Run cancelled; journal preserved.");
        }
        Err(error) => {
            state.finish(TuiStatus::Failed, format!("Error: {error}"));
        }
    }
    Ok(())
}

fn record_rejected_command(
    state: &mut TuiState,
    session: &mut TuiSession,
    name: &str,
    reason: &str,
) {
    if let Err(error) = session.record_event(SessionEvent::CommandRejected {
        name: name.into(),
        reason: reason.into(),
    }) {
        state.info(format!("无法写入 session event log：{error}"));
    }
}

fn command_menu_action(
    label: &str,
    description: &str,
    command_line: &str,
    needs_input: bool,
) -> CommandMenuItem {
    CommandMenuItem {
        id: format!(
            "{}:{command_line}",
            if needs_input { "prompt" } else { "open" }
        ),
        label: label.into(),
        description: description.into(),
    }
}

fn command_menu_actions(command: TuiCommandId) -> Vec<CommandMenuItem> {
    match command {
        TuiCommandId::Help => Vec::new(),
        TuiCommandId::Status => vec![
            command_menu_action("刷新状态", "重新读取当前运行状态", "/status", false),
            command_menu_action("上下文", "查看上下文占用与压缩", "/context show", false),
            command_menu_action("模型", "查看模型路由和候选模型", "/model show", false),
            command_menu_action("运行权限", "查看下一次运行权限", "/permissions show", false),
            command_menu_action("成本", "查看 Token 与成本汇总", "/cost", false),
        ],
        TuiCommandId::Details => Vec::new(),
        TuiCommandId::Mode => Vec::new(),
        TuiCommandId::Journal => vec![
            command_menu_action("刷新日志路径", "重新显示最近的日志", "/journal", false),
            command_menu_action("运行状态", "查看会话和运行状态", "/status", false),
            command_menu_action("成本", "查看 Token 与成本汇总", "/cost", false),
        ],
        TuiCommandId::Clear | TuiCommandId::Quit => Vec::new(),
        TuiCommandId::Frame => Vec::new(),
        TuiCommandId::Challenge => Vec::new(),
        TuiCommandId::Decide => Vec::new(),
        TuiCommandId::Postmortem => Vec::new(),
        TuiCommandId::Evidence => vec![
            command_menu_action("最近运行", "查看最近一次回答的证据", "/evidence run", false),
            command_menu_action("总控验收", "查看生产线验收证据", "/evidence master", false),
        ],
        TuiCommandId::Audit => vec![
            command_menu_action("最近运行", "审计最近一次运行", "/audit run", false),
            command_menu_action("全部运行", "审计会话内全部运行", "/audit all", false),
        ],
        TuiCommandId::Report => vec![
            command_menu_action("最近运行", "生成最近一次运行报告", "/report run", false),
            command_menu_action("生产线", "生成总控生产线报告", "/report master", false),
        ],
        TuiCommandId::Model => vec![
            command_menu_action("当前模型", "查看当前选择和可用模型", "/model show", false),
            command_menu_action(
                "质量优先",
                "优先选择质量评分",
                "/model objective quality",
                false,
            ),
            command_menu_action(
                "均衡",
                "平衡质量、成本和延迟",
                "/model objective balanced",
                false,
            ),
            command_menu_action(
                "延迟优先",
                "优先选择低延迟模型",
                "/model objective latency",
                false,
            ),
            command_menu_action(
                "成本优先",
                "优先选择低成本模型",
                "/model objective cost",
                false,
            ),
            command_menu_action("指定模型…", "输入 provider/model", "/model use ", true),
        ],
        TuiCommandId::Permissions => vec![
            command_menu_action("当前权限", "查看下一次运行权限", "/permissions show", false),
            command_menu_action(
                "允许网络",
                "为下一次运行开启网络",
                "/permissions network on",
                false,
            ),
            command_menu_action(
                "禁止网络",
                "为下一次运行关闭网络",
                "/permissions network off",
                false,
            ),
            command_menu_action(
                "允许写入",
                "为下一次运行开启写入和执行",
                "/permissions write on",
                false,
            ),
            command_menu_action(
                "禁止写入",
                "为下一次运行关闭写入和执行",
                "/permissions write off",
                false,
            ),
            command_menu_action(
                "添加目录…",
                "输入允许读取的目录",
                "/permissions root-add ",
                true,
            ),
        ],
        TuiCommandId::Session => vec![
            command_menu_action("当前会话", "查看当前会话详情", "/session show", false),
            command_menu_action("会话列表", "列出使用中的会话", "/session list", false),
            command_menu_action("全部会话", "列出包括归档的会话", "/session list all", false),
            command_menu_action("新建…", "输入新会话标题", "/session new ", true),
            command_menu_action("恢复…", "输入要恢复的会话 ID", "/session resume ", true),
            command_menu_action("复制…", "输入新副本的标题", "/session fork ", true),
            command_menu_action("重命名…", "输入新的会话标题", "/session rename ", true),
            command_menu_action("归档当前", "归档当前会话", "/session archive", false),
            command_menu_action(
                "删除…",
                "输入会话 ID 并完成两次确认",
                "/session delete ",
                true,
            ),
        ],
        TuiCommandId::Workspace => Vec::new(),
        TuiCommandId::Context => vec![
            command_menu_action("使用情况", "查看上下文预算和占用", "/context show", false),
            command_menu_action("生成回顾", "总结最近的已完成运行", "/context recap", false),
            command_menu_action(
                "压缩上下文",
                "保存本地确定性摘要",
                "/context compact",
                false,
            ),
        ],
        TuiCommandId::Preset => vec![
            command_menu_action("预设列表", "列出已保存的预设", "/preset list", false),
            command_menu_action("运行…", "输入要运行的预设名称", "/preset run ", true),
            command_menu_action("保存…", "输入名称和指令", "/preset save ", true),
            command_menu_action("查看…", "输入预设名称", "/preset show ", true),
            command_menu_action("删除…", "输入预设名称", "/preset delete ", true),
        ],
        TuiCommandId::Tools => vec![
            command_menu_action("工具列表", "列出运行时可用工具", "/tools", false),
            command_menu_action("查看工具…", "输入工具名称", "/tools inspect ", true),
        ],
        TuiCommandId::Export => vec![
            command_menu_action("会话", "导出当前会话", "/export session", false),
            command_menu_action("运行报告", "导出最近运行报告", "/export report", false),
            command_menu_action("证据", "导出最近运行证据", "/export evidence", false),
        ],
        TuiCommandId::Copy => vec![
            command_menu_action("复制回答", "复制最近最终回答", "/copy", false),
            command_menu_action("导出报告", "将报告保存为文件", "/export report", false),
        ],
        TuiCommandId::Cost => vec![
            command_menu_action("刷新成本", "重新汇总会话 Token 和成本", "/cost", false),
            command_menu_action("上下文", "查看上下文占用", "/context show", false),
            command_menu_action("运行状态", "查看会话与模型状态", "/status", false),
            command_menu_action("导出报告", "导出最近运行报告", "/export report", false),
        ],
        TuiCommandId::Config => vec![
            command_menu_action("当前配置", "查看下次运行配置", "/config show", false),
            command_menu_action("推理步骤…", "输入最大推理步数", "/config set steps ", true),
            command_menu_action(
                "工具调用…",
                "输入最大工具调用次数",
                "/config set tools ",
                true,
            ),
            command_menu_action(
                "模型尝试…",
                "输入最大模型尝试次数",
                "/config set attempts ",
                true,
            ),
            command_menu_action(
                "上下文…",
                "输入上下文字符上限",
                "/config set context ",
                true,
            ),
            command_menu_action(
                "压缩阈值…",
                "输入 50 到 95 的阈值",
                "/config set compact ",
                true,
            ),
            command_menu_action("超时…", "输入单次超时秒数", "/config set timeout ", true),
            command_menu_action("恢复默认", "恢复启动参数", "/config reset", false),
        ],
        TuiCommandId::Mission => vec![
            command_menu_action("当前任务", "查看当前生产线任务", "/mission show", false),
            command_menu_action("新建任务…", "输入任务目标", "/mission new ", true),
            command_menu_action("修订任务…", "输入修订目标", "/mission revise ", true),
            command_menu_action("关闭任务", "关闭当前生产线任务", "/mission close", false),
        ],
        TuiCommandId::Triage => vec![
            command_menu_action(
                "分析当前任务",
                "对当前 mission 做路由判断",
                "/triage",
                false,
            ),
            command_menu_action("分析新目标…", "输入要分析的目标", "/triage ", true),
        ],
        TuiCommandId::Plan => vec![
            command_menu_action("生成计划", "为当前任务生成计划", "/plan generate", false),
            command_menu_action("查看计划", "查看当前计划", "/plan show", false),
            command_menu_action("修订计划…", "输入修订要求", "/plan revise ", true),
        ],
        TuiCommandId::Approve => {
            vec![command_menu_action(
                "批准计划",
                "批准当前已校验的计划",
                "/approve",
                false,
            )]
        }
        TuiCommandId::Agents => vec![
            command_menu_action("Agent 列表", "列出可用 Agent", "/agents", false),
            command_menu_action(
                "刷新 Agent",
                "重新发现并缓存 Agent",
                "/agents refresh",
                false,
            ),
            command_menu_action("查看 Agent…", "输入 Agent ID", "/agents inspect ", true),
        ],
        TuiCommandId::Assign => vec![
            command_menu_action("查看分配", "查看当前计划的任务分配", "/assign show", false),
            command_menu_action("执行分配", "按能力和权限分配 Agent", "/assign apply", false),
        ],
        TuiCommandId::Dispatch => vec![
            command_menu_action(
                "预演派发",
                "检查派发条件但不启动任务",
                "/dispatch dry-run",
                false,
            ),
            command_menu_action("启动派发", "启动生产线任务", "/dispatch run", false),
            command_menu_action(
                "后台派发",
                "以后台 Session 启动任务",
                "/dispatch detach",
                false,
            ),
        ],
        TuiCommandId::Board => {
            vec![command_menu_action(
                "刷新任务板",
                "重新读取任务状态",
                "/board",
                false,
            )]
        }
        TuiCommandId::Watch => {
            vec![command_menu_action(
                "刷新督战",
                "重新观察运行中的任务",
                "/watch",
                false,
            )]
        }
        TuiCommandId::Attention => vec![command_menu_action(
            "刷新待办",
            "重新检查需要人工处理的事项",
            "/attention",
            false,
        )],
        TuiCommandId::Respond => {
            vec![command_menu_action(
                "回复任务…",
                "输入任务 ID 和回复内容",
                "/respond ",
                true,
            )]
        }
        TuiCommandId::Control => vec![
            command_menu_action("暂停…", "输入任务 ID 和原因", "/control pause ", true),
            command_menu_action("恢复…", "输入任务 ID", "/control resume ", true),
            command_menu_action("取消…", "输入任务 ID 和原因", "/control cancel ", true),
            command_menu_action(
                "人工接管…",
                "输入任务 ID 和原因",
                "/control takeover ",
                true,
            ),
            command_menu_action("重试…", "输入任务 ID 和原因", "/control retry ", true),
            command_menu_action("重规划…", "输入任务 ID 和原因", "/control replan ", true),
        ],
        TuiCommandId::Artifacts => vec![
            command_menu_action("全部产物", "检查全部任务交付物", "/artifacts all", false),
            command_menu_action("指定任务…", "输入任务 ID", "/artifacts ", true),
        ],
        TuiCommandId::Verify => {
            vec![command_menu_action(
                "提交验收…",
                "输入任务 ID 和验收文件",
                "/verify ",
                true,
            )]
        }
        TuiCommandId::Reframe => vec![command_menu_action(
            "判断一个请求…",
            "输入你原本想要的东西，先看它是目的还是手段",
            "/reframe ",
            true,
        )],
        TuiCommandId::Timeline => vec![command_menu_action(
            "刷新时间线",
            "重新读取总控和 Session 事件",
            "/timeline",
            false,
        )],
    }
}

fn command_needs_input(command: TuiCommandId) -> bool {
    matches!(
        command,
        TuiCommandId::Frame
            | TuiCommandId::Challenge
            | TuiCommandId::Decide
            | TuiCommandId::Postmortem
            | TuiCommandId::Respond
            | TuiCommandId::Verify
            | TuiCommandId::Control
            | TuiCommandId::Reframe
    )
}

fn display_tui_help_menu(state: &mut TuiState) {
    state.begin_command_panel("/help");
    let commands = commands_for_surface(state.host_binding(), state.command_stage(), false);
    state.info(format!(
        "帮助\n\
         当前环境      {}\n\
         可用命令      {} 个\n\
         \n\
         使用 ↑/↓ 选择命令，Enter 执行或进入下一步。\n\
         需要目标、路径或参数的命令会先准备输入。",
        state.host_binding().kind().label_zh(),
        commands.len()
    ));
    state.set_command_panel_menu(
        commands
            .into_iter()
            .map(|command| {
                command_menu_action(
                    command.name,
                    command.description,
                    command.name,
                    command_needs_input(command.id),
                )
            })
            .collect(),
    );
}

/// Applies a reframe result to the interface. Returns true when a panel opened.
///
/// A failed attempt clears any pending proposal first: leaving it behind would
/// let a later `/reframe accept 1` decide on a proposal the user never saw.
fn apply_reframe_result(state: &mut TuiState, result: Result<ReframeProposal, String>) -> bool {
    state.take_pending_reframe();
    match result {
        Err(message) => {
            state.info(format!("重构失败：{message}"));
            false
        }
        Ok(proposal) if proposal.classification == RequestRole::End => {
            state.info("这是目的类请求，按原请求执行。");
            false
        }
        Ok(proposal) => {
            let question = proposal.question.clone().unwrap_or_default();
            let mut items = Vec::new();
            for (index, option) in proposal.options.iter().enumerate() {
                items.push(command_menu_action(
                    &format!("采纳「{option}」"),
                    "记录这个判断并生成收据",
                    &format!("/reframe accept {}", index + 1),
                    false,
                ));
            }
            items.push(command_menu_action(
                "都不是，按原方案",
                "拒绝这次重构",
                "/reframe reject",
                false,
            ));
            state.begin_command_panel("/reframe");
            state.info(format!(
                "判断  手段\n追问  {question}\n选项  {}\n\n↑/↓ 选择，Enter 执行，Esc 返回。",
                proposal.options.join(" / ")
            ));
            state.set_command_panel_menu(items);
            state.set_pending_reframe(proposal);
            true
        }
    }
}

async fn run_tui_reframe(runtime: &AgentRuntime, state: &mut TuiState, args: &str) -> Result<()> {
    let args = args.trim();
    if args.is_empty() {
        state.info("用法：/reframe <你原本想要的东西>");
        return Ok(());
    }

    if let Some(rest) = args.strip_prefix("accept ") {
        let number: usize = rest
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("用法：/reframe accept <编号>"))?;
        if number == 0 {
            bail!("编号从 1 开始");
        }
        return finish_tui_reframe(
            runtime,
            state,
            ReframeChoice::Accepted {
                option_index: number - 1,
                substitute_index: None,
            },
        );
    }
    if args == "reject" {
        return finish_tui_reframe(
            runtime,
            state,
            ReframeChoice::Rejected {
                reason: RejectedReason::PreferLiteral,
            },
        );
    }

    state.info("正在判断这是目的还是手段…");
    state.take_pending_reframe();
    let result = runtime
        .reframe(args, &[], &CancellationToken::new())
        .await
        .map_err(|error| error.to_string());
    apply_reframe_result(state, result);
    Ok(())
}

/// Interactive variant: keeps the frame responsive and lets Ctrl+C cancel.
///
/// The line-oriented TUI cannot pump keys while a command runs, so it uses the
/// blocking form above. Returns true when the user asked to quit.
async fn run_tui_reframe_interactive(
    runtime: &AgentRuntime,
    state: &mut TuiState,
    terminal: &mut TerminalSession,
    inputs: &mut tokio::sync::mpsc::UnboundedReceiver<TuiInput>,
    args: &str,
) -> Result<bool> {
    let trimmed = args.trim();
    let instant = trimmed.is_empty() || trimmed.starts_with("accept ") || trimmed == "reject";
    if instant {
        run_tui_reframe(runtime, state, trimmed).await?;
        return Ok(false);
    }

    state.take_pending_reframe();
    state.info("正在判断这是目的还是手段…（Ctrl+C 取消）");
    draw_tui_frame(terminal, state)?;

    let cancellation = CancellationToken::new();
    let attempt = runtime.reframe(trimmed, &[], &cancellation);
    tokio::pin!(attempt);
    let mut ticker = tokio::time::interval(Duration::from_millis(120));
    let mut quit = false;

    let outcome = loop {
        tokio::select! {
            result = &mut attempt => break result.map_err(|error| error.to_string()),
            _ = ticker.tick() => draw_tui_frame(terminal, state)?,
            input = inputs.recv() => match input {
                Some(TuiInput::CtrlC) => cancellation.cancel(),
                Some(TuiInput::CtrlQ) => state.request_quit(),
                Some(TuiInput::Enter) if state.quit_requested() => {
                    state.take_quit_request();
                    quit = true;
                    cancellation.cancel();
                }
                Some(TuiInput::Escape) if state.quit_requested() => state.cancel_quit(),
                Some(event) if state.quit_requested()
                    && !matches!(
                        event,
                        TuiInput::Enter | TuiInput::Escape | TuiInput::CtrlQ
                    ) =>
                {
                    state.cancel_quit();
                }
                Some(_) => {}
                None => cancellation.cancel(),
            },
        }
    };

    if cancellation.is_cancelled() {
        state.take_pending_reframe();
        state.info("已取消这次判断。");
    } else {
        apply_reframe_result(state, outcome);
    }
    draw_tui_frame(terminal, state)?;
    Ok(quit)
}

fn finish_tui_reframe(
    runtime: &AgentRuntime,
    state: &mut TuiState,
    choice: ReframeChoice,
) -> Result<()> {
    let Some(proposal) = state.take_pending_reframe() else {
        state.info("没有待决定的提议。先输入 /reframe <请求>。");
        return Ok(());
    };
    choice.validate_against(&proposal)?;
    let decided_at = now_unix_ms();
    let (_, path) = record_reframe_decision(
        &proposal,
        &choice,
        "tui",
        &state.provider_label.clone(),
        &prompt_sha256(),
        runtime.journal_dir(),
        decided_at,
    )?;
    let receipt = render_receipt(&proposal, &choice, "user", decided_at);
    state.info(format!("{receipt}journal   {}", path.display()));
    Ok(())
}

fn display_tui_next_actions(state: &mut TuiState) -> bool {
    if state.host_binding().kind() == first_principles_agent::HostKind::Standalone {
        return false;
    }
    let stage = state.command_stage();
    let host = state.host_binding().clone();
    let candidates = stage_next_actions(stage);
    let actions = next_actions(&host, stage, candidates, candidates.first().copied());
    if actions.is_empty() {
        return false;
    }
    state.begin_command_panel("/next");
    state.info("下一步\n  ↑/↓ 选择，Enter 执行，Esc 返回对话。");
    state.set_command_panel_menu(
        actions
            .into_iter()
            .map(|action| {
                let label = if action.default {
                    format!("{}（默认）", action.label)
                } else {
                    action.label.to_string()
                };
                command_menu_action(
                    action.name,
                    &label,
                    action.name,
                    command_needs_input(action.id),
                )
            })
            .collect(),
    );
    true
}

async fn run_tui_line_epistemic(
    runtime: &mut AgentRuntime,
    state: &mut TuiState,
    settings: &TuiRunSettings,
    session: &mut TuiSession,
    command: TuiCommandId,
    args: &str,
) -> Result<()> {
    let Some((display, mode, request)) = epistemic_request(command, args, state, settings, session)
    else {
        state.info(format!("用法：{} [上下文或目标]", command_name(command)));
        return Ok(());
    };
    if let Err(error) = session.record_event(SessionEvent::TaskSubmitted {
        mode: mode.as_str().into(),
    }) {
        state.info(format!("无法写入 session event log：{error}"));
        return Ok(());
    }
    state.begin(&display, mode, risk_for_mode(mode));
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancellation.cancel();
        }
    });
    let result = runtime.run_with_mode(request, mode, cancellation).await;
    signal.abort();
    match result {
        Ok(result) => {
            session.set_title_from_prompt(&state.last_instruction);
            session.record_journal(&result.journal_path)?;
            state.set_last_journal(&result.journal_path);
            let output = format!(
                "{}\n\nRun journal: {}",
                render_answer(&result),
                result.journal_path.display()
            );
            state.finish(TuiStatus::Completed, output);
        }
        Err(RuntimeError::Cancelled { .. }) => {
            state.finish(TuiStatus::Cancelled, "Run cancelled; journal preserved.");
        }
        Err(error) => {
            state.finish(TuiStatus::Failed, format!("Error: {error}"));
        }
    }
    Ok(())
}

fn epistemic_request(
    command: TuiCommandId,
    args: &str,
    state: &TuiState,
    settings: &TuiRunSettings,
    session: &TuiSession,
) -> Option<(String, CognitiveMode, TaskRequest)> {
    let plan_context = if command == TuiCommandId::Challenge && args.trim().is_empty() {
        session
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| std::fs::read_to_string(&plan.path).ok())
    } else {
        None
    };
    let (prompt, mode) = epistemic_command_prompt(
        command,
        plan_context.as_deref().unwrap_or(args),
        &state.last_instruction,
    )?;
    let request = settings.request_with_known_facts(&prompt, session_context_facts(session));
    let display = if args.trim().is_empty() {
        format!("{} 上一任务", command_name(command))
    } else {
        format!("{} {}", command_name(command), args.trim())
    };
    Some((display, mode, request))
}

fn command_name(command: TuiCommandId) -> &'static str {
    match command {
        TuiCommandId::Mode => "/mode",
        TuiCommandId::Frame => "/frame",
        TuiCommandId::Challenge => "/challenge",
        TuiCommandId::Decide => "/decide",
        TuiCommandId::Postmortem => "/postmortem",
        _ => "command",
    }
}

fn handle_model_command(
    state: &mut TuiState,
    runtime: &mut AgentRuntime,
    settings: &mut TuiRunSettings,
    args: &str,
) {
    let trimmed = args.trim();
    let (subcommand, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    match subcommand {
        "" | "show" => {
            let selected = runtime
                .selected_provider()
                .map(|manifest| format!("{}/{}", manifest.provider_id, manifest.model))
                .unwrap_or_else(|| "自动选择".into());
            let models = runtime
                .provider_manifests()
                .into_iter()
                .map(|manifest| {
                    format!(
                        "- {}/{}\n\
                         质量评分      {}\n\
                         P95 延迟      {} 毫秒\n\
                         隐私范围      {}",
                        manifest.provider_id,
                        manifest.model,
                        manifest.quality_score,
                        manifest.p95_latency_ms,
                        manifest.privacy.label_zh()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            state.info(format!(
                "模型路由\n\
                 优化目标      {}\n\
                 当前选择      {selected}\n\
                 \n\
                 可用模型\n{models}",
                runtime.route_objective().label_zh()
            ));
        }
        "objective" => match rest.trim() {
            "quality" => {
                runtime.set_route_objective(RouteObjective::Quality);
                state.info("模型选择目标已设为“质量优先”。");
            }
            "balanced" => {
                runtime.set_route_objective(RouteObjective::Balanced);
                state.info("模型选择目标已设为“质量与成本均衡”。");
            }
            "latency" => {
                runtime.set_route_objective(RouteObjective::Latency);
                state.info("模型选择目标已设为“延迟优先”。");
            }
            "cost" => {
                runtime.set_route_objective(RouteObjective::Cost);
                state.info("模型选择目标已设为“成本优先”。");
            }
            _ => state.info("用法：/model objective quality|balanced|latency|cost"),
        },
        _ => {
            let target = if subcommand == "use" {
                rest.trim()
            } else {
                trimmed
            };
            let Some((provider_id, model)) = target.split_once('/') else {
                state.info("用法：/model use <provider>/<model>");
                return;
            };
            let Some(candidate) = runtime.provider_candidate(provider_id, model) else {
                state.info(format!("未找到模型服务：{provider_id}/{model}"));
                return;
            };
            if runtime.select_provider(provider_id, model) {
                settings.planning_provider = candidate.provider;
                state.info(format!("下一次运行使用：{provider_id}/{model}"));
            }
        }
    }
}

fn handle_permissions_command(settings: &mut TuiRunSettings, state: &mut TuiState, args: &str) {
    let trimmed = args.trim();
    let (subcommand, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    match subcommand {
        "" | "show" => state.info(format!(
            "运行权限（仅影响下一次运行）\n\
             允许读取的目录：\n{}\n\
             网络访问：{}\n\
             写入与执行：{}",
            settings
                .roots
                .iter()
                .map(|path| format!("- {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n"),
            if settings.allow_network {
                "允许"
            } else {
                "禁止"
            },
            if settings.allow_write {
                "允许"
            } else {
                "禁止"
            }
        )),
        "network" => match rest.trim() {
            "on" => {
                settings.allow_network = true;
                state.info("下一次运行允许网络。");
            }
            "off" => {
                settings.allow_network = false;
                state.info("下一次运行禁止网络。");
            }
            _ => state.info("用法：/permissions network on|off"),
        },
        "write" => match rest.trim() {
            "on" => {
                settings.allow_write = true;
                state.info("下一次运行允许写入和执行。");
            }
            "off" => {
                settings.allow_write = false;
                state.info("下一次运行禁止写入和执行。");
            }
            _ => state.info("用法：/permissions write on|off"),
        },
        "root-add" => {
            let path = PathBuf::from(rest.trim());
            match std::fs::canonicalize(&path) {
                Ok(path) if path.is_dir() => {
                    if !settings.roots.contains(&path) {
                        settings.roots.push(path.clone());
                    }
                    state.info(format!("已添加允许读取的目录：{}", path.display()));
                }
                Ok(_) => state.info("允许读取的路径必须是目录。"),
                Err(error) => state.info(format!("无法添加允许读取的目录：{error}")),
            }
        }
        _ => state.info("用法：/permissions [show|network on|off|write|root-add <dir>]"),
    }
}

fn display_tools(runtime: &AgentRuntime, state: &mut TuiState, args: &str) {
    let trimmed = args.trim();
    let name = trimmed.strip_prefix("inspect ").map(str::trim);
    let capabilities = runtime.tool_capabilities();
    if let Some(name) = name {
        match capabilities
            .iter()
            .find(|capability| capability.name == name)
        {
            Some(capability) => match serde_json::to_string_pretty(capability) {
                Ok(json) => state.info(format!(
                    "工具详情：{}\n\n参数定义（技术信息）\n{json}",
                    capability.name
                )),
                Err(error) => state.info(format!("无法序列化工具信息：{error}")),
            },
            None => state.info(format!("未知工具：{name}")),
        }
        return;
    }
    let output = capabilities
        .into_iter()
        .map(|capability| {
            format!(
                "- {}\n\
                 风险等级      {}\n\
                 需要写入      {}\n\
                 当前状态      {}\n\
                 说明          {}",
                capability.name,
                capability.risk.label_zh(),
                if capability.requires_write {
                    "是"
                } else {
                    "否"
                },
                capability.status.label_zh(),
                capability.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    state.info(format!(
        "可用工具\n{}",
        if output.is_empty() {
            "当前没有可用工具。"
        } else {
            &output
        }
    ));
}

fn tui_agents_path(session: &TuiSession) -> PathBuf {
    session.root().join("runtime-agents.json")
}

fn refresh_tui_agents(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
) -> Option<Vec<AgentProfile>> {
    let configured = match settings.master_agents.as_ref() {
        Some(path) => match load_agent_profiles(path) {
            Ok(profiles) => profiles,
            Err(error) => {
                state.info(format!("无法读取 Agent 配置：{error}"));
                return None;
            }
        },
        None => Vec::new(),
    };
    let profiles = match WorkshopCliAdapter::new(
        settings.workshop_prefix.clone(),
        Duration::from_secs(30),
        false,
    )
    .and_then(|adapter| adapter.merge_profiles(&configured))
    {
        Ok(profiles) => profiles,
        Err(error) if !configured.is_empty() => {
            state.info(format!("Workshop 不可用，使用配置 Agent：{error}"));
            configured
        }
        Err(error) => {
            state.info(format!("无法发现 Agent：{error}"));
            return None;
        }
    };
    if profiles.is_empty() || profiles.len() > 64 {
        state.info("Agent 数量必须为 1..=64。");
        return None;
    }
    let bytes = match serde_json::to_vec_pretty(&profiles) {
        Ok(bytes) => bytes,
        Err(error) => {
            state.info(format!("无法序列化 Agent 配置：{error}"));
            return None;
        }
    };
    if let Err(error) =
        write_private_command_file(session.root(), &tui_agents_path(session), &bytes)
    {
        state.info(format!("无法缓存 Agent 配置：{error}"));
        return None;
    }
    Some(profiles)
}

fn read_tui_agents(
    session: &TuiSession,
    settings: &TuiRunSettings,
) -> Result<Vec<AgentProfile>, String> {
    let path = tui_agents_path(session);
    if path.exists() {
        let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("Agent 缓存超过 1 MiB".into());
        }
        return serde_json::from_slice(&bytes).map_err(|error| error.to_string());
    }
    let _ = settings;
    Err("尚未发现 Agent；请运行 /agents refresh。".into())
}

fn display_agents(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let trimmed = args.trim();
    let (subcommand, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let profiles = if subcommand == "refresh" || !tui_agents_path(session).exists() {
        let Some(profiles) = refresh_tui_agents(state, session, settings) else {
            return;
        };
        profiles
    } else {
        match read_tui_agents(session, settings) {
            Ok(profiles) => profiles,
            Err(error) => {
                state.info(error);
                return;
            }
        }
    };
    if subcommand == "inspect" {
        let Some(profile) = profiles
            .iter()
            .find(|profile| profile.agent_id == rest.trim())
        else {
            state.info(format!("未知 Agent：{}", rest.trim()));
            return;
        };
        match serde_json::to_string_pretty(profile) {
            Ok(json) => state.info(format!(
                "Agent 详情：{}\n\n原始配置（技术信息）\n{json}",
                profile.agent_id
            )),
            Err(error) => state.info(format!("无法序列化 Agent：{error}")),
        }
        return;
    }
    let output = profiles
        .iter()
        .map(|profile| {
            format!(
                "- {}\n\
                 标识          {}\n\
                 运行时        {}\n\
                 当前状态      {}\n\
                 能力          {}\n\
                 工具          {}\n\
                 写入权限      {}",
                profile.display_name.as_deref().unwrap_or(&profile.agent_id),
                profile.agent_id,
                profile.runtime,
                profile.availability.label_zh(),
                profile.capabilities.join(","),
                profile.tools.join(","),
                if profile.permissions.allow_write {
                    "允许"
                } else {
                    "禁止"
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    state.info(format!("可用 Agent\n{output}"));
}

fn assign_tui_plan(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let Some(mission) = session.mission().cloned() else {
        state.info("当前没有 active mission。");
        return;
    };
    let Some(binding) = mission.plan.clone() else {
        state.info("当前 mission 还没有计划。");
        return;
    };
    if binding.approved_by.is_none() {
        state.info("请先批准计划，再执行 /assign。");
        return;
    }
    if args.trim() == "show" {
        let Some(assignment) = binding.assignment else {
            state.info("当前计划还没有 Agent assignment。");
            return;
        };
        state.info(format_assignment(&assignment));
        return;
    }
    if !args.trim().is_empty() && args.trim() != "apply" {
        state.info("用法：/assign [show|apply]");
        return;
    }
    let plan = match read_master_plan(&binding.path) {
        Ok(plan) => plan,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let profiles = match read_tui_agents(session, settings) {
        Ok(profiles) => profiles,
        Err(_) => {
            let Some(profiles) = refresh_tui_agents(state, session, settings) else {
                return;
            };
            profiles
        }
    };
    let assignment = match plan.assign(&profiles) {
        Ok(assignment) => assignment,
        Err(error) => {
            state.info(format!("无法分配 Agent：{error}"));
            return;
        }
    };
    if let Err(error) = session.bind_assignment(assignment.clone()) {
        state.info(format!("无法保存 assignment：{error}"));
        return;
    }
    state.info(format_assignment(&assignment));
}

fn format_assignment(assignment: &first_principles_agent::AssignmentProposal) -> String {
    let rows = assignment
        .assignments
        .iter()
        .map(|item| {
            format!(
                "- {} -> {} ({}) · {}",
                item.task_id, item.agent_id, item.runtime, item.selection_reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("Assignment · plan={}\n{rows}", assignment.plan_id)
}

fn prepare_master_dispatch(
    state: &mut TuiState,
    session: &mut TuiSession,
) -> std::result::Result<(String, PathBuf, PathBuf, MasterState), String> {
    let Some(mission) = session.mission().cloned() else {
        return Err("当前没有 active mission。".into());
    };
    let Some(binding) = mission.plan.clone() else {
        return Err("当前 mission 还没有计划。".into());
    };
    let Some(assignment) = binding.assignment.clone() else {
        return Err("请先运行 /assign apply。".into());
    };
    let Some(actor) = binding.approved_by.clone() else {
        return Err("计划尚未批准。".into());
    };
    let plan = read_master_plan(&binding.path)?;
    if let Some(production) = read_production_plan(&binding.path)? {
        let task_list_path = binding
            .task_list_path
            .as_ref()
            .ok_or("生产计划缺少已绑定任务清单；请再次执行 /approve。")?;
        let expected_task_list_sha = binding
            .task_list_sha256
            .as_deref()
            .ok_or("生产计划缺少任务清单摘要；请再次执行 /approve。")?;
        let expected_plan_sha = binding
            .production_plan_sha256
            .as_deref()
            .ok_or("生产计划缺少计划摘要；请再次执行 /approve。")?;
        let actual_plan_sha = production
            .plan_sha256()
            .map_err(|error| error.to_string())?;
        if actual_plan_sha != expected_plan_sha {
            return Err("生产计划摘要已变化，拒绝派工。".into());
        }
        let task_list = std::fs::read(task_list_path).map_err(|error| error.to_string())?;
        if task_list.len() > 4 * 1024 * 1024 {
            return Err("任务清单超过 4 MiB，拒绝派工。".into());
        }
        if hex::encode(Sha256::digest(&task_list)) != expected_task_list_sha {
            return Err("任务清单已变化，拒绝派工。".into());
        }
    }
    let profiles_path = tui_agents_path(session);
    if !profiles_path.is_file() {
        return Err("缺少 Agent profile 缓存；请运行 /agents refresh。".into());
    }
    let trace_id = format!("trace-{}", safe_command_component(&plan.plan_id));
    let state_root = session.root().join("master-state");
    let store = MasterStore::new(&state_root, &trace_id).map_err(|error| error.to_string())?;
    let mut state_value = match store.load() {
        Ok(existing) => {
            if existing.plan.plan_id != plan.plan_id {
                return Err("已有 MasterState 属于另一份计划。".into());
            }
            existing
        }
        Err(_) => {
            let initialized =
                MasterState::initialize(trace_id.clone(), plan, assignment, now_unix_ms())
                    .map_err(|error| error.to_string())?;
            store
                .initialize(initialized, now_unix_ms())
                .map_err(|error| error.to_string())?;
            store.load().map_err(|error| error.to_string())?
        }
    };
    if !state_value.plan.approved {
        state_value
            .approve(actor.clone(), now_unix_ms())
            .map_err(|error| error.to_string())?;
        store
            .append(
                state_value.clone(),
                MasterJournalEventKind::Approved,
                None,
                &actor,
                "session-bound plan approval materialized for dispatch",
                now_unix_ms(),
            )
            .map_err(|error| error.to_string())?;
    }
    session
        .bind_dispatch(trace_id.clone(), state_root.clone())
        .map_err(|error| error.to_string())?;
    sync_tui_stage(state, session);
    state.info(format!(
        "Dispatch state ready · trace={trace_id} · tasks={}",
        state_value.tasks.len()
    ));
    Ok((trace_id, state_root, profiles_path, state_value))
}

fn run_tui_dispatch(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let action = if args.trim().is_empty() {
        "dry-run"
    } else {
        args.trim()
    };
    if !matches!(action, "dry-run" | "run" | "detach") {
        state.info("用法：/dispatch [dry-run|run|detach]");
        return;
    }
    let (trace_id, state_root, profiles_path, mut master) =
        match prepare_master_dispatch(state, session) {
            Ok(value) => value,
            Err(error) => {
                state.info(format!("无法准备派发：{error}"));
                return;
            }
        };
    master.advance_ready(now_unix_ms());
    let ready = master
        .plan
        .work_items
        .iter()
        .filter(|item| {
            master.tasks.get(&item.id).is_some_and(|task| {
                matches!(
                    task.status,
                    WorkItemStatus::Ready | WorkItemStatus::Retrying
                ) && !task.human_takeover
            })
        })
        .take(master.plan.max_parallel.max(1))
        .cloned()
        .collect::<Vec<_>>();
    if action == "dry-run" {
        state.info(format!(
            "Dispatch dry-run · trace={trace_id}\n{}",
            ready
                .iter()
                .map(|item| format!("- {} · {}", item.id, item.title))
                .collect::<Vec<_>>()
                .join("\n")
        ));
        return;
    }
    let store = match MasterStore::new(&state_root, &trace_id) {
        Ok(store) => store,
        Err(error) => {
            state.info(format!("无法打开 MasterState：{error}"));
            return;
        }
    };
    let adapter = match WorkshopCliAdapter::new(
        settings.workshop_prefix.clone(),
        Duration::from_secs(30),
        action == "run",
    ) {
        Ok(adapter) => adapter
            .with_artifact_dir(store.trace_dir().join("executions"))
            .with_goal_context(master.trace_id.clone(), master.plan.goal_id.clone()),
        Err(error) => {
            state.info(format!("无法创建 Workshop adapter：{error}"));
            return;
        }
    };
    let profiles = match load_agent_profiles(&profiles_path) {
        Ok(profiles) => profiles,
        Err(error) => {
            state.info(format!("无法读取 Agent profiles：{error}"));
            return;
        }
    };
    for item in ready {
        let assignment = match master
            .assignment
            .assignments
            .iter()
            .find(|assignment| assignment.task_id == item.id)
        {
            Some(assignment) => assignment,
            None => {
                state.info(format!("任务 {} 没有 assignment。", item.id));
                continue;
            }
        };
        let Some(profile) = profiles
            .iter()
            .find(|profile| profile.agent_id == assignment.agent_id)
        else {
            state.info(format!("缺少 Agent profile：{}", assignment.agent_id));
            continue;
        };
        let lease = match master.lease_task(
            &item.id,
            profile,
            format!("workshop-session:{}", assignment.agent_id),
            item.budget.timeout_ms.saturating_add(30_000),
            now_unix_ms(),
        ) {
            Ok(lease) => lease,
            Err(error) => {
                state.info(format!("任务 {} 无法租约：{error}", item.id));
                continue;
            }
        };
        master = match store.append(
            master,
            MasterJournalEventKind::Leased,
            Some(item.id.clone()),
            "master",
            format!("lease generation {}", lease.generation),
            now_unix_ms(),
        ) {
            Ok(master) => master,
            Err(error) => {
                state.info(format!("无法记录租约：{error}"));
                return;
            }
        };
        if let Err(error) = master.begin_starting(&item.id, lease.generation, now_unix_ms()) {
            state.info(format!("任务 {} 无法启动：{error}", item.id));
            continue;
        }
        master = match store.append(
            master,
            MasterJournalEventKind::Starting,
            Some(item.id.clone()),
            "master",
            "starting Workshop session",
            now_unix_ms(),
        ) {
            Ok(master) => master,
            Err(error) => {
                state.info(format!("无法记录启动：{error}"));
                return;
            }
        };
        match adapter.start(
            &item,
            profile,
            lease.generation,
            &lease.idempotency_key,
            now_unix_ms(),
        ) {
            Ok(submission) => {
                if let Err(error) = master.record_submission(submission.clone(), now_unix_ms()) {
                    state.info(format!("无法记录提交：{error}"));
                    continue;
                }
                master = match store.append(
                    master,
                    MasterJournalEventKind::Submitted,
                    Some(item.id.clone()),
                    &profile.agent_id,
                    format!(
                        "submitted to session {}",
                        submission
                            .execution_target
                            .session_id
                            .as_deref()
                            .unwrap_or("-")
                    ),
                    now_unix_ms(),
                ) {
                    Ok(master) => master,
                    Err(error) => {
                        state.info(format!("无法记录提交：{error}"));
                        return;
                    }
                };
                state.info(format!(
                    "已提交 {} · agent={} · session={}",
                    item.id,
                    profile.agent_id,
                    submission
                        .execution_target
                        .session_id
                        .as_deref()
                        .unwrap_or("-")
                ));
            }
            Err(error) => {
                let receipt = failed_receipt(&item, profile, &lease, error.to_string());
                let _ = master.record_receipt(receipt, now_unix_ms());
                state.info(format!("任务 {} 启动失败：{error}", item.id));
            }
        }
    }
}

fn tui_master_store(
    session: &TuiSession,
) -> std::result::Result<(String, PathBuf, MasterStore), String> {
    let plan = session
        .mission()
        .and_then(|mission| mission.plan.as_ref())
        .ok_or_else(|| "当前没有已绑定计划。".to_string())?;
    let trace_id = plan
        .master_trace_id
        .clone()
        .ok_or_else(|| "当前计划尚未派发。".to_string())?;
    let state_root = plan
        .master_state_root
        .clone()
        .ok_or_else(|| "当前计划没有 MasterState 路径。".to_string())?;
    let store = MasterStore::new(&state_root, &trace_id).map_err(|error| error.to_string())?;
    Ok((trace_id, state_root, store))
}

fn master_task_line(
    id: &str,
    task: &first_principles_agent::TaskRuntimeState,
    item: Option<&first_principles_agent::WorkItem>,
) -> String {
    let agent = task
        .active_lease
        .as_ref()
        .map(|lease| lease.agent_id.as_str())
        .unwrap_or("-");
    let session = task
        .execution_target
        .as_ref()
        .and_then(|target| target.session_id.as_deref())
        .unwrap_or("-");
    let title = item
        .map(|item| format!(" · {}", bounded_tui_text(&item.title, 80)))
        .unwrap_or_default();
    let dependencies = item
        .filter(|item| !item.dependencies.is_empty())
        .map(|item| format!(" · deps={}", item.dependencies.join(",")))
        .unwrap_or_default();
    let lease = task
        .active_lease
        .as_ref()
        .map(|lease| format!(" · lease_until={}", lease.expires_unix_ms))
        .unwrap_or_default();
    let error = task
        .last_error
        .as_deref()
        .map(|value| format!(" · error={}", bounded_tui_text(value, 180)))
        .unwrap_or_default();
    format!(
        "- {id}{title} · {:?} · agent={agent} · session={session} · gen={}{dependencies}{lease}{error}",
        task.status, task.generation,
    )
}

fn display_master_board(state: &mut TuiState, session: &TuiSession, args: &str) {
    let (trace_id, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let master = match store.load() {
        Ok(master) => master,
        Err(error) => {
            state.info(format!("无法读取 MasterState：{error}"));
            return;
        }
    };
    let filter = args.trim();
    let rows = master
        .tasks
        .iter()
        .filter(|(id, _)| {
            filter.is_empty()
                || id.contains(filter)
                || master
                    .plan
                    .work_items
                    .iter()
                    .any(|item| item.id == **id && item.title.contains(filter))
        })
        .map(|(id, task)| {
            master_task_line(
                id,
                task,
                master.plan.work_items.iter().find(|item| item.id == *id),
            )
        })
        .collect::<Vec<_>>();
    state.info(bound_tui_command_output(format!(
        "Board · trace={trace_id} · approved={} · paused={}\n{}",
        master.plan.approved,
        master.paused,
        if rows.is_empty() {
            "没有匹配任务。".into()
        } else {
            rows.join("\n")
        }
    )));
}

fn display_attention(state: &mut TuiState, session: &TuiSession) {
    let (trace_id, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let master = match store.load() {
        Ok(master) => master,
        Err(error) => {
            state.info(format!("无法读取 MasterState：{error}"));
            return;
        }
    };
    let now = now_unix_ms();
    let rows = master
        .tasks
        .iter()
        .filter(|(_, task)| {
            matches!(
                task.status,
                WorkItemStatus::AwaitingInput
                    | WorkItemStatus::ArtifactReady
                    | WorkItemStatus::Blocked
                    | WorkItemStatus::Rejected
                    | WorkItemStatus::Retrying
                    | WorkItemStatus::Replanning
            ) || task
                .active_lease
                .as_ref()
                .is_some_and(|lease| !lease.is_active(now))
        })
        .map(|(id, task)| {
            master_task_line(
                id,
                task,
                master.plan.work_items.iter().find(|item| item.id == *id),
            )
        })
        .collect::<Vec<_>>();
    state.info(format!(
        "Attention · trace={trace_id}\n{}",
        if rows.is_empty() {
            "没有需要人工处理的任务。".into()
        } else {
            rows.join("\n")
        }
    ));
}

fn watch_tui_master(state: &mut TuiState, session: &TuiSession, settings: &TuiRunSettings) {
    let (trace_id, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let mut master = match store.load() {
        Ok(master) => master,
        Err(error) => {
            state.info(format!("无法读取 MasterState：{error}"));
            return;
        }
    };
    let adapter = match WorkshopCliAdapter::new(
        settings.workshop_prefix.clone(),
        Duration::from_secs(30),
        false,
    ) {
        Ok(adapter) => adapter
            .with_artifact_dir(store.trace_dir().join("executions"))
            .with_goal_context(master.trace_id.clone(), master.plan.goal_id.clone()),
        Err(error) => {
            state.info(format!("无法创建 Workshop adapter：{error}"));
            return;
        }
    };
    let submissions = master
        .plan
        .work_items
        .iter()
        .filter_map(|item| {
            let task = master.tasks.get(&item.id)?;
            if task.human_takeover
                || !matches!(
                    task.status,
                    WorkItemStatus::Running
                        | WorkItemStatus::AwaitingInput
                        | WorkItemStatus::Blocked
                )
            {
                return None;
            }
            let lease = task.active_lease.as_ref()?;
            Some(ExecutionSubmission {
                task_id: item.id.clone(),
                agent_id: lease.agent_id.clone(),
                lease_generation: lease.generation,
                idempotency_key: lease.idempotency_key.clone(),
                execution_target: task.execution_target.clone()?,
                submitted_unix_ms: task.submitted_unix_ms.unwrap_or_default(),
                deadline_unix_ms: task
                    .submitted_unix_ms
                    .unwrap_or_default()
                    .saturating_add(u128::from(item.budget.timeout_ms)),
            })
        })
        .collect::<Vec<_>>();
    if submissions.is_empty() {
        state.info(format!("Watch · trace={trace_id} · 没有运行中的任务"));
        return;
    }
    let mut outputs = Vec::new();
    for submission in submissions {
        let previous = master
            .tasks
            .get(&submission.task_id)
            .map(|task| task.status);
        let observation = adapter
            .observe_submission(&submission, now_unix_ms())
            .unwrap_or_else(|error| ExecutionObservation::Blocked {
                reason: error.to_string(),
            });
        let terminal = matches!(observation, ExecutionObservation::Finished { .. });
        if let Err(error) =
            master.record_observation(&submission.task_id, observation.clone(), now_unix_ms())
        {
            outputs.push(format!("{} · 状态更新失败：{error}", submission.task_id));
            continue;
        }
        let next = master
            .tasks
            .get(&submission.task_id)
            .map(|task| task.status);
        if terminal || previous != next {
            master = match store.append(
                master.clone(),
                if terminal {
                    MasterJournalEventKind::ReceiptRecorded
                } else {
                    MasterJournalEventKind::Observed
                },
                Some(submission.task_id.clone()),
                "workshop",
                format!("{observation:?}"),
                now_unix_ms(),
            ) {
                Ok(master) => master,
                Err(error) => {
                    outputs.push(format!(
                        "{} · 无法写入 journal：{error}",
                        submission.task_id
                    ));
                    continue;
                }
            };
        }
        outputs.push(format!("{} · {:?}", submission.task_id, observation));
    }
    state.info(bound_tui_command_output(format!(
        "Watch · trace={trace_id}\n{}",
        outputs.join("\n")
    )));
}

fn respond_tui_task(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let Some((task_id, message)) = args.trim().split_once(char::is_whitespace) else {
        state.info("用法：/respond <task-id> <message>");
        return;
    };
    let message = message.trim();
    if message.is_empty() {
        state.info("回复内容不能为空。");
        return;
    }
    let (_, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let mut master = match store.load() {
        Ok(master) => master,
        Err(error) => {
            state.info(format!("无法读取 MasterState：{error}"));
            return;
        }
    };
    let session_id = match master.tasks.get(task_id) {
        Some(task)
            if matches!(
                task.status,
                WorkItemStatus::AwaitingInput | WorkItemStatus::Blocked
            ) =>
        {
            task.execution_target
                .as_ref()
                .and_then(|target| target.session_id.clone())
        }
        Some(_) => {
            state.info(format!("任务 {task_id} 当前不需要人工回复。"));
            return;
        }
        None => {
            state.info(format!("未知任务：{task_id}"));
            return;
        }
    };
    let Some(session_id) = session_id else {
        state.info(format!("任务 {task_id} 没有可写入的 Session。"));
        return;
    };
    let adapter = match WorkshopCliAdapter::new(
        settings.workshop_prefix.clone(),
        Duration::from_secs(30),
        false,
    ) {
        Ok(adapter) => adapter,
        Err(error) => {
            state.info(format!("无法创建 Workshop adapter：{error}"));
            return;
        }
    };
    if let Err(error) = adapter.write_session(&session_id, message) {
        state.info(format!("无法发送回复：{error}"));
        return;
    }
    if let Err(error) =
        master.record_observation(task_id, ExecutionObservation::Running, now_unix_ms())
    {
        state.info(format!("回复已发送，但状态更新失败：{error}"));
        return;
    }
    match store.append(
        master,
        MasterJournalEventKind::Observed,
        Some(task_id.into()),
        "user",
        "human response routed to task Session",
        now_unix_ms(),
    ) {
        Ok(_) => state.info(format!("已回复 {task_id} · session={session_id}")),
        Err(error) => state.info(format!("回复已发送，但 journal 写入失败：{error}")),
    }
}

fn control_tui_master(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let result = (|| -> std::result::Result<String, String> {
        let trimmed = args.trim();
        let (command, rest) = trimmed
            .split_once(char::is_whitespace)
            .unwrap_or((trimmed, ""));
        let (_, _, store) = tui_master_store(session)?;
        let mut master = match store.load() {
            Ok(master) => master,
            Err(error) => return Err(format!("无法读取 MasterState：{error}")),
        };
        let actor = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "user".into());
        match command {
            "pause" | "resume" => {
                let paused = command == "pause";
                master
                    .set_paused(paused, actor.clone(), now_unix_ms())
                    .map_err(|error| error.to_string())?;
                store
                    .append(
                        master,
                        if paused {
                            MasterJournalEventKind::Paused
                        } else {
                            MasterJournalEventKind::Resumed
                        },
                        None,
                        &actor,
                        if paused { "run paused" } else { "run resumed" },
                        now_unix_ms(),
                    )
                    .map(|_| format!("Master run {}", if paused { "paused" } else { "resumed" }))
                    .map_err(|error| error.to_string())
            }
            "cancel" | "takeover" | "return" | "retry" | "replan" | "reclaim" => {
                let (task_id, reason) = rest
                    .split_once(char::is_whitespace)
                    .unwrap_or((rest, "user requested"));
                let reason = if reason.trim().is_empty() {
                    "user requested"
                } else {
                    reason.trim()
                };
                match command {
                    "cancel" => {
                        let status = master
                            .tasks
                            .get(task_id)
                            .ok_or_else(|| format!("未知任务：{task_id}"))?
                            .status;
                        if !status.can_transition_to(WorkItemStatus::Cancelled) {
                            return Err(format!("任务 {task_id} 当前不能取消：{status:?}"));
                        }
                        if let Err(error) = cancel_active_session_if_any(
                            &master,
                            task_id,
                            &settings.workshop_prefix,
                        ) {
                            return Err(format!("取消 Session 失败：{error}"));
                        }
                        master
                            .cancel_task(task_id, actor.clone(), reason, now_unix_ms())
                            .map_err(|error| error.to_string())?;
                        store
                            .append(
                                master,
                                MasterJournalEventKind::Cancelled,
                                Some(task_id.into()),
                                &actor,
                                reason,
                                now_unix_ms(),
                            )
                            .map(|_| format!("任务 {task_id} 已取消"))
                            .map_err(|error| error.to_string())
                    }
                    "takeover" => {
                        master
                            .take_over(task_id, actor.clone(), reason, now_unix_ms())
                            .map_err(|error| error.to_string())?;
                        store
                            .append(
                                master,
                                MasterJournalEventKind::TakenOver,
                                Some(task_id.into()),
                                &actor,
                                reason,
                                now_unix_ms(),
                            )
                            .map(|_| format!("任务 {task_id} 已由人工接管"))
                            .map_err(|error| error.to_string())
                    }
                    "return" => {
                        master
                            .end_takeover(task_id, actor.clone(), reason, now_unix_ms())
                            .map_err(|error| error.to_string())?;
                        store
                            .append(
                                master,
                                MasterJournalEventKind::TakeoverEnded,
                                Some(task_id.into()),
                                &actor,
                                reason,
                                now_unix_ms(),
                            )
                            .map(|_| format!("任务 {task_id} 已交还 Master"))
                            .map_err(|error| error.to_string())
                    }
                    "reclaim" => {
                        if let Err(error) = terminate_expired_session_if_any(
                            &master,
                            task_id,
                            &settings.workshop_prefix,
                        ) {
                            return Err(format!("终止旧 Session 失败：{error}"));
                        }
                        master
                            .reclaim_expired_lease(
                                task_id,
                                actor.clone(),
                                reason,
                                true,
                                now_unix_ms(),
                            )
                            .map_err(|error| error.to_string())?;
                        store
                            .append(
                                master,
                                MasterJournalEventKind::LeaseReclaimed,
                                Some(task_id.into()),
                                &actor,
                                reason,
                                now_unix_ms(),
                            )
                            .map(|_| format!("任务 {task_id} 的过期租约已回收"))
                            .map_err(|error| error.to_string())
                    }
                    "retry" => {
                        let task = master
                            .tasks
                            .get(task_id)
                            .ok_or_else(|| format!("未知任务：{task_id}"))?;
                        if !matches!(
                            task.status,
                            WorkItemStatus::Rejected | WorkItemStatus::Blocked
                        ) {
                            return Err(format!("任务 {task_id} 当前不能重试：{:?}", task.status));
                        }
                        if task.retries >= first_principles_agent::MAX_AUTOMATIC_RETRIES {
                            return Err(format!("任务 {task_id} 已达到自动重试上限"));
                        }
                        if let Err(error) = cancel_active_session_if_any(
                            &master,
                            task_id,
                            &settings.workshop_prefix,
                        ) {
                            return Err(format!("取消旧 Session 失败：{error}"));
                        }
                        master
                            .request_retry(task_id, actor.clone(), reason, now_unix_ms())
                            .map_err(|error| error.to_string())?;
                        store
                            .append(
                                master,
                                MasterJournalEventKind::RetryRequested,
                                Some(task_id.into()),
                                &actor,
                                reason,
                                now_unix_ms(),
                            )
                            .map(|_| format!("任务 {task_id} 已进入有界重试"))
                            .map_err(|error| error.to_string())
                    }
                    _ => {
                        let task = master
                            .tasks
                            .get(task_id)
                            .ok_or_else(|| format!("未知任务：{task_id}"))?;
                        if !matches!(
                            task.status,
                            WorkItemStatus::Rejected | WorkItemStatus::Blocked
                        ) {
                            return Err(format!(
                                "任务 {task_id} 当前不能重规划：{:?}",
                                task.status
                            ));
                        }
                        if master.replan_count >= first_principles_agent::MAX_AUTOMATIC_REPLANS {
                            return Err("已达到自动重规划上限".into());
                        }
                        if let Err(error) = cancel_active_session_if_any(
                            &master,
                            task_id,
                            &settings.workshop_prefix,
                        ) {
                            return Err(format!("取消旧 Session 失败：{error}"));
                        }
                        master
                            .request_replan(task_id, actor.clone(), reason, now_unix_ms())
                            .map_err(|error| error.to_string())?;
                        store
                            .append(
                                master,
                                MasterJournalEventKind::ReplanRequested,
                                Some(task_id.into()),
                                &actor,
                                reason,
                                now_unix_ms(),
                            )
                            .map(|_| format!("任务 {task_id} 已请求局部重规划"))
                            .map_err(|error| error.to_string())
                    }
                }
            }
            _ => Err(
                "用法：/control pause|resume|cancel|takeover|return|retry|replan|reclaim ..."
                    .into(),
            ),
        }
    })();
    match result {
        Ok(message) => state.info(message),
        Err(error) => state.info(format!("控制失败：{error}")),
    }
}

fn display_master_timeline(state: &mut TuiState, session: &TuiSession, settings: &TuiRunSettings) {
    let (trace_id, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let events = match read_master_journal_events(&store.trace_dir().join("events.jsonl")) {
        Ok(events) => events,
        Err(error) => {
            state.info(format!("无法读取 Master journal：{error}"));
            return;
        }
    };
    let session_ids = store
        .load()
        .map(|master| {
            master
                .tasks
                .values()
                .filter_map(|task| {
                    task.execution_target
                        .as_ref()
                        .and_then(|target| target.session_id.clone())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut rows = events
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|event| {
            format!(
                "- {} · {} · {:?} · {} · {}",
                event.seq,
                event.timestamp_unix_ms,
                event.kind,
                event.task_id.as_deref().unwrap_or("-"),
                bounded_tui_text(&event.reason, 240)
            )
        })
        .collect::<Vec<_>>();
    if let Ok(workshop) = WorkshopCliAdapter::new(
        settings.workshop_prefix.clone(),
        Duration::from_secs(30),
        false,
    )
    .and_then(|adapter| adapter.bounded_events(None, 20))
    {
        rows.extend(workshop.into_iter().map(|event| {
            let text = bounded_tui_text(&event.to_string(), 400);
            let session = session_ids
                .iter()
                .find(|session_id| text.contains(session_id.as_str()))
                .map(String::as_str)
                .unwrap_or("unassociated");
            format!("- workshop · session={session} · {text}")
        }));
    }
    state.info(bound_tui_command_output(format!(
        "Timeline · trace={trace_id}\n{}",
        rows.join("\n")
    )));
}

fn bounded_tui_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut output = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        output.push('…');
    }
    output.replace(['\r', '\n'], " ")
}

fn sync_tui_stage(state: &mut TuiState, session: &TuiSession) {
    if !state.host_binding().is_workshop_master() {
        state.set_command_stage(CommandStage::NoMission);
        return;
    }
    let stage = match session.mission() {
        None => CommandStage::NoMission,
        Some(mission) => match mission.plan.as_ref() {
            None => CommandStage::Framing,
            Some(plan) if plan.master_trace_id.is_some() => CommandStage::Running,
            Some(_) => CommandStage::Approval,
        },
    };
    state.set_command_stage(stage);
}

fn handle_mission_command(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let trimmed = args.trim();
    let (subcommand, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let result = match subcommand {
        "" | "show" => {
            state.info(format_mission(session.mission()));
            return;
        }
        "close" => session.close_mission().map_err(|error| error.to_string()),
        "new" => {
            if session.mission().is_some() {
                Err("已有 active mission；使用 /mission revise 或 /mission close。".into())
            } else {
                start_mission(session, settings, rest)
            }
        }
        "revise" => session
            .revise_mission(rest.trim())
            .map_err(|error| error.to_string()),
        _ => {
            if session.mission().is_some() {
                session
                    .revise_mission(trimmed)
                    .map_err(|error| error.to_string())
            } else {
                start_mission(session, settings, trimmed)
            }
        }
    };
    match result {
        Ok(()) => {
            state.info(format_mission(session.mission()));
            sync_tui_stage(state, session);
        }
        Err(error) => state.info(format!("Mission 未更新：{error}")),
    }
}

fn start_mission(
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    goal: &str,
) -> std::result::Result<(), String> {
    if goal.trim().is_empty() {
        return Err("请提供 mission 目标。".into());
    }
    session
        .start_mission(goal.trim(), settings.cwd.clone())
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn format_mission(mission: Option<&first_principles_agent::SessionMission>) -> String {
    let Some(mission) = mission else {
        return "当前没有 active mission。使用 /mission new <目标> 创建。".into();
    };
    let plan = mission.plan.as_ref();
    let approval = plan.and_then(|plan| plan.approved_by.as_deref());
    let task_list = plan
        .and_then(|plan| plan.task_list_path.as_ref())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "未生成".into());
    format!(
        "Mission · {} · revision={}\n目标: {}\n工作区: {}\nPlan: {}\nTask list: {}\nApproval: {}",
        mission.id,
        mission.revision,
        mission.goal,
        mission.workspace.display(),
        plan.map(|plan| {
            format!(
                "{} · {} · sha256={}",
                plan.plan_id,
                plan.path.display(),
                plan.sha256
            )
        })
        .unwrap_or_else(|| "未生成".into()),
        task_list,
        approval
            .map(|actor| format!("approved by {actor}"))
            .unwrap_or_else(|| "未批准".into())
    )
}

fn display_triage(state: &mut TuiState, session: &TuiSession, args: &str) {
    let goal = args.trim();
    let goal = if goal.is_empty() {
        session
            .mission()
            .map(|mission| mission.goal.clone())
            .unwrap_or_else(|| state.last_instruction.clone())
    } else {
        goal.to_string()
    };
    if goal.trim().is_empty() {
        state.info("用法：/triage [目标]，或先创建 /mission。");
        return;
    }
    let decision = triage_goal(&goal, &TriageSignals::default());
    state.info(format!(
        "Triage\nroute: {:?}\nreason: {}\nrequires_plan: {}\nrequires_approval: {}\n\
         allows_dispatch: {}\nmax_work_items: {}",
        decision.route,
        decision.reason,
        decision.requires_plan,
        decision.requires_user_approval,
        decision.allows_dispatch,
        decision.max_work_items
    ));
}

async fn handle_master_task_intake(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    instruction: &str,
) -> bool {
    if !state.host_binding().is_workshop_master() {
        return false;
    }
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return false;
    }

    if let Some(mission) = session.mission().cloned()
        && let Some(plan) = mission.plan.as_ref()
    {
        if plan.approved_by.is_none() && is_plan_confirmation(instruction) {
            approve_tui_plan(state, session, "");
            return true;
        }
        if plan.approved_by.is_some() && plan.task_list_path.is_none() {
            if is_plan_confirmation(instruction) || is_execute_confirmation(instruction) {
                approve_tui_plan(state, session, &plan.plan_id);
                state.info("任务清单已准备。检查 Agent、窗口和权限后，再回复“确认执行”。");
                return true;
            }
        } else if plan.approved_by.is_some()
            && plan.task_list_path.is_some()
            && is_execute_confirmation(instruction)
        {
            assign_tui_plan(state, session, settings, "apply");
            let assigned = session
                .mission()
                .and_then(|mission| mission.plan.as_ref())
                .and_then(|plan| plan.assignment.as_ref())
                .is_some();
            if assigned {
                run_tui_dispatch(state, session, settings, "run");
            }
            return true;
        }
        if plan.master_trace_id.is_some()
            && handle_master_supervision_input(state, session, settings, instruction)
        {
            return true;
        }
        if plan.approved_by.is_none()
            && triage_goal(instruction, &TriageSignals::default()).requires_plan
        {
            state.info(
                "当前 Mission 正在等待计划确认。请回复“确认计划”、执行 /plan revise，或先关闭当前 Mission。",
            );
            return true;
        }
        return false;
    }

    if session.mission().is_some() {
        return false;
    }
    let triage = triage_goal(instruction, &TriageSignals::default());
    if !triage.requires_plan {
        return false;
    }
    if let Err(error) = start_mission(session, settings, instruction) {
        state.info(format!("无法创建 Mission：{error}"));
        return true;
    }
    state.info(format!(
        "识别到任务目标：{}\n正在生成 Goal、可衡量 KR 和任务拆分；批准前不会执行。",
        instruction
    ));
    run_tui_master_plan(state, session, settings, "generate").await;
    true
}

fn is_plan_confirmation(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "确认计划" | "确认目标和拆分" | "是" | "yes" | "y"
    )
}

fn is_execute_confirmation(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "确认执行" | "确认执行并派工" | "开始执行" | "执行" | "是" | "yes" | "y"
    )
}

fn handle_master_supervision_input(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    instruction: &str,
) -> bool {
    let normalized = instruction.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "进度" | "状态" | "任务板" | "进度如何" | "现在进度如何" | "status" | "board"
    ) {
        watch_tui_master(state, session, settings);
        display_master_board(state, session, "");
        return true;
    }
    if matches!(
        normalized.as_str(),
        "需要我处理什么" | "待办" | "attention" | "blocked"
    ) {
        display_attention(state, session);
        return true;
    }
    match normalized.as_str() {
        "暂停督战" | "暂停调度" | "pause" => {
            control_tui_master(state, session, settings, "pause");
            return true;
        }
        "继续督战" | "恢复调度" | "resume" => {
            control_tui_master(state, session, settings, "resume");
            return true;
        }
        _ => {}
    }
    if let Some((task_id, message)) = parse_targeted_operator_message(instruction) {
        respond_tui_task(state, session, settings, &format!("{task_id} {message}"));
        return true;
    }
    if triage_goal(instruction, &TriageSignals::default()).requires_plan {
        state.info(
            "当前 Mission 正在督战。新目标请使用新的 Master 对话；调整现有任务请使用“补充任务 T3：...”，控制命令请使用“暂停任务 T3”等明确指令。",
        );
        return true;
    }
    false
}

fn parse_targeted_operator_message(instruction: &str) -> Option<(String, String)> {
    let rest = instruction
        .trim()
        .strip_prefix("回复任务")
        .or_else(|| instruction.trim().strip_prefix("补充任务"))?
        .trim();
    let (task_id, message) = rest.split_once('：').or_else(|| rest.split_once(':'))?;
    let task_id = task_id.trim();
    let message = message.trim();
    if task_id.is_empty() || message.is_empty() {
        return None;
    }
    Some((task_id.into(), message.into()))
}

async fn run_tui_master_plan(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let trimmed = args.trim();
    let (subcommand, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    if subcommand.is_empty() || subcommand == "show" {
        let Some(mission) = session.mission() else {
            state.info("当前没有 active mission。");
            return;
        };
        let Some(plan) = mission.plan.as_ref() else {
            state.info("当前 mission 还没有计划。使用 /plan generate。");
            return;
        };
        match read_production_plan(&plan.path) {
            Ok(Some(production)) => match production.render_plan_preview() {
                Ok(output) => state.info(output),
                Err(error) => state.info(format!("无法渲染生产计划：{error}")),
            },
            Ok(None) => match read_master_plan(&plan.path) {
                Ok(proposal) => state.info(proposal.to_markdown()),
                Err(error) => state.info(error),
            },
            Err(error) => state.info(error),
        }
        return;
    }

    let revision = subcommand == "revise";
    if subcommand != "generate" && !revision {
        state.info("用法：/plan [generate|show|revise] [附加约束]");
        return;
    }
    let Some(mission) = session.mission().cloned() else {
        state.info("当前没有 active mission。");
        return;
    };
    if mission
        .plan
        .as_ref()
        .is_some_and(|plan| plan.approved_by.is_some())
    {
        state.info("已批准计划不能直接重写；请先修订 mission。");
        return;
    }
    let triage = triage_goal(&mission.goal, &TriageSignals::default());
    if !triage.requires_plan {
        state.info("当前目标不需要工作计划。");
        return;
    }
    let mut context =
        match PlanningContext::new(mission.goal.clone(), mission.workspace.clone(), &triage) {
            Ok(context) => context,
            Err(error) => {
                state.info(format!("无法创建规划上下文：{error}"));
                return;
            }
        };
    context.risk = RiskLevel::Medium;
    if !rest.trim().is_empty() {
        context.constraints.push(rest.trim().into());
    }
    let mut revision_base = None;
    if revision {
        let Some(current) = mission.plan.as_ref() else {
            state.info("当前没有可修订的计划。");
            return;
        };
        let Ok(current_plan) = read_master_plan(&current.path) else {
            state.info("无法读取当前计划。");
            return;
        };
        context.plan_id = current_plan.plan_id;
        context.goal_id = current_plan.goal_id;
        revision_base = Some(current_plan.version);
    }
    let conversation_id = state
        .host_binding()
        .master_conversation_id()
        .unwrap_or(session.id());
    let production_context = match ProductionPlanningContext::new(context, conversation_id) {
        Ok(context) => context,
        Err(error) => {
            state.info(format!("无法创建生产规划上下文：{error}"));
            return;
        }
    };
    let generator = ModelProductionPlanGenerator::new(settings.planning_provider.clone())
        .with_journal_dir(session.root().join("planner-journal"));
    let production = match generator
        .generate(&production_context, &CancellationToken::new())
        .await
    {
        Ok(production) => production,
        Err(error) => {
            state.info(format!("计划生成失败：{error}"));
            return;
        }
    };
    let mut production = production;
    if let Some(version) = revision_base {
        production.plan.version = version.saturating_add(1);
    }
    if let Err(error) = production.validate() {
        state.info(format!("计划校验失败：{error}"));
        return;
    }
    let bytes = match serde_json::to_vec_pretty(&production) {
        Ok(mut bytes) => {
            bytes.push(b'\n');
            bytes
        }
        Err(error) => {
            state.info(format!("无法序列化计划：{error}"));
            return;
        }
    };
    let digest = hex::encode(Sha256::digest(&bytes));
    let directory = session.root().join("plans");
    let path = directory.join(format!(
        "{}-v{}.json",
        safe_command_component(&production.plan.plan_id),
        production.plan.version
    ));
    if let Err(error) = write_private_command_file(&directory, &path, &bytes) {
        state.info(format!("无法保存计划：{error}"));
        return;
    }
    if let Err(error) = session.bind_plan(
        path.clone(),
        production.plan.plan_id.clone(),
        production.plan.goal_id.clone(),
        digest,
    ) {
        state.info(format!("无法绑定计划：{error}"));
        return;
    }
    sync_tui_stage(state, session);
    state.info(bound_tui_command_output(format!(
        "{}\n\nPlanner journal: {}\n\n请确认目标和任务拆分。回复“确认计划”或执行 /approve；审批前不会写入项目，也不会创建 Worker Session。",
        production
            .render_plan_preview()
            .unwrap_or_else(|error| format!("无法渲染任务清单预览：{error}")),
        first_principles_agent::planning_journal_path(
            &session.root().join("planner-journal"),
            &production.plan.plan_id
        )
        .display()
    )));
}

fn approve_tui_plan(state: &mut TuiState, session: &mut TuiSession, args: &str) {
    let Some(mission) = session.mission().cloned() else {
        state.info("当前没有 active mission。");
        return;
    };
    let Some(plan) = mission.plan else {
        state.info("当前 mission 还没有计划。");
        return;
    };
    if !args.trim().is_empty() && args.trim() != plan.plan_id {
        state.info(format!("计划 ID 不匹配：{}", plan.plan_id));
        return;
    }
    let bytes = match std::fs::read(&plan.path) {
        Ok(bytes) if bytes.len() <= 4 * 1024 * 1024 => bytes,
        Ok(_) => {
            state.info("计划文件超过 4 MiB，拒绝批准。");
            return;
        }
        Err(error) => {
            state.info(format!("无法读取计划：{error}"));
            return;
        }
    };
    let digest = hex::encode(Sha256::digest(&bytes));
    if digest != plan.sha256 {
        state.info("计划内容已变化，拒绝批准；请重新生成或校验。");
        return;
    }
    let production = match serde_json::from_slice::<ProductionPlan>(&bytes) {
        Ok(production) => match production.validate() {
            Ok(()) => Some(production),
            Err(error) => {
                state.info(format!("生产计划校验失败：{error}"));
                return;
            }
        },
        Err(_) => None,
    };
    let actor = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".into());
    if plan.approved_by.is_none()
        && let Err(error) = session.approve_plan(actor.clone(), &digest, now_unix_ms())
    {
        state.info(format!("批准失败：{error}"));
        return;
    }

    if let Some(production) = production
        && plan.task_list_path.is_none()
    {
        let snapshot = match production.write_task_list(session.root().join("master-state")) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                state.info(format!(
                    "计划已批准，但任务清单生成失败：{error}；再次执行 /approve 可重试。"
                ));
                sync_tui_stage(state, session);
                return;
            }
        };
        if let Err(error) = session.bind_task_list(
            snapshot.path.clone(),
            snapshot.plan_sha256,
            snapshot.markdown_sha256,
        ) {
            state.info(format!(
                "任务清单已生成，但无法绑定到会话：{error}；再次执行 /approve 可重试。"
            ));
            sync_tui_stage(state, session);
            return;
        }
        state.info(format!(
            "计划已批准 · plan={} · actor={} · sha256={}\n任务清单：{}\n下一步：检查 Agent、窗口和权限后回复“确认执行”，或执行 /dispatch run",
            plan.plan_id,
            plan.approved_by.as_deref().unwrap_or(&actor),
            digest,
            snapshot.path.display()
        ));
    } else if plan.approved_by.is_some() {
        state.info(format!(
            "计划已由 {} 批准。",
            plan.approved_by.as_deref().unwrap_or(&actor)
        ));
    } else {
        state.info(format!(
            "计划已批准 · plan={} · actor={} · sha256={}",
            plan.plan_id, actor, digest
        ));
    }
    sync_tui_stage(state, session);
}

fn read_master_plan(path: &std::path::Path) -> std::result::Result<PlanProposal, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("计划文件超过 4 MiB".into());
    }
    let plan = if let Ok(production) = serde_json::from_slice::<ProductionPlan>(&bytes) {
        production.validate().map_err(|error| error.to_string())?;
        production.plan
    } else {
        serde_json::from_slice::<PlanProposal>(&bytes).map_err(|error| error.to_string())?
    };
    plan.validate().map_err(|error| error.to_string())?;
    Ok(plan)
}

fn read_production_plan(
    path: &std::path::Path,
) -> std::result::Result<Option<ProductionPlan>, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("计划文件超过 4 MiB".into());
    }
    let Ok(production) = serde_json::from_slice::<ProductionPlan>(&bytes) else {
        return Ok(None);
    };
    production.validate().map_err(|error| error.to_string())?;
    Ok(Some(production))
}

fn write_private_command_file(
    directory: &std::path::Path,
    path: &std::path::Path,
    bytes: &[u8],
) -> std::result::Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut temporary, bytes).map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

fn safe_command_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.') {
                byte as char
            } else {
                '_'
            }
        })
        .collect()
}

fn display_evidence(state: &mut TuiState, session: &TuiSession, args: &str) {
    if state.host_binding().is_workshop_master()
        && session
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| plan.master_trace_id.as_ref())
            .is_some()
        && args.trim() != "run"
    {
        if !matches!(args.trim(), "" | "master") {
            state.info("用法：/evidence [master|run]");
            return;
        }
        display_master_evidence(state, session);
        return;
    }
    if args.trim() == "master" {
        state.info("当前 Master 尚未派发。");
        return;
    }
    match load_latest_replay(state) {
        Ok((journal, replay)) => {
            state.info(bound_tui_command_output(evidence_summary(
                &journal, &replay,
            )));
        }
        Err(error) => state.info(error),
    }
}

fn display_report(state: &mut TuiState, session: &TuiSession, args: &str) {
    if state.host_binding().is_workshop_master()
        && session
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| plan.master_trace_id.as_ref())
            .is_some()
        && args.trim() != "run"
    {
        if !matches!(args.trim(), "" | "master") {
            state.info("用法：/report [master|run]");
            return;
        }
        display_master_report(state, session);
        return;
    }
    if args.trim() == "master" {
        state.info("当前 Master 尚未派发。");
        return;
    }
    match load_latest_replay(state) {
        Ok((journal, replay)) => {
            state.info(bound_tui_command_output(run_report(&journal, &replay)));
        }
        Err(error) => state.info(error),
    }
}

fn display_audit(state: &mut TuiState, session: &TuiSession, args: &str) {
    let action = args.trim();
    if state.host_binding().is_workshop_master()
        && session
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| plan.master_trace_id.as_ref())
            .is_some()
        && matches!(action, "" | "master" | "all")
    {
        display_master_audit(state, session, action == "all");
        return;
    }
    if action == "master" {
        state.info("当前 Master 尚未派发。");
        return;
    }
    let journals = if action == "all" {
        session
            .journals()
            .iter()
            .rev()
            .take(20)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
    } else if action.is_empty() || action == "run" {
        state.last_journal().cloned().into_iter().collect()
    } else {
        state.info("用法：/audit [run|all]");
        return;
    };
    if journals.is_empty() {
        state.info("当前会话还没有可审计的 run journal。");
        return;
    }
    let mut outputs = Vec::new();
    for journal in journals {
        match read_events(&journal) {
            Ok(events) => {
                let audit = audit_events(&events);
                outputs.push(audit_summary(&journal, &audit));
            }
            Err(error) => outputs.push(format!("无法读取 {}：{error}", journal.display())),
        }
    }
    state.info(bound_tui_command_output(outputs.join("\n\n")));
}

fn display_master_artifacts(state: &mut TuiState, session: &TuiSession, args: &str) {
    let (_, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    match store.load() {
        Ok(master) => state.info(bound_tui_command_output(artifacts_summary(&master, args))),
        Err(error) => state.info(format!("无法读取 MasterState：{error}")),
    }
}

fn verify_tui_task(state: &mut TuiState, session: &TuiSession, args: &str) {
    let Some((task_id, input)) = args.trim().split_once(char::is_whitespace) else {
        state.info("用法：/verify <task-id> <verification.json>");
        return;
    };
    let input = input.trim();
    if task_id.is_empty() || input.is_empty() {
        state.info("用法：/verify <task-id> <verification.json>");
        return;
    }
    let (_, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    match verify_master_task(&store, &PathBuf::from(input), Some(task_id)) {
        Ok((result, _)) => {
            let detail = result
                .failure_reason
                .as_deref()
                .map(|reason| format!("\n原因：{reason}"))
                .unwrap_or_default();
            state.info(format!("验收：{:?}{detail}", result.status));
        }
        Err(error) => state.info(format!("验收失败：{error}")),
    }
}

fn display_master_evidence(state: &mut TuiState, session: &TuiSession) {
    let (_, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    match store.load() {
        Ok(master) => state.info(bound_tui_command_output(master_evidence_summary(&master))),
        Err(error) => state.info(format!("无法读取 MasterState：{error}")),
    }
}

fn display_master_report(state: &mut TuiState, session: &TuiSession) {
    let (_, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    match store.load() {
        Ok(master) => state.info(bound_tui_command_output(master_report(&master))),
        Err(error) => state.info(format!("无法读取 MasterState：{error}")),
    }
}

fn display_master_audit(state: &mut TuiState, session: &TuiSession, include_runs: bool) {
    let (_, _, store) = match tui_master_store(session) {
        Ok(value) => value,
        Err(error) => {
            state.info(error);
            return;
        }
    };
    let journal = store.trace_dir().join("events.jsonl");
    let mut outputs = match replay_master_journal(&journal) {
        Ok((master, audit)) => vec![master_audit_summary(&master, &audit, &journal)],
        Err(error) => vec![format!(
            "Master Journal 审计 · 失败\njournal: {}\n无法 replay：{error}",
            journal.display()
        )],
    };
    if include_runs {
        outputs.extend(run_journal_audits(state, session, true));
    }
    state.info(bound_tui_command_output(outputs.join("\n\n")));
}

fn run_journal_audits(state: &TuiState, session: &TuiSession, all: bool) -> Vec<String> {
    let journals = if all {
        session
            .journals()
            .iter()
            .rev()
            .take(20)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        state.last_journal().cloned().into_iter().collect()
    };
    if journals.is_empty() {
        return vec!["当前会话还没有可审计的 run journal。".into()];
    }
    journals
        .into_iter()
        .map(|journal| match read_events(&journal) {
            Ok(events) => audit_summary(&journal, &audit_events(&events)),
            Err(error) => format!("无法读取 {}：{error}", journal.display()),
        })
        .collect()
}

fn display_tui_status(state: &mut TuiState, session: &TuiSession, settings: &TuiRunSettings) {
    let facts = session_context_facts(session);
    let context_used = facts.iter().map(|fact| fact.chars().count()).sum::<usize>();
    let headroom = settings.max_context_chars.saturating_sub(context_used);
    let summary = session
        .context_summary()
        .filter(|summary| summary.covered_journals == session.journals().len())
        .map(|summary| {
            format!(
                "已生成（覆盖 {} 次运行，约 {} 字符）",
                summary.covered_journals,
                summary.text.chars().count()
            )
        })
        .unwrap_or_else(|| "尚未生成".into());
    let workspace = session
        .workspace()
        .map(|workspace| workspace.display().to_string())
        .unwrap_or_else(|| "尚未设置".into());
    let (provider, model) = state.provider_label.rsplit_once('/').map_or(
        (state.provider_label.as_str(), "未指定"),
        |(provider, model)| (provider, model),
    );
    state.info(format!(
        "运行概览\n\
         当前状态      {}\n\
         任务类型      {}\n\
         风险等级      {}风险\n\
         运行环境      {}\n\
         \n\
         会话与工作区\n\
         会话标题      {}\n\
         会话 ID       {}\n\
         会话状态      {}\n\
         持久化工作区  {}\n\
         本次运行目录  {}\n\
         已引用路径    {} 个\n\
         \n\
         模型与运行限制\n\
         模型服务      {}\n\
         模型          {}\n\
         推理步骤      最多 {} 步\n\
         工具调用      最多 {} 次\n\
         模型尝试      最多 {} 次\n\
         单次超时      {} 秒\n\
         \n\
         上下文与压缩\n\
         上下文上限    {} 字符\n\
         当前占用      {} 字符\n\
         剩余空间      {} 字符\n\
         自动压缩      达到 {}% 时触发\n\
         压缩摘要      {}",
        state.status.label_zh(),
        state.mode.label_zh(),
        state.risk.label_zh(),
        state.host_binding().kind().label_zh(),
        session.title(),
        session.id(),
        if session.archived() {
            "已归档"
        } else {
            "使用中"
        },
        workspace,
        settings.cwd.display(),
        session.mentions().len(),
        provider,
        model,
        settings.max_steps,
        settings.max_tool_calls,
        settings.max_provider_attempts,
        settings.timeout_seconds,
        settings.max_context_chars,
        context_used,
        headroom,
        settings.auto_compact_percent,
        summary
    ));
}

fn display_journal_paths(state: &mut TuiState, session: &TuiSession) {
    let mut output = String::new();
    if let Some(journal) = state.last_journal() {
        output.push_str(&format!("最近运行日志：{}", journal.display()));
    } else {
        output.push_str("最近运行日志：尚未生成");
    }
    if let Some(plan) = session.mission().and_then(|mission| mission.plan.as_ref())
        && let (Some(trace_id), Some(root)) = (&plan.master_trace_id, &plan.master_state_root)
    {
        output.push_str(&format!(
            "\n生产线任务：{}\n总控日志：{}",
            trace_id,
            root.join(trace_id).join("events.jsonl").display()
        ));
    }
    state.info(output);
}

fn context_recap(session: &TuiSession, max_chars: usize) -> Result<(String, usize), String> {
    const MAX_TURNS: usize = 8;
    let mut recap = String::new();
    let mut covered = 0;
    for journal in session.journals().iter().rev().take(MAX_TURNS).rev() {
        let Ok(events) = read_events(journal) else {
            continue;
        };
        let Ok(replay) = replay_events(&events) else {
            continue;
        };
        let (Some(request), Some(answer)) = (replay.request, replay.answer) else {
            continue;
        };
        let entry = format!(
            "- 任务：{}\n  结论：{}\n",
            bounded_tui_text(request.objective.trim(), 500),
            bounded_tui_text(answer.conclusion.trim(), 700)
        );
        if recap.chars().count().saturating_add(entry.chars().count()) > max_chars {
            break;
        }
        recap.push_str(&entry);
        covered += 1;
    }
    if recap.is_empty() {
        return Err("当前会话没有可压缩的已完成运行。".into());
    }
    Ok((recap, covered))
}

fn handle_context_command(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) {
    let action = if args.trim().is_empty() {
        "show"
    } else {
        args.trim()
    };
    match action {
        "show" => {
            let facts = session_context_facts(session);
            let used = facts.iter().map(|fact| fact.chars().count()).sum::<usize>();
            state.info(format!(
                "上下文使用情况\n\
                 预算上限      {} 字符\n\
                 已注入        {} 字符\n\
                 剩余空间      {} 字符\n\
                 自动压缩      达到 {}% 时触发\n\
                 压缩摘要      {}\n\
                 \n\
                 自动控制方式：裁剪旧工具结果、限制工具输出、保留最近 6 轮事实，\n\
                 并执行模型、工具和推理步骤预算。字符预算不是精确 token 计数。",
                settings.max_context_chars,
                used,
                settings.max_context_chars.saturating_sub(used),
                settings.auto_compact_percent,
                session
                    .context_summary()
                    .map(|summary| format!(
                        "已生成（覆盖 {} 次运行，约 {} 字符）",
                        summary.covered_journals,
                        summary.text.chars().count()
                    ))
                    .unwrap_or_else(|| "尚未生成".into())
            ));
        }
        "recap" => match context_recap(session, 6_000) {
            Ok((recap, covered)) => {
                state.info(format!("上下文回顾（覆盖 {covered} 次运行）\n{recap}"))
            }
            Err(error) => state.info(error),
        },
        "compact" => {
            let max_chars = settings.max_context_chars.clamp(1_000, 8_000);
            match context_recap(session, max_chars) {
                Ok((recap, covered)) => match session.set_context_summary(
                    recap.clone(),
                    session.journals().len(),
                    now_unix_ms(),
                ) {
                    Ok(()) => state.info(format!(
                        "上下文已压缩\n\
                         覆盖运行      {covered} 次\n\
                         摘要长度      {} 字符\n\
                         后续运行将只注入该摘要",
                        recap.chars().count()
                    )),
                    Err(error) => state.info(format!("无法保存上下文摘要：{error}")),
                },
                Err(error) => state.info(error),
            }
        }
        _ => state.info("用法：/context [show|recap|compact]"),
    }
}

fn apply_session_preferences(
    state: &mut TuiState,
    settings: &mut TuiRunSettings,
    session: &TuiSession,
) {
    settings.mentions = session.mentions().to_vec();
    if let Some(workspace) = session.workspace()
        && workspace.is_dir()
    {
        settings.cwd = workspace.to_path_buf();
    }
    let preferences = session
        .preferences()
        .cloned()
        .unwrap_or_else(|| settings.default_preferences.clone());
    settings.max_steps = preferences.max_steps;
    settings.max_tool_calls = preferences.max_tool_calls;
    settings.max_provider_attempts = preferences.max_provider_attempts;
    settings.max_context_chars = preferences.max_context_chars;
    settings.auto_compact_percent = preferences.auto_compact_percent;
    settings.timeout_seconds = preferences.timeout_seconds;
    state.max_steps = preferences.max_steps;
    state.timeout_seconds = preferences.timeout_seconds;
}

fn handle_config_command(
    state: &mut TuiState,
    settings: &mut TuiRunSettings,
    session: &mut TuiSession,
    args: &str,
) {
    let trimmed = args.trim();
    if trimmed.is_empty() || trimmed == "show" {
        state.info(format!(
            "运行配置\n\
             配置来源      {}\n\
             推理步骤      最多 {} 步\n\
             工具调用      最多 {} 次\n\
             模型尝试      最多 {} 次\n\
             上下文上限    {} 字符\n\
             自动压缩      达到 {}% 时触发\n\
             单次超时      {} 秒\n\
             \n\
             修改方法：/config set <steps|tools|attempts|context|compact|timeout> <value>\n\
             恢复默认：/config reset",
            if session.preferences().is_some() {
                "当前会话设置"
            } else {
                "启动参数"
            },
            settings.max_steps,
            settings.max_tool_calls,
            settings.max_provider_attempts,
            settings.max_context_chars,
            settings.auto_compact_percent,
            settings.timeout_seconds
        ));
        return;
    }
    if trimmed == "reset" {
        match session.set_preferences(None) {
            Ok(()) => {
                apply_session_preferences(state, settings, session);
                state.info("运行配置已恢复为启动参数。");
            }
            Err(error) => state.info(format!("无法恢复运行配置：{error}")),
        }
        return;
    }
    let Some(rest) = trimmed.strip_prefix("set ") else {
        state.info("用法：/config set <steps|tools|attempts|context|compact|timeout> <value>");
        return;
    };
    let Some((key, value)) = rest.split_once(char::is_whitespace) else {
        state.info("用法：/config set <配置项> <数值>");
        return;
    };
    let Ok(value) = value.trim().parse::<u64>() else {
        state.info("配置值必须是正整数。");
        return;
    };
    let mut preferences = session
        .preferences()
        .cloned()
        .unwrap_or_else(|| settings.default_preferences.clone());
    let Ok(value_usize) = usize::try_from(value) else {
        state.info("配置值超出平台范围。");
        return;
    };
    match key {
        "steps" => preferences.max_steps = value_usize,
        "tools" => preferences.max_tool_calls = value_usize,
        "attempts" => preferences.max_provider_attempts = value_usize,
        "context" => preferences.max_context_chars = value_usize,
        "compact" => {
            let Ok(value) = u8::try_from(value) else {
                state.info("自动压缩阈值超出平台范围。");
                return;
            };
            preferences.auto_compact_percent = value;
        }
        "timeout" => preferences.timeout_seconds = value,
        _ => {
            state.info("未知配置项。");
            return;
        }
    }
    if let Err(error) = session.set_preferences(Some(preferences)) {
        state.info(format!("运行配置超出安全范围：{error}"));
        return;
    }
    apply_session_preferences(state, settings, session);
    state.info(format!(
        "运行配置已更新\n\
         推理步骤      最多 {} 步\n\
         工具调用      最多 {} 次\n\
         模型尝试      最多 {} 次\n\
         上下文上限    {} 字符\n\
         自动压缩      达到 {}% 时触发\n\
         单次超时      {} 秒",
        settings.max_steps,
        settings.max_tool_calls,
        settings.max_provider_attempts,
        settings.max_context_chars,
        settings.auto_compact_percent,
        settings.timeout_seconds
    ));
}

fn path_within_roots(path: &std::path::Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

fn workspace_overview_text(session: &TuiSession, settings: &TuiRunSettings) -> String {
    format!(
        "工作区概览\n\
         当前运行目录  {}\n\
         会话工作区    {}\n\
         任务工作区    {}\n\
         授权目录：\n{}\n\
         已引用路径：\n{}",
        settings.cwd.display(),
        session
            .workspace()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".into()),
        session
            .mission()
            .map(|mission| mission.workspace.display().to_string())
            .unwrap_or_else(|| "-".into()),
        settings
            .roots
            .iter()
            .map(|root| format!("- {}", root.display()))
            .collect::<Vec<_>>()
            .join("\n"),
        session
            .mentions()
            .iter()
            .map(|path| format!("- {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn workspace_main_menu() -> Vec<CommandMenuItem> {
    vec![
        CommandMenuItem {
            id: "workspace.set".into(),
            label: "设置工作区".into(),
            description: "选择授权根目录或输入路径".into(),
        },
        CommandMenuItem {
            id: "workspace.mention".into(),
            label: "引用路径".into(),
            description: "将路径加入当前会话的已知事实".into(),
        },
        CommandMenuItem {
            id: "workspace.diff".into(),
            label: "Git diff".into(),
            description: "查看当前工作区状态和变更".into(),
        },
        CommandMenuItem {
            id: "workspace.init".into(),
            label: "初始化 AGENTS.md".into(),
            description: "创建项目开发规则文件".into(),
        },
        CommandMenuItem {
            id: "workspace.worktree".into(),
            label: "Git worktrees".into(),
            description: "列出关联 worktree".into(),
        },
        CommandMenuItem {
            id: "workspace.worktree.add".into(),
            label: "新建 worktree…".into(),
            description: "输入路径和分支后创建".into(),
        },
    ]
}

fn show_workspace_panel(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
    notice: Option<String>,
) {
    state.begin_command_panel("/workspace");
    state.info(workspace_overview_text(session, settings));
    if let Some(notice) = notice {
        state.info(notice);
    }
    state.set_command_panel_menu(workspace_main_menu());
}

fn show_workspace_set_menu(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
    notice: Option<String>,
) {
    state.begin_command_subpanel("设置工作区");
    state.info(format!(
        "当前运行目录：{}\n选择授权根目录，或输入自定义路径。",
        settings.cwd.display()
    ));
    if let Some(notice) = notice {
        state.info(notice);
    }
    let mut items = settings
        .roots
        .iter()
        .enumerate()
        .map(|(index, root)| CommandMenuItem {
            id: format!("workspace.set.root.{index}"),
            label: format!("根目录 {}", index + 1),
            description: format!(
                "{}{}",
                root.display(),
                if session.workspace() == Some(root) {
                    "（当前）"
                } else {
                    ""
                }
            ),
        })
        .collect::<Vec<_>>();
    items.push(CommandMenuItem {
        id: "workspace.set.manual".into(),
        label: "输入路径…".into(),
        description: "输入授权 roots 内的目录".into(),
    });
    items.push(CommandMenuItem {
        id: "workspace.back".into(),
        label: "返回".into(),
        description: "回到工作区菜单".into(),
    });
    state.set_command_panel_menu(items);
}

fn switch_workspace(
    session: &mut TuiSession,
    settings: &mut TuiRunSettings,
    raw_path: &str,
) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(raw_path.trim())
        .map_err(|_| "目标 workspace 不存在。".to_string())?;
    if !path.is_dir() || !path_within_roots(&path, &settings.roots) {
        return Err("目标 workspace 必须是现有目录，并位于已授权 read roots 内。".into());
    }
    session
        .set_workspace(path.clone())
        .map_err(|error| format!("无法保存 workspace：{error}"))?;
    settings.cwd = path.clone();
    Ok(path)
}

fn mention_workspace_path(
    session: &mut TuiSession,
    settings: &mut TuiRunSettings,
    raw_path: &str,
) -> Result<PathBuf, String> {
    let path =
        std::fs::canonicalize(raw_path.trim()).map_err(|_| "mention 路径不存在。".to_string())?;
    if !path_within_roots(&path, &settings.roots) {
        return Err("mention 路径不在授权 read roots 内。".into());
    }
    session
        .add_mention(path.clone())
        .map_err(|error| format!("无法保存 mention：{error}"))?;
    settings.mentions = session.mentions().to_vec();
    Ok(path)
}

fn activate_workspace_menu_item(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &mut TuiRunSettings,
    id: &str,
) -> bool {
    match id {
        "workspace.set" => show_workspace_set_menu(state, session, settings, None),
        "workspace.set.manual" => {
            state.close_command_panel();
            state.set_input("/workspace cd ");
            state.set_notice("输入工作区路径后按 Enter");
        }
        "workspace.mention" => {
            state.close_command_panel();
            state.set_input("/workspace mention ");
            state.set_notice("输入要引用的路径后按 Enter");
        }
        "workspace.diff" => handle_workspace_command(state, session, settings, "diff"),
        "workspace.init" => handle_workspace_command(state, session, settings, "init"),
        "workspace.worktree" => handle_workspace_command(state, session, settings, "worktree"),
        "workspace.worktree.add" => {
            state.close_command_panel();
            state.set_input("/workspace worktree add ");
            state.set_notice("输入 worktree 路径和分支后按 Enter");
        }
        "workspace.back" => {
            state.back_command_panel();
        }
        id if id.starts_with("workspace.set.root.") => {
            let Some(index) = id
                .strip_prefix("workspace.set.root.")
                .and_then(|index| index.parse::<usize>().ok())
            else {
                return false;
            };
            let Some(root) = settings.roots.get(index).cloned() else {
                state.info("授权根目录已变化，请重新选择。");
                return true;
            };
            match switch_workspace(session, settings, &root.display().to_string()) {
                Ok(path) => show_workspace_panel(
                    state,
                    session,
                    settings,
                    Some(format!("工作区已切换：{}", path.display())),
                ),
                Err(error) => {
                    state.info(error);
                }
            }
        }
        _ => return false,
    }
    true
}

fn handle_workspace_command(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &mut TuiRunSettings,
    args: &str,
) {
    let trimmed = args.trim();
    if trimmed.is_empty() || trimmed == "show" {
        show_workspace_panel(state, session, settings, None);
        return;
    }
    if let Some(path) = trimmed.strip_prefix("cd ") {
        match switch_workspace(session, settings, path) {
            Ok(path) => show_workspace_panel(
                state,
                session,
                settings,
                Some(format!("工作区已切换：{}", path.display())),
            ),
            Err(error) => {
                state.info(error);
                state.set_command_panel_menu(workspace_main_menu());
            }
        }
        return;
    }
    if let Some(path) = trimmed.strip_prefix("mention ") {
        match mention_workspace_path(session, settings, path) {
            Ok(path) => show_workspace_panel(
                state,
                session,
                settings,
                Some(format!("已记录引用：{}", path.display())),
            ),
            Err(error) => {
                state.info(error);
                state.set_command_panel_menu(workspace_main_menu());
            }
        }
        return;
    }
    match trimmed {
        "diff" => match git_diff(&settings.cwd) {
            Ok(output) => {
                state.info(bound_tui_command_output(output));
                state.set_command_panel_menu(workspace_main_menu());
            }
            Err(error) => {
                state.info(format!("无法读取 git diff：{error}"));
                state.set_command_panel_menu(workspace_main_menu());
            }
        },
        "init" => match init_agents_file(&settings.cwd) {
            Ok(path) => {
                state.info(format!("已创建 {}", path.display()));
                state.set_command_panel_menu(workspace_main_menu());
            }
            Err(error) => {
                state.info(format!("无法创建 AGENTS.md：{error}"));
                state.set_command_panel_menu(workspace_main_menu());
            }
        },
        "worktree" => match git_worktrees(&settings.cwd) {
            Ok(output) => {
                state.info(bound_tui_command_output(format!("Git worktrees\n{output}")));
                state.set_command_panel_menu(workspace_main_menu());
            }
            Err(error) => {
                state.info(format!("无法列出 worktree：{error}"));
                state.set_command_panel_menu(workspace_main_menu());
            }
        },
        _ => {
            if let Some(rest) = trimmed.strip_prefix("worktree add ") {
                let Some((path, branch)) = rest.split_once(char::is_whitespace) else {
                    state.info("用法：/workspace worktree add <path> <branch>");
                    state.set_command_panel_menu(workspace_main_menu());
                    return;
                };
                let path = PathBuf::from(path.trim());
                let path = if path.is_absolute() {
                    path
                } else {
                    settings.cwd.join(path)
                };
                let Some(parent) = path.parent() else {
                    state.info("worktree 路径无效。");
                    state.set_command_panel_menu(workspace_main_menu());
                    return;
                };
                let Ok(parent) = std::fs::canonicalize(parent) else {
                    state.info("worktree 父目录不存在。");
                    state.set_command_panel_menu(workspace_main_menu());
                    return;
                };
                let path = parent.join(
                    path.file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("worktree")),
                );
                if !path_within_roots(&path, &settings.roots) {
                    state.info("worktree 路径不在授权 read roots 内。");
                    state.set_command_panel_menu(workspace_main_menu());
                    return;
                }
                match add_git_worktree(&settings.cwd, &path, branch.trim()) {
                    Ok(path) => {
                        state.info(format!("Worktree 已创建：{}", path.display()));
                        state.set_command_panel_menu(workspace_main_menu());
                    }
                    Err(error) => {
                        state.info(format!("无法创建 worktree：{error}"));
                        state.set_command_panel_menu(workspace_main_menu());
                    }
                }
            } else {
                state.info(
                    "用法：/workspace [show|cd <dir>|mention <path>|diff|init|worktree [add <path> <branch>]]",
                );
                state.set_command_panel_menu(workspace_main_menu());
            }
        }
    }
}

fn switch_tui_session(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &mut TuiRunSettings,
    next: TuiSession,
) {
    *session = next;
    state.clear();
    state.set_session_info(session.id());
    apply_session_preferences(state, settings, session);
    sync_tui_stage(state, session);
    restore_tui_session(state, session);
    state.info(format!(
        "当前会话已切换\n会话 ID       {}\n会话标题      {}",
        session.id(),
        session.title()
    ));
}

fn handle_session_command(
    state: &mut TuiState,
    session: &mut TuiSession,
    settings: &mut TuiRunSettings,
    args: &str,
) {
    let trimmed = args.trim();
    let (action, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    match action {
        "" | "show" => state.info(format!(
            "当前会话\n\
             会话 ID       {}\n\
             会话标题      {}\n\
             会话状态      {}\n\
             运行记录      {} 次\n\
             继续方式      {}",
            session.id(),
            session.title(),
            if session.archived() {
                "已归档"
            } else {
                "使用中"
            },
            session.journals().len(),
            session.continue_command()
        )),
        "list" => {
            let include_archived = rest.trim() == "all";
            match TuiSession::list(session.journal_root()) {
                Ok(sessions) => {
                    let rows = sessions
                        .into_iter()
                        .filter(|summary| include_archived || !summary.archived)
                        .map(|summary| {
                            format!(
                                "- {}{}\n\
                                 标题          {}\n\
                                 运行记录      {} 次\n\
                                 会话状态      {}",
                                summary.id,
                                if summary.id == session.id() {
                                    "（当前）"
                                } else {
                                    ""
                                },
                                summary.title,
                                summary.runs,
                                if summary.archived {
                                    "已归档"
                                } else {
                                    "使用中"
                                }
                            )
                        })
                        .collect::<Vec<_>>();
                    state.info(if rows.is_empty() {
                        "没有匹配的会话。".into()
                    } else {
                        format!("会话列表\n{}", rows.join("\n"))
                    });
                }
                Err(error) => state.info(format!("无法列出会话：{error}")),
            }
        }
        "new" => match TuiSession::open(session.journal_root(), None) {
            Ok(mut next) => {
                if !rest.trim().is_empty()
                    && let Err(error) = next.rename(rest.trim())
                {
                    state.info(format!("无法设置新会话标题：{error}"));
                    return;
                }
                switch_tui_session(state, session, settings, next);
            }
            Err(error) => state.info(format!("无法创建会话：{error}")),
        },
        "resume" => {
            let id = rest.trim();
            if id.is_empty() {
                state.info("用法：/session resume <session-id>");
                return;
            }
            match TuiSession::open(session.journal_root(), Some(id)) {
                Ok(next) => switch_tui_session(state, session, settings, next),
                Err(error) => state.info(format!("无法恢复会话：{error}")),
            }
        }
        "fork" => match session.fork((!rest.trim().is_empty()).then_some(rest.trim())) {
            Ok(next) => switch_tui_session(state, session, settings, next),
            Err(error) => state.info(format!("无法复制会话：{error}")),
        },
        "rename" => {
            if rest.trim().is_empty() {
                state.info("用法：/session rename <title>");
                return;
            }
            match session.rename(rest.trim()) {
                Ok(()) => state.info(format!("会话已重命名为：{}", session.title())),
                Err(error) => state.info(format!("重命名失败：{error}")),
            }
        }
        "archive" => {
            let target = rest.trim();
            let result = if target.is_empty() || target == session.id() {
                session.set_archived(true)
            } else {
                TuiSession::open(session.journal_root(), Some(target))
                    .and_then(|mut session| session.set_archived(true))
            };
            match result {
                Ok(()) => state.info("会话已归档。"),
                Err(error) => state.info(format!("归档会话失败：{error}")),
            }
        }
        "delete" => {
            let mut parts = rest.split_whitespace();
            let target = parts.next().unwrap_or_default();
            let confirmation = (parts.next() == Some("confirm")).then(|| parts.next());
            if target.is_empty()
                || confirmation.flatten() != Some(target)
                || parts.next().is_some()
            {
                state.info("用法：/session delete <session-id> confirm <session-id>");
                return;
            }
            if target == session.id() {
                state.info("不能删除当前会话；请先新建或切换到其他会话。");
                return;
            }
            match TuiSession::open(session.journal_root(), Some(target)) {
                Ok(target_session) => match target_session.trash() {
                    Ok(path) => state.info(format!("会话已移入回收目录：{}", path.display())),
                    Err(error) => state.info(format!("删除会话失败：{error}")),
                },
                Err(error) => state.info(format!("无法打开目标会话：{error}")),
            }
        }
        _ => state.info(
            "用法：/session [show|list [all]|new [title]|resume <id>|fork [title]|rename <title>|archive [id]|delete <id> confirm <id>]",
        ),
    }
}

fn handle_preset_command(
    state: &mut TuiState,
    session: &TuiSession,
    settings: &TuiRunSettings,
    args: &str,
) -> Option<TaskRequest> {
    let trimmed = args.trim();
    if trimmed.is_empty() || trimmed == "list" {
        match list_presets(session) {
            Ok(presets) => {
                let rows = presets
                    .iter()
                    .map(|preset| {
                        format!(
                            "- {}\n  内容：{}",
                            preset.name,
                            bounded_tui_text(&preset.instruction, 120)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                state.info(if rows.is_empty() {
                    "没有已保存的预设。".into()
                } else {
                    format!("已保存的预设\n{rows}")
                });
            }
            Err(error) => state.info(format!("无法列出预设：{error}")),
        }
        return None;
    }
    let (action, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    match action {
        "save" => {
            let (name, instruction) = match rest.split_once(char::is_whitespace) {
                Some((name, instruction)) => (name, instruction.to_string()),
                None if !state.last_instruction.trim().is_empty() => {
                    (rest.trim(), state.last_instruction.clone())
                }
                None => {
                    state.info("用法：/preset save <name> [instruction]");
                    return None;
                }
            };
            match TuiPreset::new(name, &instruction)
                .and_then(|preset| save_preset(session, &preset).map(|path| (preset, path)))
            {
                Ok((preset, path)) => state.info(format!(
                    "预设已保存：{}\n位置：{}",
                    preset.name,
                    path.display()
                )),
                Err(error) => state.info(format!("无法保存预设：{error}")),
            }
        }
        "show" => match load_preset(session, rest.trim()) {
            Ok(preset) => state.info(format!(
                "预设：{}\n内容：{}",
                preset.name, preset.instruction
            )),
            Err(error) => state.info(format!("无法读取预设：{error}")),
        },
        "delete" => match trash_preset(session, rest.trim()) {
            Ok(path) => state.info(format!("预设已移入回收目录：{}", path.display())),
            Err(error) => state.info(format!("无法删除预设：{error}")),
        },
        "run" => match load_preset(session, rest.trim()) {
            Ok(preset) => return Some(settings.request(&preset.instruction)),
            Err(error) => state.info(format!("无法运行预设：{error}")),
        },
        _ => state.info(
            "用法：/preset [list|save <name> <instruction>|show <name>|run <name>|delete <name>]",
        ),
    }
    None
}

fn export_tui_data(
    state: &mut TuiState,
    session: &TuiSession,
    kind: &str,
    destination: Option<&str>,
) -> Result<PathBuf, String> {
    let text = match kind {
        "session" => format!(
            "# 1A Session\n\n- id: {}\n- title: {}\n- archived: {}\n- runs: {}\n- workspace: {}\n\n## Journals\n{}",
            session.id(),
            session.title(),
            session.archived(),
            session.journals().len(),
            session
                .workspace()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".into()),
            session
                .journals()
                .iter()
                .map(|path| format!("- {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        ),
        "report" => {
            if state.host_binding().is_workshop_master()
                && let Ok((_, _, store)) = tui_master_store(session)
                && let Ok(master) = store.load()
            {
                master_report(&master)
            } else {
                let (journal, replay) = load_latest_replay(state)?;
                run_report(&journal, &replay)
            }
        }
        "evidence" => {
            if state.host_binding().is_workshop_master()
                && let Ok((_, _, store)) = tui_master_store(session)
                && let Ok(master) = store.load()
            {
                master_evidence_summary(&master)
            } else {
                let (journal, replay) = load_latest_replay(state)?;
                evidence_summary(&journal, &replay)
            }
        }
        _ => return Err("用法：/export <session|report|evidence> [path]".into()),
    };
    let path = destination
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                session.journal_root().join(path)
            }
        })
        .unwrap_or_else(|| {
            session
                .root()
                .join("exports")
                .join(format!("{kind}-{}.md", now_unix_ms()))
        });
    let parent = path
        .parent()
        .ok_or_else(|| "export 路径没有父目录。".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| error.to_string())?;
    std::io::Write::write_all(&mut file, text.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(path)
}

fn handle_export_command(state: &mut TuiState, session: &TuiSession, args: &str) {
    let trimmed = args.trim();
    let (kind, destination) = trimmed
        .split_once(char::is_whitespace)
        .map_or((trimmed, None), |(kind, path)| {
            (kind, (!path.trim().is_empty()).then_some(path.trim()))
        });
    match export_tui_data(state, session, kind, destination) {
        Ok(path) => state.info(format!("已导出：{}", path.display())),
        Err(error) => state.info(format!("导出失败：{error}")),
    }
}

fn handle_copy_command(state: &mut TuiState) {
    match load_latest_replay(state) {
        Ok((_, replay)) => {
            let Some(answer) = replay.answer else {
                state.info("最近运行没有最终回答。");
                return;
            };
            match copy_to_clipboard(&answer.conclusion) {
                Ok(()) => state.info("最近回答已复制。"),
                Err(error) => state.info(format!("复制失败：{error}")),
            }
        }
        Err(error) => state.info(error),
    }
}

fn handle_cost_command(state: &mut TuiState, session: &TuiSession) {
    match build_usage_ledger(session.journals().iter().cloned()) {
        Ok(ledger) => state.info(bound_tui_command_output(render_usage_ledger(&ledger))),
        Err(error) => state.info(format!("无法读取 usage ledger：{error}")),
    }
}

fn bound_tui_command_output(mut text: String) -> String {
    const MAX_CHARS: usize = 24_000;
    if text.chars().count() > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect();
        text.push_str("\n…（命令输出已截断，完整内容见 journal）");
    }
    text
}

fn load_latest_replay(
    state: &TuiState,
) -> std::result::Result<(PathBuf, first_principles_agent::ReplayState), String> {
    let journal = state
        .last_journal()
        .cloned()
        .ok_or_else(|| "当前会话还没有可读取的 run journal。".to_string())?;
    let events = read_events(&journal).map_err(|error| error.to_string())?;
    let replay = replay_events(&events)?;
    Ok((journal, replay))
}

async fn run_tui_interactive(
    mut runtime: AgentRuntime,
    mut state: TuiState,
    mut settings: TuiRunSettings,
    mut session: TuiSession,
) -> Result<()> {
    let mut terminal = TerminalSession::enter().context("cannot initialize terminal")?;
    let mut inputs = spawn_input_reader();
    state.set_session_info(session.id());
    if session.journals().is_empty() {
        // The empty transcript renders the dedicated welcome surface.
    } else {
        restore_tui_session(&mut state, &session);
    }
    draw_tui_frame(&mut terminal, &mut state)?;

    let mut selection_autoscroll_ticker = tokio::time::interval(Duration::from_millis(70));
    selection_autoscroll_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let input = tokio::select! {
            input = inputs.recv() => input,
            _ = selection_autoscroll_ticker.tick(), if state.selection_autoscroll_active() => {
                if state.autoscroll_selection_once() {
                    draw_tui_frame(&mut terminal, &mut state)?;
                }
                continue;
            }
        };
        let Some(mut input) = input else { break };
        if !matches!(&input, TuiInput::MouseUp { .. }) {
            state.clear_notice();
        }
        if state.pending_confirmation().is_some()
            && !matches!(
                &input,
                TuiInput::Enter | TuiInput::Escape | TuiInput::CtrlQ | TuiInput::Resize
            )
        {
            state.clear_confirmation();
        }
        if matches!(&input, TuiInput::CtrlQ) {
            state.close_mode_menu();
            state.close_details();
            state.close_command_panel();
            state.clear_confirmation();
            state.request_quit();
            draw_tui_frame(&mut terminal, &mut state)?;
            continue;
        }
        if state.quit_requested()
            && !matches!(
                &input,
                TuiInput::Enter | TuiInput::Escape | TuiInput::CtrlQ | TuiInput::Resize
            )
        {
            state.cancel_quit();
        }
        if state.mode_menu_open() {
            match input {
                TuiInput::CtrlTab | TuiInput::Escape | TuiInput::Left => {
                    state.close_mode_menu();
                }
                TuiInput::Up => state.move_mode_menu_selection(-1),
                TuiInput::Down => state.move_mode_menu_selection(1),
                TuiInput::Enter | TuiInput::Right => state.apply_mode_menu(),
                TuiInput::Resize => {}
                _ => {}
            }
            draw_tui_frame(&mut terminal, &mut state)?;
            continue;
        }
        if state.details_open() {
            match input {
                TuiInput::Escape => state.close_details(),
                TuiInput::Up => state.scroll_details_up(1),
                TuiInput::Down => state.scroll_details_down(1),
                TuiInput::PageUp | TuiInput::ScrollUp => state.scroll_details_up(8),
                TuiInput::PageDown | TuiInput::ScrollDown => state.scroll_details_down(8),
                TuiInput::Home => state.scroll_details_up(usize::MAX / 2),
                TuiInput::End => state.scroll_details_to_bottom(),
                TuiInput::Resize => {}
                _ => {}
            }
            draw_tui_frame(&mut terminal, &mut state)?;
            continue;
        }
        if state.command_panel_open() {
            if matches!(&input, TuiInput::Left) {
                if !state.back_command_panel() {
                    state.close_command_panel();
                }
                draw_tui_frame(&mut terminal, &mut state)?;
                continue;
            }
            if matches!(&input, TuiInput::Right) {
                input = TuiInput::Enter;
            }
            // A panel owns the keyboard until it closes; otherwise typing
            // silently edits the composer hidden behind the panel.
            if matches!(
                &input,
                TuiInput::Char(_)
                    | TuiInput::Backspace
                    | TuiInput::Delete
                    | TuiInput::Tab
                    | TuiInput::Home
                    | TuiInput::End
                    | TuiInput::Paste(_)
                    | TuiInput::PasteClipboard
            ) {
                continue;
            }
        }
        match input {
            TuiInput::Char(character) => state.insert_char(character),
            TuiInput::Backspace => state.backspace(),
            TuiInput::Delete => state.delete(),
            TuiInput::Left => state.move_left(),
            TuiInput::Right => state.move_right(),
            TuiInput::Home => state.move_home(),
            TuiInput::End => state.move_end(),
            TuiInput::Up if state.slash_drawer_open() => state.move_command_selection(-1),
            TuiInput::Down if state.slash_drawer_open() => state.move_command_selection(1),
            TuiInput::Up if state.command_panel_menu_open() && state.input().is_empty() => {
                state.move_command_panel_selection(-1);
            }
            TuiInput::Down if state.command_panel_menu_open() && state.input().is_empty() => {
                state.move_command_panel_selection(1);
            }
            TuiInput::Up if state.command_panel_open() => state.scroll_command_panel_up(1),
            TuiInput::Down if state.command_panel_open() => state.scroll_command_panel_down(1),
            TuiInput::Up
                if (state.input().is_empty() || state.history_active())
                    && state.history_previous() => {}
            TuiInput::Down if state.history_active() && state.history_next() => {}
            TuiInput::Up => state.scroll_up(1),
            TuiInput::Down => state.scroll_down(1),
            TuiInput::PageUp if state.command_panel_open() => {
                state.scroll_command_panel_up(8);
            }
            TuiInput::PageDown if state.command_panel_open() => {
                state.scroll_command_panel_down(8);
            }
            TuiInput::PageUp => state.scroll_up(8),
            TuiInput::PageDown => state.scroll_down(8),
            TuiInput::Tab if state.slash_drawer_open() => {
                state.complete_selected_command();
            }
            TuiInput::Tab => {}
            TuiInput::CtrlTab => state.toggle_mode_menu(),
            TuiInput::CommandPalette => {
                state.close_command_panel();
                state.set_input("/");
            }
            TuiInput::MouseDown { column, row } => {
                if !state.handle_follow_control(column, row) {
                    state.begin_selection_drag(column, row);
                }
            }
            TuiInput::MouseDrag { column, row } => state.update_selection_drag(column, row),
            TuiInput::MouseUp { .. } => {
                copy_selected_text(&mut state);
                state.end_selection_drag();
            }
            TuiInput::MouseMove { column, row } => {
                state.set_pointer(column, row);
                state.handle_follow_control_hover(column, row);
            }
            TuiInput::ScrollUp => state.scroll_up(3),
            TuiInput::ScrollDown => state.scroll_down(3),
            TuiInput::Paste(text) => state.insert_text(&text),
            TuiInput::PasteClipboard => match read_clipboard() {
                Ok(text) => {
                    state.insert_text(&text);
                    state.set_notice("已从剪贴板粘贴");
                }
                Err(error) => state.set_notice(format!("剪贴板粘贴失败：{error}")),
            },
            // Ctrl+Q is handled before overlays and never reaches this dispatch.
            TuiInput::CtrlQ => {}
            TuiInput::CtrlC => {
                if copy_selected_text(&mut state) {
                    state.clear_selection();
                }
            }
            TuiInput::Escape => {
                if state.quit_requested() {
                    state.cancel_quit();
                    state.info("已取消退出");
                } else if state.slash_drawer_open() {
                    state.dismiss_slash_drawer();
                } else if state.command_panel_open() {
                    if !state.back_command_panel() {
                        state.close_command_panel();
                    }
                } else if state.selected_text().is_some() {
                    state.clear_selection();
                } else if let Some(command) = state.take_confirmation() {
                    state.info(format!("已取消：{command}"));
                } else {
                    state.scroll_to_bottom();
                }
            }
            TuiInput::Resize => {}
            TuiInput::Enter => {
                if state.quit_requested() {
                    state.take_quit_request();
                    break;
                }
                let mut confirmed = false;
                let submission = if state.input().trim().is_empty() {
                    if let Some(command) = state.take_confirmation() {
                        confirmed = true;
                        command
                    } else {
                        state.take_submission()
                    }
                } else {
                    state.take_submission()
                };
                let mut queued_instruction = None;
                if submission.trim().is_empty()
                    && let Some(item) = state.selected_command_panel_item()
                {
                    if let Some(command_line) = item.id.strip_prefix("open:") {
                        queued_instruction = Some(command_line.to_string());
                    } else if let Some(command_line) = item.id.strip_prefix("prompt:") {
                        state.close_command_panel();
                        state.set_input(command_line);
                        state.set_notice("继续输入命令参数后按 Enter");
                        draw_tui_frame(&mut terminal, &mut state)?;
                        continue;
                    } else if activate_workspace_menu_item(
                        &mut state,
                        &mut session,
                        &mut settings,
                        &item.id,
                    ) {
                        draw_tui_frame(&mut terminal, &mut state)?;
                        continue;
                    }
                }
                let instruction = queued_instruction
                    .as_deref()
                    .unwrap_or_else(|| submission.trim());
                if instruction.is_empty() {
                    draw_tui_frame(&mut terminal, &mut state)?;
                    continue;
                }
                if !confirmed && is_destructive_input(instruction) {
                    state.close_command_panel();
                    state.arm_confirmation(instruction);
                    draw_tui_frame(&mut terminal, &mut state)?;
                    continue;
                }
                state.record_input(instruction);
                let resolution = resolve_input(
                    instruction,
                    state.host_binding(),
                    state.command_stage(),
                    false,
                );
                match resolution {
                    TuiInputResolution::Command { spec, args } => {
                        if let Err(error) = session.record_event(SessionEvent::CommandAccepted {
                            name: spec.name.into(),
                            scope: spec.scope.as_str().into(),
                        }) {
                            state.info(format!("无法写入 session event log：{error}"));
                            continue;
                        }
                        state.record_command_use(spec.id);
                        if spec.id.uses_command_panel() {
                            state.begin_command_panel(spec.name);
                            state.set_command_panel_menu(command_menu_actions(spec.id));
                        } else {
                            state.close_command_panel();
                        }
                        match spec.id {
                            TuiCommandId::Quit => break,
                            TuiCommandId::Help => {
                                display_tui_help_menu(&mut state);
                            }
                            TuiCommandId::Status => {
                                display_tui_status(&mut state, &session, &settings);
                            }
                            TuiCommandId::Journal => {
                                display_journal_paths(&mut state, &session);
                            }
                            TuiCommandId::Details => {
                                state.open_details();
                            }
                            TuiCommandId::Mode => state.toggle_mode_menu(),
                            TuiCommandId::Clear => {
                                state.clear();
                                state.info("Conversation cleared.");
                            }
                            TuiCommandId::Reframe => {
                                if run_tui_reframe_interactive(
                                    &runtime,
                                    &mut state,
                                    &mut terminal,
                                    &mut inputs,
                                    args,
                                )
                                .await?
                                {
                                    break;
                                }
                            }
                            TuiCommandId::Evidence => {
                                display_evidence(&mut state, &session, args);
                            }
                            TuiCommandId::Audit => display_audit(&mut state, &session, args),
                            TuiCommandId::Report => {
                                display_report(&mut state, &session, args);
                            }
                            TuiCommandId::Frame
                            | TuiCommandId::Challenge
                            | TuiCommandId::Decide
                            | TuiCommandId::Postmortem => {
                                let Some((display, mode, request)) =
                                    epistemic_request(spec.id, args, &state, &settings, &session)
                                else {
                                    state.info(format!(
                                        "用法：{} [上下文或目标]",
                                        command_name(spec.id)
                                    ));
                                    continue;
                                };
                                if let Err(error) =
                                    session.record_event(SessionEvent::TaskSubmitted {
                                        mode: mode.as_str().into(),
                                    })
                                {
                                    state.info(format!("无法写入 session event log：{error}"));
                                    continue;
                                }
                                state.begin(&display, mode, risk_for_mode(mode));
                                draw_tui_frame(&mut terminal, &mut state)?;
                                let quit = run_tui_task(
                                    &mut runtime,
                                    &mut state,
                                    &mut terminal,
                                    &mut inputs,
                                    &mut session,
                                    request,
                                    Some(mode),
                                )
                                .await?;
                                if quit {
                                    break;
                                }
                            }
                            TuiCommandId::Mission => {
                                handle_mission_command(&mut state, &mut session, &settings, args);
                            }
                            TuiCommandId::Triage => {
                                display_triage(&mut state, &session, args);
                            }
                            TuiCommandId::Plan => {
                                run_tui_master_plan(&mut state, &mut session, &settings, args)
                                    .await;
                            }
                            TuiCommandId::Approve => {
                                approve_tui_plan(&mut state, &mut session, args);
                            }
                            TuiCommandId::Model => {
                                handle_model_command(&mut state, &mut runtime, &mut settings, args);
                            }
                            TuiCommandId::Permissions => {
                                handle_permissions_command(&mut settings, &mut state, args);
                            }
                            TuiCommandId::Tools => {
                                display_tools(&runtime, &mut state, args);
                            }
                            TuiCommandId::Agents => {
                                display_agents(&mut state, &session, &settings, args);
                            }
                            TuiCommandId::Assign => {
                                assign_tui_plan(&mut state, &mut session, &settings, args);
                            }
                            TuiCommandId::Dispatch => {
                                run_tui_dispatch(&mut state, &mut session, &settings, args);
                            }
                            TuiCommandId::Board => {
                                display_master_board(&mut state, &session, args);
                            }
                            TuiCommandId::Watch => {
                                watch_tui_master(&mut state, &session, &settings);
                            }
                            TuiCommandId::Attention => {
                                display_attention(&mut state, &session);
                            }
                            TuiCommandId::Respond => {
                                respond_tui_task(&mut state, &session, &settings, args);
                            }
                            TuiCommandId::Control => {
                                control_tui_master(&mut state, &session, &settings, args);
                            }
                            TuiCommandId::Timeline => {
                                display_master_timeline(&mut state, &session, &settings);
                            }
                            TuiCommandId::Artifacts => {
                                display_master_artifacts(&mut state, &session, args);
                            }
                            TuiCommandId::Verify => {
                                verify_tui_task(&mut state, &session, args);
                            }
                            TuiCommandId::Session => {
                                handle_session_command(
                                    &mut state,
                                    &mut session,
                                    &mut settings,
                                    args,
                                );
                            }
                            TuiCommandId::Workspace => {
                                handle_workspace_command(
                                    &mut state,
                                    &mut session,
                                    &mut settings,
                                    args,
                                );
                            }
                            TuiCommandId::Context => {
                                handle_context_command(&mut state, &mut session, &settings, args);
                            }
                            TuiCommandId::Config => {
                                handle_config_command(
                                    &mut state,
                                    &mut settings,
                                    &mut session,
                                    args,
                                );
                            }
                            TuiCommandId::Export => {
                                handle_export_command(&mut state, &session, args);
                            }
                            TuiCommandId::Copy => {
                                handle_copy_command(&mut state);
                            }
                            TuiCommandId::Cost => {
                                handle_cost_command(&mut state, &session);
                            }
                            TuiCommandId::Preset => {
                                if let Some(request) =
                                    handle_preset_command(&mut state, &session, &settings, args)
                                {
                                    let display = request.objective.clone();
                                    let inferred = infer_mode(&request);
                                    state.set_last_inferred(inferred);
                                    let mode = state.effective_mode(inferred);
                                    if let Err(error) =
                                        session.record_event(SessionEvent::TaskSubmitted {
                                            mode: mode.as_str().into(),
                                        })
                                    {
                                        state.info(format!("无法写入 session event log：{error}"));
                                        continue;
                                    }
                                    state.begin(&display, mode, risk_for_mode(mode));
                                    draw_tui_frame(&mut terminal, &mut state)?;
                                    let quit = run_tui_task(
                                        &mut runtime,
                                        &mut state,
                                        &mut terminal,
                                        &mut inputs,
                                        &mut session,
                                        request,
                                        Some(mode),
                                    )
                                    .await?;
                                    if quit {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    TuiInputResolution::UnavailableMaster(spec) => {
                        record_rejected_command(
                            &mut state,
                            &mut session,
                            spec.name,
                            "requires Workshop Master context",
                        );
                        state.info(format!(
                            "{} 仅在 Skynet Workshop Master-Agent 上下文可用。",
                            spec.name
                        ));
                    }
                    TuiInputResolution::UnavailableInStage(spec) => {
                        record_rejected_command(
                            &mut state,
                            &mut session,
                            spec.name,
                            "unavailable in current lifecycle stage",
                        );
                        state.info(format!("{} 不在当前生产线阶段可用。", spec.name));
                    }
                    TuiInputResolution::UnavailableWhileRunning(spec) => {
                        record_rejected_command(
                            &mut state,
                            &mut session,
                            spec.name,
                            "unavailable while a run is active",
                        );
                        state.info(format!("{} 无法在运行中执行。", spec.name));
                    }
                    TuiInputResolution::UnknownCommand(name) => {
                        record_rejected_command(&mut state, &mut session, name, "unknown command");
                        state.info(format!("Unknown command: {name}"));
                    }
                    TuiInputResolution::Task(task) => {
                        state.close_command_panel();
                        if handle_master_task_intake(&mut state, &mut session, &settings, task)
                            .await
                        {
                            continue;
                        }
                        let request = settings
                            .request_with_known_facts(task, session_context_facts(&session));
                        let inferred = infer_mode(&request);
                        state.set_last_inferred(inferred);
                        let mode = state.effective_mode(inferred);
                        let risk = risk_for_mode(mode);
                        if let Err(error) = session.record_event(SessionEvent::TaskSubmitted {
                            mode: mode.as_str().into(),
                        }) {
                            state.info(format!("无法写入 session event log：{error}"));
                            continue;
                        }
                        state.begin(task, mode, risk);
                        draw_tui_frame(&mut terminal, &mut state)?;
                        let quit = run_tui_task(
                            &mut runtime,
                            &mut state,
                            &mut terminal,
                            &mut inputs,
                            &mut session,
                            request,
                            Some(mode),
                        )
                        .await?;
                        if quit {
                            break;
                        }
                        if display_tui_next_actions(&mut state) {
                            draw_tui_frame(&mut terminal, &mut state)?;
                        }
                    }
                }
            }
        }
        draw_tui_frame(&mut terminal, &mut state)?;
    }
    session.record_event(SessionEvent::Closed)?;
    state.info("Session ended.");
    draw_tui_frame(&mut terminal, &mut state)?;
    drop(terminal);
    print_session_banner(session.title(), &session.continue_command())
        .context("cannot print session banner")?;
    Ok(())
}

async fn run_tui_task(
    runtime: &mut AgentRuntime,
    state: &mut TuiState,
    terminal: &mut TerminalSession,
    inputs: &mut tokio::sync::mpsc::UnboundedReceiver<TuiInput>,
    session: &mut TuiSession,
    request: TaskRequest,
    requested_mode: Option<CognitiveMode>,
) -> Result<bool> {
    let cancellation = CancellationToken::new();
    let run_started_ms = current_time_ms();
    let mut active_journal: Option<(PathBuf, u64)> = None;
    let run = async {
        match requested_mode {
            Some(mode) => {
                runtime
                    .run_with_mode(request, mode, cancellation.clone())
                    .await
            }
            None => runtime.run(request, cancellation.clone()).await,
        }
    };
    tokio::pin!(run);
    let mut ticker = tokio::time::interval(Duration::from_millis(120));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker.tick().await;
    let mut quit_after_run = false;

    loop {
        tokio::select! {
            result = &mut run => {
                match result {
                    Ok(result) => {
                        session.set_title_from_prompt(&state.last_instruction);
                        if let Err(error) = session.record_journal(&result.journal_path) {
                            state.info(format!("无法保存会话：{error}"));
                        }
                        record_tui_result(state, &result);
                    }
                    Err(RuntimeError::Cancelled { .. }) => {
                        state.finish(TuiStatus::Cancelled, "Run cancelled; journal preserved.");
                    }
                    Err(error) => {
                        state.finish(TuiStatus::Failed, format!("Error: {error}"));
                    }
                }
                return Ok(quit_after_run);
            }
            input = inputs.recv() => {
                if let Some(event) = input.as_ref()
                    && state.quit_requested()
                    && !matches!(
                        event,
                        TuiInput::Enter | TuiInput::Escape | TuiInput::CtrlQ
                    )
                {
                    state.cancel_quit();
                }
                match input {
                    Some(TuiInput::CtrlC) => {
                        if !copy_selected_text(state) {
                            cancellation.cancel();
                        }
                    }
                    Some(TuiInput::CtrlQ) => state.request_quit(),
                    Some(TuiInput::Enter) if state.quit_requested() => {
                        state.take_quit_request();
                        quit_after_run = true;
                        cancellation.cancel();
                    }
                    Some(TuiInput::Escape) if state.quit_requested() => {
                        state.cancel_quit();
                        state.info("已取消退出");
                    }
                    Some(TuiInput::MouseDown { column, row }) => {
                        if !state.handle_follow_control(column, row) {
                            state.begin_selection_drag(column, row);
                        }
                    }
                    Some(TuiInput::MouseDrag { column, row }) => {
                        state.update_selection_drag(column, row);
                    }
                    Some(TuiInput::MouseUp { .. }) => {
                        copy_selected_text(state);
                        state.end_selection_drag();
                    }
                    Some(TuiInput::MouseMove { column, row }) => {
                        state.set_pointer(column, row);
                        state.handle_follow_control_hover(column, row);
                    }
                    Some(TuiInput::Up) => state.scroll_up(1),
                    Some(TuiInput::Down) => state.scroll_down(1),
                    Some(TuiInput::PageUp) => state.scroll_up(8),
                    Some(TuiInput::PageDown) => state.scroll_down(8),
                    Some(TuiInput::Home) => state.scroll_up(usize::MAX / 2),
                    Some(TuiInput::End) => state.scroll_to_bottom(),
                    Some(TuiInput::ScrollUp) => state.scroll_up(3),
                    Some(TuiInput::ScrollDown) => state.scroll_down(3),
                    Some(_) => {}
                    None => {
                        quit_after_run = true;
                        cancellation.cancel();
                    }
                }
                draw_tui_frame(terminal, state)?;
            }
            _ = ticker.tick() => {
                state.tick();
                if let Some((path, size)) =
                    newest_journal_since(runtime.journal_dir(), run_started_ms)
                    && active_journal
                        .as_ref()
                        .is_none_or(|(active, active_size)| active != &path || *active_size != size)
                {
                    let metrics = run_metrics_from_journal(&path);
                    if metrics.usage_seen {
                        state.set_run_metrics(metrics);
                    }
                    active_journal = Some((path, size));
                }
                if state.selection_autoscroll_active() {
                    state.autoscroll_selection_once();
                }
                draw_tui_frame(terminal, state)?;
            }
            _ = tokio::signal::ctrl_c() => cancellation.cancel(),
        }
    }
}

fn draw_tui_frame(terminal: &mut TerminalSession, state: &mut TuiState) -> Result<()> {
    terminal.draw(state).context("cannot draw terminal")
}

fn copy_selected_text(state: &mut TuiState) -> bool {
    let Some(selection) = state.selected_text() else {
        return false;
    };
    match copy_to_clipboard(&selection) {
        Ok(()) => state.set_notice("已复制选择内容"),
        Err(error) => state.set_notice(format!("剪贴板复制失败：{error}")),
    }
    true
}

fn restore_tui_session(state: &mut TuiState, session: &TuiSession) {
    state.clear();
    for journal_path in session.journals() {
        let events = match read_events(journal_path) {
            Ok(events) => events,
            Err(error) => {
                state.info(format!(
                    "无法恢复 journal {}：{error}",
                    journal_path.display()
                ));
                continue;
            }
        };
        let replay = match replay_events(&events) {
            Ok(replay) => replay,
            Err(error) => {
                state.info(format!(
                    "无法重放 journal {}：{error}",
                    journal_path.display()
                ));
                continue;
            }
        };
        let (Some(request), Some(answer), Some(stop_reason)) =
            (replay.request, replay.answer, replay.stop_reason)
        else {
            state.info(format!("journal {} 没有完整结果", journal_path.display()));
            continue;
        };
        let mode = replay.mode.unwrap_or_else(|| infer_mode(&request));
        state.begin(&request.objective, mode, risk_for_mode(mode));
        record_tui_result(
            state,
            &first_principles_agent::RunResult {
                run_id: replay.run_id,
                journal_path: journal_path.clone(),
                answer,
                stop_reason,
            },
        );
    }
    state.set_status(TuiStatus::Idle);
}

fn session_context_facts(session: &TuiSession) -> Vec<String> {
    const MAX_TURNS: usize = 6;
    const MAX_CHARS_PER_FACT: usize = 1_000;
    const MAX_TOTAL_CHARS: usize = 6_000;

    if let Some(summary) = session.context_summary()
        && summary.covered_journals == session.journals().len()
    {
        return vec![format!("Compacted session context: {}", summary.text)];
    }
    let mut facts = Vec::new();
    let mut total_chars = 0;
    for journal_path in session.journals().iter().rev().take(MAX_TURNS).rev() {
        let Ok(events) = read_events(journal_path) else {
            continue;
        };
        let Ok(replay) = replay_events(&events) else {
            continue;
        };
        let (Some(request), Some(answer)) = (replay.request, replay.answer) else {
            continue;
        };
        for (label, text) in [
            ("Previous user request", request.objective),
            ("Previous answer conclusion", answer.conclusion),
        ] {
            let text = text.chars().take(MAX_CHARS_PER_FACT).collect::<String>();
            let fact = format!("{label}: {text}");
            total_chars += fact.chars().count();
            if total_chars > MAX_TOTAL_CHARS {
                return facts;
            }
            facts.push(fact);
        }
    }
    facts
}

fn record_tui_result(state: &mut TuiState, result: &first_principles_agent::RunResult) {
    let answer = &result.answer;
    state.set_status(TuiStatus::Completed);
    state.set_last_journal(&result.journal_path);
    state.set_run_metrics(run_metrics_from_journal(&result.journal_path));
    state.finish_run_clock();
    if let Some(chars) = latest_context_chars(&result.journal_path) {
        state.set_context_chars(chars);
    }
    let mut details = format!(
        "{}\n\n终止原因：{:?}",
        render_answer(result),
        result.stop_reason,
    );
    if let Some(query_details) = query_result_details(&result.journal_path) {
        details.push_str("\n\n");
        details.push_str(&query_details);
    }
    details.push_str(&format!(
        "\n\nRun journal：{}",
        result.journal_path.display()
    ));
    state.set_last_details(details);

    if state.mode == first_principles_agent::CognitiveMode::DirectAnswer {
        record_direct_answer(state, answer);
    } else {
        record_structured_answer(state, answer);
    }
}

fn newest_journal_since(directory: &std::path::Path, since_ms: u128) -> Option<(PathBuf, u64)> {
    let since_ms = since_ms.saturating_sub(2_000);
    let mut newest = None;
    for entry in std::fs::read_dir(directory).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        if modified_ms < since_ms {
            continue;
        }
        let size = metadata.len();
        if newest
            .as_ref()
            .is_none_or(|(_, _, newest_ms)| modified_ms >= *newest_ms)
        {
            newest = Some((path, size, modified_ms));
        }
    }
    newest.map(|(path, size, _)| (path, size))
}

fn run_metrics_from_journal(
    journal_path: &std::path::Path,
) -> first_principles_agent::TuiRunMetrics {
    let mut metrics = first_principles_agent::TuiRunMetrics::default();
    let Ok(events) = read_events(journal_path) else {
        return metrics;
    };
    for event in events {
        let JournalEventKind::ModelResponded { response } = event.event else {
            continue;
        };
        metrics.input_tokens = metrics
            .input_tokens
            .saturating_add(response.usage.input_tokens);
        metrics.output_tokens = metrics
            .output_tokens
            .saturating_add(response.usage.output_tokens);
        metrics.cache_hit_tokens = metrics
            .cache_hit_tokens
            .saturating_add(response.usage.cache_hit_tokens);
        metrics.cache_miss_tokens = metrics
            .cache_miss_tokens
            .saturating_add(response.usage.cache_miss_tokens);
        if response.usage.input_tokens > 0 || response.usage.output_tokens > 0 {
            metrics.usage_seen = true;
        }
    }
    metrics
}

/// The character count of the most recent model request in a run journal. It
/// uses the same metric the runtime compacts against, so the percentage on the
/// status bar matches what triggers compaction.
fn latest_context_chars(journal_path: &std::path::Path) -> Option<usize> {
    let events = read_events(journal_path).ok()?;
    events.iter().rev().find_map(|event| match &event.event {
        JournalEventKind::ModelRequested { request } => Some(
            first_principles_agent::context::total_chars(&request.messages),
        ),
        _ => None,
    })
}

fn query_result_details(journal_path: &std::path::Path) -> Option<String> {
    let Ok(events) = read_events(journal_path) else {
        return None;
    };
    let Ok(replay) = replay_events(&events) else {
        return None;
    };
    query_observation_details(&replay.observations)
}

fn query_observation_details(observations: &[ToolObservation]) -> Option<String> {
    if observations.is_empty() {
        return None;
    }
    let mut output = "查阅详情 · 工具返回，非模型原创".to_string();
    for observation in observations {
        let status = match observation.status {
            ToolStatus::Ok => "成功",
            ToolStatus::InvalidArguments => "参数错误",
            ToolStatus::PermissionDenied => "无权限",
            ToolStatus::TimedOut => "超时",
            ToolStatus::Failed => "失败",
        };
        output.push_str(&format!(
            "\n\n{} · {status}\n{}",
            query_tool_label(&observation.tool),
            query_result_text(observation)
        ));
    }
    Some(output)
}

fn query_tool_label(tool: &str) -> &str {
    match tool {
        "search_files" => "本地文件检索",
        "read_file" => "读取文件",
        "echo" => "回显工具",
        _ => tool,
    }
}

fn query_result_text(observation: &ToolObservation) -> String {
    if observation.tool == "search_files"
        && let Ok(summary) =
            serde_json::from_str::<first_principles_agent::SearchSummary>(&observation.content)
    {
        return bounded_query_result_text(format_search_result(observation, &summary));
    }

    let mut text = observation.summary.trim().to_string();
    if observation.tool == "read_file" {
        let path = observation
            .metadata
            .get("relative_path")
            .map(String::as_str)
            .unwrap_or("文件");
        let bytes = observation
            .metadata
            .get("bytes_read")
            .map(String::as_str)
            .unwrap_or("未知");
        text = format!("读取 {path}（{bytes} 字节）");
    }
    let content = observation.content.trim();
    if !content.is_empty() && content != text {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        if observation.tool != "read_file" && looks_like_structured_tool_output(content) {
            text.push_str("工具返回了结构化数据；完整原始内容保留在 run journal 中。");
        } else {
            text.push_str(content);
        }
    }
    bounded_query_result_text(text)
}

fn bounded_query_result_text(mut text: String) -> String {
    const MAX_CHARS: usize = 4_000;
    if text.chars().count() > MAX_CHARS {
        text = text.chars().take(MAX_CHARS).collect::<String>();
        text.push_str("\n…（查阅结果已截断，完整内容见 /journal）");
    }
    text
}

fn format_search_result(
    observation: &ToolObservation,
    summary: &first_principles_agent::SearchSummary,
) -> String {
    const MAX_HITS: usize = 3;
    let mut output = format!(
        "找到 {} 处匹配，扫描 {} 个文件。",
        summary.hits.len(),
        summary.files_scanned
    );
    if summary.skipped_unsafe > 0 {
        output.push_str(&format!(" 跳过 {} 个不安全文件。", summary.skipped_unsafe));
    }
    for hit in summary.hits.iter().take(MAX_HITS) {
        output.push_str(&format!(
            "\n\n{}:{}\n  {}",
            hit.path,
            hit.line,
            hit.excerpt.trim()
        ));
    }
    if summary.hits.len() > MAX_HITS {
        output.push_str(&format!(
            "\n\n另有 {} 处匹配，完整结果保留在 run journal 中。",
            summary.hits.len() - MAX_HITS
        ));
    } else if summary.truncated {
        output.push_str("\n\n检索达到结果或文件上限；完整状态保留在 run journal 中。");
    }
    if observation
        .metadata
        .get("truncated")
        .is_some_and(|value| value == "true")
        && !summary.truncated
    {
        output.push_str("\n\n工具观测内容已截断；完整内容保留在 run journal 中。");
    }
    output
}

fn looks_like_structured_tool_output(content: &str) -> bool {
    matches!(
        content.chars().find(|character| !character.is_whitespace()),
        Some('{' | '[')
    )
}

fn record_direct_answer(state: &mut TuiState, answer: &first_principles_agent::AgentAnswer) {
    // Direct mode is deliberately conversation-first: the answer is the
    // conclusion. Structured evidence/claims/flip-condition sections are kept
    // in /details, not appended to the transcript.
    state.assistant_markdown(&answer.conclusion);
}

fn record_structured_answer(state: &mut TuiState, answer: &first_principles_agent::AgentAnswer) {
    state.assistant_heading("结论", tone_for_answer_status(answer.status));
    state.assistant_markdown(&answer.conclusion);

    if !answer.claims.is_empty() {
        state.assistant_heading("主张", TuiTone::Accent);
        for claim in &answer.claims {
            state.assistant_item(
                format!("[{}]", claim_status_label(claim.status)),
                tone_for_claim_status(claim.status),
                &claim.text,
            );
        }
    }

    if !answer.evidence.is_empty() {
        state.assistant_heading("依据", TuiTone::Accent);
        for evidence in &answer.evidence {
            state.assistant_evidence(&evidence.source, &evidence.statement);
        }
    }

    if !answer.assumptions.is_empty() {
        state.assistant_heading("假设", TuiTone::Accent);
        for assumption in &answer.assumptions {
            state.assistant_item(
                format!("[假设·{}]", answer_impact_label(assumption.impact)),
                tone_for_impact(assumption.impact),
                &assumption.description,
            );
        }
    }

    if !answer.constraints.is_empty() {
        state.assistant_heading("约束", TuiTone::Accent);
        for constraint in &answer.constraints {
            state.assistant_item("[约束]", TuiTone::Muted, constraint);
        }
    }

    if !answer.unknowns.is_empty() {
        state.assistant_heading("未知", TuiTone::Accent);
        for unknown in &answer.unknowns {
            state.assistant_item(
                format!("[未知·{}]", answer_impact_label(unknown.impact)),
                tone_for_impact(unknown.impact),
                &unknown.description,
            );
        }
    }

    if !answer.conflicts.is_empty() {
        state.assistant_heading("冲突", TuiTone::Accent);
        for conflict in &answer.conflicts {
            let resolved = conflict.status == first_principles_agent::ConflictStatus::Resolved;
            state.assistant_item(
                if resolved {
                    "[已解决]"
                } else {
                    "[未解决]"
                },
                if resolved {
                    TuiTone::Success
                } else {
                    TuiTone::Danger
                },
                conflict
                    .resolution
                    .as_deref()
                    .unwrap_or(&conflict.claim_ids.join(" ↔ ")),
            );
        }
    }

    if !answer.flip_conditions.is_empty() {
        state.assistant_heading("翻转条件", TuiTone::Accent);
        for condition in &answer.flip_conditions {
            state.assistant_item(
                "⇒",
                TuiTone::Accent,
                format!("若 {}，则 {}", condition.if_observed, condition.then),
            );
        }
    }
}

fn tone_for_answer_status(status: first_principles_agent::AnswerStatus) -> TuiTone {
    use first_principles_agent::AnswerStatus;
    match status {
        AnswerStatus::Completed => TuiTone::Success,
        AnswerStatus::Conditional | AnswerStatus::NeedsEvidence => TuiTone::Warning,
        AnswerStatus::NeedsAuthorization => TuiTone::Warning,
        AnswerStatus::Failed => TuiTone::Danger,
    }
}

fn tone_for_claim_status(status: first_principles_agent::ClaimStatus) -> TuiTone {
    use first_principles_agent::ClaimStatus;
    match status {
        ClaimStatus::Verified => TuiTone::Success,
        ClaimStatus::UserAsserted | ClaimStatus::Derived => TuiTone::Accent,
        ClaimStatus::Assumed | ClaimStatus::ModelPrior => TuiTone::Warning,
        ClaimStatus::Conflicting => TuiTone::Danger,
        ClaimStatus::Unknown => TuiTone::Muted,
    }
}

fn tone_for_impact(impact: first_principles_agent::ImpactLevel) -> TuiTone {
    use first_principles_agent::ImpactLevel;
    match impact {
        ImpactLevel::Low => TuiTone::Muted,
        ImpactLevel::Medium => TuiTone::Warning,
        ImpactLevel::High => TuiTone::Danger,
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tui_rendering_tests {
    use super::*;
    use async_trait::async_trait;
    use first_principles_agent::{CognitiveMode, RiskLevel};

    #[test]
    fn natural_supervision_message_requires_an_explicit_task_target() {
        assert_eq!(
            parse_targeted_operator_message("补充任务 T3：不要修改公开 API"),
            Some(("T3".into(), "不要修改公开 API".into()))
        );
        assert_eq!(
            parse_targeted_operator_message("回复任务 T2: 保留旧格式"),
            Some(("T2".into(), "保留旧格式".into()))
        );
        assert_eq!(parse_targeted_operator_message("不要修改公开 API"), None);
    }

    #[test]
    fn every_panel_command_exposes_executable_menu_actions() {
        for command in first_principles_agent::COMMANDS {
            if !command.id.uses_command_panel()
                || matches!(command.id, TuiCommandId::Help | TuiCommandId::Workspace)
            {
                continue;
            }
            let actions = command_menu_actions(command.id);
            assert!(
                !actions.is_empty(),
                "{} has no interactive actions",
                command.name
            );
            for action in actions {
                assert!(
                    action.id.starts_with("open:") || action.id.starts_with("prompt:"),
                    "{} has invalid action {}",
                    command.name,
                    action.id
                );
                if action.id.starts_with("prompt:") {
                    assert!(
                        action.id.ends_with(' '),
                        "{} prompt must prepare an argument",
                        action.id
                    );
                }
            }
        }
    }

    #[test]
    fn help_menu_is_selectable_and_scoped_to_the_current_host() {
        let mut state = TuiState::new("mock", 6, 120);
        display_tui_help_menu(&mut state);
        assert!(state.command_panel_menu_open());
        let first = state.selected_command_panel_item().unwrap();
        assert!(first.id.starts_with("open:/help"));
        let rendered = state.render_screen(100, 30);
        assert!(rendered.contains("帮助"));
        assert!(rendered.contains("/status"));
        assert!(!rendered.contains("/dispatch"));

        state.set_host_binding(HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-1790000000000-1234-1",
        ));
        state.set_command_stage(CommandStage::Approval);
        display_tui_help_menu(&mut state);
        let rendered = state.render_screen(120, 40);
        assert!(rendered.contains("/dispatch"));
    }

    #[test]
    fn tui_provider_budget_defaults_to_requested_steps_and_stays_bounded() {
        assert_eq!(tui_provider_attempt_budget(None, 6), 6);
        assert_eq!(tui_provider_attempt_budget(None, 0), 1);
        assert_eq!(tui_provider_attempt_budget(Some(10), 6), 10);
        assert_eq!(tui_provider_attempt_budget(Some(100), 6), 16);
    }

    #[test]
    fn tui_request_carries_provider_attempt_budget() {
        let settings = TuiRunSettings {
            cwd: PathBuf::from("/tmp"),
            roots: vec![PathBuf::from("/tmp")],
            allow_network: false,
            allow_write: false,
            experience_scope: None,
            max_steps: 6,
            max_tool_calls: 8,
            max_provider_attempts: 6,
            master_agents: None,
            workshop_prefix: "skynet-workshop".into(),
            timeout_seconds: 120,
            max_context_chars: 100_000,
            auto_compact_percent: 80,
            default_preferences: SessionPreferences {
                max_steps: 6,
                max_tool_calls: 8,
                max_provider_attempts: 6,
                max_context_chars: 100_000,
                auto_compact_percent: 80,
                timeout_seconds: 120,
            },
            mentions: Vec::new(),
            planning_provider: Arc::new(DeterministicProvider::new()),
        };
        assert_eq!(settings.request("test").budget.max_provider_attempts, 6);
    }

    struct PlanProvider;

    #[async_trait]
    impl ModelProvider for PlanProvider {
        fn id(&self) -> &str {
            "plan-provider"
        }

        fn model(&self) -> &str {
            "plan-model"
        }

        async fn complete(
            &self,
            request: first_principles_agent::ModelRequest,
            _cancellation: &CancellationToken,
        ) -> std::result::Result<
            first_principles_agent::ModelResponse,
            first_principles_agent::ProviderError,
        > {
            let context_text = request
                .messages
                .last()
                .map(|message| message.content.as_str())
                .unwrap_or_default();
            let context: first_principles_agent::ProductionPlanningContext =
                serde_json::from_str(context_text).unwrap();
            let production = first_principles_agent::ProductionPlan {
                schema_version: first_principles_agent::PRODUCTION_LINE_SCHEMA_VERSION,
                goal: first_principles_agent::GoalContract {
                    goal_id: context.planning.goal_id.clone(),
                    master_conversation_id: context.master_conversation_id.clone(),
                    objective: context.planning.goal.clone(),
                    non_goals: Vec::new(),
                    constraints: Vec::new(),
                    key_results: vec![first_principles_agent::KeyResult {
                        id: "verified-output".into(),
                        statement: "verified output is produced".into(),
                        baseline: "missing".into(),
                        target: "artifact exists".into(),
                        measurement_method: "artifact verification".into(),
                        work_item_ids: vec!["produce".into()],
                    }],
                },
                plan: PlanProposal {
                    schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
                    plan_id: context.planning.plan_id.clone(),
                    goal_id: context.planning.goal_id.clone(),
                    version: 1,
                    summary: "Test production plan".into(),
                    work_items: vec![first_principles_agent::WorkItem {
                        id: "produce".into(),
                        title: "Produce output".into(),
                        objective: "Produce a verified artifact".into(),
                        dependencies: Vec::new(),
                        required_capabilities: vec!["general".into()],
                        required_tools: vec!["read_file".into()],
                        permissions: first_principles_agent::PermissionSet {
                            read_roots: vec![context.planning.workspace.clone()],
                            allow_network: false,
                            allow_write: false,
                        },
                        acceptance_criteria: vec!["artifact exists".into()],
                        expected_artifacts: vec!["result.json".into()],
                        workspace: context.planning.workspace.clone(),
                        budget: first_principles_agent::WorkBudget::default(),
                        deadline_unix_ms: None,
                        risk: RiskLevel::Low,
                        status: first_principles_agent::WorkItemStatus::Pending,
                    }],
                    max_parallel: 1,
                    approved: false,
                },
            };
            let answer = first_principles_agent::AgentAnswer {
                schema_version: first_principles_agent::ANSWER_SCHEMA_VERSION,
                status: first_principles_agent::AnswerStatus::Completed,
                conclusion: serde_json::to_string(&production).unwrap(),
                claims: Vec::new(),
                evidence: Vec::new(),
                assumptions: Vec::new(),
                constraints: Vec::new(),
                unknowns: Vec::new(),
                conflicts: Vec::new(),
                flip_conditions: Vec::new(),
                action_receipts: Vec::new(),
            };
            Ok(first_principles_agent::ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn: first_principles_agent::ModelTurn::Final { answer },
                usage: first_principles_agent::Usage::default(),
            })
        }
    }

    #[tokio::test]
    async fn tui_master_mission_plan_and_approval_are_digest_bound() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_dir = workspace.path().join("journal");
        let mut session = TuiSession::open(&journal_dir, None).unwrap();
        session
            .start_mission(
                "Create and verify a production result",
                workspace.path().to_path_buf(),
            )
            .unwrap();
        let host = HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-1790000000000-1234-1",
        );
        let mut state = TuiState::new("plan-provider/plan-model", 6, 120);
        state.set_host_binding(host);
        let mut settings = TuiRunSettings {
            cwd: workspace.path().to_path_buf(),
            roots: vec![workspace.path().to_path_buf()],
            allow_network: false,
            allow_write: false,
            experience_scope: None,
            max_steps: 6,
            max_tool_calls: 8,
            max_provider_attempts: 6,
            master_agents: None,
            workshop_prefix: "skynet-workshop".into(),
            timeout_seconds: 120,
            max_context_chars: 100_000,
            auto_compact_percent: 80,
            default_preferences: SessionPreferences {
                max_steps: 6,
                max_tool_calls: 8,
                max_provider_attempts: 6,
                max_context_chars: 100_000,
                auto_compact_percent: 80,
                timeout_seconds: 120,
            },
            mentions: Vec::new(),
            planning_provider: Arc::new(PlanProvider),
        };

        run_tui_master_plan(&mut state, &mut session, &settings, "generate").await;
        let plan_binding = session
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .cloned()
            .expect("plan should be bound");
        let plan = read_master_plan(&plan_binding.path).unwrap();
        assert!(!plan.approved);
        assert_eq!(state.command_stage(), CommandStage::Approval);
        let (_, challenge_mode, challenge_request) =
            epistemic_request(TuiCommandId::Challenge, "", &state, &settings, &session).unwrap();
        assert_eq!(challenge_mode, CognitiveMode::Diagnosis);
        assert!(challenge_request.objective.contains("Test production plan"));

        approve_tui_plan(&mut state, &mut session, &plan.plan_id);
        let reopened = TuiSession::open(&journal_dir, Some(session.id())).unwrap();
        let approved = reopened
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| plan.approved_by.as_deref());
        assert!(approved.is_some());
        let task_list = reopened
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| plan.task_list_path.as_ref())
            .cloned()
            .expect("approved production plan should have a task list");
        let task_list_text = std::fs::read_to_string(&task_list).unwrap();
        assert!(task_list_text.contains("status: prepared"));
        assert!(task_list_text.contains("### verified-output"));
        let persisted = read_master_plan(&plan_binding.path).unwrap();
        assert!(!persisted.approved);
        drop(reopened);

        let profile = first_principles_agent::AgentProfile {
            agent_id: "test-worker".into(),
            runtime: "mock".into(),
            display_name: Some("Test Worker".into()),
            capabilities: vec!["general".into()],
            tools: vec!["read_file".into()],
            runtime_command: vec!["true".into()],
            permissions: first_principles_agent::PermissionSet {
                read_roots: vec![workspace.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            availability: first_principles_agent::AgentAvailability::Idle,
            current_load: 0,
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: RiskLevel::Low,
        };
        let profiles = serde_json::to_vec_pretty(&vec![profile]).unwrap();
        write_private_command_file(session.root(), &tui_agents_path(&session), &profiles).unwrap();
        assign_tui_plan(&mut state, &mut session, &settings, "apply");
        let assigned = session
            .mission()
            .and_then(|mission| mission.plan.as_ref())
            .and_then(|plan| plan.assignment.as_ref())
            .cloned()
            .expect("assignment should be bound");
        assert_eq!(assigned.assignments[0].agent_id, "test-worker");

        std::fs::write(&task_list, "tampered").unwrap();
        assert!(
            prepare_master_dispatch(&mut state, &mut session)
                .unwrap_err()
                .contains("任务清单已变化")
        );
        std::fs::write(&task_list, task_list_text).unwrap();
        let (trace_id, state_root, _, master) =
            prepare_master_dispatch(&mut state, &mut session).unwrap();
        assert!(trace_id.starts_with("trace-plan-"));
        assert!(state_root.starts_with(session.root()));
        assert!(master.plan.approved);
        assert_eq!(master.assignment, assigned);
        run_tui_dispatch(&mut state, &mut session, &settings, "dry-run");
        assert!(state.render_screen(120, 40).contains("Dispatch dry-run"));

        let fake_prefix = workspace.path().join("skynet-workshop");
        let fake_script = |name: &str, body: &str| {
            let path = workspace.path().join(format!("skynet-workshop-{name}"));
            std::fs::write(&path, body).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        };
        fake_script(
            "session",
            "#!/bin/sh\ncase \"$1\" in create) printf '%s\\n' '{\"session_id\":\"term-test\"}';; esac\n",
        );
        fake_script(
            "lease",
            "#!/bin/sh\nif [ \"$1\" = request ]; then printf '%s\\n' '{\"ok\":true,\"lease\":{\"lease_id\":\"lease-test\"}}'; else printf '%s\\n' '{\"ok\":true}'; fi\n",
        );
        fake_script("agents", "#!/bin/sh\nexit 0\n");
        settings.workshop_prefix = fake_prefix.display().to_string();
        run_tui_dispatch(&mut state, &mut session, &settings, "detach");
        let dispatched = MasterStore::new(&state_root, &trace_id)
            .unwrap()
            .load()
            .unwrap();
        let task = dispatched.tasks.values().next().unwrap();
        assert_eq!(task.status, WorkItemStatus::Running);
        assert!(task.active_lease.is_some());
        assert_eq!(
            task.execution_target
                .as_ref()
                .and_then(|target| target.session_id.as_deref()),
            Some("term-test")
        );

        std::fs::write(workspace.path().join("result.json"), b"verified output").unwrap();
        let executions = state_root.join(&trace_id).join("executions");
        std::fs::create_dir_all(&executions).unwrap();
        std::fs::write(executions.join("produce-1.output"), "verified output\n").unwrap();
        std::fs::write(executions.join("produce-1.exit"), "0\n").unwrap();
        watch_tui_master(&mut state, &session, &settings);
        assert_eq!(
            MasterStore::new(&state_root, &trace_id)
                .unwrap()
                .load()
                .unwrap()
                .tasks["produce"]
                .status,
            WorkItemStatus::ArtifactReady
        );
        let verification_path = workspace.path().join("verification.json");
        std::fs::write(
            &verification_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "task_id": "produce",
                "criteria_results": [{
                    "criterion": "artifact exists",
                    "passed": true,
                    "evidence": ["independent verifier inspected result.json"]
                }],
                "test_results": [],
                "evidence": ["full TUI workflow verification"],
                "verified_by": "workflow-verifier"
            }))
            .unwrap(),
        )
        .unwrap();
        verify_tui_task(
            &mut state,
            &session,
            &format!("produce {}", verification_path.display()),
        );
        let verified = MasterStore::new(&state_root, &trace_id)
            .unwrap()
            .load()
            .unwrap();
        assert_eq!(verified.tasks["produce"].status, WorkItemStatus::Verified);
        display_evidence(&mut state, &session, "master");
        display_report(&mut state, &session, "master");
        display_master_audit(&mut state, &session, false);
        let workflow = state.render_screen(140, 90);
        assert!(workflow.contains("Master 证据板"));
        assert!(workflow.contains("生产线报告"));
        assert!(workflow.contains("Master Journal 审计 · 通过"));
    }

    #[tokio::test]
    async fn natural_language_master_intake_uses_two_stage_confirmation() {
        let workspace = tempfile::tempdir().unwrap();
        let project = workspace.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let mut state = TuiState::new("plan-provider/plan-model", 6, 120);
        state.set_host_binding(HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-1790000000000-1234-1",
        ));
        let mut settings = TuiRunSettings {
            cwd: project.clone(),
            roots: vec![project.clone()],
            allow_network: false,
            allow_write: false,
            experience_scope: None,
            max_steps: 6,
            max_tool_calls: 8,
            max_provider_attempts: 6,
            master_agents: None,
            workshop_prefix: "skynet-workshop".into(),
            timeout_seconds: 120,
            max_context_chars: 100_000,
            auto_compact_percent: 80,
            default_preferences: SessionPreferences {
                max_steps: 6,
                max_tool_calls: 8,
                max_provider_attempts: 6,
                max_context_chars: 100_000,
                auto_compact_percent: 80,
                timeout_seconds: 120,
            },
            mentions: Vec::new(),
            planning_provider: Arc::new(PlanProvider),
        };

        assert!(
            !handle_master_task_intake(&mut state, &mut session, &settings, "如何优化这个项目？")
                .await
        );
        assert!(session.mission().is_none());
        assert!(
            handle_master_task_intake(&mut state, &mut session, &settings, "请开发一个功能并测试")
                .await
        );
        let mission = session.mission().unwrap();
        assert_eq!(mission.goal, "请开发一个功能并测试");
        let plan = mission.plan.as_ref().unwrap();
        assert!(plan.approved_by.is_none());
        assert!(plan.task_list_path.is_none());
        assert_eq!(state.command_stage(), CommandStage::Approval);

        assert!(handle_master_task_intake(&mut state, &mut session, &settings, "确认计划").await);
        let mission = session.mission().unwrap();
        let plan = mission.plan.as_ref().unwrap();
        assert!(plan.approved_by.is_some());
        let task_list = plan.task_list_path.as_ref().unwrap();
        assert!(
            std::fs::read_to_string(task_list)
                .unwrap()
                .contains("status: prepared")
        );
        assert!(plan.assignment.is_none());

        let profile = AgentProfile {
            agent_id: "test-worker".into(),
            runtime: "mock".into(),
            display_name: Some("Test Worker".into()),
            capabilities: vec!["general".into()],
            tools: vec!["read_file".into()],
            runtime_command: vec!["true".into()],
            permissions: first_principles_agent::PermissionSet {
                read_roots: vec![project.clone()],
                allow_network: false,
                allow_write: false,
            },
            availability: first_principles_agent::AgentAvailability::Idle,
            current_load: 0,
            observed_success_permille: None,
            observed_runs: 0,
            cost_micros: None,
            max_risk: RiskLevel::Low,
        };
        let profiles = serde_json::to_vec_pretty(&vec![profile]).unwrap();
        write_private_command_file(session.root(), &tui_agents_path(&session), &profiles).unwrap();
        let fake_prefix = workspace.path().join("fake-skynet-workshop");
        let fake_agent_run = workspace.path().join("fake-skynet-workshop-agent-run");
        std::fs::write(
            &fake_agent_run,
            "#!/bin/sh\nprintf '%s\\n' '{\"schema\":\"skynet.term.agent-run/v1\",\"agent\":{\"agent_id\":\"test-worker\"},\"session_id\":\"term-test\",\"gui\":{\"started\":false},\"exec\":{\"status\":\"ok\",\"exit\":0,\"duration_ms\":1,\"artifact\":{\"path\":\"/tmp/out\",\"sha256\":\"abc\"}}}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake_agent_run, std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        settings.workshop_prefix = fake_prefix.display().to_string();

        assert!(handle_master_task_intake(&mut state, &mut session, &settings, "确认执行").await);
        let mission = session.mission().unwrap();
        let plan = mission.plan.as_ref().unwrap();
        assert!(plan.assignment.is_some());
        assert!(plan.master_trace_id.is_some());
    }

    struct SupervisorFixture {
        _workspace: tempfile::TempDir,
        session: TuiSession,
        state: TuiState,
        settings: TuiRunSettings,
        store: MasterStore,
        task_id: String,
        workspace: PathBuf,
        capture: PathBuf,
    }

    fn supervisor_fixture() -> SupervisorFixture {
        supervisor_fixture_with_artifact("result.json")
    }

    fn supervisor_fixture_with_artifact(artifact_spec: &str) -> SupervisorFixture {
        let workspace = tempfile::tempdir().unwrap();
        let workdir = workspace.path().join("workspace");
        std::fs::create_dir_all(&workdir).unwrap();
        let journal_dir = workspace.path().join("journal");
        let mut session = TuiSession::open(&journal_dir, None).unwrap();
        session
            .start_mission("Exercise production supervision", workdir.clone())
            .unwrap();

        let task_id = "task-1".to_string();
        let plan = PlanProposal {
            schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
            plan_id: "plan-supervision".into(),
            goal_id: "goal-supervision".into(),
            version: 1,
            summary: "Supervision test plan".into(),
            work_items: vec![first_principles_agent::WorkItem {
                id: task_id.clone(),
                title: "Produce supervised output".into(),
                objective: "Produce a result that requires supervision".into(),
                dependencies: Vec::new(),
                required_capabilities: vec!["general".into()],
                required_tools: vec!["read_file".into()],
                permissions: first_principles_agent::PermissionSet {
                    read_roots: vec![workdir.clone()],
                    allow_network: false,
                    allow_write: false,
                },
                acceptance_criteria: vec!["result exists".into()],
                expected_artifacts: vec![artifact_spec.into()],
                workspace: workdir.clone(),
                budget: first_principles_agent::WorkBudget::default(),
                deadline_unix_ms: None,
                risk: RiskLevel::Low,
                status: WorkItemStatus::Pending,
            }],
            max_parallel: 1,
            approved: false,
        };
        plan.validate().unwrap();
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
        let assignment = first_principles_agent::AssignmentProposal {
            schema_version: first_principles_agent::MASTER_PLAN_SCHEMA_VERSION,
            plan_id: plan.plan_id.clone(),
            assignments: vec![first_principles_agent::AssignmentCandidate {
                task_id: task_id.clone(),
                agent_id: "test-worker".into(),
                runtime: "mock".into(),
                selection_reason: "test fixture".into(),
            }],
        };
        session.bind_assignment(assignment).unwrap();

        let fake_root = workspace.path().join("fake");
        std::fs::create_dir_all(&fake_root).unwrap();
        let prefix = fake_root.join("skynet-workshop");
        let capture = workspace.path().join("workshop-capture.log");
        let session_script = format!(
            "#!/bin/sh\n\
             case \"$1\" in\n\
               write|kill) printf '%s\\n' \"$@\" >> '{}' ;;\n\
               status) printf '%s\\n' '{{\"running\":true}}' ;;\n\
             esac\n",
            capture.display()
        );
        let script_path = PathBuf::from(format!("{}-session", prefix.display()));
        std::fs::write(&script_path, session_script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let events_path = PathBuf::from(format!("{}-events", prefix.display()));
        std::fs::write(
            &events_path,
            "#!/bin/sh\nprintf '%s\\n' '{\"event\":\"fake-workshop-event\",\"session_id\":\"term-test\"}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&events_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        let mut state = TuiState::new("deterministic/test", 6, 120);
        state.set_host_binding(HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-supervision",
        ));
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
            max_risk: RiskLevel::Low,
        };
        write_private_command_file(
            session.root(),
            &tui_agents_path(&session),
            &serde_json::to_vec_pretty(&vec![profile.clone()]).unwrap(),
        )
        .unwrap();
        let settings = TuiRunSettings {
            cwd: workdir.clone(),
            roots: vec![workdir.clone()],
            allow_network: false,
            allow_write: false,
            experience_scope: None,
            max_steps: 6,
            max_tool_calls: 8,
            max_provider_attempts: 6,
            master_agents: None,
            workshop_prefix: prefix.display().to_string(),
            timeout_seconds: 120,
            max_context_chars: 100_000,
            auto_compact_percent: 80,
            default_preferences: SessionPreferences {
                max_steps: 6,
                max_tool_calls: 8,
                max_provider_attempts: 6,
                max_context_chars: 100_000,
                auto_compact_percent: 80,
                timeout_seconds: 120,
            },
            mentions: Vec::new(),
            planning_provider: Arc::new(DeterministicProvider::new()),
        };
        let (trace_id, state_root, _, _) =
            prepare_master_dispatch(&mut state, &mut session).unwrap();
        let store = MasterStore::new(&state_root, &trace_id).unwrap();
        std::fs::create_dir_all(store.trace_dir().join("executions")).unwrap();

        let mut master = store.load().unwrap();
        let lease = master
            .lease_task(
                &task_id,
                &profile,
                "workshop-session:test-worker".into(),
                600_000,
                now_unix_ms(),
            )
            .unwrap();
        master = store
            .append(
                master,
                MasterJournalEventKind::Leased,
                Some(task_id.clone()),
                "master",
                "supervision fixture lease",
                now_unix_ms(),
            )
            .unwrap();
        master
            .begin_starting(&task_id, lease.generation, now_unix_ms())
            .unwrap();
        master = store
            .append(
                master,
                MasterJournalEventKind::Starting,
                Some(task_id.clone()),
                "master",
                "supervision fixture start",
                now_unix_ms(),
            )
            .unwrap();
        let submitted = now_unix_ms();
        master
            .record_submission(
                ExecutionSubmission {
                    task_id: task_id.clone(),
                    agent_id: lease.agent_id,
                    lease_generation: lease.generation,
                    idempotency_key: lease.idempotency_key,
                    execution_target: ExecutionTarget {
                        session_id: Some("term-test".into()),
                        pane_id: None,
                        job_id: None,
                    },
                    submitted_unix_ms: submitted,
                    deadline_unix_ms: submitted.saturating_add(600_000),
                },
                now_unix_ms(),
            )
            .unwrap();
        store
            .append(
                master,
                MasterJournalEventKind::Submitted,
                Some(task_id.clone()),
                "test-worker",
                "submitted to term-test",
                now_unix_ms(),
            )
            .unwrap();

        SupervisorFixture {
            _workspace: workspace,
            session,
            state,
            settings,
            store,
            task_id,
            workspace: workdir,
            capture,
        }
    }

    fn block_fixture_task(fixture: &SupervisorFixture) {
        let mut master = fixture.store.load().unwrap();
        master
            .record_observation(
                &fixture.task_id,
                ExecutionObservation::Blocked {
                    reason: "missing operator approval".into(),
                },
                now_unix_ms(),
            )
            .unwrap();
        fixture
            .store
            .append(
                master,
                MasterJournalEventKind::Observed,
                Some(fixture.task_id.clone()),
                "test-worker",
                "blocked on operator approval",
                now_unix_ms(),
            )
            .unwrap();
    }

    fn make_fixture_artifact_ready(fixture: &SupervisorFixture, contents: Option<&[u8]>) {
        let artifact = fixture.workspace.join("result.json");
        let digest = if let Some(contents) = contents {
            std::fs::write(&artifact, contents).unwrap();
            hex::encode(Sha256::digest(contents))
        } else {
            String::new()
        };
        let mut master = fixture.store.load().unwrap();
        let task = master.tasks[&fixture.task_id].clone();
        let lease = task.active_lease.unwrap();
        let now = now_unix_ms();
        master
            .record_receipt(
                ExecutionReceipt {
                    task_id: fixture.task_id.clone(),
                    agent_id: lease.agent_id,
                    lease_generation: lease.generation,
                    idempotency_key: lease.idempotency_key,
                    execution_target: task.execution_target.unwrap(),
                    status: ExecutionStatus::Completed,
                    exit_code: Some(0),
                    artifacts: (!digest.is_empty())
                        .then(|| artifact.display().to_string())
                        .into_iter()
                        .collect(),
                    result_digest: (!digest.is_empty()).then_some(digest),
                    error: None,
                    started_unix_ms: now,
                    finished_unix_ms: now,
                },
                now,
            )
            .unwrap();
        fixture
            .store
            .append(
                master,
                MasterJournalEventKind::ReceiptRecorded,
                Some(fixture.task_id.clone()),
                "test-worker",
                "artifact ready for verification",
                now,
            )
            .unwrap();
    }

    fn write_verification_input(
        fixture: &SupervisorFixture,
        criterion: Option<&str>,
        verified_by: &str,
    ) -> PathBuf {
        let input = fixture.workspace.join("verification.json");
        let criteria = criterion
            .map(|criterion| {
                vec![serde_json::json!({
                    "criterion": criterion,
                    "passed": true,
                    "evidence": ["independent artifact and criterion check"]
                })]
            })
            .unwrap_or_default();
        std::fs::write(
            &input,
            serde_json::to_vec_pretty(&serde_json::json!({
                "task_id": fixture.task_id,
                "criteria_results": criteria,
                "test_results": [],
                "evidence": ["independent verifier inspected the delivered Artifact"],
                "verified_by": verified_by
            }))
            .unwrap(),
        )
        .unwrap();
        input
    }

    #[test]
    fn master_supervision_routes_human_response_and_preserves_timeline_evidence() {
        let mut fixture = supervisor_fixture();
        display_master_board(&mut fixture.state, &fixture.session, "");
        let board = fixture.state.render_screen(120, 40);
        assert!(
            board.contains("Board · trace=trace-plan-supervision"),
            "{board}"
        );
        assert!(
            board.contains("task-1 · Produce supervised output · Running"),
            "{board}"
        );
        assert!(board.contains("lease_until="), "{board}");

        let master = fixture.store.load().unwrap();
        let generation = master.tasks[&fixture.task_id].generation;
        std::fs::write(
            fixture
                .store
                .trace_dir()
                .join("executions")
                .join(format!("task-1-{generation}.output")),
            "[1A:AWAITING_INPUT] approve deployment\n",
        )
        .unwrap();
        watch_tui_master(&mut fixture.state, &fixture.session, &fixture.settings);
        let master = fixture.store.load().unwrap();
        assert_eq!(
            master.tasks[&fixture.task_id].status,
            WorkItemStatus::AwaitingInput
        );

        display_attention(&mut fixture.state, &fixture.session);
        let attention = fixture.state.render_screen(120, 40);
        assert!(attention.contains("Attention · trace=trace-plan-supervision"));
        assert!(attention.contains("task-1 · AwaitingInput"), "{attention}");

        respond_tui_task(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "task-1 approve deployment",
        );
        let master = fixture.store.load().unwrap();
        assert_eq!(
            master.tasks[&fixture.task_id].status,
            WorkItemStatus::Running
        );
        let capture = std::fs::read_to_string(&fixture.capture).unwrap();
        assert!(capture.contains("write\nterm-test\n--require-idle\napprove deployment"));

        std::fs::write(
            fixture
                .store
                .trace_dir()
                .join("executions")
                .join(format!("task-1-{generation}.output")),
            "[1A:BLOCKED] missing deployment token\n",
        )
        .unwrap();
        watch_tui_master(&mut fixture.state, &fixture.session, &fixture.settings);
        assert_eq!(
            fixture.store.load().unwrap().tasks[&fixture.task_id].status,
            WorkItemStatus::Blocked
        );
        respond_tui_task(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "task-1 token supplied by operator",
        );
        assert_eq!(
            fixture.store.load().unwrap().tasks[&fixture.task_id].status,
            WorkItemStatus::Running
        );

        display_master_timeline(&mut fixture.state, &fixture.session, &fixture.settings);
        let timeline = fixture.state.render_screen(120, 50);
        assert!(timeline.contains("Timeline · trace=trace-plan-supervision"));
        assert!(timeline.contains("human response routed to task Session"));
        assert!(timeline.contains("fake-workshop-event"));
        assert!(timeline.contains("session=term-test"));
    }

    #[test]
    fn master_control_pause_resume_takeover_and_cancel_are_journaled() {
        let mut fixture = supervisor_fixture();
        control_tui_master(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "retry task-1 invalid while running",
        );
        control_tui_master(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "replan task-1 invalid while running",
        );
        assert!(
            !fixture.capture.exists(),
            "invalid controls must not terminate a live Session"
        );
        control_tui_master(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "pause",
        );
        assert!(fixture.store.load().unwrap().paused);
        control_tui_master(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "resume",
        );
        assert!(!fixture.store.load().unwrap().paused);
        control_tui_master(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "takeover task-1 inspect workspace",
        );
        assert!(fixture.store.load().unwrap().tasks[&fixture.task_id].human_takeover);
        control_tui_master(
            &mut fixture.state,
            &fixture.session,
            &fixture.settings,
            "cancel task-1 operator stopped run",
        );
        let master = fixture.store.load().unwrap();
        assert_eq!(
            master.tasks[&fixture.task_id].status,
            WorkItemStatus::Cancelled
        );
        let events =
            read_master_journal_events(&fixture.store.trace_dir().join("events.jsonl")).unwrap();
        assert!(
            events
                .iter()
                .any(|event| event.kind == MasterJournalEventKind::Paused)
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == MasterJournalEventKind::Resumed)
        );
        assert!(
            events
                .iter()
                .any(|event| event.kind == MasterJournalEventKind::TakenOver)
        );
        assert_eq!(
            events.last().unwrap().kind,
            MasterJournalEventKind::Cancelled
        );
        assert!(
            first_principles_agent::audit_master_events(&events).ok,
            "{:?}",
            first_principles_agent::audit_master_events(&events).issues
        );
    }

    #[test]
    fn master_control_retry_and_replan_remain_bounded_and_explicit() {
        let mut retry = supervisor_fixture();
        block_fixture_task(&retry);
        control_tui_master(
            &mut retry.state,
            &retry.session,
            &retry.settings,
            "retry task-1 transient worker failure",
        );
        let master = retry.store.load().unwrap();
        assert_eq!(
            master.tasks[&retry.task_id].status,
            WorkItemStatus::Retrying
        );
        assert_eq!(master.retry_count, 1);

        let mut replan = supervisor_fixture();
        block_fixture_task(&replan);
        control_tui_master(
            &mut replan.state,
            &replan.session,
            &replan.settings,
            "replan task-1 objective is invalid",
        );
        let master = replan.store.load().unwrap();
        assert_eq!(
            master.tasks[&replan.task_id].status,
            WorkItemStatus::Replanning
        );
        assert_eq!(master.replan_count, 1);
        assert_eq!(
            master
                .replan_scope
                .as_ref()
                .map(|scope| scope.source_task.as_str()),
            Some(replan.task_id.as_str())
        );
    }

    #[test]
    fn master_attention_includes_artifact_ready_for_verification() {
        let mut fixture = supervisor_fixture();
        let mut master = fixture.store.load().unwrap();
        let task = master.tasks[&fixture.task_id].clone();
        let lease = task.active_lease.unwrap();
        let now = now_unix_ms();
        master
            .record_receipt(
                ExecutionReceipt {
                    task_id: fixture.task_id.clone(),
                    agent_id: lease.agent_id,
                    lease_generation: lease.generation,
                    idempotency_key: lease.idempotency_key,
                    execution_target: task.execution_target.unwrap(),
                    status: ExecutionStatus::Completed,
                    exit_code: Some(0),
                    artifacts: vec!["result.json".into()],
                    result_digest: Some("a".repeat(64)),
                    error: None,
                    started_unix_ms: now,
                    finished_unix_ms: now,
                },
                now,
            )
            .unwrap();
        fixture
            .store
            .append(
                master,
                MasterJournalEventKind::ReceiptRecorded,
                Some(fixture.task_id.clone()),
                "test-worker",
                "artifact ready for verification",
                now,
            )
            .unwrap();

        display_attention(&mut fixture.state, &fixture.session);
        let attention = fixture.state.render_screen(120, 40);
        assert!(attention.contains("task-1 · Produce supervised output · ArtifactReady"));
    }

    #[test]
    fn master_artifacts_verify_evidence_report_and_audit_form_a_closed_loop() {
        let mut fixture = supervisor_fixture();
        make_fixture_artifact_ready(&fixture, Some(b"delivered result"));

        display_master_artifacts(&mut fixture.state, &fixture.session, "");
        let artifacts = fixture.state.render_screen(140, 50);
        assert!(artifacts.contains("Artifacts · trace=trace-plan-supervision"));
        assert!(artifacts.contains("actual=present"), "{artifacts}");
        assert!(artifacts.contains("verdict=ok"), "{artifacts}");
        assert!(artifacts.contains("exit=Some(0)"), "{artifacts}");

        let input =
            write_verification_input(&fixture, Some("result exists"), "independent-verifier");
        verify_tui_task(
            &mut fixture.state,
            &fixture.session,
            &format!("task-1 {}", input.display()),
        );
        let master = fixture.store.load().unwrap();
        assert_eq!(
            master.tasks[&fixture.task_id].status,
            WorkItemStatus::Verified
        );

        display_evidence(&mut fixture.state, &fixture.session, "master");
        let evidence = fixture.state.render_screen(140, 60);
        assert!(evidence.contains("Master 证据板"));
        assert!(evidence.contains("claim [pass] result exists"));
        assert!(evidence.contains("artifact [pass] result.json"));
        assert!(evidence.contains("independent verifier inspected"));

        display_report(&mut fixture.state, &fixture.session, "master");
        let report = fixture.state.render_screen(140, 70);
        assert!(report.contains("生产线报告 · trace=trace-plan-supervision"));
        assert!(report.contains("verified: Verified"));
        assert!(report.contains("independent-verifier"));

        display_master_audit(&mut fixture.state, &fixture.session, false);
        let audit = fixture.state.render_screen(140, 80);
        assert!(audit.contains("Master Journal 审计 · 通过"), "{audit}");
    }

    #[test]
    fn master_verify_rejects_false_success_missing_criteria_and_sha_drift() {
        let mut missing_artifact = supervisor_fixture();
        make_fixture_artifact_ready(&missing_artifact, None);
        let input = write_verification_input(
            &missing_artifact,
            Some("result exists"),
            "independent-verifier",
        );
        verify_tui_task(
            &mut missing_artifact.state,
            &missing_artifact.session,
            &format!("task-1 {}", input.display()),
        );
        let master = missing_artifact.store.load().unwrap();
        assert_eq!(
            master.tasks[&missing_artifact.task_id].status,
            WorkItemStatus::Rejected
        );
        assert!(
            master.tasks[&missing_artifact.task_id]
                .last_error
                .as_deref()
                .is_some_and(|reason| reason.contains("artifact is missing"))
        );

        let mut missing_criterion = supervisor_fixture();
        make_fixture_artifact_ready(&missing_criterion, Some(b"delivered result"));
        let input = write_verification_input(&missing_criterion, None, "independent-verifier");
        verify_tui_task(
            &mut missing_criterion.state,
            &missing_criterion.session,
            &format!("task-1 {}", input.display()),
        );
        let master = missing_criterion.store.load().unwrap();
        assert_eq!(
            master.tasks[&missing_criterion.task_id].status,
            WorkItemStatus::Rejected
        );
        assert!(
            master.tasks[&missing_criterion.task_id]
                .last_error
                .as_deref()
                .is_some_and(|reason| reason.contains("exactly one result"))
        );

        let expected_digest = "b".repeat(64);
        let mut sha_drift =
            supervisor_fixture_with_artifact(&format!("result.json@sha256:{expected_digest}"));
        make_fixture_artifact_ready(&sha_drift, Some(b"different bytes"));
        let input =
            write_verification_input(&sha_drift, Some("result exists"), "independent-verifier");
        verify_tui_task(
            &mut sha_drift.state,
            &sha_drift.session,
            &format!("task-1 {}", input.display()),
        );
        let master = sha_drift.store.load().unwrap();
        assert_eq!(
            master.tasks[&sha_drift.task_id].status,
            WorkItemStatus::Rejected
        );
        assert!(
            master.tasks[&sha_drift.task_id]
                .last_error
                .as_deref()
                .is_some_and(|reason| reason.contains("artifact SHA-256 mismatch"))
        );
    }

    #[test]
    fn master_verify_requires_an_independent_verifier_and_audit_detects_tampering() {
        let mut fixture = supervisor_fixture();
        make_fixture_artifact_ready(&fixture, Some(b"delivered result"));
        let input = write_verification_input(&fixture, Some("result exists"), "test-worker");
        verify_tui_task(
            &mut fixture.state,
            &fixture.session,
            &format!("task-1 {}", input.display()),
        );
        let master = fixture.store.load().unwrap();
        assert_eq!(
            master.tasks[&fixture.task_id].status,
            WorkItemStatus::ArtifactReady
        );
        let verification = fixture.state.render_screen(120, 50);
        assert!(verification.contains("验收人不能与执行任务的 Agent 相同"));

        let journal = fixture.store.trace_dir().join("events.jsonl");
        let contents = std::fs::read_to_string(&journal).unwrap();
        std::fs::write(
            &journal,
            contents.replace(
                "artifact ready for verification",
                "tampered verification evidence",
            ),
        )
        .unwrap();
        display_master_audit(&mut fixture.state, &fixture.session, false);
        let audit = fixture.state.render_screen(140, 80);
        assert!(audit.contains("Master Journal 审计 · 失败"));
        assert!(audit.contains("content hash mismatch"), "{audit}");
    }

    fn phase_six_settings(cwd: PathBuf) -> TuiRunSettings {
        TuiRunSettings {
            cwd: cwd.clone(),
            roots: vec![cwd],
            allow_network: false,
            allow_write: false,
            experience_scope: None,
            max_steps: 6,
            max_tool_calls: 8,
            max_provider_attempts: 6,
            master_agents: None,
            workshop_prefix: "skynet-workshop".into(),
            timeout_seconds: 120,
            max_context_chars: 100_000,
            auto_compact_percent: 80,
            default_preferences: SessionPreferences {
                max_steps: 6,
                max_tool_calls: 8,
                max_provider_attempts: 6,
                max_context_chars: 100_000,
                auto_compact_percent: 80,
                timeout_seconds: 120,
            },
            mentions: Vec::new(),
            planning_provider: Arc::new(DeterministicProvider::new()),
        }
    }

    fn write_replay_journal(path: &std::path::Path, objective: &str) {
        let mut journal = first_principles_agent::Journal::create(path, Uuid::new_v4()).unwrap();
        journal
            .append(first_principles_agent::JournalEventKind::RunStarted {
                request: TaskRequest::from_objective(
                    objective,
                    path.parent().unwrap().to_path_buf(),
                ),
                mode: CognitiveMode::DirectAnswer,
            })
            .unwrap();
        journal
            .append(first_principles_agent::JournalEventKind::ModelResponded {
                response: first_principles_agent::ModelResponse {
                    provider_id: "fixture-provider".into(),
                    model: "fixture-model".into(),
                    turn: first_principles_agent::ModelTurn::ToolCalls { calls: Vec::new() },
                    usage: first_principles_agent::Usage {
                        input_tokens: 100,
                        output_tokens: 50,
                        cost_micros: 10,
                        cache_hit_tokens: 40,
                        cache_miss_tokens: 60,
                    },
                },
            })
            .unwrap();
        journal
            .append(first_principles_agent::JournalEventKind::AnswerProduced {
                answer: result().answer,
            })
            .unwrap();
        journal
            .append(first_principles_agent::JournalEventKind::RunStopped {
                reason: first_principles_agent::StopReason::Completed,
            })
            .unwrap();
    }

    #[test]
    fn phase_six_context_config_status_and_exports_are_operational() {
        let workspace = tempfile::tempdir().unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let first = workspace.path().join("first.jsonl");
        let second = workspace.path().join("second.jsonl");
        write_replay_journal(&first, "First reviewed task");
        write_replay_journal(&second, "Second reviewed task");
        session.record_journal(&first).unwrap();
        session.record_journal(&second).unwrap();
        let mut settings = phase_six_settings(workspace.path().to_path_buf());
        let mut state = TuiState::new("deterministic/test", 6, 120);
        state.set_session_info(session.id());
        state.set_last_journal(&second);

        handle_context_command(&mut state, &mut session, &settings, "show");
        let context = state.render_screen(120, 50);
        assert!(context.contains("剩余空间"), "{context}");
        handle_context_command(&mut state, &mut session, &settings, "compact");
        let summary = session.context_summary().unwrap();
        assert_eq!(summary.covered_journals, 2);
        assert!(summary.text.contains("First reviewed task"));
        let facts = session_context_facts(&session);
        assert_eq!(facts.len(), 1);
        assert!(facts[0].contains("Compacted session context"));

        handle_config_command(&mut state, &mut settings, &mut session, "set context 20000");
        assert_eq!(settings.max_context_chars, 20_000);
        assert_eq!(session.preferences().unwrap().max_context_chars, 20_000);
        handle_config_command(&mut state, &mut settings, &mut session, "set compact 70");
        assert_eq!(settings.auto_compact_percent, 70);
        handle_config_command(&mut state, &mut settings, &mut session, "reset");
        assert_eq!(settings.max_context_chars, 100_000);
        assert_eq!(settings.auto_compact_percent, 80);
        handle_cost_command(&mut state, &session);
        let cost = state.render_screen(120, 70);
        assert!(cost.contains("模型调用      2 次"), "{cost}");
        assert!(cost.contains("估算成本      20"), "{cost}");

        let report_path = workspace.path().join("report.md");
        let exported = export_tui_data(
            &mut state,
            &session,
            "report",
            Some(&report_path.display().to_string()),
        )
        .unwrap();
        assert!(
            std::fs::read_to_string(exported)
                .unwrap()
                .contains("任务报告")
        );
        let session_path = workspace.path().join("session.md");
        let exported = export_tui_data(
            &mut state,
            &session,
            "session",
            Some(&session_path.display().to_string()),
        )
        .unwrap();
        assert!(
            std::fs::read_to_string(exported)
                .unwrap()
                .contains("Session")
        );

        display_tui_status(&mut state, &session, &settings);
        let status = state.render_screen(120, 60);
        assert!(status.contains("推理步骤      最多 6 步"));
        assert!(status.contains("运行概览"));
        assert!(status.contains("压缩摘要"));
        assert!(!status.contains("status="));
    }

    #[test]
    fn phase_six_workspace_presets_and_session_commands_are_bounded() {
        let workspace = tempfile::tempdir().unwrap();
        let child = workspace.path().join("child");
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(&child).unwrap();
        let file = child.join("notes.md");
        std::fs::write(&file, "notes").unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let mut settings = phase_six_settings(workspace.path().to_path_buf());
        let mut state = TuiState::new("deterministic/test", 6, 120);
        state.set_session_info(session.id());

        handle_workspace_command(
            &mut state,
            &mut session,
            &mut settings,
            &format!("cd {}", child.display()),
        );
        assert_eq!(settings.cwd, child);
        handle_workspace_command(
            &mut state,
            &mut session,
            &mut settings,
            &format!("mention {}", file.display()),
        );
        assert_eq!(session.mentions(), &[file]);
        handle_workspace_command(&mut state, &mut session, &mut settings, "cd ../../");
        assert_eq!(settings.cwd, child);
        let outside_path = outside.path().display().to_string();
        handle_workspace_command(
            &mut state,
            &mut session,
            &mut settings,
            &format!("cd {outside_path}"),
        );
        assert_eq!(settings.cwd, child);

        handle_preset_command(
            &mut state,
            &session,
            &settings,
            "save review Review every changed line",
        );
        assert_eq!(list_presets(&session).unwrap()[0].name, "review");
        let request = handle_preset_command(&mut state, &session, &settings, "run review").unwrap();
        assert_eq!(request.objective, "Review every changed line");
        assert!(
            request
                .known_facts
                .iter()
                .any(|fact| fact.contains("Mentioned workspace path"))
        );
        handle_preset_command(&mut state, &session, &settings, "delete review");
        assert!(list_presets(&session).unwrap().is_empty());

        let original = session.id().to_string();
        handle_session_command(&mut state, &mut session, &mut settings, "new Phase six");
        assert_ne!(session.id(), original);
        assert_eq!(session.title(), "Phase six");
        handle_session_command(
            &mut state,
            &mut session,
            &mut settings,
            "rename Phase six renamed",
        );
        assert_eq!(session.title(), "Phase six renamed");
        let source = session.id().to_string();
        handle_session_command(
            &mut state,
            &mut session,
            &mut settings,
            "fork Forked phase six",
        );
        assert_ne!(session.id(), source);
        assert!(session.mission().is_none());
        handle_session_command(&mut state, &mut session, &mut settings, "list all");
        let listing = state.render_screen(120, 70);
        assert!(listing.contains(&source));
        assert!(listing.contains(session.id()));
    }

    #[test]
    fn workspace_menu_selects_an_authorized_root_and_refreshes_the_panel() {
        let workspace = tempfile::tempdir().unwrap();
        let child = workspace.path().join("child");
        std::fs::create_dir_all(&child).unwrap();
        let mut session = TuiSession::open(workspace.path(), None).unwrap();
        let mut settings = phase_six_settings(workspace.path().to_path_buf());
        let mut state = TuiState::new("deterministic/test", 6, 120);
        settings.cwd = child;

        handle_workspace_command(&mut state, &mut session, &mut settings, "");
        assert!(state.command_panel_menu_open());
        assert_eq!(
            state.selected_command_panel_item().unwrap().id,
            "workspace.set"
        );
        let item = state.selected_command_panel_item().unwrap();
        assert!(activate_workspace_menu_item(
            &mut state,
            &mut session,
            &mut settings,
            &item.id
        ));
        assert_eq!(
            state.selected_command_panel_item().unwrap().id,
            "workspace.set.root.0"
        );
        let item = state.selected_command_panel_item().unwrap();
        assert!(activate_workspace_menu_item(
            &mut state,
            &mut session,
            &mut settings,
            &item.id
        ));
        assert_eq!(settings.cwd, workspace.path());
        assert_eq!(session.workspace(), Some(workspace.path()));
        let panel = state.render_screen(120, 40);
        assert!(panel.contains("工作区已切换"), "{panel}");
        assert!(panel.contains("设置工作区"), "{panel}");
    }

    fn result() -> first_principles_agent::RunResult {
        serde_json::from_value(serde_json::json!({
            "run_id": "f7d8a417-5d6d-4e57-87de-2ea1d2ec7aa1",
            "journal_path": ".journal/test.jsonl",
            "stop_reason": "completed",
            "answer": {
                "schema_version": 2,
                "status": "completed",
                "conclusion": "连接测试成功。",
                "claims": [{
                    "id": "c1",
                    "text": "模型返回了结构化结果。",
                    "status": "verified",
                    "sources": ["probe"],
                    "scope": null
                }],
                "evidence": [{
                    "source": "probe",
                    "statement": "探测返回了结构化结果。",
                    "observed_unix_ms": null,
                    "expires_unix_ms": null,
                    "claim_ids": ["c1"]
                }],
                "assumptions": [{
                    "description": "只测试通信能力。",
                    "impact": "medium",
                    "verification_method": null
                }],
                "constraints": ["allow_write=false"],
                "unknowns": [{
                    "id": "u1",
                    "description": "尚未测试文件工具。",
                    "impact": "medium",
                    "why_it_matters": null,
                    "reduction_method": null
                }],
                "conflicts": [],
                "flip_conditions": [{
                    "if_observed": "文件工具失败",
                    "then": "结论需要调整",
                    "target_ids": ["c1"]
                }],
                "action_receipts": []
            }
        }))
        .unwrap()
    }

    #[test]
    fn direct_answer_hides_repetitive_schema_sections_but_keeps_details() {
        let mut state = TuiState::new("deepseek", 6, 120);
        state.begin("连接测试", CognitiveMode::DirectAnswer, RiskLevel::Low);
        let mut run = result();
        run.answer.status = first_principles_agent::AnswerStatus::Conditional;
        record_tui_result(&mut state, &run);
        let visible = state.render_screen(80, 24);
        assert!(visible.contains("连接测试成功。"));
        assert!(!visible.contains("终止原因"));
        assert!(!visible.contains("allow_write"));
        assert!(!visible.contains("主张"));
        assert!(!visible.contains("依据"));
        assert!(!visible.contains("需要注意"));
        assert!(!visible.contains("如何确认"));
        assert!(!visible.contains("翻转条件"));

        let details = state.last_details().unwrap();
        assert!(details.contains("终止原因"));
        assert!(details.contains("allow_write"));
        assert!(details.contains("主张"));
    }

    #[test]
    fn decision_answer_keeps_structured_reasoning_visible() {
        let mut state = TuiState::new("deepseek", 6, 120);
        state.begin("做决策", CognitiveMode::DecisionAnalysis, RiskLevel::Medium);
        record_tui_result(&mut state, &result());
        let visible = state.render_screen(80, 24);
        assert!(visible.contains("结论"));
        assert!(visible.contains("主张"));
        assert!(visible.contains("依据"));
        assert!(visible.contains("约束"));
        assert!(visible.contains("翻转条件"));
    }

    #[test]
    fn tool_observations_are_available_only_in_details() {
        let mut state = TuiState::new("deepseek", 6, 120);
        state.assistant_body("原回答");
        let observation = ToolObservation {
            call_id: "call-1".into(),
            tool: "search_files".into(),
            status: ToolStatus::Ok,
            summary: "found 1 match in 177 files".into(),
            content: serde_json::json!({
                "hits": [{
                    "path": "src/tui.rs",
                    "line": 1600,
                    "column": 10,
                    "excerpt": "fn unfenced_code_start(...)"
                }],
                "files_scanned": 177,
                "skipped_unsafe": 0,
                "truncated": false
            })
            .to_string(),
            error_code: None,
            metadata: std::collections::BTreeMap::new(),
        };
        state.set_last_details(query_observation_details(&[observation]).unwrap());
        let visible = state.render_screen(100, 30);
        assert!(!visible.contains("查阅详情"));
        assert!(!visible.contains("找到 1 处匹配"));
        assert!(!visible.contains("src/tui.rs:1600"));

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let compact_screen = screen.split_whitespace().collect::<String>();
        assert!(!compact_screen.contains("查阅详情"), "{screen}");
        assert!(!compact_screen.contains("找到1处匹配"), "{screen}");

        let details = state.last_details().unwrap();
        assert!(details.contains("查阅详情 · 工具返回，非模型原创"));
        assert!(details.contains("本地文件检索 · 成功"));
        assert!(details.contains("找到 1 处匹配，扫描 177 个文件"));
        assert!(details.contains("src/tui.rs:1600"));
        assert!(details.contains("fn unfenced_code_start"));
        assert!(!details.contains("\"hits\""));

        state.open_details();
        terminal.draw(|frame| state.draw(frame)).unwrap();
        let overlay = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let compact_overlay = overlay.split_whitespace().collect::<String>();
        assert!(compact_overlay.contains("运行详情"), "overlay={overlay}");
        assert!(
            compact_overlay.contains("查阅详情·工具返回，非模型原创"),
            "overlay={overlay}"
        );
        state.close_details();
        assert!(!state.details_open());

        let observation = ToolObservation {
            call_id: "call-2".into(),
            tool: "read_file".into(),
            status: ToolStatus::Ok,
            summary: "读取大文件".into(),
            content: "字".repeat(4_100),
            error_code: None,
            metadata: std::collections::BTreeMap::new(),
        };
        let bounded = query_result_text(&observation);
        assert!(bounded.contains("查阅结果已截断"));
        assert!(bounded.chars().count() < 4_100);
    }

    #[test]
    fn journal_command_output_is_bounded() {
        let output = bound_tui_command_output("x".repeat(30_000));
        assert!(output.chars().count() < 30_000);
        assert!(output.contains("命令输出已截断"));
    }

    #[test]
    fn search_results_show_three_representative_human_readable_hits() {
        let hits = (1..=5)
            .map(|line| {
                serde_json::json!({
                    "path": format!("src/example_{line}.rs"),
                    "line": line,
                    "column": 1,
                    "excerpt": format!("matched line {line}")
                })
            })
            .collect::<Vec<_>>();
        let observation = ToolObservation {
            call_id: "call-search".into(),
            tool: "search_files".into(),
            status: ToolStatus::Ok,
            summary: "found 5 matches".into(),
            content: serde_json::json!({
                "hits": hits,
                "files_scanned": 20,
                "skipped_unsafe": 0,
                "truncated": false
            })
            .to_string(),
            error_code: None,
            metadata: std::collections::BTreeMap::new(),
        };
        let formatted = query_result_text(&observation);
        assert!(formatted.contains("找到 5 处匹配，扫描 20 个文件。"));
        assert!(formatted.contains("src/example_1.rs:1"));
        assert!(formatted.contains("src/example_2.rs:2"));
        assert!(formatted.contains("src/example_3.rs:3"));
        assert!(!formatted.contains("src/example_4.rs:4"));
        assert!(formatted.contains("另有 2 处匹配"));
        assert!(!formatted.contains("\"hits\""));
    }
}

async fn write_host_response(
    stdout: &Arc<tokio::sync::Mutex<tokio::io::Stdout>>,
    response: &HostResponse,
) -> Result<()> {
    let mut stdout = stdout.lock().await;
    stdout
        .write_all(serde_json::to_string(response)?.as_bytes())
        .await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

fn render_answer(result: &first_principles_agent::RunResult) -> String {
    let answer = &result.answer;
    let mut output = format!(
        "状态：{}\n结论：{}\n终止原因：{:?}",
        answer_status_label(answer.status),
        answer.conclusion,
        result.stop_reason
    );
    if !answer.claims.is_empty() {
        output.push_str("\n主张：");
        for claim in &answer.claims {
            output.push_str(&format!(
                "\n- [{}] {}",
                claim_status_label(claim.status),
                claim.text
            ));
        }
    }
    if !answer.evidence.is_empty() {
        output.push_str("\n依据：");
        for evidence in &answer.evidence {
            output.push_str(&format!("\n- [{}] {}", evidence.source, evidence.statement));
        }
    }
    if !answer.assumptions.is_empty() {
        output.push_str("\n假设：");
        for assumption in &answer.assumptions {
            output.push_str(&format!(
                "\n- [{}] {}",
                answer_impact_label(assumption.impact),
                assumption.description
            ));
        }
    }
    if !answer.constraints.is_empty() {
        output.push_str("\n约束：");
        for constraint in &answer.constraints {
            output.push_str(&format!("\n- {constraint}"));
        }
    }
    if !answer.unknowns.is_empty() {
        output.push_str("\n未知：");
        for unknown in &answer.unknowns {
            output.push_str(&format!("\n- {}", unknown.description));
        }
    }
    if !answer.flip_conditions.is_empty() {
        output.push_str("\n翻转条件：");
        for condition in &answer.flip_conditions {
            output.push_str(&format!(
                "\n- 若 {}，则 {}",
                condition.if_observed, condition.then
            ));
        }
    }
    output
}

fn answer_impact_label(impact: first_principles_agent::ImpactLevel) -> &'static str {
    use first_principles_agent::ImpactLevel;
    match impact {
        ImpactLevel::Low => "低",
        ImpactLevel::Medium => "中",
        ImpactLevel::High => "高",
    }
}

fn print_route_trace(journal_path: &std::path::Path) -> Result<()> {
    let events = read_events(journal_path)?;
    if let Some(summary) = summarize_route_trace(&events) {
        println!("\n{}", summary.render());
    }
    Ok(())
}

fn answer_status_label(status: first_principles_agent::AnswerStatus) -> &'static str {
    use first_principles_agent::AnswerStatus;
    match status {
        AnswerStatus::Completed => "完成",
        AnswerStatus::Conditional => "有条件完成",
        AnswerStatus::NeedsEvidence => "需证据",
        AnswerStatus::NeedsAuthorization => "需授权",
        AnswerStatus::Failed => "无法完成",
    }
}

fn claim_status_label(status: first_principles_agent::ClaimStatus) -> &'static str {
    use first_principles_agent::ClaimStatus;
    match status {
        ClaimStatus::Verified => "已证实",
        ClaimStatus::UserAsserted => "用户提供",
        ClaimStatus::Derived => "推导",
        ClaimStatus::Assumed => "假设",
        ClaimStatus::ModelPrior => "模型先验",
        ClaimStatus::Conflicting => "冲突",
        ClaimStatus::Unknown => "未知",
    }
}

#[cfg(test)]
mod reframe_failure_tests {
    use super::*;

    fn proposal(role: RequestRole) -> ReframeProposal {
        ReframeProposal {
            schema_version: first_principles_agent::reframe::REFRAME_SCHEMA_VERSION,
            stated_request: "我要买个电钻".into(),
            classification: role,
            confidence_bps: Some(8_000),
            question: (role == RequestRole::Means).then(|| "做什么用？".to_string()),
            options: if role == RequestRole::Means {
                vec!["家里用".into(), "专业用".into()]
            } else {
                Vec::new()
            },
            hypotheses: Vec::new(),
            substitutes: if role == RequestRole::Means {
                vec!["借用".into()]
            } else {
                Vec::new()
            },
            stop_rule: first_principles_agent::reframe::StopRule {
                verifiable: true,
                deliverable: true,
                better_substitute_exists: true,
            },
            stop_rule_source: first_principles_agent::reframe::StopRuleSource::Derived,
        }
    }

    #[test]
    fn a_failed_attempt_clears_the_pending_proposal() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_pending_reframe(proposal(RequestRole::Means));
        let opened = apply_reframe_result(&mut state, Err("network down".into()));
        assert!(!opened);
        assert!(
            state.pending_reframe().is_none(),
            "a stale proposal would let a later accept target something the user never saw"
        );
    }

    #[test]
    fn an_end_proposal_clears_the_pending_proposal_and_opens_no_panel() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_pending_reframe(proposal(RequestRole::Means));
        let opened = apply_reframe_result(&mut state, Ok(proposal(RequestRole::End)));
        assert!(!opened);
        assert!(state.pending_reframe().is_none());
    }

    #[test]
    fn a_means_proposal_is_kept_and_the_panel_opens() {
        let mut state = TuiState::new("mock", 6, 120);
        let opened = apply_reframe_result(&mut state, Ok(proposal(RequestRole::Means)));
        assert!(opened);
        assert!(state.pending_reframe().is_some());
        assert!(state.command_panel_open());
    }
}

#[cfg(test)]
mod next_action_panel_tests {
    use super::*;

    #[test]
    fn the_next_action_panel_opens_for_a_master_with_a_default_entry() {
        let mut state = TuiState::new("mock", 6, 120);
        state.set_host_binding(HostBinding::workshop_context(
            "master",
            "main-coordinator",
            "1agent",
            "term-1790000000000-1234-1",
        ));
        assert!(display_tui_next_actions(&mut state));
        assert!(state.command_panel_open());
        assert!(state.command_panel_menu_open());
    }

    #[test]
    fn a_standalone_turn_does_not_open_the_next_action_panel() {
        let mut state = TuiState::new("mock", 6, 120);
        assert!(!display_tui_next_actions(&mut state));
        assert!(!state.command_panel_open());
    }
}
