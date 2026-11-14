//! Runtime shadow execution for provider comparison.
//!
//! `ShadowProvider` sends the same request to a primary and a shadow provider,
//! returns the primary result unchanged and records both outcomes. It is an
//! explicit opt-in: the shadow call adds cost and latency, so callers should
//! enable it only for evaluation/canary windows.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::domain::{ModelRequest, ModelResponse, Usage};
use crate::provider::{ModelProvider, ProviderError, ProviderErrorCode};

const MAX_SHADOW_OBSERVATIONS: usize = 256;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ShadowObservation {
    pub primary_provider: String,
    pub shadow_provider: String,
    pub primary_ok: bool,
    pub shadow_ok: bool,
    pub primary_usage: Option<Usage>,
    pub shadow_usage: Option<Usage>,
    pub primary_error: Option<ProviderErrorCode>,
    pub shadow_error: Option<ProviderErrorCode>,
    pub text_match: Option<bool>,
}

impl ShadowObservation {
    pub fn agrees(&self) -> bool {
        self.primary_ok == self.shadow_ok && self.text_match.unwrap_or(true)
    }
}

pub struct ShadowProvider {
    primary: Arc<dyn ModelProvider>,
    shadow: Arc<dyn ModelProvider>,
    observations: Mutex<Vec<ShadowObservation>>,
}

impl ShadowProvider {
    pub fn new(
        primary: impl ModelProvider + 'static,
        shadow: impl ModelProvider + 'static,
    ) -> Self {
        Self {
            primary: Arc::new(primary),
            shadow: Arc::new(shadow),
            observations: Mutex::new(Vec::new()),
        }
    }

    pub fn observations(&self) -> Vec<ShadowObservation> {
        self.observations
            .lock()
            .map(|observations| observations.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl ModelProvider for ShadowProvider {
    fn id(&self) -> &str {
        self.primary.id()
    }

    fn model(&self) -> &str {
        self.primary.model()
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: &CancellationToken,
    ) -> Result<ModelResponse, ProviderError> {
        let shadow_request = request.clone();
        let (primary, shadow) = tokio::join!(
            self.primary.complete(request, cancellation),
            self.shadow.complete(shadow_request, cancellation),
        );
        let observation = ShadowObservation {
            primary_provider: self.primary.id().into(),
            shadow_provider: self.shadow.id().into(),
            primary_ok: primary.is_ok(),
            shadow_ok: shadow.is_ok(),
            primary_usage: primary.as_ref().ok().map(|response| response.usage.clone()),
            shadow_usage: shadow.as_ref().ok().map(|response| response.usage.clone()),
            primary_error: primary.as_ref().err().map(|error| error.code),
            shadow_error: shadow.as_ref().err().map(|error| error.code),
            text_match: None,
        };
        self.record(observation);
        primary
    }

    async fn complete_text(
        &self,
        system: &str,
        user: &str,
        cancellation: &CancellationToken,
    ) -> Result<String, ProviderError> {
        let (primary, shadow) = tokio::join!(
            self.primary.complete_text(system, user, cancellation),
            self.shadow.complete_text(system, user, cancellation),
        );
        let text_match = match (&primary, &shadow) {
            (Ok(left), Ok(right)) => Some(left == right),
            (Err(_), Err(_)) => Some(true),
            _ => Some(false),
        };
        self.record(ShadowObservation {
            primary_provider: self.primary.id().into(),
            shadow_provider: self.shadow.id().into(),
            primary_ok: primary.is_ok(),
            shadow_ok: shadow.is_ok(),
            primary_usage: None,
            shadow_usage: None,
            primary_error: primary.as_ref().err().map(|error| error.code),
            shadow_error: shadow.as_ref().err().map(|error| error.code),
            text_match,
        });
        primary
    }
}

impl ShadowProvider {
    fn record(&self, observation: ShadowObservation) {
        if let Ok(mut observations) = self.observations.lock() {
            if observations.len() >= MAX_SHADOW_OBSERVATIONS {
                observations.remove(0);
            }
            observations.push(observation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CognitiveMode, ModelMessage, ModelTurn};

    struct FakeProvider {
        id: &'static str,
        usage: Usage,
        failure: Option<ProviderErrorCode>,
        text: &'static str,
    }

    #[async_trait]
    impl ModelProvider for FakeProvider {
        fn id(&self) -> &str {
            self.id
        }

        fn model(&self) -> &str {
            "model"
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            if let Some(code) = self.failure {
                return Err(ProviderError {
                    code,
                    message: "fake".into(),
                    retryable: false,
                });
            }
            Ok(ModelResponse {
                provider_id: self.id.into(),
                model: "model".into(),
                turn: ModelTurn::ToolCalls { calls: Vec::new() },
                usage: self.usage.clone(),
            })
        }

        async fn complete_text(
            &self,
            _system: &str,
            _user: &str,
            _cancellation: &CancellationToken,
        ) -> Result<String, ProviderError> {
            Ok(self.text.into())
        }
    }

    fn request() -> ModelRequest {
        ModelRequest {
            run_id: uuid::Uuid::nil(),
            mode: CognitiveMode::DirectAnswer,
            messages: vec![ModelMessage::user("shadow")],
            tools: Vec::new(),
        }
    }

    fn usage(input: u64, output: u64) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            cost_micros: 0,
            cache_hit_tokens: 0,
            cache_miss_tokens: 0,
        }
    }

    #[tokio::test]
    async fn returns_primary_and_records_both_sides() {
        let provider = ShadowProvider::new(
            FakeProvider {
                id: "primary",
                usage: usage(10, 2),
                failure: None,
                text: "primary",
            },
            FakeProvider {
                id: "shadow",
                usage: usage(20, 4),
                failure: None,
                text: "shadow",
            },
        );
        let response = provider
            .complete(request(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(response.provider_id, "primary");
        let observations = provider.observations();
        assert_eq!(observations.len(), 1);
        assert!(observations[0].agrees());
        assert_eq!(
            observations[0].shadow_usage.as_ref().unwrap().input_tokens,
            20
        );
    }

    #[tokio::test]
    async fn primary_result_survives_shadow_failure() {
        let provider = ShadowProvider::new(
            FakeProvider {
                id: "primary",
                usage: usage(10, 2),
                failure: None,
                text: "primary",
            },
            FakeProvider {
                id: "shadow",
                usage: usage(0, 0),
                failure: Some(ProviderErrorCode::ProviderUnavailable),
                text: "shadow",
            },
        );
        let response = provider
            .complete(request(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(response.provider_id, "primary");
        let observations = provider.observations();
        assert!(!observations[0].agrees());
        assert_eq!(
            observations[0].shadow_error,
            Some(ProviderErrorCode::ProviderUnavailable)
        );
    }

    #[tokio::test]
    async fn text_calls_are_shadowed_and_observations_are_bounded() {
        let provider = ShadowProvider::new(
            FakeProvider {
                id: "primary",
                usage: usage(0, 0),
                failure: None,
                text: "left",
            },
            FakeProvider {
                id: "shadow",
                usage: usage(0, 0),
                failure: None,
                text: "right",
            },
        );
        let text = provider
            .complete_text("system", "user", &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(text, "left");
        assert!(!provider.observations()[0].agrees());
        for _ in 0..MAX_SHADOW_OBSERVATIONS + 10 {
            provider
                .complete_text("system", "user", &CancellationToken::new())
                .await
                .unwrap();
        }
        assert_eq!(provider.observations().len(), MAX_SHADOW_OBSERVATIONS);
    }
}
