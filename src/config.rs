use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::{CognitiveMode, RiskLevel};
use crate::provider::{DeterministicProvider, OpenAiCompatibleProvider};
use crate::router::{PrivacyClass, ProviderManifest};
use crate::runtime::ProviderCandidate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub id: String,
    pub adapter: ProviderAdapter,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub privacy: PrivacyClass,
    #[serde(default)]
    pub quality_score: u32,
    #[serde(default)]
    pub input_cost_micros_per_k: u64,
    #[serde(default)]
    pub output_cost_micros_per_k: u64,
    #[serde(default)]
    pub p95_latency_ms: u64,
    #[serde(default = "default_max_risk")]
    pub max_risk: RiskLevel,
    #[serde(default = "default_supported_modes")]
    pub supported_modes: Vec<CognitiveMode>,
    #[serde(default = "default_true")]
    pub supports_tools: bool,
    #[serde(default = "default_true")]
    pub supports_structured_output: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAdapter {
    Deterministic,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    NotRequired,
    Present,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderReadiness {
    pub provider_id: String,
    pub model: String,
    pub adapter: ProviderAdapter,
    pub credential_status: CredentialStatus,
    pub credential_variable: Option<String>,
    pub ready: bool,
    pub configuration_error: Option<String>,
}

fn default_max_risk() -> RiskLevel {
    RiskLevel::Medium
}

fn default_supported_modes() -> Vec<CognitiveMode> {
    vec![
        CognitiveMode::DirectAnswer,
        CognitiveMode::EvidenceResearch,
        CognitiveMode::DecisionAnalysis,
        CognitiveMode::Execute,
        CognitiveMode::Diagnosis,
        CognitiveMode::Escalate,
    ]
}

fn default_true() -> bool {
    true
}

impl AgentConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(ConfigError::Io)?;
        let config: Self = serde_json::from_str(&content).map_err(ConfigError::Json)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.providers.is_empty() {
            return Err(ConfigError::NoProviders);
        }
        let mut identities = BTreeSet::new();
        for provider in &self.providers {
            if provider.id.trim().is_empty() || provider.model.trim().is_empty() {
                return Err(ConfigError::InvalidProvider(
                    "id and model must not be empty".into(),
                ));
            }
            if !identities.insert((provider.id.clone(), provider.model.clone())) {
                return Err(ConfigError::DuplicateProvider {
                    id: provider.id.clone(),
                    model: provider.model.clone(),
                });
            }
            if provider.adapter == ProviderAdapter::Deterministic && provider.id != "deterministic"
            {
                return Err(ConfigError::InvalidProvider(
                    "deterministic adapter id must be 'deterministic'".into(),
                ));
            }
            if provider.adapter == ProviderAdapter::OpenAiCompatible
                && provider
                    .base_url
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
            {
                return Err(ConfigError::InvalidProvider(format!(
                    "provider '{}' requires base_url",
                    provider.id
                )));
            }
            if provider
                .api_key_env
                .as_deref()
                .is_some_and(|name| name.trim().is_empty())
            {
                return Err(ConfigError::InvalidProvider(format!(
                    "provider '{}' has an empty api_key_env",
                    provider.id
                )));
            }
            if provider.adapter == ProviderAdapter::Deterministic && provider.api_key_env.is_some()
            {
                return Err(ConfigError::InvalidProvider(format!(
                    "deterministic provider '{}' cannot declare api_key_env",
                    provider.id
                )));
            }
        }
        Ok(())
    }

    pub fn build_providers(
        &self,
        timeout: Duration,
    ) -> Result<Vec<ProviderCandidate>, ConfigError> {
        self.validate()?;
        self.providers
            .iter()
            .map(|provider| {
                let manifest = ProviderManifest {
                    provider_id: provider.id.clone(),
                    model: provider.model.clone(),
                    privacy: provider.privacy,
                    supported_modes: provider.supported_modes.clone(),
                    supports_tools: provider.supports_tools,
                    supports_structured_output: provider.supports_structured_output,
                    max_risk: provider.max_risk,
                    quality_score: provider.quality_score,
                    input_cost_micros_per_k: provider.input_cost_micros_per_k,
                    output_cost_micros_per_k: provider.output_cost_micros_per_k,
                    p95_latency_ms: provider.p95_latency_ms,
                };
                match provider.adapter {
                    ProviderAdapter::Deterministic => Ok(ProviderCandidate {
                        provider: Arc::new(DeterministicProvider::new()),
                        manifest,
                    }),
                    ProviderAdapter::OpenAiCompatible => {
                        let base_url = provider.base_url.as_deref().ok_or_else(|| {
                            ConfigError::InvalidProvider("missing base_url".into())
                        })?;
                        let api_key = provider
                            .api_key_env
                            .as_deref()
                            .map(|name| {
                                std::env::var(name).map_err(|_| ConfigError::MissingCredential {
                                    provider_id: provider.id.clone(),
                                    variable: name.into(),
                                })
                            })
                            .transpose()?;
                        let model_provider = OpenAiCompatibleProvider::new(
                            provider.id.clone(),
                            provider.model.clone(),
                            base_url,
                            api_key,
                            timeout,
                        )
                        .map_err(|error| ConfigError::Provider(error.to_string()))?;
                        Ok(ProviderCandidate {
                            provider: Arc::new(model_provider),
                            manifest,
                        })
                    }
                }
            })
            .collect()
    }

    pub fn manifests(&self) -> Result<Vec<ProviderManifest>, ConfigError> {
        self.validate()?;
        Ok(self.providers.iter().map(provider_to_manifest).collect())
    }

    pub fn inspect_readiness(&self) -> Result<Vec<ProviderReadiness>, ConfigError> {
        self.validate()?;
        Ok(self
            .providers
            .iter()
            .map(|provider| {
                let (credential_status, credential_variable) = match provider.api_key_env.as_deref()
                {
                    Some(name) if std::env::var(name).is_ok() => {
                        (CredentialStatus::Present, Some(name.into()))
                    }
                    Some(name) => (CredentialStatus::Missing, Some(name.into())),
                    None => (CredentialStatus::NotRequired, None),
                };
                let configuration_error = if provider.adapter == ProviderAdapter::OpenAiCompatible {
                    provider
                        .base_url
                        .as_deref()
                        .and_then(|base_url| {
                            OpenAiCompatibleProvider::new(
                                provider.id.clone(),
                                provider.model.clone(),
                                base_url,
                                provider
                                    .api_key_env
                                    .as_ref()
                                    .map(|_| "readiness-placeholder".into()),
                                Duration::from_secs(1),
                            )
                            .err()
                        })
                        .map(|error| error.to_string())
                } else {
                    None
                };
                ProviderReadiness {
                    provider_id: provider.id.clone(),
                    model: provider.model.clone(),
                    adapter: provider.adapter,
                    credential_status,
                    credential_variable,
                    ready: credential_status != CredentialStatus::Missing
                        && configuration_error.is_none(),
                    configuration_error,
                }
            })
            .collect())
    }
}

