//! Run one model task across two providers and compare cost and health.
//!
//! The comparison is evidence for routing decisions, not a routing policy: it
//! records per-provider usage, model-reported cost and the cost estimated from
//! the provider manifest. A failed provider is kept in the result so callers
//! can see both sides of the comparison.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::domain::ModelRequest;
use crate::provider::{ModelProvider, ProviderErrorCode};
use crate::router::ProviderManifest;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderComparisonError {
    #[error("at least two providers are required")]
    NotEnoughProviders,
    #[error("provider {0} appears more than once")]
    DuplicateProvider(String),
    #[error("provider {provider_id} does not match manifest {manifest_provider_id}")]
    ManifestMismatch {
        provider_id: String,
        manifest_provider_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderRunOutcome {
    pub provider_id: String,
    pub model: String,
    pub ok: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reported_cost_micros: u64,
    pub estimated_cost_micros: u64,
    pub quality_score: u32,
    pub error_code: Option<ProviderErrorCode>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderComparison {
    pub task_id: String,
    pub outcomes: Vec<ProviderRunOutcome>,
}

impl ProviderComparison {
    pub fn successful(&self) -> impl Iterator<Item = &ProviderRunOutcome> {
        self.outcomes.iter().filter(|outcome| outcome.ok)
    }

    pub fn cheapest_successful(&self) -> Option<&ProviderRunOutcome> {
        self.successful()
            .min_by_key(|outcome| (outcome.estimated_cost_micros, outcome.provider_id.clone()))
    }

    /// Difference between the most and least expensive successful run.
    pub fn cost_spread_micros(&self) -> Option<u64> {
        let mut costs = self
            .successful()
            .map(|outcome| outcome.estimated_cost_micros);
        let first = costs.next()?;
        let (mut low, mut high) = (first, first);
        for cost in costs {
            low = low.min(cost);
            high = high.max(cost);
        }
        Some(high - low)
    }
}

/// Estimate cost from manifest prices, rounding each component up to a micro.
pub fn estimate_cost_micros(
    manifest: &ProviderManifest,
    input_tokens: u64,
    output_tokens: u64,
) -> u64 {
    let input = u128::from(input_tokens) * u128::from(manifest.input_cost_micros_per_k);
    let output = u128::from(output_tokens) * u128::from(manifest.output_cost_micros_per_k);
    let total = input.div_ceil(1_000) + output.div_ceil(1_000);
    u64::try_from(total).unwrap_or(u64::MAX)
}

/// Run the same request on every provider. Provider failures are recorded in
/// the result; configuration errors (ids, manifests) fail the whole comparison.
pub async fn compare_same_task(
    task_id: impl Into<String>,
    providers: &[(&dyn ModelProvider, &ProviderManifest)],
    request: ModelRequest,
    cancellation: &CancellationToken,
) -> Result<ProviderComparison, ProviderComparisonError> {
    if providers.len() < 2 {
        return Err(ProviderComparisonError::NotEnoughProviders);
    }
    for (index, (provider, manifest)) in providers.iter().enumerate() {
        if providers[..index]
            .iter()
            .any(|(existing, _)| existing.id() == provider.id())
        {
            return Err(ProviderComparisonError::DuplicateProvider(
                provider.id().into(),
            ));
        }
        if provider.id() != manifest.provider_id {
            return Err(ProviderComparisonError::ManifestMismatch {
                provider_id: provider.id().into(),
                manifest_provider_id: manifest.provider_id.clone(),
            });
        }
    }

    let mut outcomes = Vec::with_capacity(providers.len());
    for (provider, manifest) in providers {
        let outcome = match provider.complete(request.clone(), cancellation).await {
            Ok(response) => ProviderRunOutcome {
                provider_id: provider.id().into(),
                model: provider.model().into(),
                ok: true,
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                reported_cost_micros: response.usage.cost_micros,
                estimated_cost_micros: estimate_cost_micros(
                    manifest,
                    response.usage.input_tokens,
                    response.usage.output_tokens,
                ),
                quality_score: manifest.quality_score,
                error_code: None,
                retryable: false,
            },
            Err(error) => ProviderRunOutcome {
                provider_id: provider.id().into(),
                model: provider.model().into(),
                ok: false,
                input_tokens: 0,
                output_tokens: 0,
                reported_cost_micros: 0,
                estimated_cost_micros: 0,
                quality_score: manifest.quality_score,
                error_code: Some(error.code),
                retryable: error.retryable,
            },
        };
        outcomes.push(outcome);
    }
    Ok(ProviderComparison {
        task_id: task_id.into(),
        outcomes,
    })
}

pub fn render_provider_comparison(comparison: &ProviderComparison) -> String {
    let mut output = format!("Provider comparison: {}", comparison.task_id);
    for outcome in &comparison.outcomes {
        let status = if outcome.ok { "ok" } else { "failed" };
        output.push_str(&format!(
            "\n- {}/{}: {status}, tokens {}+{}, reported cost {}, estimated cost {}, quality {}{}",
            outcome.provider_id,
            outcome.model,
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.reported_cost_micros,
            outcome.estimated_cost_micros,
            outcome.quality_score,
            outcome
                .error_code
                .map(|code| format!(", error {code:?}"))
                .unwrap_or_default()
        ));
    }
    if let Some(spread) = comparison.cost_spread_micros() {
        output.push_str(&format!("\ncost spread: {spread} micros"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CognitiveMode, ModelResponse, ModelTurn, Usage};
    use crate::provider::ProviderError;
    use async_trait::async_trait;

    struct FakeProvider {
        id: &'static str,
        model: &'static str,
        input_tokens: u64,
        output_tokens: u64,
        failure: Option<ProviderErrorCode>,
    }

    #[async_trait]
    impl ModelProvider for FakeProvider {
        fn id(&self) -> &str {
            self.id
        }

        fn model(&self) -> &str {
            self.model
        }

        async fn complete(
            &self,
            _request: ModelRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ModelResponse, ProviderError> {
            if let Some(code) = self.failure {
                return Err(ProviderError {
                    code,
                    message: "fake failure".into(),
                    retryable: true,
                });
            }
            Ok(ModelResponse {
                provider_id: self.id.into(),
                model: self.model.into(),
                turn: ModelTurn::ToolCalls { calls: Vec::new() },
                usage: Usage {
                    input_tokens: self.input_tokens,
                    output_tokens: self.output_tokens,
                    cost_micros: 0,
                    cache_hit_tokens: 0,
                    cache_miss_tokens: 0,
                },
            })
        }
    }

    fn manifest(id: &str, input_rate: u64, output_rate: u64, quality: u32) -> ProviderManifest {
        ProviderManifest {
            provider_id: id.into(),
            model: format!("{id}-model"),
            privacy: crate::router::PrivacyClass::Remote,
            supported_modes: vec![CognitiveMode::DirectAnswer],
            supports_tools: false,
            supports_structured_output: false,
            max_risk: crate::domain::RiskLevel::Low,
            quality_score: quality,
            input_cost_micros_per_k: input_rate,
            output_cost_micros_per_k: output_rate,
            p95_latency_ms: 100,
        }
    }

    fn request() -> ModelRequest {
        ModelRequest {
            run_id: uuid::Uuid::nil(),
            mode: CognitiveMode::DirectAnswer,
            messages: vec![crate::domain::ModelMessage::user("compare")],
            tools: Vec::new(),
        }
    }

    #[tokio::test]
    async fn compares_cost_and_keeps_failed_provider() {
        let cheap = FakeProvider {
            id: "cheap",
            model: "cheap-model",
            input_tokens: 1_000,
            output_tokens: 500,
            failure: None,
        };
        let expensive = FakeProvider {
            id: "expensive",
            model: "expensive-model",
            input_tokens: 2_000,
            output_tokens: 1_000,
            failure: None,
        };
        let broken = FakeProvider {
            id: "broken",
            model: "broken-model",
            input_tokens: 0,
            output_tokens: 0,
            failure: Some(ProviderErrorCode::ProviderUnavailable),
        };
        let cheap_manifest = manifest("cheap", 1_000, 2_000, 60);
        let expensive_manifest = manifest("expensive", 10_000, 20_000, 90);
        let broken_manifest = manifest("broken", 1_000, 1_000, 50);
        let cancellation = CancellationToken::new();

        let comparison = compare_same_task(
            "task-1",
            &[
                (&cheap, &cheap_manifest),
                (&expensive, &expensive_manifest),
                (&broken, &broken_manifest),
            ],
            request(),
            &cancellation,
        )
        .await
        .unwrap();

        assert_eq!(comparison.outcomes.len(), 3);
        assert_eq!(comparison.successful().count(), 2);
        assert_eq!(
            comparison.cheapest_successful().unwrap().provider_id,
            "cheap"
        );
        assert_eq!(comparison.cost_spread_micros(), Some(38_000));
        let failed = comparison
            .outcomes
            .iter()
            .find(|outcome| outcome.provider_id == "broken")
            .unwrap();
        assert!(!failed.ok);
        assert_eq!(
            failed.error_code,
            Some(ProviderErrorCode::ProviderUnavailable)
        );
        assert!(failed.retryable);
        let rendered = render_provider_comparison(&comparison);
        assert!(rendered.contains("estimated cost 2000"));
        assert!(rendered.contains("ProviderUnavailable"));
    }

    #[tokio::test]
    async fn rejects_duplicate_and_mismatched_manifests() {
        let first = FakeProvider {
            id: "first",
            model: "first-model",
            input_tokens: 1,
            output_tokens: 1,
            failure: None,
        };
        let second = FakeProvider {
            id: "second",
            model: "second-model",
            input_tokens: 1,
            output_tokens: 1,
            failure: None,
        };
        let first_manifest = manifest("first", 1, 1, 1);
        let second_manifest = manifest("second", 1, 1, 1);
        let cancellation = CancellationToken::new();

        let duplicate = compare_same_task(
            "task",
            &[(&first, &first_manifest), (&first, &first_manifest)],
            request(),
            &cancellation,
        )
        .await
        .unwrap_err();
        assert_eq!(
            duplicate,
            ProviderComparisonError::DuplicateProvider("first".into())
        );

        let mismatch = compare_same_task(
            "task",
            &[(&first, &second_manifest), (&second, &first_manifest)],
            request(),
            &cancellation,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            mismatch,
            ProviderComparisonError::ManifestMismatch { .. }
        ));

        let single = compare_same_task(
            "task",
            &[(&first, &first_manifest)],
            request(),
            &cancellation,
        )
        .await
        .unwrap_err();
        assert_eq!(single, ProviderComparisonError::NotEnoughProviders);
    }

    #[test]
    fn estimates_cost_with_ceiling_per_component() {
        let manifest = manifest("priced", 1_000, 3_000, 50);
        assert_eq!(estimate_cost_micros(&manifest, 0, 0), 0);
        assert_eq!(estimate_cost_micros(&manifest, 1, 1), 4);
        assert_eq!(estimate_cost_micros(&manifest, 1_500, 500), 3_000);
    }
}
