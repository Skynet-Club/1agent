use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use first_principles_agent::{
    AgentRuntime, AgentTool, CognitiveMode, Journal, JournalEvent, JournalEventKind, MessageRole,
    ModelProvider, ModelRequest, ModelResponse, ModelTurn, PermissionSet, PrivacyClass,
    ProviderCandidate, ProviderError, ProviderManifest, RiskLevel, RuntimeError, StopReason,
    TaskRequest, ToolCall, ToolError, ToolOutcome, ToolRegistry, ToolSpec, Usage,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct SideEffectTool;

#[async_trait]
impl AgentTool for SideEffectTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "side_effect".into(),
            description: "Append one line to a counter file.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["path", "value"],
                "properties": {
                    "path": {"type": "string"},
                    "value": {"type": "string"}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn requires_write(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_write {
            return Err(ToolError::PermissionDenied(
                "write permission is disabled".into(),
            ));
        }
        let path = arguments
            .get("path")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("path is required".into()))?;
        let value = arguments
            .get("value")
            .and_then(|value| value.as_str())
            .unwrap_or("cycle");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        writeln!(file, "{value}").map_err(|error| ToolError::Failed(error.to_string()))?;
        file.flush()
            .map_err(|error| ToolError::Failed(error.to_string()))?;
        Ok(ToolOutcome {
            summary: "side effect recorded".into(),
            content: "ok".into(),
            metadata: Default::default(),
        })
    }
}

struct WriteOnceProvider {
    counter: PathBuf,
    value: String,
}

#[async_trait]
impl ModelProvider for WriteOnceProvider {
    fn id(&self) -> &str {
        "soak-provider"
    }

    fn model(&self) -> &str {
        "soak-v1"
    }

    async fn complete(
        &self,
        request: ModelRequest,
        _cancellation: &CancellationToken,
    ) -> Result<ModelResponse, ProviderError> {
        let observed = request
            .messages
            .iter()
            .any(|message| message.role == MessageRole::Tool);
        let turn = if observed {
            ModelTurn::Final {
                answer: first_principles_agent::AgentAnswer {
                    schema_version: first_principles_agent::ANSWER_SCHEMA_VERSION,
                    status: first_principles_agent::AnswerStatus::Completed,
                    conclusion: "recovery soak complete".into(),
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
        } else {
            ModelTurn::ToolCalls {
                calls: vec![ToolCall {
                    id: "side-effect-1".into(),
                    name: "side_effect".into(),
                    arguments: serde_json::json!({
                        "path": self.counter.display().to_string(),
                        "value": self.value,
                    }),
                }],
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

fn manifest() -> ProviderManifest {
    ProviderManifest {
        provider_id: "soak-provider".into(),
        model: "soak-v1".into(),
        privacy: PrivacyClass::Local,
        supported_modes: vec![CognitiveMode::Execute, CognitiveMode::DirectAnswer],
        supports_tools: true,
        supports_structured_output: true,
        max_risk: RiskLevel::High,
        quality_score: 100,
        input_cost_micros_per_k: 0,
        output_cost_micros_per_k: 0,
        p95_latency_ms: 0,
    }
}

fn copy_prefix(events: &[JournalEvent], upto: usize, path: &Path) {
    let mut journal = Journal::create(path, Uuid::new_v4()).unwrap();
    for event in &events[..=upto] {
        journal.append(event.event.clone()).unwrap();
    }
}

fn counter_lines(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .count()
}

#[tokio::test]
#[ignore = "soak; run manually before a release gate"]
async fn crash_injection_resumes_cleanly_or_fails_closed() {
    let cycles = std::env::var("SOAK_RECOVERY_CYCLES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(5);
    let seconds = std::env::var("SOAK_RECOVERY_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let started = std::time::Instant::now();
    let mut completed = 0usize;
    loop {
        if seconds > 0 {
            if started.elapsed() >= std::time::Duration::from_secs(seconds) {
                break;
            }
        } else if completed >= cycles {
            break;
        }
        let cycle = completed;
        let workspace = tempfile::tempdir().unwrap();
        let counter = workspace.path().join("counter.log");
        let runtime = AgentRuntime::new(
            vec![ProviderCandidate {
                provider: Arc::new(WriteOnceProvider {
                    counter: counter.clone(),
                    value: format!("cycle-{cycle}"),
                }),
                manifest: manifest(),
            }],
            {
                let mut tools = ToolRegistry::default();
                tools.register(Arc::new(SideEffectTool));
                tools
            },
            workspace.path().join(".journal"),
        );
        let mut request = TaskRequest::from_objective(
            "write one side effect and finish",
            workspace.path().into(),
        );
        request.permissions.allow_write = true;
        request.budget.max_provider_attempts = 0;
        request.budget.max_steps = 0;
        request.budget.max_tool_calls = 0;

        let complete = runtime
            .run(request, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(complete.stop_reason, StopReason::Completed);
        assert_eq!(counter_lines(&counter), 1);
        let events = first_principles_agent::read_events(&complete.journal_path).unwrap();
        let checkpoint = events
            .iter()
            .position(|event| {
                matches!(
                    event.event,
                    JournalEventKind::Checkpoint { .. } | JournalEventKind::CheckpointDelta { .. }
                )
            })
            .unwrap();
        let tool_called = events
            .iter()
            .position(|event| matches!(event.event, JournalEventKind::ToolCalled { .. }))
            .unwrap();
        let tool_observed = events
            .iter()
            .position(|event| matches!(event.event, JournalEventKind::ToolObserved { .. }))
            .unwrap();

        let resumed_path = workspace.path().join("resumed.jsonl");
        copy_prefix(&events, checkpoint, &resumed_path);
        let resumed = runtime
            .resume(&resumed_path, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resumed.answer.conclusion, complete.answer.conclusion);
        assert_eq!(
            counter_lines(&counter),
            1,
            "a completed side effect must not be replayed on resume"
        );

        let mid_call_path = workspace.path().join("mid-call.jsonl");
        copy_prefix(&events, tool_called, &mid_call_path);
        match runtime
            .resume(&mid_call_path, CancellationToken::new())
            .await
        {
            Err(RuntimeError::Resume(message)) => {
                assert!(message.contains("paired"), "message: {message}");
            }
            other => panic!("mid-call resume should fail closed, got {other:?}"),
        }
        assert_eq!(counter_lines(&counter), 1);

        let no_checkpoint_path = workspace.path().join("no-checkpoint.jsonl");
        copy_prefix(&events, tool_observed, &no_checkpoint_path);
        match runtime
            .resume(&no_checkpoint_path, CancellationToken::new())
            .await
        {
            Err(RuntimeError::Resume(message)) => {
                assert!(message.contains("no checkpoint"), "message: {message}");
            }
            other => panic!("resume without a checkpoint should fail, got {other:?}"),
        }
        assert_eq!(counter_lines(&counter), 1);

        completed += 1;
        if completed.is_multiple_of(100) {
            eprintln!(
                "recovery soak: {completed} cycles in {:?}",
                started.elapsed()
            );
        }
        if seconds > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
    eprintln!(
        "recovery soak: {completed} cycles in {:?}",
        started.elapsed()
    );
    assert!(completed > 0);
}
