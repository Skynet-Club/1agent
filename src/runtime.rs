use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::alerts::{AlertLog, AlertWebhook, provider_health_alert};
use crate::context::{compact_messages_at_tokens, estimate_tokens};
use crate::domain::{
    AgentAnswer, AnswerStatus, CognitiveMode, FlipCondition, ImpactLevel, InjectedExperience,
    MAX_PROVIDER_RETRY_BACKOFF_MS, ModelMessage, ModelRequest, ModelResponse, ModelTurn,
    ProviderRouteKey, RiskLevel, RunResult, RuntimeCheckpoint, StopReason, TaskRequest,
    ToolObservation, ToolStatus, UnknownItem,
};
use crate::execution_gate::{ExecutionGate, ToolCallRequest};
use crate::experience::ExperienceStore;
use crate::journal::{
    Journal, JournalEventKind, ProviderFailure, ResumablePrefix, RetryOutcome, RetryTiming,
    find_resumable_prefix, read_events,
};
use crate::provider::{ModelProvider, ProviderError, agent_system_prompt};
use crate::provider_health::ProviderHealth;
use crate::router::{
    PolicyValueSource, PrivacyClass, ProviderManifest, RouteObjective, RoutePolicySources,
    RouteRequest, Router,
};
use crate::task_graph::MandateStrength;
use crate::tools::ToolRegistry;

#[derive(Clone)]
pub struct ProviderCandidate {
    pub provider: Arc<dyn ModelProvider>,
    pub manifest: ProviderManifest,
}

#[derive(Clone)]
struct ToolGateContext {
    gate: ExecutionGate,
    actor_id: String,
    scope: String,
    strength: MandateStrength,
    now_ms: u128,
}

#[derive(Clone)]
pub struct AgentRuntime {
    providers: Vec<ProviderCandidate>,
    router: Router,
    tools: ToolRegistry,
    journal_dir: PathBuf,
    experience_dir: Option<PathBuf>,
    shadow_experience: Vec<crate::experience::ExperienceCandidate>,
    route_objective: Option<RouteObjective>,
    route_policy_sources: RoutePolicySources,
    selected_route: Option<ProviderRouteKey>,
    tool_gate: Option<ToolGateContext>,
    provider_health: Option<Arc<Mutex<ProviderHealth>>>,
    alerts: Option<Arc<Mutex<AlertLog>>>,
    alert_webhook: Option<Arc<AlertWebhook>>,
}

struct AnswerRepairContext<'a> {
    journal: &'a mut Journal,
    journal_path: PathBuf,
    request: &'a TaskRequest,
    model_request: &'a ModelRequest,
    provider_id: String,
    model: String,
    invalid_content: String,
    failure: String,
    initial_elapsed: Duration,
}

struct RepairedAnswer {
    answer: AgentAnswer,
    stop_reason: Option<StopReason>,
}

impl AgentRuntime {
    pub fn new(
        providers: Vec<ProviderCandidate>,
        tools: ToolRegistry,
        journal_dir: impl Into<PathBuf>,
    ) -> Self {
        let router = Router::new(
            providers
                .iter()
                .map(|candidate| candidate.manifest.clone())
                .collect(),
        );
        Self {
            providers,
            router,
            tools,
            journal_dir: journal_dir.into(),
            experience_dir: None,
            shadow_experience: Vec::new(),
            route_objective: None,
            route_policy_sources: RoutePolicySources {
                objective: PolicyValueSource::RuntimeDefault,
                latency_budget: PolicyValueSource::Unset,
                unit_cost_budget: PolicyValueSource::Unset,
            },
            selected_route: None,
            tool_gate: None,
            provider_health: None,
            alerts: None,
            alert_webhook: None,
        }
    }

    pub fn with_provider_health(mut self, health: Arc<Mutex<ProviderHealth>>) -> Self {
        self.provider_health = Some(health);
        self
    }

    pub fn with_alert_log(mut self, alerts: Arc<Mutex<AlertLog>>) -> Self {
        self.alerts = Some(alerts);
        self
    }

    pub fn with_alert_webhook(mut self, webhook: Arc<AlertWebhook>) -> Self {
        self.alert_webhook = Some(webhook);
        self
    }

    /// Require an Authority/Policy check before every tool call. The gate is
    /// opt-in so the read-only V0 runtime remains unchanged by default.
    pub fn with_execution_gate(
        mut self,
        gate: ExecutionGate,
        actor_id: impl Into<String>,
        scope: impl Into<String>,
        strength: MandateStrength,
        now_ms: u128,
    ) -> Self {
        self.tool_gate = Some(ToolGateContext {
            gate,
            actor_id: actor_id.into(),
            scope: scope.into(),
            strength,
            now_ms,
        });
        self
    }

    pub fn with_route_objective(mut self, objective: RouteObjective) -> Self {
        self.route_objective = Some(objective);
        self
    }

    pub fn with_route_policy_sources(mut self, sources: RoutePolicySources) -> Self {
        self.route_policy_sources = sources;
        self
    }

    pub fn set_route_objective(&mut self, objective: RouteObjective) {
        self.route_objective = Some(objective);
    }

    pub fn route_objective(&self) -> RouteObjective {
        self.route_objective.unwrap_or_default()
    }

    pub fn select_provider(&mut self, provider_id: &str, model: &str) -> bool {
        let Some(candidate) = self.providers.iter().find(|candidate| {
            candidate.manifest.provider_id == provider_id && candidate.manifest.model == model
        }) else {
            return false;
        };
        self.selected_route = Some(ProviderRouteKey {
            provider_id: candidate.manifest.provider_id.clone(),
            model: candidate.manifest.model.clone(),
        });
        true
    }

    pub fn selected_provider(&self) -> Option<&ProviderManifest> {
        let selected = self.selected_route.as_ref()?;
        self.providers
            .iter()
            .find(|candidate| {
                candidate.manifest.provider_id == selected.provider_id
                    && candidate.manifest.model == selected.model
            })
            .map(|candidate| &candidate.manifest)
    }

    pub fn provider_candidate(&self, provider_id: &str, model: &str) -> Option<ProviderCandidate> {
        self.providers
            .iter()
            .find(|candidate| {
                candidate.manifest.provider_id == provider_id && candidate.manifest.model == model
            })
            .cloned()
    }

    pub fn provider_manifests(&self) -> Vec<ProviderManifest> {
        self.providers
            .iter()
            .map(|candidate| candidate.manifest.clone())
            .collect()
    }

    pub fn journal_dir(&self) -> &std::path::Path {
        &self.journal_dir
    }

    pub fn tool_capabilities(&self) -> Vec<crate::tools::ToolCapability> {
        self.tools.capabilities()
    }

    async fn execute_tool(
        &self,
        call: &crate::domain::ToolCall,
        permissions: &crate::domain::PermissionSet,
        cancellation: &CancellationToken,
    ) -> ToolObservation {
        if let Some(context) = &self.tool_gate
            && let Err(error) = context.gate.authorize(
                &ToolCallRequest {
                    actor_id: context.actor_id.clone(),
                    tool: call.name.clone(),
                    scope: context.scope.clone(),
                    strength: context.strength,
                    risk: RiskLevel::Low,
                },
                context.now_ms,
            )
        {
            return ToolObservation {
                call_id: call.id.clone(),
                tool: call.name.clone(),
                status: ToolStatus::PermissionDenied,
                summary: error.to_string(),
                content: String::new(),
                error_code: Some("authority_denied".into()),
                metadata: Default::default(),
            };
        }
        self.tools.execute(call, permissions, cancellation).await
    }

