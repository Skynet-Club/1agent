use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::context::compact_messages;
use crate::domain::{
    AnswerStatus, CognitiveMode, FlipCondition, ImpactLevel, InjectedExperience, ModelMessage,
    ModelRequest, ModelTurn, ProviderRouteKey, RiskLevel, RunResult, RuntimeCheckpoint, StopReason,
    TaskRequest, UnknownItem,
};
use crate::experience::ExperienceStore;
use crate::journal::{
    Journal, JournalEventKind, ResumablePrefix, find_resumable_prefix, read_events,
};
use crate::provider::{ModelProvider, ProviderError, agent_system_prompt};
use crate::router::{
    PolicyValueSource, PrivacyClass, ProviderManifest, RouteObjective, RoutePolicySources,
    RouteRequest, Router,
};
use crate::tools::ToolRegistry;

#[derive(Clone)]
pub struct ProviderCandidate {
    pub provider: Arc<dyn ModelProvider>,
    pub manifest: ProviderManifest,
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
        }
    }

    pub fn with_route_objective(mut self, objective: RouteObjective) -> Self {
        self.route_objective = Some(objective);
        self
    }

    pub fn with_route_policy_sources(mut self, sources: RoutePolicySources) -> Self {
        self.route_policy_sources = sources;
        self
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

    pub async fn run(
        &self,
        request: TaskRequest,
        cancellation: CancellationToken,
    ) -> Result<RunResult, RuntimeError> {
        self.run_internal(request, None, None, cancellation).await
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
        self.run_internal(request, None, Some(client_request_id), cancellation)
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
        self.run_internal(prefix.request.clone(), Some(prefix), None, cancellation)
            .await
    }

    async fn run_internal(
        &self,
        request: TaskRequest,
        resume: Option<ResumablePrefix>,
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

        let tool_capabilities =
            self.tools
                .capability_snapshot()
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

        let tool_specs = self.tools.specs();
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
        let route_plan = self.router.plan(&route_request);
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
        let (mut messages, mut tool_calls, mut unavailable_routes, start_step, initial_elapsed) =
            if let Some(prefix) = resume {
                (
                    prefix.checkpoint.messages,
                    prefix.checkpoint.tool_calls_used,
                    prefix
                        .checkpoint
                        .unavailable_routes
                        .into_iter()
                        .collect::<BTreeSet<_>>(),
                    prefix.checkpoint.next_step,
                    Duration::from_millis(prefix.checkpoint.elapsed_ms),
                )
            } else {
                let mut initial_messages = vec![
                    ModelMessage::system(agent_system_prompt()),
                    ModelMessage::user(format!(
                        "Task objective:\n{}\n\nStructured task:\n{}",
                        request.objective,
                        serde_json::to_string_pretty(&request)
                            .unwrap_or_else(|_| request.objective.clone())
                    )),
                ];
                if let Some(scope) = &request.experience_scope {
                    let mut selected = if let Some(experience_dir) = &self.experience_dir {
                        let store = ExperienceStore::open(experience_dir)
                            .map_err(|error| RuntimeError::Experience(error.to_string()))?;
                        store
                            .retrieve_active(
                                mode,
                                scope,
                                request.budget.max_context_chars.min(4_000),
                            )
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
                    let mut used_chars = 0usize;
                    for candidate in selected {
                        let cost =
                            candidate.title.chars().count() + candidate.content.chars().count();
                        if used_chars + cost > request.budget.max_context_chars.min(4_000) {
                            continue;
                        }
                        used_chars += cost;
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
                (initial_messages, 0, BTreeSet::new(), 0, Duration::ZERO)
            };

        for step in start_step..request.budget.max_steps {
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
            if remaining_budget(started_at, request.budget.timeout_ms, initial_elapsed).is_none() {
                return finish_with_answer(
                    &mut journal,
                    run_id,
                    journal_path,
                    budget_answer("run timeout exhausted"),
                    StopReason::BudgetExhausted,
                );
            }

            let (model_messages, context_compaction) =
                compact_messages(&messages, request.budget.max_context_chars, 4);
            if let Some(compaction) = context_compaction {
                journal
                    .append(JournalEventKind::ContextCompacted {
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
            for (attempt, route) in routes.iter().enumerate() {
                let route_key = ProviderRouteKey {
                    provider_id: route.manifest.provider_id.clone(),
                    model: route.manifest.model.clone(),
                };
                if unavailable_routes.contains(&route_key) {
                    continue;
                }
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
                let provider_result = tokio::time::timeout(
                    remaining,
                    candidate
                        .provider
                        .complete(model_request.clone(), &cancellation),
                )
                .await;
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
                    Ok(Ok(candidate_response)) => {
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
                        response = Some(candidate_response);
                        break;
                    }
                    Ok(Err(source)) if source.retryable => {
                        unavailable_routes.insert(route_key);
                        last_retryable_error = Some(source);
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

            match response.turn {
                ModelTurn::Final { answer } => {
                    answer
                        .validate(&request)
                        .map_err(|error| RuntimeError::InvalidOutput {
                            path: journal_path.clone(),
                            message: error.to_string(),
                        })?;
                    let stop_reason = stop_reason_for_answer(answer.status);
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
                    return Ok(RunResult {
                        run_id,
                        journal_path,
                        answer,
                        stop_reason,
                    });
                }
                ModelTurn::ToolCalls { calls } => {
                    if calls.is_empty() {
                        return Err(RuntimeError::InvalidOutput {
                            path: journal_path,
                            message: "model returned an empty tool-call list".into(),
                        });
                    }
                    if tool_calls + calls.len() > request.budget.max_tool_calls {
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
                    tool_calls += calls.len();
                    messages.push(ModelMessage::assistant("", calls.clone()));

                    for call in calls {
                        if remaining_budget(started_at, request.budget.timeout_ms, initial_elapsed)
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
                            .tools
                            .execute(&call, &request.permissions, &cancellation)
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

fn authorization_answer() -> crate::domain::AgentAnswer {
    crate::domain::AgentAnswer {
        schema_version: crate::domain::ANSWER_SCHEMA_VERSION,
        status: AnswerStatus::NeedsAuthorization,
        conclusion: "当前任务被识别为执行操作，但 V0 默认只读；未执行任何外部动作。".into(),
        claims: Vec::new(),
        evidence: Vec::new(),
        assumptions: Vec::new(),
        constraints: vec!["allow_write=false".into()],
        unknowns: vec![UnknownItem {
            id: "authorization-scope".into(),
            description: "缺少明确的写入权限和审批范围".into(),
            impact: ImpactLevel::High,
            why_it_matters: Some("没有授权就无法安全执行副作用动作".into()),
            reduction_method: Some("由用户明确授权具体动作、范围和回滚条件".into()),
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
        }
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
    use crate::domain::{PermissionSet, RunBudget};
    use crate::experience::{
        ExperienceCandidateSpec, ExperienceKind, ExperienceStore, ExperienceVerificationInput,
    };
    use crate::journal::{audit_events, read_events, replay_events, summarize_route_trace};
    use crate::provider::{DeterministicProvider, ModelProvider, ProviderError, ProviderErrorCode};
    use crate::tools::{EchoTool, default_registry};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RetryableFailureProvider;

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
                message: "injected authentication failure".into(),
                retryable: false,
            })
        }
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
        assert_eq!(
            replay
                .tool_capabilities
                .as_ref()
                .map(|value| value.tools.len()),
            Some(2)
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
        let request = TaskRequest {
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
        assert_eq!(summary.selected_provider_id.as_deref(), Some("failing"));
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
        assert!(matches!(
            error,
            RuntimeError::Provider {
                source: ProviderError {
                    code: ProviderErrorCode::Authentication,
                    ..
                },
                ..
            }
        ));
        assert_eq!(failing.calls.load(Ordering::SeqCst), 1);
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
        store.review(active.id, "reviewer").unwrap();
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
        store.approve(active.id, "owner").unwrap();
        store.activate(active.id).unwrap();
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
}