fn provider_to_manifest(provider: &ProviderConfig) -> ProviderManifest {
    ProviderManifest {
        provider_id: provider.id.clone(),
        model: provider.model.clone(),
        privacy: provider.privacy,
        supported_modes: provider.supported_modes.clone(),
        supports_tools: provider.supports_tools,
        supports_structured_output: provider.supports_structured_output,
        max_risk: provider.max_risk,
        quality_score: provider.quality_score,
        input_cost_micros_per_k: provider.input_cost_micros_per_k,
        output_cost_micros_per_k: provider.output_cost_micros_per_k,
        p95_latency_ms: provider.p95_latency_ms,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("config must contain at least one provider")]
    NoProviders,
    #[error("invalid provider: {0}")]
    InvalidProvider(String),
    #[error("duplicate provider identity: {id}/{model}")]
    DuplicateProvider { id: String, model: String },
    #[error("provider construction failed: {0}")]
    Provider(String),
    #[error("provider '{provider_id}' requires a value in environment variable {variable}")]
    MissingCredential {
        provider_id: String,
        variable: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AgentConfig {
        AgentConfig {
            providers: vec![ProviderConfig {
                id: "deterministic".into(),
                adapter: ProviderAdapter::Deterministic,
                model: "offline-v1".into(),
                base_url: None,
                api_key_env: None,
                privacy: PrivacyClass::Local,
                quality_score: 1,
                input_cost_micros_per_k: 0,
                output_cost_micros_per_k: 0,
                p95_latency_ms: 0,
                max_risk: RiskLevel::High,
                supported_modes: default_supported_modes(),
                supports_tools: true,
                supports_structured_output: true,
            }],
        }
    }

    #[test]
    fn builds_deterministic_candidate() {
        let candidates = config().build_providers(Duration::from_secs(1)).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].manifest.provider_id, "deterministic");
    }

    #[test]
    fn rejects_duplicate_provider_identity() {
        let mut config = config();
        config.providers.push(config.providers[0].clone());
        assert!(matches!(
            config.validate(),
            Err(ConfigError::DuplicateProvider { .. })
        ));
    }

    #[test]
    fn rejects_openai_adapter_without_base_url() {
        let mut config = config();
        config.providers[0].adapter = ProviderAdapter::OpenAiCompatible;
        config.providers[0].id = "remote".into();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidProvider(_))
        ));
    }

    #[test]
    fn openai_adapter_without_declared_key_allows_local_endpoint() {
        let mut config = config();
        config.providers[0].adapter = ProviderAdapter::OpenAiCompatible;
        config.providers[0].id = "ollama".into();
        config.providers[0].base_url = Some("http://127.0.0.1:11434/v1".into());
        let candidates = config.build_providers(Duration::from_secs(1)).unwrap();
        assert_eq!(candidates[0].manifest.provider_id, "ollama");
    }

    #[test]
    fn missing_declared_credential_fails_before_runtime() {
        let mut config = config();
        config.providers[0].adapter = ProviderAdapter::OpenAiCompatible;
        config.providers[0].id = "remote".into();
        config.providers[0].base_url = Some("https://example.invalid/v1".into());
        config.providers[0].api_key_env =
            Some("FIRST_PRINCIPLES_AGENT_TEST_MISSING_KEY_8B7E".into());
        assert!(matches!(
            config.build_providers(Duration::from_secs(1)),
            Err(ConfigError::MissingCredential { .. })
        ));
    }

    #[test]
    fn readiness_reports_credentials_without_loading_providers() {
        let mut config = config();
        config.providers[0].adapter = ProviderAdapter::OpenAiCompatible;
        config.providers[0].id = "remote".into();
        config.providers[0].base_url = Some("https://example.invalid/v1".into());
        config.providers[0].api_key_env = Some("FIRST_PRINCIPLES_AGENT_READINESS_TEST".into());
        let readiness = config.inspect_readiness().unwrap();
        assert_eq!(readiness.len(), 1);
        assert_eq!(readiness[0].credential_status, CredentialStatus::Missing);
        assert!(!readiness[0].ready);
        assert!(
            !readiness[0]
                .credential_variable
                .as_deref()
                .unwrap()
                .contains("secret")
        );
    }

    #[test]
    fn readiness_rejects_remote_http_with_declared_credentials() {
        let mut config = config();
        config.providers[0].adapter = ProviderAdapter::OpenAiCompatible;
        config.providers[0].id = "remote".into();
        config.providers[0].base_url = Some("http://example.invalid/v1".into());
        config.providers[0].api_key_env = Some("FIRST_PRINCIPLES_AGENT_HTTP_READINESS_TEST".into());
        let readiness = config.inspect_readiness().unwrap();
        assert!(!readiness[0].ready);
        assert!(
            readiness[0]
                .configuration_error
                .as_deref()
                .unwrap()
                .contains("require HTTPS")
        );
    }
}
