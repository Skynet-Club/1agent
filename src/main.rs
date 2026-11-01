use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use first_principles_agent::{
    AgentConfig, AgentRuntime, DeterministicProvider, ExperienceCandidateSpec, ExperienceStore,
    HostError, HostRequest, HostResponse, OpenAiCompatibleProvider, ProviderCandidate, RunBudget,
    TaskRequest, ToolRegistry, audit_events, default_registry, load_suite, local_manifest,
    read_events, remote_manifest, replay_events, run_suite_with_providers,
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
            let mut lines = BufReader::new(tokio::io::stdin()).lines();
            let mut stdout = tokio::io::stdout();
            while let Some(line) = lines.next_line().await? {
                if line.trim().is_empty() {
                    continue;
                }
                let response = match HostRequest::parse(&line) {
                    Ok(request) => {
                        match runtime.run(request.task, cancellation.child_token()).await {
                            Ok(result) => HostResponse::success(request.request_id, result),
                            Err(error) => HostResponse::failure(
                                request.request_id,
                                HostError {
                                    code: "run_failed".into(),
                                    message: error.to_string(),
                                },
                            ),
                        }
                    }
                    Err(error) => HostResponse::failure("<unknown>", error),
                };
                stdout
                    .write_all(serde_json::to_string(&response)?.as_bytes())
                    .await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                if cancellation.is_cancelled() {
                    break;
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
