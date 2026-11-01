use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use first_principles_agent::{
    AgentConfig, AgentRuntime, DeterministicProvider, ExperienceCandidateSpec, ExperienceStore,
    HostCommand, HostError, HostRegistry, HostRequestState, HostResponse, OpenAiCompatibleProvider,
    ProviderCandidate, RunBudget, RuntimeError, TaskRequest, ToolRegistry, audit_events,
    default_registry, load_suite, local_manifest, read_events, remote_manifest, replay_events,
    run_suite_with_providers, scan_history,
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
            let mut runtime = AgentRuntime::new(candidates, tools, journal_dir);
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
                    "评测：{}，通过 {}/{}，硬门禁失败 {}",
                    report.suite, report.passed, report.total, report.hard_gate_failures
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
        Command::Providers { config, json } => {
            let value = if let Some(config_path) = config {
                let config = AgentConfig::load(config_path)?;
                serde_json::to_value(config)?
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
            json,
        } => {
            let timeout = Duration::from_secs(timeout_seconds.max(1));
            let (candidates, tools) = if let Some(config_path) = config {
                let config = AgentConfig::load(&config_path)?;
                (config.build_providers(timeout)?, default_registry())
            } else {
                build_runtime_parts(&provider, model, base_url, api_key, allow_network, timeout)?
            };
            let runtime = AgentRuntime::new(candidates, tools, journal_dir);
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
            handle_experience(command)?;
        }
    }
    Ok(())
}

fn handle_experience(command: ExperienceCommand) -> Result<()> {
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
