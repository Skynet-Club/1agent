use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::domain::{
    AgentAnswer, AnswerStatus, Evidence, FlipCondition, ModelRequest, ModelResponse, ModelTurn,
    ToolCall, ToolObservation, ToolStatus, UnknownItem, Usage,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorCode {
    InvalidConfiguration,
    Network,
    Authentication,
    RateLimited,
    Timeout,
    InvalidResponse,
    ProviderUnavailable,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderError {
    pub code: ProviderErrorCode,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    fn new(code: ProviderErrorCode, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProviderError {}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;
    fn model(&self) -> &str;

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: &CancellationToken,
    ) -> Result<ModelResponse, ProviderError>;
}

pub struct DeterministicProvider {
    id: String,
    model: String,
}

impl DeterministicProvider {
    pub fn new() -> Self {
        Self {
            id: "deterministic".into(),
            model: "offline-v1".into(),
        }
    }

    fn extract_read_target(text: &str) -> Option<(String, usize)> {
        let marker = "@read-file:";
        let start = text.find(marker)? + marker.len();
        let tail = &text[start..];
        let directive = tail.lines().next()?.trim();
        let (path, options) = if let Some(quoted) = directive.strip_prefix('"') {
            let close = quoted.find('"')?;
            (
                quoted[..close].to_string(),
                quoted[close + 1..].trim().trim_start_matches('|').trim(),
            )
        } else if let Some((path, options)) = directive.split_once('|') {
            (path.trim().to_string(), options.trim())
        } else {
            (directive.to_string(), "")
        };
        let max_bytes = options
            .strip_prefix("max_bytes=")
            .and_then(|part| part.parse::<usize>().ok())
            .unwrap_or(65_536);
        Some((path, max_bytes))
    }

    fn latest_observation(messages: &[crate::domain::ModelMessage]) -> Option<ToolObservation> {
        messages.iter().rev().find_map(|message| {
            if message.role == crate::domain::MessageRole::Tool {
                serde_json::from_str::<ToolObservation>(&message.content).ok()
            } else {
                None
            }
        })
    }
}

impl Default for DeterministicProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelProvider for DeterministicProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: &CancellationToken,
    ) -> Result<ModelResponse, ProviderError> {
        if cancellation.is_cancelled() {
            return Err(ProviderError::new(
                ProviderErrorCode::Cancelled,
                "provider call cancelled",
                false,
            ));
        }

        if let Some(observation) = Self::latest_observation(&request.messages) {
            let status = if observation.status == ToolStatus::Ok {
                AnswerStatus::Completed
            } else {
                AnswerStatus::NeedsEvidence
            };
            let evidence = if observation.status == ToolStatus::Ok {
                vec![Evidence {
                    source: observation
                        .metadata
                        .get("relative_path")
                        .cloned()
                        .unwrap_or_else(|| observation.tool.clone()),
                    statement: observation.summary.clone(),
                    claim_ids: vec!["claim-1".into()],
                }]
            } else {
                Vec::new()
            };
            let answer = AgentAnswer {
                status,
                conclusion: if observation.status == ToolStatus::Ok {
                    format!("已读取证据源：{}", observation.summary)
                } else {
                    format!("证据读取失败：{}", observation.summary)
                },
                claims: if observation.status == ToolStatus::Ok {
                    vec![crate::domain::Claim {
                        id: "claim-1".into(),
                        text: observation.summary.clone(),
                        status: crate::domain::ClaimStatus::Verified,
                        sources: vec![
                            observation
                                .metadata
                                .get("relative_path")
                                .cloned()
                                .unwrap_or_else(|| observation.tool.clone()),
                        ],
                        scope: Some("current task evidence".into()),
                    }]
                } else {
                    Vec::new()
                },
                evidence,
                assumptions: Vec::new(),
                constraints: Vec::new(),
                unknowns: if observation.status == ToolStatus::Ok {
                    Vec::new()
                } else {
                    vec![UnknownItem {
                        description: observation
                            .error_code
                            .unwrap_or_else(|| "tool_failure".into()),
                        why_it_matters: Some(observation.summary),
                        reduction_method: None,
                    }]
                },
                flip_conditions: if observation.status == ToolStatus::Ok {
                    vec![FlipCondition {
                        if_observed: "证据文件与任务约束冲突".into(),
                        then: "结论应改为有条件或需证据".into(),
                    }]
                } else {
                    Vec::new()
                },
                action_receipts: Vec::new(),
            };
            return Ok(ModelResponse {
                provider_id: self.id.clone(),
                model: self.model.clone(),
                turn: ModelTurn::Final { answer },
                usage: Usage::default(),
            });
        }

        let user_text = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == crate::domain::MessageRole::User)
            .map(|message| message.content.as_str())
            .unwrap_or_default();

        if let Some((path, max_bytes)) = Self::extract_read_target(user_text) {
            return Ok(ModelResponse {
                provider_id: self.id.clone(),
                model: self.model.clone(),
                turn: ModelTurn::ToolCalls {
                    calls: vec![ToolCall {
                        id: "deterministic-call-1".into(),
                        name: "read_file".into(),
                        arguments: serde_json::json!({
                            "path": path,
                            "max_bytes": max_bytes
                        }),
                    }],
                },
                usage: Usage::default(),
            });
        }

        let answer = AgentAnswer {
            status: AnswerStatus::NeedsEvidence,
            conclusion: "当前离线 Provider 不对自然语言任务作可靠性判断；请配置推理 Provider，或使用 @read-file:<path> 验证只读证据链路。".into(),
            claims: Vec::new(),
            evidence: Vec::new(),
            assumptions: Vec::new(),
            constraints: request
                .messages
                .first()
                .map(|message| vec![format!("protocol_context_chars={}", message.content.len())])
                .unwrap_or_default(),
            unknowns: vec![UnknownItem {
                description: "缺少能够回答当前任务的推理模型".into(),
                why_it_matters: Some("离线确定性 Provider 只能验证运行时链路，不能替代真实推理。".into()),
                reduction_method: Some("配置 OpenAI-compatible Provider".into()),
            }],
            flip_conditions: vec![FlipCondition {
                if_observed: "配置并验证可用推理 Provider".into(),
                then: "应重新运行任务并基于其证据输出结论".into(),
            }],
            action_receipts: Vec::new(),
        };
        Ok(ModelResponse {
            provider_id: self.id.clone(),
            model: self.model.clone(),
            turn: ModelTurn::Final { answer },
            usage: Usage::default(),
        })
    }
}

