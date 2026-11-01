use serde::{Deserialize, Serialize};

use crate::domain::{CognitiveMode, RiskLevel};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    #[default]
    Local,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderManifest {
    pub provider_id: String,
    pub model: String,
    pub privacy: PrivacyClass,
    pub supported_modes: Vec<CognitiveMode>,
    pub supports_tools: bool,
    pub supports_structured_output: bool,
    pub max_risk: RiskLevel,
    pub quality_score: u32,
    pub input_cost_micros_per_k: u64,
    pub output_cost_micros_per_k: u64,
    pub p95_latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteRequest {
    pub mode: CognitiveMode,
    pub risk: RiskLevel,
    pub requires_tools: bool,
    pub requires_structured_output: bool,
    pub allow_remote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteDecision {
    pub manifest: ProviderManifest,
    pub reason: String,
}

#[derive(Debug, Default, Clone)]
pub struct Router {
    manifests: Vec<ProviderManifest>,
}

impl Router {
    pub fn new(manifests: Vec<ProviderManifest>) -> Self {
        Self { manifests }
    }

    pub fn route(&self, request: &RouteRequest) -> Option<RouteDecision> {
        self.rank(request).into_iter().next()
    }

    pub fn rank(&self, request: &RouteRequest) -> Vec<RouteDecision> {
        let mut candidates: Vec<&ProviderManifest> = self
            .manifests
            .iter()
            .filter(|manifest| {
                manifest.supported_modes.contains(&request.mode)
                    && request.risk <= manifest.max_risk
                    && (!request.requires_tools || manifest.supports_tools)
                    && (!request.requires_structured_output || manifest.supports_structured_output)
                    && (request.allow_remote || manifest.privacy == PrivacyClass::Local)
            })
            .collect();

        candidates.sort_by_key(|manifest| {
            (
                std::cmp::Reverse(manifest.quality_score),
                manifest.input_cost_micros_per_k + manifest.output_cost_micros_per_k,
                manifest.p95_latency_ms,
                manifest.provider_id.clone(),
                manifest.model.clone(),
            )
        });

        candidates
            .into_iter()
            .map(|manifest| RouteDecision {
                manifest: manifest.clone(),
                reason: format!(
                    "quality-first candidate for {:?}, risk {:?}, tools={}, structured={}, remote={}",
                    request.mode,
                    request.risk,
                    request.requires_tools,
                    request.requires_structured_output,
                    request.allow_remote
                ),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(
        id: &str,
        privacy: PrivacyClass,
        supports_tools: bool,
        quality: u32,
    ) -> ProviderManifest {
        ProviderManifest {
            provider_id: id.into(),
            model: format!("{id}-model"),
            privacy,
            supported_modes: vec![
                CognitiveMode::DirectAnswer,
                CognitiveMode::EvidenceResearch,
                CognitiveMode::DecisionAnalysis,
            ],
            supports_tools,
            supports_structured_output: true,
            max_risk: RiskLevel::Medium,
            quality_score: quality,
            input_cost_micros_per_k: 10,
            output_cost_micros_per_k: 20,
            p95_latency_ms: 100,
        }
    }

    #[test]
    fn remote_provider_is_filtered_when_network_is_disabled() {
        let router = Router::new(vec![
            manifest("remote", PrivacyClass::Remote, true, 100),
            manifest("local", PrivacyClass::Local, true, 60),
        ]);
        let decision = router
            .route(&RouteRequest {
                mode: CognitiveMode::EvidenceResearch,
                risk: RiskLevel::Low,
                requires_tools: true,
                requires_structured_output: true,
                allow_remote: false,
            })
            .unwrap();
        assert_eq!(decision.manifest.provider_id, "local");
    }

    #[test]
    fn router_rejects_manifest_without_required_tool_support() {
        let router = Router::new(vec![manifest("no-tools", PrivacyClass::Local, false, 100)]);
        let decision = router.route(&RouteRequest {
            mode: CognitiveMode::EvidenceResearch,
            risk: RiskLevel::Low,
            requires_tools: true,
            requires_structured_output: true,
            allow_remote: false,
        });
        assert!(decision.is_none());
    }

    #[test]
    fn rank_returns_all_filtered_candidates_in_priority_order() {
        let router = Router::new(vec![
            manifest("low", PrivacyClass::Local, true, 10),
            manifest("high", PrivacyClass::Local, true, 100),
        ]);
        let ranked = router.rank(&RouteRequest {
            mode: CognitiveMode::EvidenceResearch,
            risk: RiskLevel::Low,
            requires_tools: true,
            requires_structured_output: true,
            allow_remote: false,
        });
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].manifest.provider_id, "high");
        assert_eq!(ranked[1].manifest.provider_id, "low");
    }
}