    pub fn with_experience_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.experience_dir = Some(dir.into());
        self
    }

    pub fn with_shadow_experience_candidates(
        mut self,
        candidates: Vec<crate::experience::ExperienceCandidate>,
    ) -> Self {
        self.shadow_experience = candidates;
        self
    }

    async fn repair_answer(
        &self,
        context: AnswerRepairContext<'_>,
        provider_attempts_used: &mut usize,
        started_at: Instant,
        cancellation: &CancellationToken,
    ) -> Result<RepairedAnswer, RuntimeError> {
        const MAX_REPAIR_CONTENT_CHARS: usize = 20_000;
        let AnswerRepairContext {
            journal,
            journal_path,
            request,
            model_request,
            provider_id,
            model,
            mut invalid_content,
            failure,
            initial_elapsed,
        } = context;
        if request.budget.max_provider_attempts != 0
            && *provider_attempts_used >= request.budget.max_provider_attempts
        {
            return Err(RuntimeError::InvalidOutput {
                path: journal_path,
                message: format!("{failure}; provider-attempt budget exhausted before repair"),
            });
        }
        *provider_attempts_used += 1;
        let candidate = self
            .providers
            .iter()
            .find(|candidate| {
                candidate.manifest.provider_id == provider_id && candidate.manifest.model == model
            })
            .ok_or_else(|| RuntimeError::NoRoute {
                path: journal_path.clone(),
                mode: model_request.mode,
            })?;
        if invalid_content.chars().count() > MAX_REPAIR_CONTENT_CHARS {
            invalid_content = invalid_content
                .chars()
                .take(MAX_REPAIR_CONTENT_CHARS)
                .collect::<String>();
            invalid_content.push_str("\n…（原始响应过长，已截断）");
        }
        let mut repair_messages = model_request.messages.clone();
        repair_messages.push(ModelMessage::assistant(invalid_content, Vec::new()));
        repair_messages.push(ModelMessage::user(format!(
            "{failure}. Return exactly one corrected AgentAnswer JSON object with no prose or \
             outer Markdown fence. Markdown and code fences requested by the user may appear only \
             inside the JSON conclusion string. Preserve the intended answer. Every \
             evidence.source must exactly equal one entry in the sources array of every referenced \
             claim. Use null for unknown evidence timestamps. Read-only action_receipts must be \
             empty. model_prior claims must have empty sources."
        )));
        let repair_request = ModelRequest {
            run_id: model_request.run_id,
            mode: model_request.mode,
            messages: repair_messages,
            tools: model_request.tools.clone(),
        };
        journal
            .append(JournalEventKind::ModelRequested {
                request: repair_request.clone(),
            })
            .map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;
        let repair_result = if request.budget.timeout_ms == 0 {
            Ok(candidate
                .provider
                .complete(repair_request, cancellation)
                .await)
        } else {
            let Some(remaining) =
                remaining_budget(started_at, request.budget.timeout_ms, initial_elapsed)
            else {
                return Ok(RepairedAnswer {
                    answer: budget_answer("answer repair timeout exhausted"),
                    stop_reason: Some(StopReason::BudgetExhausted),
                });
            };
            tokio::time::timeout(
                remaining,
                candidate.provider.complete(repair_request, cancellation),
            )
            .await
        };
        let mut repair_response = match repair_result {
            Err(_) => {
                return Ok(RepairedAnswer {
                    answer: budget_answer("answer repair call timed out"),
                    stop_reason: Some(StopReason::BudgetExhausted),
                });
            }
            Ok(Ok(response)) => response,
            Ok(Err(source)) => {
                append_provider_failure(journal, &journal_path, &provider_id, &model, &source)?;
                let _ = journal.append(JournalEventKind::RunStopped {
                    reason: if source.code == crate::provider::ProviderErrorCode::Cancelled {
                        StopReason::Cancelled
                    } else {
                        StopReason::ProviderError
                    },
                });
                return if source.code == crate::provider::ProviderErrorCode::Cancelled {
                    Err(RuntimeError::Cancelled { path: journal_path })
                } else {
                    Err(RuntimeError::Provider {
                        path: journal_path,
                        source,
                    })
                };
            }
        };
        if repair_response.provider_id != provider_id || repair_response.model != model {
            return Err(RuntimeError::InvalidOutput {
                path: journal_path,
                message: "repair response changed provider identity".into(),
            });
        }
        apply_manifest_cost(&mut repair_response, &candidate.manifest);
        journal
            .append(JournalEventKind::ModelResponded {
                response: repair_response.clone(),
            })
            .map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;
        let answer = match repair_response.turn {
            ModelTurn::Final { answer } => answer,
            ModelTurn::InvalidFinal { error, .. } => {
                return Err(RuntimeError::InvalidOutput {
                    path: journal_path,
                    message: format!(
                        "automatic repair did not produce valid AgentAnswer JSON: {error}"
                    ),
                });
            }
            ModelTurn::ToolCalls { .. } => {
                return Err(RuntimeError::InvalidOutput {
                    path: journal_path,
                    message: "answer repair returned tool calls instead of final JSON".into(),
                });
            }
        };
        answer
            .validate(request)
            .map_err(|error| RuntimeError::InvalidOutput {
                path: journal_path,
                message: format!("{error}; automatic repair did not produce a valid answer"),
            })?;
        Ok(RepairedAnswer {
            answer,
            stop_reason: None,
        })
    }

    pub async fn run(
        &self,
        request: TaskRequest,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RuntimeError> {
        self.run_internal(request, None, None, None, cancellation)
            .await
    }

    pub async fn run_with_mode(
        &self,
        request: TaskRequest,
        mode: CognitiveMode,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RuntimeError> {
        self.run_internal(request, None, Some(mode), None, cancellation)
            .await
    }

    pub async fn run_with_client_request_id(
        &self,
        request: TaskRequest,
        client_request_id: impl Into<String>,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RuntimeError> {
        let client_request_id = client_request_id.into();
        if client_request_id.trim().is_empty() {
            return Err(RuntimeError::Resume(
                "client request id must not be empty".into(),
            ));
        }
        self.run_internal(request, None, None, Some(client_request_id), cancellation)
            .await
    }

    pub async fn resume(
        &self,
        journal_path: impl AsRef<Path>,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RuntimeError> {
        let events = read_events(journal_path.as_ref())
            .map_err(|error| RuntimeError::Resume(error.to_string()))?;
        let prefix = find_resumable_prefix(&events)
            .map_err(|error| RuntimeError::Resume(error.to_string()))?;
        self.run_internal(
            prefix.request.clone(),
            Some(prefix),
            None,
            None,
            cancellation,
        )
        .await
    }

    async fn run_internal(
        &self,
        request: TaskRequest,
        resume: Option<ResumablePrefix>,
        requested_mode: Option<CognitiveMode>,
        client_request_id: Option<String>,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RuntimeError> {
        let run_id = Uuid::new_v4();
        let journal_path = self.journal_dir.join(format!("{run_id}.jsonl"));
        let mut journal =
            Journal::create(&journal_path, run_id).map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;
        let mode = resume
            .as_ref()
            .map(|prefix| prefix.checkpoint.mode)
            .or(requested_mode)
            .unwrap_or_else(|| crate::domain::infer_mode(&request));
        if let Some(prefix) = &resume {
            journal
                .append(JournalEventKind::ResumeStarted {
                    original_run_id: prefix.original_run_id,
                    checkpoint_seq: prefix.checkpoint_seq,
                    request: request.clone(),
                    checkpoint: prefix.checkpoint.clone(),
                })
                .map_err(|source| RuntimeError::Journal {
                    path: journal_path.clone(),
                    source,
                })?;
        } else {
            journal
                .append(JournalEventKind::RunStarted {
                    request: request.clone(),
                    mode,
                })
                .map_err(|source| RuntimeError::Journal {
                    path: journal_path.clone(),
                    source,
                })?;
            if let Some(client_request_id) = client_request_id {
                journal
                    .append(JournalEventKind::ClientRequestLinked { client_request_id })
                    .map_err(|source| RuntimeError::Journal {
                        path: journal_path.clone(),
                        source,
                    })?;
            }
        }

        let tool_capabilities = self
            .tools
            .capability_snapshot_for(&request.permissions)
            .map_err(|message| RuntimeError::ToolCapabilities {
                path: journal_path.clone(),
                message,
            })?;
        if let Some(prefix) = &resume
            && let Some(expected) = &prefix.checkpoint.tool_capabilities_sha256
            && expected != &tool_capabilities.sha256
        {
            return Err(RuntimeError::Resume(
                "tool capabilities changed since checkpoint".into(),
            ));
        }
        journal
            .append(JournalEventKind::ToolCapabilitiesAdvertised {
                snapshot: tool_capabilities.clone(),
            })
            .map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;

        if cancellation.is_cancelled() {
            journal
                .append(JournalEventKind::RunStopped {
                    reason: StopReason::Cancelled,
                })
                .map_err(|source| RuntimeError::Journal {
                    path: journal_path.clone(),
                    source,
                })?;
            return Err(RuntimeError::Cancelled { path: journal_path });
        }

        if resume.is_none() && mode == CognitiveMode::Execute && !request.permissions.allow_write {
            let answer = authorization_answer();
            journal
                .append(JournalEventKind::AnswerProduced {
                    answer: answer.clone(),
                })
                .map_err(|source| RuntimeError::Journal {
                    path: journal_path.clone(),
                    source,
                })?;
            journal
                .append(JournalEventKind::RunStopped {
                    reason: StopReason::NeedsAuthorization,
                })
                .map_err(|source| RuntimeError::Journal {
                    path: journal_path.clone(),
                    source,
                })?;
            return Ok(RunResult {
                run_id,
                journal_path,
                answer,
                stop_reason: StopReason::NeedsAuthorization,
            });
        }

        let tool_specs = self.tools.specs_for(&request.permissions);
        let stored_objective = resume
            .as_ref()
            .and_then(|prefix| prefix.checkpoint.route_objective);
        if let (Some(requested), Some(stored)) = (self.route_objective, stored_objective)
            && requested != stored
        {
            return Err(RuntimeError::Resume(
                "route objective changed since checkpoint".into(),
            ));
        }
        let route_objective = self
            .route_objective
            .or(stored_objective)
            .unwrap_or(RouteObjective::Quality);
        let route_request = RouteRequest {
            mode,
            risk: risk_for_mode(mode),
            requires_tools: !tool_specs.is_empty(),
            requires_structured_output: true,
            allow_remote: request.permissions.allow_network,
            objective: route_objective,
            max_provider_p95_latency_ms: request.budget.max_provider_p95_latency_ms,
            max_unit_cost_micros_per_k: request.budget.max_unit_cost_micros_per_k,
        };
        let mut route_plan = self.router.plan(&route_request);
        if let Some(selected) = &self.selected_route {
            route_plan.ranked.retain(|decision| {
                decision.manifest.provider_id == selected.provider_id
                    && decision.manifest.model == selected.model
            });
        }
        let stored_sources = resume
            .as_ref()
            .and_then(|prefix| prefix.checkpoint.route_policy_sources.clone());
        let mut route_policy_sources = stored_sources
            .clone()
            .unwrap_or_else(|| self.route_policy_sources.clone());
        if self.route_objective.is_some()
            && self.route_policy_sources.objective == PolicyValueSource::CliOverride
        {
            route_policy_sources.objective = PolicyValueSource::CliOverride;
        }
        if resume.is_none() && route_policy_sources.latency_budget == PolicyValueSource::Unset {
            route_policy_sources.latency_budget =
                if request.budget.max_provider_p95_latency_ms.is_some() {
                    PolicyValueSource::TaskRequest
                } else {
                    PolicyValueSource::Unset
                };
        }
        if resume.is_none() && route_policy_sources.unit_cost_budget == PolicyValueSource::Unset {
            route_policy_sources.unit_cost_budget =
                if request.budget.max_unit_cost_micros_per_k.is_some() {
                    PolicyValueSource::TaskRequest
                } else {
                    PolicyValueSource::Unset
                };
        }
        let route_plan_digest =
            route_plan_digest(&route_plan, &route_policy_sources).map_err(|message| {
                RuntimeError::RoutePlan {
                    path: journal_path.clone(),
                    message,
                }
            })?;
        if let Some(prefix) = &resume
            && let Some(expected) = &prefix.checkpoint.route_plan_sha256
            && expected != &route_plan_digest
        {
            return Err(RuntimeError::Resume(
                "route plan or policy changed since checkpoint".into(),
            ));
        }
        journal
            .append(JournalEventKind::RoutePlanned {
                plan: route_plan.clone(),
                sources: route_policy_sources.clone(),
            })
            .map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;
        let routes = route_plan.ranked;
        if routes.is_empty() {
            return Err(RuntimeError::NoRoute {
                path: journal_path.clone(),
                mode,
            });
        }

        let started_at = Instant::now();
        let (
            mut messages,
            mut tool_calls,
            mut unavailable_routes,
            mut provider_attempts_used,
            start_step,
            initial_elapsed,
        ) = if let Some(prefix) = resume {
            (
                prefix.checkpoint.messages,
                prefix.checkpoint.tool_calls_used,
                prefix
                    .checkpoint
                    .unavailable_routes
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
                prefix.checkpoint.provider_attempts_used,
                prefix.checkpoint.next_step,
                Duration::from_millis(prefix.checkpoint.elapsed_ms),
            )
        } else {
            let mut initial_messages = vec![ModelMessage::system(agent_system_prompt())];
            if !request.known_facts.is_empty() {
                initial_messages.push(ModelMessage::system(format!(
                    "CONVERSATION CONTEXT\nThese facts come from earlier turns and the local prompt library. The new task may be a follow-up; resolve short replies, names and pronouns against them. Prefer matching prompt-library entries over web search when the user asks about prompts or a named prompt author.\n{}",
                    request.known_facts.join("\n")
                )));
            }
            initial_messages.push(ModelMessage::user(format!(
                "Task objective:\n{}\n\nStructured task:\n{}",
                request.objective,
                serde_json::to_string_pretty(&request)
                    .unwrap_or_else(|_| request.objective.clone())
            )));
            if let Some(scope) = &request.experience_scope {
                let mut selected = if let Some(experience_dir) = &self.experience_dir {
                    let store = ExperienceStore::open(experience_dir)
                        .map_err(|error| RuntimeError::Experience(error.to_string()))?;
                    store
                        .retrieve_active(mode, scope, request.budget.max_context_tokens.min(4_000))
                        .map_err(|error| RuntimeError::Experience(error.to_string()))?
                } else {
                    Vec::new()
                };
                selected.extend(
                    self.shadow_experience
                        .iter()
                        .filter(|candidate| {
                            candidate.status == crate::experience::ExperienceStatus::Reviewed
                                && candidate.applicable_modes.contains(&mode)
                                && (candidate.scope == "*" || candidate.scope == scope.as_str())
                        })
                        .cloned(),
                );
                selected.sort_by(|left, right| {
                    right
                        .updated_unix_ms
                        .cmp(&left.updated_unix_ms)
                        .then_with(|| left.id.cmp(&right.id))
                });
                let mut bounded = Vec::new();
                let mut used_tokens = 0usize;
                for candidate in selected {
                    let cost = estimate_tokens(&candidate.title)
                        .saturating_add(estimate_tokens(&candidate.content));
                    if used_tokens + cost > request.budget.max_context_tokens.min(4_000) {
                        continue;
                    }
                    used_tokens += cost;
                    bounded.push(candidate);
                }
                let selected = bounded;
                if !selected.is_empty() {
                    let guidance = selected
                        .iter()
                        .map(|candidate| {
                            format!(
                                "- [{} v{}] {}: {}",
                                experience_kind_label(candidate.kind),
                                candidate.version,
                                candidate.title,
                                candidate.content
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    initial_messages.push(ModelMessage::system(format!(
                            "EXPERIENCE GUIDANCE\nThis is approved but unverified policy guidance, not factual evidence. Apply only within the stated scope and never override current task constraints.\n{guidance}"
                        )));
                    journal
                        .append(JournalEventKind::ExperienceInjected {
                            injected: selected
                                .iter()
                                .map(|candidate| InjectedExperience {
                                    id: candidate.id,
                                    version: candidate.version,
                                    reason: format!(
                                        "{}{}; mode={mode:?}; scope={}",
                                        if candidate.status
                                            == crate::experience::ExperienceStatus::Active
                                        {
                                            "active"
                                        } else {
                                            "shadow-reviewed"
                                        },
                                        "",
                                        candidate.scope
                                    ),
                                })
                                .collect(),
                        })
                        .map_err(|source| RuntimeError::Journal {
                            path: journal_path.clone(),
                            source,
                        })?;
                }
            }
            (initial_messages, 0, BTreeSet::new(), 0, 0, Duration::ZERO)
        };
        let max_provider_attempts = match request.budget.max_provider_attempts {
            0 => usize::MAX,
            value => value.clamp(1, 4096),
        };
        let mut routes = routes;
        if let Some(health) = &self.provider_health {
            let now_ms = initial_elapsed
                .saturating_add(started_at.elapsed())
                .as_millis();
            let available: Vec<_> = routes
                .iter()
                .filter(|route| {
                    health
                        .lock()
                        .map(|health| {
                            health.available(
                                &route.manifest.provider_id,
                                &route.manifest.model,
                                now_ms,
                            )
                        })
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            if !available.is_empty() {
                routes = available;
            }
        }

        let max_steps = match request.budget.max_steps {
            0 => usize::MAX,
            value => value,
        };
        for step in start_step..max_steps {
            if cancellation.is_cancelled() {
                journal
                    .append(JournalEventKind::RunStopped {
                        reason: StopReason::Cancelled,
                    })
                    .map_err(|source| RuntimeError::Journal {
                        path: journal_path.clone(),
                        source,
                    })?;
                return Err(RuntimeError::Cancelled { path: journal_path });
            }
            if request.budget.timeout_ms != 0
                && remaining_budget(started_at, request.budget.timeout_ms, initial_elapsed)
                    .is_none()
            {
                return finish_with_answer(
                    &mut journal,
                    run_id,
                    journal_path,
                    budget_answer("run timeout exhausted"),
                    StopReason::BudgetExhausted,
                );
            }

            let trigger_tokens = request
                .budget
                .auto_compact_tokens
                .min(request.budget.max_context_tokens);
            let (model_messages, context_compaction) =
                compact_messages_at_tokens(&messages, trigger_tokens, 4);
            if let Some(compaction) = context_compaction {
                journal
                    .append(JournalEventKind::ContextCompacted {
                        before_tokens: compaction.before_tokens,
                        after_tokens: compaction.after_tokens,
                        before_chars: compaction.before_chars,
                        after_chars: compaction.after_chars,
                        compacted_tool_results: compaction.compacted_tool_results,
                    })
                    .map_err(|source| RuntimeError::Journal {
                        path: journal_path.clone(),
                        source,
                    })?;
            }
            let model_request = ModelRequest {
                run_id,
                mode,
                messages: model_messages,
                tools: tool_specs.clone(),
            };
            let mut response = None;
            let mut last_retryable_error = None;
            let mut attempt_limit_reached = false;
            for (attempt, route) in routes.iter().enumerate() {
                let route_key = ProviderRouteKey {
                    provider_id: route.manifest.provider_id.clone(),
                    model: route.manifest.model.clone(),
                };
                if unavailable_routes.contains(&route_key) {
                    continue;
                }
                if provider_attempts_used >= max_provider_attempts {
                    attempt_limit_reached = true;
                    break;
                }
                provider_attempts_used += 1;
                let candidate = self
                    .providers
                    .iter()
                    .find(|candidate| {
                        candidate.manifest.provider_id == route.manifest.provider_id
                            && candidate.manifest.model == route.manifest.model
                    })
                    .ok_or_else(|| RuntimeError::NoRoute {
                        path: journal_path.clone(),
                        mode,
                    })?;
                let reason = if attempt == 0 {
                    route.reason.clone()
                } else {
                    format!("fallback attempt {}: {}", attempt + 1, route.reason)
                };
                journal
                    .append(JournalEventKind::RouteSelected {
                        provider_id: route.manifest.provider_id.clone(),
                        model: route.manifest.model.clone(),
                        reason,
                    })
                    .map_err(|source| RuntimeError::Journal {
                        path: journal_path.clone(),
                        source,
                    })?;
                journal
                    .append(JournalEventKind::ModelRequested {
                        request: model_request.clone(),
                    })
                    .map_err(|source| RuntimeError::Journal {
                        path: journal_path.clone(),
                        source,
                    })?;
                let provider_result = if request.budget.timeout_ms == 0 {
                    Ok(candidate
                        .provider
                        .complete(model_request.clone(), &cancellation)
                        .await)
                } else {
                    let Some(remaining) =
                        remaining_budget(started_at, request.budget.timeout_ms, initial_elapsed)
                    else {
                        return finish_with_answer(
                            &mut journal,
                            run_id,
                            journal_path,
                            budget_answer("provider fallback timeout exhausted"),
                            StopReason::BudgetExhausted,
                        );
                    };
                    tokio::time::timeout(
                        remaining,
                        candidate
                            .provider
                            .complete(model_request.clone(), &cancellation),
                    )
                    .await
                };
                match provider_result {
                    Err(_) => {
                        return finish_with_answer(
                            &mut journal,
                            run_id,
                            journal_path,
                            budget_answer("provider call timeout exhausted"),
                            StopReason::BudgetExhausted,
                        );
                    }
                    Ok(Ok(mut candidate_response)) => {
                        if candidate_response.provider_id != route.manifest.provider_id
                            || candidate_response.model != route.manifest.model
                        {
                            let _ = journal.append(JournalEventKind::RunStopped {
                                reason: StopReason::InvalidOutput,
                            });
                            return Err(RuntimeError::InvalidOutput {
                                path: journal_path,
                                message: format!(
                                    "provider identity mismatch: expected {}/{}, got {}/{}",
                                    route.manifest.provider_id,
                                    route.manifest.model,
                                    candidate_response.provider_id,
                                    candidate_response.model
                                ),
                            });
                        }
                        apply_manifest_cost(&mut candidate_response, &route.manifest);
                        if let Some(health) = &self.provider_health
                            && let Ok(mut health) = health.lock()
                        {
                            let now_ms = initial_elapsed
                                .saturating_add(started_at.elapsed())
                                .as_millis();
                            health.record_success(
                                &route.manifest.provider_id,
                                &route.manifest.model,
                                now_ms,
                            );
                        }
                        response = Some(candidate_response);
                        break;
                    }
                    Ok(Err(source)) if source.retryable => {
                        let health_now_ms = initial_elapsed
                            .saturating_add(started_at.elapsed())
                            .as_millis();
                        let mut alert_info = None;
                        if let Some(health) = &self.provider_health
                            && let Ok(mut health) = health.lock()
                        {
                            let crossed = health.record_failure(
                                &route.manifest.provider_id,
                                &route.manifest.model,
                                health_now_ms,
                            );
                            if crossed {
                                let failures = health
                                    .snapshot()
                                    .into_iter()
                                    .find(|entry| {
                                        entry.provider_id == route.manifest.provider_id
                                            && entry.model == route.manifest.model
                                    })
                                    .map(|entry| entry.consecutive_failures)
                                    .unwrap_or(1);
                                alert_info = Some((failures, health.policy().failure_threshold));
                            }
                        }
                        if let Some((failures, threshold)) = alert_info {
                            let alert = provider_health_alert(
                                &route.manifest.provider_id,
                                &route.manifest.model,
                                health_now_ms,
                                failures,
                                threshold,
                            );
                            if let Some(alerts) = &self.alerts
                                && let Ok(alerts) = alerts.lock()
                            {
                                let _ = alerts.append(&alert);
                            }
                            if let Some(webhook) = &self.alert_webhook {
                                let _ = tokio::time::timeout(
                                    std::time::Duration::from_secs(5),
                                    webhook.send(&alert),
                                )
                                .await;
                            }
                        }
                        append_provider_failure(
                            &mut journal,
                            &journal_path,
                            &route.manifest.provider_id,
                            &route.manifest.model,
                            &source,
                        )?;
                        unavailable_routes.insert(route_key);
                        last_retryable_error = Some(source);
                        let next_route = routes.iter().skip(attempt + 1).find(|route| {
                            !unavailable_routes.contains(&ProviderRouteKey {
                                provider_id: route.manifest.provider_id.clone(),
                                model: route.manifest.model.clone(),
                            })
                        });
                        if let Some(next_route) = next_route
                            && provider_attempts_used < max_provider_attempts
                        {
                            let delay_ms = provider_retry_backoff_ms(
                                request.budget.provider_retry_backoff_ms,
                                attempt + 1,
                            );
                            let delay = Duration::from_millis(delay_ms);
                            if request.budget.timeout_ms != 0 {
                                let Some(remaining) = remaining_budget(
                                    started_at,
                                    request.budget.timeout_ms,
                                    initial_elapsed,
                                ) else {
                                    return finish_with_answer(
                                        &mut journal,
                                        run_id,
                                        journal_path,
                                        budget_answer("retry backoff exhausted run time"),
                                        StopReason::BudgetExhausted,
                                    );
                                };
                                if remaining <= delay {
                                    return finish_with_answer(
                                        &mut journal,
                                        run_id,
                                        journal_path,
                                        budget_answer("retry backoff exceeds remaining run time"),
                                        StopReason::BudgetExhausted,
                                    );
                                }
                            }
                            journal
                                .append(JournalEventKind::ProviderRetryScheduled {
                                    delay_ms,
                                    next_provider_id: next_route.manifest.provider_id.clone(),
                                    next_model: next_route.manifest.model.clone(),
                                })
                                .map_err(|source| RuntimeError::Journal {
                                    path: journal_path.clone(),
                                    source,
                                })?;
                            let sleep_started = Instant::now();
                            let outcome = tokio::select! {
                                _ = cancellation.cancelled() => {
                                    RetryOutcome::Cancelled
                                }
                                _ = tokio::time::sleep(delay) => RetryOutcome::Completed
                            };
                            journal
                                .append(JournalEventKind::ProviderRetryElapsed {
                                    timing: RetryTiming {
                                        delay_ms,
                                        elapsed_ms: sleep_started
                                            .elapsed()
                                            .as_millis()
                                            .min(u128::from(u64::MAX))
                                            as u64,
                                        outcome,
                                    },
                                })
                                .map_err(|source| RuntimeError::Journal {
                                    path: journal_path.clone(),
                                    source,
                                })?;
                            if outcome == RetryOutcome::Cancelled {
                                journal
                                    .append(JournalEventKind::RunStopped {
                                        reason: StopReason::Cancelled,
                                    })
                                    .map_err(|source| RuntimeError::Journal {
                                        path: journal_path.clone(),
                                        source,
                                    })?;
                                return Err(RuntimeError::Cancelled {
                                    path: journal_path.clone(),
                                });
                            }
                        }
                    }
                    Ok(Err(source))
                        if source.code == crate::provider::ProviderErrorCode::Cancelled =>
                    {
                        let _ = journal.append(JournalEventKind::RunStopped {
                            reason: StopReason::Cancelled,
                        });
                        return Err(RuntimeError::Cancelled {
                            path: journal_path.clone(),
                        });
                    }
                    Ok(Err(source)) => {
                        append_provider_failure(
                            &mut journal,
                            &journal_path,
                            &route.manifest.provider_id,
                            &route.manifest.model,
                            &source,
                        )?;
                        let _ = journal.append(JournalEventKind::RunStopped {
                            reason: StopReason::ProviderError,
                        });
                        return Err(RuntimeError::Provider {
                            path: journal_path.clone(),
                            source,
                        });
                    }
                }
            }
            let response = match response {
                Some(response) => response,
                None => {
                    if attempt_limit_reached {
                        return finish_with_answer(
                            &mut journal,
                            run_id,
                            journal_path,
                            budget_answer("provider attempt budget exhausted"),
                            StopReason::BudgetExhausted,
                        );
                    }
                    let _ = journal.append(JournalEventKind::RunStopped {
                        reason: StopReason::ProviderError,
                    });
                    return Err(RuntimeError::Provider {
                        path: journal_path.clone(),
                        source: last_retryable_error.unwrap_or_else(|| ProviderError {
                            code: crate::provider::ProviderErrorCode::ProviderUnavailable,
                            message: "all provider candidates failed".into(),
                            retryable: false,
                        }),
                    });
                }
            };
            journal
                .append(JournalEventKind::ModelResponded {
                    response: response.clone(),
                })
                .map_err(|source| RuntimeError::Journal {
                    path: journal_path.clone(),
                    source,
                })?;
            let response_provider_id = response.provider_id.clone();
            let response_model = response.model.clone();

            match response.turn {
                ModelTurn::Final { answer } => {
                    let mut answer = answer;
                    if let Err(validation_error) = answer.validate(&request) {
                        let invalid_json = serde_json::to_string(&answer).map_err(|error| {
                            RuntimeError::InvalidOutput {
                                path: journal_path.clone(),
                                message: format!(
                                    "cannot serialize invalid answer for repair: {error}"
                                ),
                            }
                        })?;
                        let repaired = self
                            .repair_answer(
                                AnswerRepairContext {
                                    journal: &mut journal,
                                    journal_path: journal_path.clone(),
                                    request: &request,
                                    model_request: &model_request,
                                    provider_id: response_provider_id,
                                    model: response_model,
                                    invalid_content: invalid_json,
                                    failure: format!(
                                        "Your previous JSON failed validation: {validation_error}"
                                    ),
                                    initial_elapsed,
                                },
                                &mut provider_attempts_used,
                                started_at,
                                &cancellation,
                            )
                            .await?;
                        answer = repaired.answer;
                        if let Some(stop_reason) = repaired.stop_reason {
                            return finish_with_answer(
                                &mut journal,
                                run_id,
                                journal_path,
                                answer,
                                stop_reason,
                            );
                        }
                    }
                    let stop_reason = stop_reason_for_answer(answer.status);
                    return finish_with_answer(
                        &mut journal,
                        run_id,
                        journal_path,
                        answer,
                        stop_reason,
                    );
                }
                ModelTurn::InvalidFinal { content, error } => {
                    let repaired = self
                        .repair_answer(
                            AnswerRepairContext {
                                journal: &mut journal,
                                journal_path: journal_path.clone(),
                                request: &request,
                                model_request: &model_request,
                                provider_id: response_provider_id,
                                model: response_model,
                                invalid_content: content,
                                failure: format!(
                                    "Your previous response could not be parsed as a valid \
                                     AgentAnswer JSON object: {error}"
                                ),
                                initial_elapsed,
                            },
                            &mut provider_attempts_used,
                            started_at,
                            &cancellation,
                        )
                        .await?;
                    let stop_reason = repaired
                        .stop_reason
                        .unwrap_or_else(|| stop_reason_for_answer(repaired.answer.status));
                    return finish_with_answer(
                        &mut journal,
                        run_id,
                        journal_path,
                        repaired.answer,
                        stop_reason,
                    );
                }
                ModelTurn::ToolCalls { calls } => {
                    if calls.is_empty() {
                        return Err(RuntimeError::InvalidOutput {
                            path: journal_path,
                            message: "model returned an empty tool-call list".into(),
                        });
                    }
                    if request.budget.max_tool_calls != 0
                        && tool_calls.saturating_add(calls.len()) > request.budget.max_tool_calls
                    {
                        let answer = budget_answer("tool-call budget exhausted");
                        journal
                            .append(JournalEventKind::AnswerProduced {
                                answer: answer.clone(),
                            })
                            .map_err(|source| RuntimeError::Journal {
                                path: journal_path.clone(),
                                source,
                            })?;
                        journal
                            .append(JournalEventKind::RunStopped {
                                reason: StopReason::BudgetExhausted,
                            })
                            .map_err(|source| RuntimeError::Journal {
                                path: journal_path.clone(),
                                source,
                            })?;
                        return Ok(RunResult {
                            run_id,
                            journal_path,
                            answer,
                            stop_reason: StopReason::BudgetExhausted,
                        });
                    }
                    tool_calls = tool_calls.saturating_add(calls.len());
                    messages.push(ModelMessage::assistant("", calls.clone()));

                    for call in calls {
                        if request.budget.timeout_ms != 0
                            && remaining_budget(
                                started_at,
                                request.budget.timeout_ms,
                                initial_elapsed,
                            )
                            .is_none()
                        {
                            return finish_with_answer(
                                &mut journal,
                                run_id,
                                journal_path,
                                budget_answer("tool phase timeout exhausted"),
                                StopReason::BudgetExhausted,
                            );
                        }
                        journal
                            .append(JournalEventKind::ToolCalled { call: call.clone() })
                            .map_err(|source| RuntimeError::Journal {
                                path: journal_path.clone(),
                                source,
                            })?;
                        let observation = self
                            .execute_tool(&call, &request.permissions, &cancellation)
                            .await;
                        journal
                            .append(JournalEventKind::ToolObserved {
                                observation: observation.clone(),
                            })
                            .map_err(|source| RuntimeError::Journal {
                                path: journal_path.clone(),
                                source,
                            })?;
                        messages.push(ModelMessage::tool(call.id, observation.model_content()));
                    }
                    journal
                        .append(JournalEventKind::Checkpoint {
                            checkpoint: RuntimeCheckpoint {
                                mode,
                                messages: messages.clone(),
                                tool_calls_used: tool_calls,
                                provider_attempts_used,
                                unavailable_routes: unavailable_routes.iter().cloned().collect(),
                                next_step: step + 1,
                                elapsed_ms: initial_elapsed
                                    .saturating_add(started_at.elapsed())
                                    .as_millis()
                                    .min(u128::from(u64::MAX))
                                    as u64,
                                route_plan_sha256: Some(route_plan_digest.clone()),
                                route_policy_sources: Some(route_policy_sources.clone()),
                                route_objective: Some(route_request.objective),
                                tool_capabilities_sha256: Some(tool_capabilities.sha256.clone()),
                            },
                        })
                        .map_err(|source| RuntimeError::Journal {
                            path: journal_path.clone(),
                            source,
                        })?;
                }
            }
        }

        let answer = budget_answer("step budget exhausted");
        journal
            .append(JournalEventKind::AnswerProduced {
                answer: answer.clone(),
            })
            .map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;
        journal
            .append(JournalEventKind::RunStopped {
                reason: StopReason::BudgetExhausted,
            })
            .map_err(|source| RuntimeError::Journal {
                path: journal_path.clone(),
                source,
            })?;
        Ok(RunResult {
            run_id,
            journal_path,
            answer,
            stop_reason: StopReason::BudgetExhausted,
        })
    }
}

pub fn local_manifest() -> ProviderManifest {
    ProviderManifest {
        provider_id: "deterministic".into(),
        model: "offline-v1".into(),
        privacy: PrivacyClass::Local,
        supported_modes: vec![
            CognitiveMode::DirectAnswer,
            CognitiveMode::EvidenceResearch,
            CognitiveMode::DecisionAnalysis,
            CognitiveMode::Execute,
            CognitiveMode::Diagnosis,
            CognitiveMode::Escalate,
        ],
        supports_tools: true,
        supports_structured_output: true,
        max_risk: RiskLevel::High,
        quality_score: 1,
        input_cost_micros_per_k: 0,
        output_cost_micros_per_k: 0,
        p95_latency_ms: 0,
    }
}

fn apply_manifest_cost(response: &mut ModelResponse, manifest: &ProviderManifest) {
    if response.usage.cost_micros != 0 {
        return;
    }
    let input_cost = response
        .usage
        .input_tokens
        .saturating_mul(manifest.input_cost_micros_per_k)
        .div_ceil(1_000);
    let output_cost = response
        .usage
        .output_tokens
        .saturating_mul(manifest.output_cost_micros_per_k)
        .div_ceil(1_000);
    response.usage.cost_micros = input_cost.saturating_add(output_cost);
}

pub fn remote_manifest(
    provider_id: impl Into<String>,
    model: impl Into<String>,
) -> ProviderManifest {
    ProviderManifest {
        provider_id: provider_id.into(),
        model: model.into(),
        privacy: PrivacyClass::Remote,
        supported_modes: vec![
            CognitiveMode::DirectAnswer,
            CognitiveMode::EvidenceResearch,
            CognitiveMode::DecisionAnalysis,
            CognitiveMode::Execute,
            CognitiveMode::Diagnosis,
            CognitiveMode::Escalate,
        ],
        supports_tools: true,
        supports_structured_output: true,
        max_risk: RiskLevel::High,
        quality_score: 100,
        input_cost_micros_per_k: 1,
        output_cost_micros_per_k: 1,
        p95_latency_ms: 1_000,
    }
}

pub fn risk_for_mode(mode: CognitiveMode) -> RiskLevel {
    match mode {
        CognitiveMode::Execute => RiskLevel::High,
        CognitiveMode::DecisionAnalysis | CognitiveMode::Diagnosis => RiskLevel::Medium,
        CognitiveMode::DirectAnswer | CognitiveMode::EvidenceResearch | CognitiveMode::Escalate => {
            RiskLevel::Low
        }
    }
}

fn stop_reason_for_answer(status: AnswerStatus) -> StopReason {
    match status {
        AnswerStatus::Completed | AnswerStatus::Conditional => StopReason::Completed,
        AnswerStatus::NeedsEvidence => StopReason::NeedsEvidence,
        AnswerStatus::NeedsAuthorization => StopReason::NeedsAuthorization,
        AnswerStatus::Failed => StopReason::InvalidOutput,
    }
}

fn provider_retry_backoff_ms(base_ms: u64, retry_number: usize) -> u64 {
    let exponent = retry_number.saturating_sub(1).min(5) as u32;
    base_ms
        .min(MAX_PROVIDER_RETRY_BACKOFF_MS)
        .saturating_mul(1_u64 << exponent)
        .min(MAX_PROVIDER_RETRY_BACKOFF_MS)
}

fn append_provider_failure(
    journal: &mut Journal,
    journal_path: &Path,
    provider_id: &str,
    model: &str,
    error: &ProviderError,
) -> Result<(), RuntimeError> {
    journal
        .append(JournalEventKind::ProviderFailed {
            failure: ProviderFailure::from_error(provider_id, model, error),
        })
        .map_err(|source| RuntimeError::Journal {
            path: journal_path.to_path_buf(),
            source,
        })?;
    Ok(())
}

fn authorization_answer() -> crate::domain::AgentAnswer {
    crate::domain::AgentAnswer {
        schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
        status: AnswerStatus::NeedsAuthorization,
        conclusion: "当前任务被识别为执行操作，但本次运行被设为只读；未执行任何外部动作。在 TUI 输入 /permissions write on 后重试，或去掉 --read-only 后重新运行。".into(),
        claims: Vec::new(),
        evidence: Vec::new(),
        assumptions: Vec::new(),
        constraints: vec!["allow_write=false".into()],
        unknowns: vec![UnknownItem {
            id: "authorization-scope".into(),
            description: "缺少明确的写入权限和审批范围".into(),
            impact: ImpactLevel::High,
            why_it_matters: Some("没有授权就无法安全执行副作用动作".into()),
            reduction_method: Some(
                "在 TUI 输入 /permissions write on，或去掉 --read-only，并限定具体动作、范围和回滚条件"
                    .into(),
            ),
        }],
        conflicts: Vec::new(),
        flip_conditions: vec![FlipCondition {
            if_observed: "用户授权具体低风险动作及其作用范围".into(),
            then: "可以进入受控执行流程".into(),
            target_ids: vec!["authorization-scope".into()],
        }],
        action_receipts: Vec::new(),
    }
}

fn budget_answer(reason: &str) -> crate::domain::AgentAnswer {
    crate::domain::AgentAnswer {
        schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
        status: AnswerStatus::NeedsEvidence,
        conclusion: format!("本轮未形成可验证结论：{reason}。"),
        claims: Vec::new(),
        evidence: Vec::new(),
        assumptions: Vec::new(),
        constraints: vec![reason.into()],
        unknowns: vec![UnknownItem {
            id: "budget-verification".into(),
            description: "任务尚未完成验证".into(),
            impact: ImpactLevel::High,
            why_it_matters: Some("预算停止时不能把部分工作报告为成功".into()),
            reduction_method: Some("提高预算或缩小任务范围后重新运行".into()),
        }],
        conflicts: Vec::new(),
        flip_conditions: vec![FlipCondition {
            if_observed: "在更高预算下完成外部验证".into(),
            then: "应重新评估是否形成结论".into(),
            target_ids: vec!["budget-verification".into()],
        }],
        action_receipts: Vec::new(),
    }
}

fn remaining_budget(
    started_at: Instant,
    timeout_ms: u64,
    initial_elapsed: Duration,
) -> Option<Duration> {
    Duration::from_millis(timeout_ms)
        .checked_sub(initial_elapsed.saturating_add(started_at.elapsed()))
        .filter(|remaining| !remaining.is_zero())
}

fn finish_with_answer(
    journal: &mut Journal,
    run_id: Uuid,
    journal_path: PathBuf,
    answer: crate::domain::AgentAnswer,
    stop_reason: StopReason,
) -> Result<RunResult, RuntimeError> {
    journal
        .append(JournalEventKind::AnswerProduced {
            answer: answer.clone(),
        })
        .map_err(|source| RuntimeError::Journal {
            path: journal_path.clone(),
            source,
        })?;
    journal
        .append(JournalEventKind::RunStopped {
            reason: stop_reason,
        })
        .map_err(|source| RuntimeError::Journal {
            path: journal_path.clone(),
            source,
        })?;
    Ok(RunResult {
        run_id,
        journal_path,
        answer,
        stop_reason,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("journal operation failed at {path}: {source}")]
    Journal {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("provider failed for run {path}: {source}")]
    Provider {
        path: PathBuf,
        #[source]
        source: ProviderError,
    },
    #[error("no provider route for mode {mode:?}; journal: {path}")]
    NoRoute { path: PathBuf, mode: CognitiveMode },
    #[error("invalid model output: {message}; journal: {path}")]
    InvalidOutput { path: PathBuf, message: String },
    #[error("run cancelled; journal: {path}")]
    Cancelled { path: PathBuf },
    #[error("run cannot be resumed: {0}")]
    Resume(String),
    #[error("route plan failed: {message}; journal: {path}")]
    RoutePlan { path: PathBuf, message: String },
    #[error("tool capability snapshot failed: {message}; journal: {path}")]
    ToolCapabilities { path: PathBuf, message: String },
    #[error("experience loading failed: {0}")]
    Experience(String),
    #[error("reframe failed: {0}")]
    Reframe(String),
}

impl RuntimeError {
    pub fn journal_path(&self) -> &Path {
        match self {
            Self::Journal { path, .. }
            | Self::Provider { path, .. }
            | Self::NoRoute { path, .. }
            | Self::InvalidOutput { path, .. }
            | Self::RoutePlan { path, .. }
            | Self::ToolCapabilities { path, .. }
            | Self::Cancelled { path } => path,
            Self::Resume(_) => Path::new(""),
            Self::Experience(_) => Path::new(""),
            Self::Reframe(_) => Path::new(""),
        }
    }
}

impl AgentRuntime {
    /// Ask the routed provider for a reframe proposal.
    ///
    /// No journal is written here: an undecided proposal is a fork, not a fact.
    /// The caller records the decision, and that entry then carries both.
    pub async fn reframe(
        &self,
        task: &str,
        constraints: &[String],
        cancellation: &CancellationToken,
    ) -> Result<crate::reframe::ReframeProposal, RuntimeError> {
        let candidate = match &self.selected_route {
            Some(key) => self.providers.iter().find(|candidate| {
                candidate.manifest.provider_id == key.provider_id
                    && candidate.manifest.model == key.model
            }),
            None => self.providers.first(),
        }
        .ok_or_else(|| RuntimeError::Reframe("no provider is configured".into()))?;

        let text = candidate
            .provider
            .complete_text(
                crate::reframe::REFRAME_SYSTEM_PROMPT,
                &crate::reframe::build_user_message(task, constraints),
                cancellation,
            )
            .await
            .map_err(|error| RuntimeError::Reframe(error.to_string()))?;

        let proposal = crate::reframe::parse_model_output(&text)
            .and_then(|parsed| parsed.into_proposal(task))
            .map_err(|error| RuntimeError::Reframe(error.to_string()))?;
        proposal
            .validate()
            .map_err(|error| RuntimeError::Reframe(error.to_string()))?;
        Ok(proposal)
    }
}

fn route_plan_digest(
    plan: &crate::router::RoutePlan,
    sources: &RoutePolicySources,
) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(plan, sources)).map_err(|error| error.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn experience_kind_label(kind: crate::experience::ExperienceKind) -> &'static str {
    use crate::experience::ExperienceKind;
    match kind {
        ExperienceKind::Memory => "memory",
        ExperienceKind::Skill => "skill",
        ExperienceKind::RoutePolicy => "route-policy",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::{AuthorityProfile, AuthorityRegistry};
    use crate::domain::{
        AgentAnswer, AnswerStatus, PermissionSet, RunBudget, ToolCall, ToolStatus, Usage,
    };
    use crate::execution_gate::ExecutionGate;
    use crate::experience::{
        ExperienceCandidateSpec, ExperienceKind, ExperienceStore, ExperienceVerificationInput,
    };
    use crate::journal::{audit_events, read_events, replay_events, summarize_route_trace};
    use crate::provider::{DeterministicProvider, ModelProvider, ProviderError, ProviderErrorCode};
    use crate::tools::{EchoTool, ToolRegistry, default_registry};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ReframeStub;

    #[async_trait]
    impl ModelProvider for ReframeStub {
        fn id(&self) -> &str {
            "reframe-stub"
        }
        fn model(&self) -> &str {
            "stub-v1"
        }
        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            Err(ProviderError {
                code: ProviderErrorCode::InvalidResponse,
                message: "not used by reframe".into(),
                retryable: false,
            })
        }
        async fn complete_text(
            &self,
            _system: &str,
            user: &str,
            _cancellation: &CancellationToken,
        ) -> Result<String, ProviderError> {
            assert!(user.contains("电钻"), "user message must carry the request");
            Ok(r#"{"classification":"means","question":"穿线还是挂东西？","options":["穿线","挂东西"],"substitutes":["路由器"],"confidence":0.8}"#.into())
        }
    }

    fn reframe_runtime(
        workspace: &std::path::Path,
        provider: Arc<dyn ModelProvider>,
    ) -> AgentRuntime {
        let mut manifest = local_manifest();
        manifest.provider_id = provider.id().into();
        manifest.model = provider.model().into();
        AgentRuntime::new(
            vec![ProviderCandidate { provider, manifest }],
            default_registry(),
            workspace.join(".journal"),
        )
    }

    #[tokio::test]
    async fn reframe_returns_a_validated_proposal_from_the_provider() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = reframe_runtime(workspace.path(), Arc::new(ReframeStub));
        let proposal = runtime
            .reframe("我要买个电钻", &[], &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(proposal.classification, crate::reframe::RequestRole::Means);
        assert_eq!(
            proposal.stop_rule_source,
            crate::reframe::StopRuleSource::Derived
        );
        assert_eq!(proposal.stated_request, "我要买个电钻");
        assert_eq!(proposal.validate(), Ok(()));
    }

    struct NoRawTextProvider;

    #[async_trait]
    impl ModelProvider for NoRawTextProvider {
        fn id(&self) -> &str {
            "no-raw-text"
        }
        fn model(&self) -> &str {
            "v1"
        }
        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            Err(ProviderError {
                code: ProviderErrorCode::InvalidResponse,
                message: "not used".into(),
                retryable: false,
            })
        }
    }

    #[tokio::test]
    async fn reframe_reports_a_provider_without_raw_text_support() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = reframe_runtime(workspace.path(), Arc::new(NoRawTextProvider));
        let error = runtime
            .reframe("x", &[], &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(error, RuntimeError::Reframe(_)), "got {error:?}");
    }

    struct RetryableFailureProvider;

    fn gated_runtime(dir: &std::path::Path, allowed_tool: &str) -> AgentRuntime {
        let mut authority = AuthorityRegistry::default();
        authority
            .register(AuthorityProfile {
                profile_id: "tool-profile".into(),
                actor_id: "owner-1".into(),
                scopes: vec!["repo*".into()],
                max_strength: crate::task_graph::MandateStrength::ReadOnly,
                allowed_tools: vec![allowed_tool.into()],
                denied_tools: Vec::new(),
                max_risk: RiskLevel::Low,
                expires_ms: None,
            })
            .unwrap();
        authority.bind("owner-1", "tool-profile").unwrap();
        let mut registry = default_registry();
        registry.register(std::sync::Arc::new(EchoTool));
        let gate = ExecutionGate::new(authority, &registry);
        AgentRuntime::new(Vec::new(), registry, dir.join(".journal")).with_execution_gate(
            gate,
            "owner-1",
            "repo:1-agent",
            crate::task_graph::MandateStrength::ReadOnly,
            10,
        )
    }

    #[tokio::test]
    async fn execution_gate_denies_tools_before_they_run() {
        let dir = tempfile::tempdir().unwrap();
        let denied = gated_runtime(dir.path(), "read_file")
            .execute_tool(
                &ToolCall {
                    id: "call-1".into(),
                    name: "echo".into(),
                    arguments: serde_json::json!({"text": "hi"}),
                },
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await;
        assert_eq!(denied.status, ToolStatus::PermissionDenied);
        assert_eq!(denied.error_code.as_deref(), Some("authority_denied"));

        let allowed = gated_runtime(dir.path(), "echo")
            .execute_tool(
                &ToolCall {
                    id: "call-2".into(),
                    name: "echo".into(),
                    arguments: serde_json::json!({"text": "hi"}),
                },
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await;
        assert_eq!(allowed.status, ToolStatus::Ok, "{allowed:?}");
    }

    #[test]
    fn manifest_pricing_is_applied_when_provider_cost_is_unknown() {
        let mut manifest = local_manifest();
        manifest.input_cost_micros_per_k = 1_000;
        manifest.output_cost_micros_per_k = 2_000;
        let mut response = ModelResponse {
            provider_id: manifest.provider_id.clone(),
            model: manifest.model.clone(),
            turn: ModelTurn::ToolCalls { calls: Vec::new() },
            usage: Usage {
                input_tokens: 1_500,
                output_tokens: 500,
                cost_micros: 0,
                cache_hit_tokens: 0,
                cache_miss_tokens: 0,
            },
        };
        apply_manifest_cost(&mut response, &manifest);
        assert_eq!(response.usage.cost_micros, 2_500);
        response.usage.cost_micros = 7;
        apply_manifest_cost(&mut response, &manifest);
        assert_eq!(response.usage.cost_micros, 7);
    }

    #[async_trait]
    impl ModelProvider for RetryableFailureProvider {
        fn id(&self) -> &str {
            "failing"
        }

        fn model(&self) -> &str {
            "fail-v1"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<crate::domain::ModelResponse, ProviderError> {
            Err(ProviderError {
                code: ProviderErrorCode::ProviderUnavailable,
                message: "injected failure".into(),
                retryable: true,
            })
        }
    }

    struct NonRetryableFailureProvider {
        calls: AtomicUsize,
    }

    struct RepairingProvider {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ModelProvider for RepairingProvider {
        fn id(&self) -> &str {
            "repairing"
        }

        fn model(&self) -> &str {
            "repair-v1"
        }

        async fn complete(
            &self,
            request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<crate::domain::ModelResponse, ProviderError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            if call == 1 {
                assert!(
                    request
                        .messages
                        .last()
                        .is_some_and(|message| message.content.contains("failed validation"))
                );
            }
            let evidence_source = if call == 0 { "wrong.source" } else { "task" };
            let answer: AgentAnswer = serde_json::from_value(serde_json::json!({
                "schema_version": 2,
                "status": "completed",
                "conclusion": "repaired",
                "claims": [{
                    "id": "c1",
                    "text": "read-only",
                    "status": "verified",
                    "sources": ["task"],
                    "scope": "test"
                }],
                "evidence": [{
                    "source": evidence_source,
                    "statement": "read-only",
                    "observed_unix_ms": null,
                    "expires_unix_ms": null,
                    "claim_ids": ["c1"]
                }],
                "assumptions": [],
                "constraints": [],
                "unknowns": [],
                "conflicts": [],
                "flip_conditions": [],
                "action_receipts": []
            }))
            .unwrap();
            Ok(crate::domain::ModelResponse {
                provider_id: "repairing".into(),
                model: "repair-v1".into(),
                turn: ModelTurn::Final { answer },
                usage: Usage::default(),
            })
        }
    }

    struct ParsingRepairProvider {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl ModelProvider for ParsingRepairProvider {
        fn id(&self) -> &str {
            "parsing-repair"
        }

        fn model(&self) -> &str {
            "parsing-repair-v1"
        }

        async fn complete(
            &self,
            request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<crate::domain::ModelResponse, ProviderError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let turn = if call == 0 {
                ModelTurn::InvalidFinal {
                    content: "c\n#include <stdio.h>\nint main(void) { return 0; }".into(),
                    error: "provider answer is not valid AgentAnswer JSON: expected value".into(),
                }
            } else {
                assert!(
                    request
                        .messages
                        .last()
                        .is_some_and(|message| message.content.contains("could not be parsed"))
                );
                assert!(
                    request
                        .messages
                        .iter()
                        .any(|message| message.content.contains("#include <stdio.h>"))
                );
                ModelTurn::Final {
                    answer: AgentAnswer {
                        schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                        status: AnswerStatus::Completed,
                        conclusion:
                            "C uses #include and main; Python, Java and Rust use their own entry syntax."
                                .into(),
                        claims: Vec::new(),
                        evidence: Vec::new(),
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        conflicts: Vec::new(),
                        flip_conditions: Vec::new(),
                        action_receipts: Vec::new(),
                    },
                }
            };
            Ok(crate::domain::ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn,
                usage: Usage::default(),
            })
        }
    }

    #[async_trait]
    impl ModelProvider for NonRetryableFailureProvider {
        fn id(&self) -> &str {
            "unauthorized"
        }

        fn model(&self) -> &str {
            "unauthorized-v1"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<crate::domain::ModelResponse, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(ProviderError {
                code: ProviderErrorCode::Authentication,
                message: format!("auth\u{202e}{}", "x".repeat(1_000)),
                retryable: false,
            })
        }
    }

    #[tokio::test]
    async fn invalid_final_answer_is_repaired_once_and_revalidated() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_dir = workspace.path().join(".journal");
        let mut manifest = local_manifest();
        manifest.provider_id = "repairing".into();
        manifest.model = "repair-v1".into();
        let provider = Arc::new(RepairingProvider {
            calls: AtomicUsize::new(0),
        });
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: provider.clone(),
                manifest,
            }],
            default_registry(),
            &journal_dir,
        );
        let request =
            TaskRequest::from_objective("repair invalid output", workspace.path().to_path_buf());
        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.answer.conclusion, "repaired");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        let audit = audit_events(&read_events(&result.journal_path).unwrap());
        assert!(audit.ok, "{:?}", audit.issues);
    }

    #[tokio::test]
    async fn unparseable_provider_text_is_preserved_and_repaired() {
        let workspace = tempfile::tempdir().unwrap();
        let journal_dir = workspace.path().join(".journal");
        let mut manifest = local_manifest();
        manifest.provider_id = "parsing-repair".into();
        manifest.model = "parsing-repair-v1".into();
        let provider = Arc::new(ParsingRepairProvider {
            calls: AtomicUsize::new(0),
        });
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: provider.clone(),
                manifest,
            }],
            default_registry(),
            &journal_dir,
        );
        let request = TaskRequest::from_objective(
            "使用 C、Python、Java、Rust 输出 Hello! World.",
            workspace.path().to_path_buf(),
        );
        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        assert!(result.answer.conclusion.contains("C uses"));
        let events = read_events(&result.journal_path).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, JournalEventKind::ModelRequested { .. }))
                .count(),
            2
        );
        assert!(events.iter().any(|event| matches!(
            &event.event,
            JournalEventKind::ModelResponded {
                response: crate::domain::ModelResponse {
                    turn: ModelTurn::InvalidFinal { content, .. },
                    ..
                }
            } if content.contains("#include <stdio.h>")
        )));
        assert!(audit_events(&events).ok);
    }

    #[tokio::test]
    async fn deterministic_runtime_reads_evidence_and_writes_auditable_journal() {
        let workspace = tempfile::tempdir().unwrap();
        let evidence = workspace.path().join("evidence.txt");
        std::fs::write(
            &evidence,
            "A is preferred because its verified cost is lower.",
        )
        .unwrap();
        let journal_dir = workspace.path().join(".journal");
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }],
            default_registry(),
            &journal_dir,
        );
        let request = TaskRequest {
            objective: format!("Compare A and B using @read-file:{}", evidence.display()),
            deliverable: Some("decision".into()),
            known_facts: Vec::new(),
            constraints: vec!["read only".into()],
            unknowns: Vec::new(),
            decision_criteria: vec!["verified cost".into()],
            success_criteria: vec!["cite evidence".into()],
            evidence_max_age_ms: None,
            experience_scope: None,
            permissions: PermissionSet {
                read_roots: vec![workspace.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            budget: RunBudget::default(),
        };
        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.stop_reason, StopReason::Completed);
        assert_eq!(result.answer.evidence.len(), 1);
        let events = read_events(&result.journal_path).unwrap();
        let audit = audit_events(&events);
        assert!(audit.ok, "{:?}", audit.issues);
        let planned = events.iter().find_map(|event| match &event.event {
            JournalEventKind::RoutePlanned { plan, sources } => Some((plan, sources)),
            _ => None,
        });
        let (plan, sources) = planned.expect("route plan should be journaled");
        assert_eq!(plan.ranked.len(), 1);
        assert_eq!(sources.objective, PolicyValueSource::RuntimeDefault);
        let replay = replay_events(&events).unwrap();
        assert_eq!(replay.route_plan.as_ref(), Some(plan));
        assert_eq!(replay.route_policy_sources.as_ref(), Some(sources));
        let capabilities = replay
            .tool_capabilities
            .as_ref()
            .expect("tool capabilities should be journaled");
        for tool in ["read_file", "search_files", "fetch_url"] {
            assert!(
                capabilities.tools.iter().any(|name| name == tool),
                "missing {tool}"
            );
        }
        assert!(
            !capabilities.tools.iter().any(|name| name == "write_file"),
            "write tools must be hidden in a read-only run"
        );
        let summary = summarize_route_trace(&events).unwrap();
        assert_eq!(summary.candidates, 1);
        assert_eq!(summary.rejected, 0);
        assert_eq!(summary.fallback_attempts, 1);
        assert_eq!(
            summary.selected_provider_id.as_deref(),
            Some("deterministic")
        );
        let mut bounded = summary;
        bounded.selection_reason = Some("x".repeat(300));
        let rendered = bounded.render();
        assert!(rendered.contains('…'));
        assert!(!rendered.contains(&"x".repeat(241)));
    }

    #[tokio::test]
    async fn execute_mode_requires_authorization_without_calling_provider() {
        let workspace = tempfile::tempdir().unwrap();
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }],
            default_registry(),
            workspace.path().join(".journal"),
        );
        let request =
            TaskRequest::from_objective("Delete the stale files", workspace.path().into());
        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.stop_reason, StopReason::NeedsAuthorization);
        assert!(result.answer.action_receipts.is_empty());
    }

    #[tokio::test]
    async fn provider_failure_records_health_alert_and_falls_back() {
        let workspace = tempfile::tempdir().unwrap();
        let evidence = workspace.path().join("evidence.txt");
        std::fs::write(&evidence, "verified evidence").unwrap();
        let mut failing_manifest = local_manifest();
        failing_manifest.provider_id = "failing".into();
        failing_manifest.model = "fail-v1".into();
        failing_manifest.quality_score = 100;
        let health = std::sync::Arc::new(std::sync::Mutex::new(
            crate::provider_health::ProviderHealth::new(
                crate::provider_health::ProviderHealthPolicy {
                    failure_threshold: 1,
                    cooldown_ms: 60_000,
                },
            )
            .unwrap(),
        ));
        let alert_path = workspace.path().join("alerts.jsonl");
        let alerts = std::sync::Arc::new(std::sync::Mutex::new(
            crate::alerts::AlertLog::open(&alert_path).unwrap(),
        ));
        let runtime = AgentRuntime::new(
            vec![
                ProviderCandidate {
                    provider: Arc::new(RetryableFailureProvider),
                    manifest: failing_manifest,
                },
                ProviderCandidate {
                    provider: Arc::new(DeterministicProvider::new()),
                    manifest: local_manifest(),
                },
            ],
            default_registry(),
            workspace.path().join(".journal"),
        )
        .with_provider_health(health.clone())
        .with_alert_log(alerts);
        let mut request = TaskRequest::from_objective(
            format!(
                "Compare options using @read-file:\"{}\"",
                evidence.display()
            ),
            workspace.path().into(),
        );
        request.budget.provider_retry_backoff_ms = 0;
        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.stop_reason, StopReason::Completed);
        let alerts = crate::alerts::AlertLog::open(&alert_path).unwrap();
        let records = alerts.read().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, "provider_health");
        let health = health.lock().unwrap();
        assert!(
            !health.available("failing", "fail-v1", 1),
            "failing provider should be demoted"
        );
    }

    #[tokio::test]
    async fn retryable_provider_failure_falls_back_and_is_journaled() {
        let workspace = tempfile::tempdir().unwrap();
        let evidence = workspace.path().join("evidence.txt");
        std::fs::write(&evidence, "verified evidence").unwrap();
        let mut failing_manifest = local_manifest();
        failing_manifest.provider_id = "failing".into();
        failing_manifest.model = "fail-v1".into();
        failing_manifest.quality_score = 100;
        let runtime = AgentRuntime::new(
            vec![
                ProviderCandidate {
                    provider: Arc::new(RetryableFailureProvider),
                    manifest: failing_manifest,
                },
                ProviderCandidate {
                    provider: Arc::new(DeterministicProvider::new()),
                    manifest: local_manifest(),
                },
            ],
            default_registry(),
            workspace.path().join(".journal"),
        );
        let mut request = TaskRequest {
            objective: format!(
                "Compare options using @read-file:\"{}\"",
                evidence.display()
            ),
            deliverable: None,
            known_facts: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            decision_criteria: Vec::new(),
            success_criteria: Vec::new(),
            evidence_max_age_ms: None,
            experience_scope: None,
            permissions: PermissionSet {
                read_roots: vec![workspace.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            budget: RunBudget::default(),
        };
        request.budget.provider_retry_backoff_ms = 0;
        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.stop_reason, StopReason::Completed);
        let events = read_events(&result.journal_path).unwrap();
        assert!(audit_events(&events).ok);
        let failing_attempts = events
            .iter()
            .filter(|event| {
                matches!(
                    &event.event,
                    JournalEventKind::RouteSelected { provider_id, .. }
                        if provider_id == "failing"
                )
            })
            .count();
        let deterministic_attempts = events
            .iter()
            .filter(|event| {
                matches!(
                    &event.event,
                    JournalEventKind::RouteSelected { provider_id, .. }
                        if provider_id == "deterministic"
                )
            })
            .count();
        assert_eq!(failing_attempts, 1);
        assert_eq!(deterministic_attempts, 2);
        let summary = summarize_route_trace(&events).unwrap();
        assert_eq!(summary.fallback_attempts, 2);
        assert_eq!(summary.provider_failures, 1);
        assert_eq!(
            summary.failure_codes,
            vec![crate::provider::ProviderErrorCode::ProviderUnavailable]
        );
        assert_eq!(summary.selected_provider_id.as_deref(), Some("failing"));
        let failures = events
            .iter()
            .filter_map(|event| match &event.event {
                JournalEventKind::ProviderFailed { failure } => Some(failure),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].provider_id, "failing");
        assert_eq!(
            failures[0].code,
            crate::provider::ProviderErrorCode::ProviderUnavailable
        );
        assert!(failures[0].retryable);
        let replay = replay_events(&events).unwrap();
        assert_eq!(replay.provider_failures.len(), 1);
        assert_eq!(replay.retry_delays_ms, vec![0]);
        assert_eq!(replay.retry_timings.len(), 1);
        assert_eq!(replay.retry_timings[0].delay_ms, 0);
        assert_eq!(replay.retry_timings[0].outcome, RetryOutcome::Completed);
        assert!(events.iter().any(|event| matches!(
            &event.event,
            JournalEventKind::ProviderRetryScheduled { delay_ms: 0, .. }
        )));
    }

    struct CountingToolProvider {
        calls: AtomicUsize,
        tool_steps: usize,
    }

    #[async_trait]
    impl ModelProvider for CountingToolProvider {
        fn id(&self) -> &str {
            "counting-tool"
        }

        fn model(&self) -> &str {
            "counting-tool-v1"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let turn = if call < self.tool_steps {
                ModelTurn::ToolCalls {
                    calls: vec![ToolCall {
                        id: format!("call-{call}"),
                        name: "echo".into(),
                        arguments: serde_json::json!({"text": "tick"}),
                    }],
                }
            } else {
                ModelTurn::Final {
                    answer: AgentAnswer {
                        schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
                        status: AnswerStatus::Completed,
                        conclusion: "长任务完成".into(),
                        claims: Vec::new(),
                        evidence: Vec::new(),
                        assumptions: Vec::new(),
                        constraints: Vec::new(),
                        unknowns: Vec::new(),
                        conflicts: Vec::new(),
                        flip_conditions: Vec::new(),
                        action_receipts: Vec::new(),
                    },
                }
            };
            Ok(ModelResponse {
                provider_id: self.id().into(),
                model: self.model().into(),
                turn,
                usage: Usage::default(),
            })
        }
    }

    #[tokio::test]
    async fn zero_budgets_do_not_stop_a_long_run() {
        let workspace = tempfile::tempdir().unwrap();
        let mut manifest = local_manifest();
        manifest.provider_id = "counting-tool".into();
        manifest.model = "counting-tool-v1".into();
        let mut tools = ToolRegistry::default();
        tools.register(Arc::new(EchoTool));
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(CountingToolProvider {
                    calls: AtomicUsize::new(0),
                    tool_steps: 17,
                }),
                manifest,
            }],
            tools,
            workspace.path().join(".journal"),
        );
        let mut request = TaskRequest::from_objective("keep working", workspace.path().into());
        request.budget = RunBudget {
            max_steps: 0,
            max_tool_calls: 0,
            max_provider_attempts: 0,
            timeout_ms: 0,
            ..RunBudget::default()
        };

        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.stop_reason, StopReason::Completed);
        assert_eq!(result.answer.conclusion, "长任务完成");
    }

    #[tokio::test]
    async fn provider_attempt_budget_stops_further_fallback() {
        let workspace = tempfile::tempdir().unwrap();
        let mut failing_manifest = local_manifest();
        failing_manifest.provider_id = "failing".into();
        failing_manifest.model = "fail-v1".into();
        failing_manifest.quality_score = 100;
        let runtime = AgentRuntime::new(
            vec![
                ProviderCandidate {
                    provider: Arc::new(RetryableFailureProvider),
                    manifest: failing_manifest,
                },
                ProviderCandidate {
                    provider: Arc::new(DeterministicProvider::new()),
                    manifest: local_manifest(),
                },
            ],
            default_registry(),
            workspace.path().join(".journal"),
        );
        let mut request = TaskRequest::from_objective("Compare A and B", workspace.path().into());
        request.budget.max_provider_attempts = 1;

        let result = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.stop_reason, StopReason::BudgetExhausted);
        assert_eq!(result.answer.status, AnswerStatus::NeedsEvidence);
        assert!(result.answer.conclusion.contains("provider attempt budget"));
        let events = read_events(&result.journal_path).unwrap();
        let selected = events
            .iter()
            .filter(|event| matches!(&event.event, JournalEventKind::RouteSelected { .. }))
            .count();
        assert_eq!(selected, 1);
        let summary = summarize_route_trace(&events).unwrap();
        assert_eq!(summary.provider_failures, 1);
        assert_eq!(
            summary.failure_codes,
            vec![crate::provider::ProviderErrorCode::ProviderUnavailable]
        );
        let audit = audit_events(&events);
        assert!(audit.ok, "{:?}", audit.issues);
    }

    #[test]
    fn explicit_provider_selection_tracks_manifest_and_tools() {
        let manifest = local_manifest();
        let mut runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: manifest.clone(),
            }],
            default_registry(),
            std::env::temp_dir(),
        );
        assert!(runtime.select_provider(&manifest.provider_id, &manifest.model));
        assert_eq!(
            runtime
                .selected_provider()
                .map(|value| value.model.as_str()),
            Some(manifest.model.as_str())
        );
        assert!(
            runtime
                .tool_capabilities()
                .iter()
                .any(|tool| tool.name == "read_file")
        );
        runtime.set_route_objective(RouteObjective::Balanced);
        assert_eq!(runtime.route_objective(), RouteObjective::Balanced);
    }

    #[tokio::test]
    async fn non_retryable_provider_failure_does_not_fall_back() {
        let workspace = tempfile::tempdir().unwrap();
        let failing = Arc::new(NonRetryableFailureProvider {
            calls: AtomicUsize::new(0),
        });
        let mut unauthorized_manifest = local_manifest();
        unauthorized_manifest.provider_id = "unauthorized".into();
        unauthorized_manifest.model = "unauthorized-v1".into();
        unauthorized_manifest.quality_score = 100;
        let runtime = AgentRuntime::new(
            vec![
                ProviderCandidate {
                    provider: failing.clone(),
                    manifest: unauthorized_manifest,
                },
                ProviderCandidate {
                    provider: Arc::new(DeterministicProvider::new()),
                    manifest: local_manifest(),
                },
            ],
            default_registry(),
            workspace.path().join(".journal"),
        );
        let request = TaskRequest::from_objective("Compare A and B", workspace.path().into());

        let error = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap_err();
        let path = match error {
            RuntimeError::Provider { path, source } => {
                assert_eq!(source.code, ProviderErrorCode::Authentication);
                assert!(!source.retryable);
                path
            }
            other => panic!("expected provider error, got {other:?}"),
        };
        assert_eq!(failing.calls.load(Ordering::SeqCst), 1);
        let events = read_events(path).unwrap();
        let failures = events
            .iter()
            .filter_map(|event| match &event.event {
                JournalEventKind::ProviderFailed { failure } => Some(failure),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].provider_id, "unauthorized");
        assert_eq!(failures[0].code, ProviderErrorCode::Authentication);
        assert!(!failures[0].retryable);
        assert!(failures[0].message.chars().count() <= 512);
        assert!(!crate::safety::contains_disallowed_control(
            &failures[0].message
        ));
    }

    #[tokio::test]
    async fn retry_backoff_is_bounded_deterministic_and_cancellable() {
        assert_eq!(provider_retry_backoff_ms(0, 1), 0);
        assert_eq!(provider_retry_backoff_ms(50, 1), 50);
        assert_eq!(provider_retry_backoff_ms(50, 2), 100);
        assert_eq!(provider_retry_backoff_ms(50, 20), 1_000);
        assert_eq!(provider_retry_backoff_ms(5_000, 1), 1_000);

        let workspace = tempfile::tempdir().unwrap();
        let mut failing_manifest = local_manifest();
        failing_manifest.provider_id = "failing".into();
        failing_manifest.model = "fail-v1".into();
        failing_manifest.quality_score = 100;
        let runtime = AgentRuntime::new(
            vec![
                ProviderCandidate {
                    provider: Arc::new(RetryableFailureProvider),
                    manifest: failing_manifest,
                },
                ProviderCandidate {
                    provider: Arc::new(DeterministicProvider::new()),
                    manifest: local_manifest(),
                },
            ],
            default_registry(),
            workspace.path().join(".journal"),
        );
        let mut request = TaskRequest::from_objective("Compare A and B", workspace.path().into());
        request.budget.provider_retry_backoff_ms = 1_000;
        let cancellation = CancellationToken::new();
        let signal = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            signal.cancel();
        });

        let error = runtime
            .run(request, cancellation)
            .await
            .expect_err("backoff should be cancelled");
        let path = match error {
            RuntimeError::Cancelled { path } => path,
            other => panic!("expected cancellation, got {other:?}"),
        };
        let events = read_events(path).unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.event,
            JournalEventKind::ProviderRetryScheduled { delay_ms: 1000, .. }
        )));
        assert_eq!(summarize_route_trace(&events).unwrap().provider_failures, 1);
        let timings = replay_events(&events).unwrap().retry_timings;
        assert_eq!(timings.len(), 1);
        assert_eq!(timings[0].outcome, RetryOutcome::Cancelled);
        assert!(timings[0].elapsed_ms <= crate::domain::MAX_PROVIDER_RETRY_ELAPSED_MS);
        assert!(audit_events(&events).ok);
    }

    #[tokio::test]
    async fn interrupted_run_resumes_from_checkpoint_with_same_observable_answer() {
        let workspace = tempfile::tempdir().unwrap();
        let evidence = workspace.path().join("evidence.txt");
        std::fs::write(&evidence, "verified checkpoint evidence").unwrap();
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }],
            default_registry(),
            workspace.path().join(".journal"),
        );
        let request = TaskRequest {
            objective: format!("Research @read-file:\"{}\"", evidence.display()),
            deliverable: None,
            known_facts: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            decision_criteria: Vec::new(),
            success_criteria: Vec::new(),
            evidence_max_age_ms: None,
            experience_scope: None,
            permissions: PermissionSet {
                read_roots: vec![workspace.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            budget: RunBudget::default(),
        };
        let complete = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        let events = read_events(&complete.journal_path).unwrap();
        let checkpoint_index = events
            .iter()
            .position(|event| matches!(event.event, JournalEventKind::Checkpoint { .. }))
            .unwrap();
        let checkpoint = events
            .iter()
            .find_map(|event| match &event.event {
                JournalEventKind::Checkpoint { checkpoint } => Some(checkpoint),
                _ => None,
            })
            .unwrap();
        assert!(checkpoint.route_plan_sha256.is_some());
        assert!(checkpoint.route_policy_sources.is_some());
        assert!(checkpoint.tool_capabilities_sha256.is_some());
        assert_eq!(checkpoint.provider_attempts_used, 1);
        let interrupted_path = workspace.path().join("interrupted.jsonl");
        let mut interrupted = Journal::create(&interrupted_path, Uuid::new_v4()).unwrap();
        for event in &events[..=checkpoint_index] {
            interrupted.append(event.event.clone()).unwrap();
        }
        drop(interrupted);

        let changed_policy = runtime.clone().with_route_objective(RouteObjective::Cost);
        assert!(matches!(
            changed_policy
                .resume(&interrupted_path, CancellationToken::new())
                .await,
            Err(RuntimeError::Resume(message))
                if message.contains("route objective changed")
                    || message.contains("route plan or policy changed")
        ));
        let mut changed_tools = default_registry();
        changed_tools.register(Arc::new(EchoTool));
        let changed_capabilities = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }],
            changed_tools,
            workspace.path().join(".changed-journal"),
        );
        assert!(matches!(
            changed_capabilities
                .resume(&interrupted_path, CancellationToken::new())
                .await,
            Err(RuntimeError::Resume(message))
                if message.contains("tool capabilities changed")
        ));
        let resumed = runtime
            .resume(&interrupted_path, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resumed.stop_reason, complete.stop_reason);
        assert_eq!(resumed.answer.status, complete.answer.status);
        assert_eq!(resumed.answer.conclusion, complete.answer.conclusion);
        assert_eq!(resumed.answer.evidence, complete.answer.evidence);
        let resumed_events = read_events(&resumed.journal_path).unwrap();
        assert!(audit_events(&resumed_events).ok);
        assert!(matches!(
            resumed_events.first().map(|event| &event.event),
            Some(JournalEventKind::ResumeStarted { .. })
        ));
    }

    #[tokio::test]
    async fn active_experience_is_injected_but_unverified_drafts_are_not() {
        let workspace = tempfile::tempdir().unwrap();
        let evidence = workspace.path().join("evidence.txt");
        std::fs::write(&evidence, "verified evidence").unwrap();
        let source_runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }],
            default_registry(),
            workspace.path().join(".source-journal"),
        );
        let source_request = TaskRequest {
            objective: format!(
                "Compare options using @read-file:\"{}\"",
                evidence.display()
            ),
            deliverable: None,
            known_facts: Vec::new(),
            constraints: Vec::new(),
            unknowns: Vec::new(),
            decision_criteria: vec!["verified cost".into()],
            success_criteria: Vec::new(),
            evidence_max_age_ms: None,
            experience_scope: Some("repository decisions".into()),
            permissions: PermissionSet {
                read_roots: vec![workspace.path().to_path_buf()],
                allow_network: false,
                allow_write: false,
            },
            budget: RunBudget::default(),
        };
        let source = source_runtime
            .run(source_request.clone(), CancellationToken::new())
            .await
            .unwrap();
        let source_events = read_events(&source.journal_path).unwrap();
        let evidence_seq = source_events
            .iter()
            .find(|event| matches!(event.event, JournalEventKind::ToolObserved { .. }))
            .unwrap()
            .seq;
        let experience_dir = workspace.path().join(".experience");
        let mut store = ExperienceStore::open(&experience_dir).unwrap();
        let active = store
            .propose(
                ExperienceCandidateSpec {
                    kind: ExperienceKind::Skill,
                    title: "Inspect evidence first".into(),
                    content: "Read the cited source before making a repository decision.".into(),
                    scope: "repository decisions".into(),
                    evidence_event_seqs: vec![evidence_seq],
                    sensitive: false,
                    applicable_modes: vec![CognitiveMode::DecisionAnalysis],
                    expires_at_unix_ms: None,
                    supersedes: None,
                },
                &source_events,
            )
            .unwrap();
        store
            .review_with_source(active.id, "reviewer", Some(&source_events))
            .unwrap();
        store
            .verify(
                active.id,
                ExperienceVerificationInput {
                    suite_id: "heldout-runtime-v1".into(),
                    suite_version: "1".into(),
                    case_ids: vec!["runtime-case-1".into()],
                    suite_sha256: "b".repeat(64),
                    training_group: "A".into(),
                    evaluation_group: "B".into(),
                    baseline_success_bps: 5_000,
                    learned_success_bps: 6_000,
                    baseline_unsupported_bps: 1_000,
                    learned_unsupported_bps: 900,
                    cost_delta_bps: 100,
                    evidence: "runtime injection report".into(),
                },
            )
            .unwrap();
        store
            .approve_with_source(active.id, "owner", Some(&source_events))
            .unwrap();
        store
            .activate_with_source(active.id, Some(&source_events))
            .unwrap();
        store
            .propose(
                ExperienceCandidateSpec {
                    kind: ExperienceKind::Memory,
                    title: "Draft should not be used".into(),
                    content: "This draft must not be injected.".into(),
                    scope: "repository decisions".into(),
                    evidence_event_seqs: vec![evidence_seq],
                    sensitive: false,
                    applicable_modes: vec![CognitiveMode::DecisionAnalysis],
                    expires_at_unix_ms: None,
                    supersedes: None,
                },
                &source_events,
            )
            .unwrap();

        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(DeterministicProvider::new()),
                manifest: local_manifest(),
            }],
            default_registry(),
            workspace.path().join(".journal"),
        )
        .with_experience_dir(&experience_dir);
        let result = runtime
            .run(source_request, CancellationToken::new())
            .await
            .unwrap();
        let events = read_events(&result.journal_path).unwrap();
        let injection = events
            .iter()
            .find_map(|event| match &event.event {
                JournalEventKind::ExperienceInjected { injected } => Some(injected),
                _ => None,
            })
            .unwrap();
        assert_eq!(injection.len(), 1);
        assert_eq!(injection[0].id, active.id);
        let first_model_request = events
            .iter()
            .find_map(|event| match &event.event {
                JournalEventKind::ModelRequested { request } => Some(request),
                _ => None,
            })
            .unwrap();
        let model_context = first_model_request
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(model_context.contains("EXPERIENCE GUIDANCE"));
        assert!(model_context.contains("Inspect evidence first"));
        assert!(!model_context.contains("Draft should not be used"));
    }

    #[test]
    fn authorization_answer_names_the_write_permission_switch() {
        let answer = authorization_answer();
        assert_eq!(answer.status, AnswerStatus::NeedsAuthorization);
        assert!(answer.conclusion.contains("/permissions write on"));
        assert!(answer.conclusion.contains("--read-only"));
    }
}