pub struct OpenAiCompatibleProvider {
    id: String,
    model: String,
    endpoint: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        id: impl Into<String>,
        model: impl Into<String>,
        base_url: &str,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self, ProviderError> {
        let model = model.into();
        if model.trim().is_empty() {
            return Err(ProviderError::new(
                ProviderErrorCode::InvalidConfiguration,
                "model must not be empty",
                false,
            ));
        }
        let trimmed = base_url.trim_end_matches('/');
        let endpoint = if trimmed.ends_with("/chat/completions") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/chat/completions")
        };
        let url = reqwest::Url::parse(&endpoint).map_err(|error| {
            ProviderError::new(
                ProviderErrorCode::InvalidConfiguration,
                format!("invalid base URL: {error}"),
                false,
            )
        })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ProviderError::new(
                ProviderErrorCode::InvalidConfiguration,
                "base URL must use http or https",
                false,
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| {
                ProviderError::new(
                    ProviderErrorCode::InvalidConfiguration,
                    format!("failed to construct HTTP client: {error}"),
                    false,
                )
            })?;
        Ok(Self {
            id: id.into(),
            model,
            endpoint,
            api_key,
            client,
        })
    }

    fn parse_answer(content: &str) -> Result<AgentAnswer, ProviderError> {
        let cleaned = strip_json_fence(content.trim());
        serde_json::from_str(cleaned).map_err(|error| {
            ProviderError::new(
                ProviderErrorCode::InvalidResponse,
                format!("provider answer is not valid AgentAnswer JSON: {error}"),
                false,
            )
        })
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: &CancellationToken,
    ) -> Result<ModelResponse, ProviderError> {
        let messages: Vec<WireMessage> = request
            .messages
            .iter()
            .map(|message| WireMessage {
                role: match message.role {
                    crate::domain::MessageRole::System => "system",
                    crate::domain::MessageRole::User => "user",
                    crate::domain::MessageRole::Assistant => "assistant",
                    crate::domain::MessageRole::Tool => "tool",
                }
                .into(),
                content: (!message.content.is_empty()).then(|| message.content.clone()),
                tool_calls: (!message.tool_calls.is_empty()).then(|| {
                    message
                        .tool_calls
                        .iter()
                        .map(|call| WireToolCall {
                            id: call.id.clone(),
                            kind: "function".into(),
                            function: WireFunctionCall {
                                name: call.name.clone(),
                                arguments: call.arguments.to_string(),
                            },
                        })
                        .collect()
                }),
                tool_call_id: message.tool_call_id.clone(),
            })
            .collect();
        let tools: Vec<WireTool> = request
            .tools
            .iter()
            .map(|tool| WireTool {
                kind: "function".into(),
                function: WireToolDefinition {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.parameters.clone(),
                },
            })
            .collect();
        let body = WireRequest {
            model: self.model.clone(),
            messages,
            tools: (!tools.is_empty()).then_some(tools),
            temperature: 0.0,
        };
        let mut builder = self.client.post(&self.endpoint).json(&body);
        if let Some(api_key) = &self.api_key {
            builder = builder.bearer_auth(api_key);
        }

        let response = tokio::select! {
            _ = cancellation.cancelled() => {
                return Err(ProviderError::new(
                    ProviderErrorCode::Cancelled,
                    "provider call cancelled",
                    false,
                ));
            }
            response = builder.send() => response.map_err(|error| {
                ProviderError::new(
                    if error.is_timeout() {
                        ProviderErrorCode::Timeout
                    } else {
                        ProviderErrorCode::Network
                    },
                    format!("provider request failed: {error}"),
                    error.is_timeout() || error.is_connect(),
                )
            })?,
        };

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let code = match status.as_u16() {
                401 | 403 => ProviderErrorCode::Authentication,
                429 => ProviderErrorCode::RateLimited,
                408 | 504 => ProviderErrorCode::Timeout,
                500..=599 => ProviderErrorCode::ProviderUnavailable,
                _ => ProviderErrorCode::InvalidResponse,
            };
            let excerpt: String = body.chars().take(512).collect();
            return Err(ProviderError::new(
                code,
                format!("provider returned HTTP {status}: {excerpt}"),
                code != ProviderErrorCode::Authentication,
            ));
        }

        let wire: WireResponse = response.json().await.map_err(|error| {
            ProviderError::new(
                ProviderErrorCode::InvalidResponse,
                format!("invalid provider response JSON: {error}"),
                false,
            )
        })?;
        let choice = wire.choices.into_iter().next().ok_or_else(|| {
            ProviderError::new(
                ProviderErrorCode::InvalidResponse,
                "provider returned no choices",
                false,
            )
        })?;
        let usage = wire
            .usage
            .map(|usage| Usage {
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
                cost_micros: 0,
            })
            .unwrap_or_default();

        let turn = if let Some(calls) = choice.message.tool_calls {
            let calls = calls
                .into_iter()
                .map(|call| {
                    let arguments =
                        serde_json::from_str(&call.function.arguments).map_err(|error| {
                            ProviderError::new(
                                ProviderErrorCode::InvalidResponse,
                                format!("tool call arguments are invalid JSON: {error}"),
                                false,
                            )
                        })?;
                    Ok(ToolCall {
                        id: call.id,
                        name: call.function.name,
                        arguments,
                    })
                })
                .collect::<Result<Vec<_>, ProviderError>>()?;
            if calls.is_empty() {
                ModelTurn::Final {
                    answer: Self::parse_answer(choice.message.content.as_deref().unwrap_or(""))?,
                }
            } else {
                ModelTurn::ToolCalls { calls }
            }
        } else {
            ModelTurn::Final {
                answer: Self::parse_answer(choice.message.content.as_deref().unwrap_or(""))?,
            }
        };

        Ok(ModelResponse {
            provider_id: self.id.clone(),
            model: self.model.clone(),
            turn,
            usage,
        })
    }
}

