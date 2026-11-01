use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use first_principles_agent::{
    AgentConfig, AgentRuntime, AnnotationLabel, AnnotationRecord, AnnotationStore,
    ApprovalEnvelope, ApprovalKeyRegistry, ApprovalKeyStatus, ArtifactKind, CalibrationPrediction,
    DeterministicProvider, ExperienceCandidateSpec, ExperienceStore, ExperienceVerificationInput,
    GoldSetManifest, HostCommand, HostError, HostRegistry, HostRequestState, HostResponse,
    OpenAiCompatibleProvider, ProviderAdapter, ProviderCandidate, ProviderProbeReport,
    ProviderToolProbeReport, RegistrySnapshot, ReleasePolicy, ReleasePolicyMode, RouteObjective,
    RunBudget, RuntimeError, TaskRequest, ToolRegistry, TrustAnchorManifest, audit_events,
    build_release_verification_report, collect_provenance, collect_release_manifest,
    create_goldset_manifest, create_trust_anchor, default_registry, evaluate_experience_suite,
    load_suite, local_manifest, probe_provider, probe_provider_tools, read_events,
    rebuild_and_compare, remote_manifest, replay_events, run_suite_with_providers, scan_history,
    verify_goldset_manifest, verify_provenance, verify_release_approval, verify_release_manifest,
    verify_snapshot_chain, verify_snapshot_chain_with_anchor,
};
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
        #[arg(long, default_value = ".journal")]
        journal_dir: PathBuf,
        #[arg(long)]
        experience_dir: Option<PathBuf>,
        #[arg(long)]
        experience_scope: Option<String>,
        #[arg(long, default_value_t = 6)]
        max_steps: usize,
        #[arg(long, default_value_t = 8)]
        max_tool_calls: usize,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 100_000)]
        max_context_chars: usize,
        #[arg(long, default_value = "quality")]
        route_objective: String,
        #[arg(long)]
        json: bool,
    },
    Audit {
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        json: bool,
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
        #[arg(long, default_value = ".journal")]
        journal_dir: PathBuf,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 4)]
        max_concurrency: usize,
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
    History {
        #[arg(long, default_value = ".journal")]
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
        #[arg(long, default_value = ".journal")]
        journal_dir: PathBuf,
        #[arg(long, default_value_t = 120)]
        timeout_seconds: u64,
        #[arg(long, default_value = "quality")]
        route_objective: String,
        #[arg(long)]
        json: bool,
    },
    Experience {
        #[command(subcommand)]
        command: ExperienceCommand,
    },
}

#[derive(Subcommand)]
enum ExperienceCommand {
    Propose {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        journal: PathBuf,
        #[arg(long)]
        input: PathBuf,
    },
    Review {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        actor: String,
    },
    Verify {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        input: PathBuf,
    },
    Evaluate {
        #[arg(long, default_value = ".experience")]
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
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        actor: String,
    },
    Reject {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        reason: String,
    },
    Activate {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
    },
    Degrade {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        reason: String,
    },
    Rollback {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
        #[arg(long)]
        id: Uuid,
        #[arg(long)]
        reason: String,
    },
    List {
        #[arg(long, default_value = ".experience")]
        store: PathBuf,
    },
}

#[derive(Subcommand)]
enum AnnotateCommand {
    Add {
        #[arg(long, default_value = ".annotations")]
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
        #[arg(long, default_value = ".annotations")]
        store: PathBuf,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long, default_value = ".annotations")]
        store: PathBuf,
    },
    Calibrate {
        #[arg(long, default_value = ".annotations")]
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
            timeout_seconds,
            max_context_chars,
            route_objective,
            json,
        } => {
            let mut request = load_request(task, input)?;
            request.budget = RunBudget {
                max_steps,
                max_tool_calls,
                timeout_ms: timeout_seconds.saturating_mul(1_000),
                max_context_chars,
            };
            request.permissions.allow_network = allow_network;
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
            let mut runtime = AgentRuntime::new(candidates, tools, journal_dir)
                .with_route_objective(parse_route_objective(&route_objective)?);
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
                println!("\n运行日志：{}", result.journal_path.display());
            }
        }
        Command::Audit { journal, json } => {
            let events = read_events(&journal)
                .with_context(|| format!("cannot read {}", journal.display()))?;
            let audit = audit_events(&events);
            if json {
                println!("{}", serde_json::to_string_pretty(&audit)?);
            } else if audit.ok {
                println!(
                    "审计通过：{} 个事件，run_id={}，终止原因={:?}",
                    audit.event_count,
                    audit.run_id.map(|id| id.to_string()).unwrap_or_default(),
                    audit.terminal_reason
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
                    "评测：{}，通过 {}/{}，硬门禁失败 {}，rubric {}/{}",
                    report.suite,
                    report.passed,
                    report.total,
                    report.hard_gate_failures,
                    report.rubric_passed,
                    report.rubric_checks
                );
                for case in &report.cases {
                    println!(
                        "- {}: {}",
                        case.id,
                        if case.passed { "PASS" } else { "FAIL" }
                    );
                    for issue in &case.issues {
                        println!("  - {issue}");
                    }
                }
            }
            if report.failed > 0 {
                bail!("evaluation failed");
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
            let runtime = Arc::new(AgentRuntime::new(candidates, tools, journal_dir));
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
        Command::Annotate { command } => {
            handle_annotations(command)?;
        }
        Command::Approval { command } => {
            handle_approval(command)?;
        }
        Command::Provenance { command } => {
            handle_provenance(command)?;
        }
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
            let runtime = AgentRuntime::new(candidates, tools, journal_dir)
                .with_route_objective(parse_route_objective(&route_objective)?);
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
                println!("\n运行日志：{}", result.journal_path.display());
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
        ExperienceCommand::Review { store, id, actor } => {
            ExperienceStore::open(store)?.review(id, &actor)?;
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
        ExperienceCommand::Approve { store, id, actor } => {
            ExperienceStore::open(store)?.approve(id, &actor)?;
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
        ExperienceCommand::Activate { store, id } => {
            ExperienceStore::open(store)?.activate(id)?;
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
            output.push_str(&format!("\n- {assumption}"));
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
