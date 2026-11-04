use serde::{Deserialize, Serialize};

use crate::domain::{CognitiveMode, RiskLevel};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    #[default]
    Local,
    Remote,
}

impl PrivacyClass {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Local => "本地",
            Self::Remote => "远程",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteObjective {
    #[default]
    Quality,
    Balanced,
    Latency,
    Cost,
}

impl RouteObjective {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Quality => "质量优先",
            Self::Balanced => "质量与成本均衡",
            Self::Latency => "延迟优先",
            Self::Cost => "成本优先",
        }
    }
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
    #[serde(default)]
    pub objective: RouteObjective,
    #[serde(default)]
    pub max_provider_p95_latency_ms: Option<u64>,
    #[serde(default)]
    pub max_unit_cost_micros_per_k: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteDecision {
    pub manifest: ProviderManifest,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteRejectionReason {
    UnsupportedMode,
    RiskExceeded,
    ToolsUnsupported,
    StructuredOutputUnsupported,
    RemoteDisallowed,
    ProviderP95LatencyExceeded,
    UnitCostExceeded,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyValueSource {
    #[default]
    Unset,
    RuntimeDefault,
    TaskRequest,
    CliOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoutePolicySources {
    pub objective: PolicyValueSource,
    pub latency_budget: PolicyValueSource,
    pub unit_cost_budget: PolicyValueSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RejectedRoute {
    pub provider_id: String,
    pub model: String,
    pub reasons: Vec<RouteRejectionReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoutePlan {
    pub request: RouteRequest,
    pub ranked: Vec<RouteDecision>,
    pub rejected: Vec<RejectedRoute>,
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
        self.plan(request).ranked.into_iter().next()
    }

    pub fn rank(&self, request: &RouteRequest) -> Vec<RouteDecision> {
        self.plan(request).ranked
    }

    pub fn plan(&self, request: &RouteRequest) -> RoutePlan {
        let mut candidates: Vec<&ProviderManifest> = self
            .manifests
            .iter()
            .filter(|manifest| rejection_reasons(manifest, request).is_empty())
            .collect();
        let mut rejected = self
            .manifests
            .iter()
            .filter_map(|manifest| {
                let reasons = rejection_reasons(manifest, request);
                (!reasons.is_empty()).then(|| RejectedRoute {
                    provider_id: manifest.provider_id.clone(),
                    model: manifest.model.clone(),
                    reasons,
                })
            })
            .collect::<Vec<_>>();
        rejected.sort_by(|left, right| {
            left.provider_id
                .cmp(&right.provider_id)
                .then_with(|| left.model.cmp(&right.model))
        });

        match request.objective {
            RouteObjective::Quality => candidates.sort_by_key(|manifest| {
                (
                    std::cmp::Reverse(manifest.quality_score),
                    total_cost(manifest),
                    manifest.p95_latency_ms,
                    manifest.provider_id.clone(),
                    manifest.model.clone(),
                )
            }),
            RouteObjective::Latency => candidates.sort_by_key(|manifest| {
                (
                    manifest.p95_latency_ms,
                    std::cmp::Reverse(manifest.quality_score),
                    total_cost(manifest),
                    manifest.provider_id.clone(),
                    manifest.model.clone(),
                )
            }),
            RouteObjective::Cost => candidates.sort_by_key(|manifest| {
                (
                    total_cost(manifest),
                    std::cmp::Reverse(manifest.quality_score),
                    manifest.p95_latency_ms,
                    manifest.provider_id.clone(),
                    manifest.model.clone(),
                )
            }),
            RouteObjective::Balanced => {
                let quality = rank_by(&candidates, |left, right| {
                    right
                        .quality_score
                        .cmp(&left.quality_score)
                        .then_with(|| left.provider_id.cmp(&right.provider_id))
                        .then_with(|| left.model.cmp(&right.model))
                });
                let cost = rank_by(&candidates, |left, right| {
                    total_cost(left)
                        .cmp(&total_cost(right))
                        .then_with(|| left.provider_id.cmp(&right.provider_id))
                        .then_with(|| left.model.cmp(&right.model))
                });
                let latency = rank_by(&candidates, |left, right| {
                    left.p95_latency_ms
                        .cmp(&right.p95_latency_ms)
                        .then_with(|| left.provider_id.cmp(&right.provider_id))
                        .then_with(|| left.model.cmp(&right.model))
                });
                candidates.sort_by_key(|manifest| {
                    let key = (manifest.provider_id.clone(), manifest.model.clone());
                    (
                        quality[&key] + cost[&key] + latency[&key],
                        std::cmp::Reverse(manifest.quality_score),
                        manifest.provider_id.clone(),
                        manifest.model.clone(),
                    )
                });
            }
        }

        let ranked = candidates
            .into_iter()
            .map(|manifest| RouteDecision {
                manifest: manifest.clone(),
                reason: format!(
                    "{:?}-first candidate for {:?}, risk {:?}, tools={}, structured={}, remote={}",
                    request.objective,
                    request.mode,
                    request.risk,
                    request.requires_tools,
                    request.requires_structured_output,
                    request.allow_remote
                ),
            })
            .collect();
        RoutePlan {
            request: request.clone(),
            ranked,
            rejected,
        }
    }
}

fn rejection_reasons(
    manifest: &ProviderManifest,
    request: &RouteRequest,
) -> Vec<RouteRejectionReason> {
    let mut reasons = Vec::new();
    if !manifest.supported_modes.contains(&request.mode) {
        reasons.push(RouteRejectionReason::UnsupportedMode);
    }
    if request.risk > manifest.max_risk {
        reasons.push(RouteRejectionReason::RiskExceeded);
    }
    if request.requires_tools && !manifest.supports_tools {
        reasons.push(RouteRejectionReason::ToolsUnsupported);
    }
    if request.requires_structured_output && !manifest.supports_structured_output {
        reasons.push(RouteRejectionReason::StructuredOutputUnsupported);
    }
    if !request.allow_remote && manifest.privacy == PrivacyClass::Remote {
        reasons.push(RouteRejectionReason::RemoteDisallowed);
    }
    if request
        .max_provider_p95_latency_ms
        .is_some_and(|limit| manifest.p95_latency_ms > limit)
    {
        reasons.push(RouteRejectionReason::ProviderP95LatencyExceeded);
    }
    if request
        .max_unit_cost_micros_per_k
        .is_some_and(|limit| total_cost(manifest) > limit)
    {
        reasons.push(RouteRejectionReason::UnitCostExceeded);
    }
    reasons
}

fn total_cost(manifest: &ProviderManifest) -> u64 {
    manifest
        .input_cost_micros_per_k
        .saturating_add(manifest.output_cost_micros_per_k)
}

fn rank_by(
    candidates: &[&ProviderManifest],
    compare: impl Fn(&ProviderManifest, &ProviderManifest) -> std::cmp::Ordering,
) -> std::collections::HashMap<(String, String), usize> {
    let mut ranked = candidates.to_vec();
    ranked.sort_by(|left, right| compare(left, right));
    ranked
        .into_iter()
        .enumerate()
        .map(|(rank, manifest)| ((manifest.provider_id.clone(), manifest.model.clone()), rank))
        .collect()
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
                objective: RouteObjective::Quality,
                max_provider_p95_latency_ms: None,
                max_unit_cost_micros_per_k: None,
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
            objective: RouteObjective::Quality,
            max_provider_p95_latency_ms: None,
            max_unit_cost_micros_per_k: None,
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
            objective: RouteObjective::Quality,
            max_provider_p95_latency_ms: None,
            max_unit_cost_micros_per_k: None,
        });
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].manifest.provider_id, "high");
        assert_eq!(ranked[1].manifest.provider_id, "low");
    }

    #[test]
    fn route_objective_changes_ranking_but_not_hard_filters() {
        let mut quality = manifest("quality", PrivacyClass::Local, true, 100);
        quality.input_cost_micros_per_k = 80;
        quality.output_cost_micros_per_k = 20;
        quality.p95_latency_ms = 100;
        let mut balanced = manifest("balanced", PrivacyClass::Local, true, 90);
        balanced.input_cost_micros_per_k = 10;
        balanced.output_cost_micros_per_k = 0;
        balanced.p95_latency_ms = 50;
        let mut latency = manifest("latency", PrivacyClass::Local, true, 80);
        latency.input_cost_micros_per_k = 40;
        latency.output_cost_micros_per_k = 10;
        latency.p95_latency_ms = 10;
        let router = Router::new(vec![quality, balanced, latency]);
        let route = |objective| {
            router
                .route(&RouteRequest {
                    mode: CognitiveMode::EvidenceResearch,
                    risk: RiskLevel::Low,
                    requires_tools: true,
                    requires_structured_output: true,
                    allow_remote: false,
                    objective,
                    max_provider_p95_latency_ms: None,
                    max_unit_cost_micros_per_k: None,
                })
                .unwrap()
                .manifest
                .provider_id
        };
        assert_eq!(route(RouteObjective::Quality), "quality");
        assert_eq!(route(RouteObjective::Balanced), "balanced");
        assert_eq!(route(RouteObjective::Latency), "latency");
        assert_eq!(route(RouteObjective::Cost), "balanced");
    }
}