fn strip_json_fence(content: &str) -> &str {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }
    let after_header = trimmed
        .find('\n')
        .map(|index| &trimmed[index + 1..])
        .unwrap_or("");
    after_header
        .strip_suffix("```")
        .map(str::trim)
        .unwrap_or(after_header)
}

#[derive(Serialize)]
struct WireRequest {
    model: String,
    messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WireTool>>,
    temperature: f32,
}

#[derive(Serialize)]
struct WireMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<WireToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Serialize)]
struct WireTool {
    #[serde(rename = "type")]
    kind: String,
    function: WireToolDefinition,
}

#[derive(Serialize)]
struct WireToolDefinition {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Serialize)]
struct WireToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: WireFunctionCall,
}

#[derive(Serialize)]
struct WireFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct WireResponse {
    choices: Vec<WireChoice>,
    usage: Option<WireUsage>,
}

#[derive(Deserialize)]
struct WireChoice {
    message: WireAssistantMessage,
}

#[derive(Deserialize)]
struct WireAssistantMessage {
    content: Option<String>,
    tool_calls: Option<Vec<WireResponseToolCall>>,
}

#[derive(Deserialize)]
struct WireResponseToolCall {
    id: String,
    function: WireResponseFunction,
}

#[derive(Deserialize)]
struct WireResponseFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct WireUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_json_markdown_fence() {
        assert_eq!(
            strip_json_fence("```json\n{\"status\":\"completed\"}\n```"),
            "{\"status\":\"completed\"}"
        );
    }

    #[test]
    fn provider_rejects_invalid_endpoint_scheme() {
        let error = OpenAiCompatibleProvider::new(
            "test",
            "model",
            "file:///tmp/endpoint",
            None,
            Duration::from_secs(1),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ProviderErrorCode::InvalidConfiguration);
    }
}
