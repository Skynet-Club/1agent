use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::{CognitiveMode, PermissionSet, RiskLevel};
use crate::provider::{DeterministicProvider, OpenAiCompatibleProvider};
use crate::router::{PrivacyClass, ProviderManifest};
use crate::runtime::ProviderCandidate;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionPreset {
    ReadOnly,
    #[default]
    Safe,
    FullLocal,
}

impl PermissionPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::Safe => "safe",
            Self::FullLocal => "full-local",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "read-only" | "read_only" | "readonly" => Some(Self::ReadOnly),
            "safe" => Some(Self::Safe),
            "full-local" | "full_local" | "full" => Some(Self::FullLocal),
            _ => None,
        }
    }

    /// Resolve a preset to the existing runtime permission gate. This is
    /// deliberately a pure projection: presets are sugar, not a second policy.
    pub fn permission_set(
        self,
        workspace: Option<&Path>,
        read_roots: &[PathBuf],
        allow_network: bool,
    ) -> PermissionSet {
        let roots = if read_roots.is_empty() {
            workspace
                .map(Path::to_path_buf)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            read_roots.to_vec()
        };
        let (allow_write, network) = match self {
            Self::ReadOnly => (false, false),
            Self::Safe => (true, allow_network),
            Self::FullLocal => (true, true),
        };
        PermissionSet {
            read_roots: roots,
            allow_network: network,
            allow_write,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PermissionConfig {
    #[serde(default)]
    pub preset: PermissionPreset,
    #[serde(default)]
    pub read_roots: Vec<PathBuf>,
    #[serde(default)]
    pub allow_network: bool,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub workspace: Option<PathBuf>,
    #[serde(default)]
    pub permissions: Option<PermissionConfig>,
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
    pub api_key_file: Option<PathBuf>,
    #[serde(default)]
    pub reasoning_effort: Option<crate::provider::ReasoningEffort>,
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
        if self.schema_version == 0 || self.schema_version > 2 {
            return Err(ConfigError::InvalidConfigVersion(self.schema_version));
        }
        if self.providers.is_empty() {
            return Err(ConfigError::NoProviders);
        }
        if let Some(default_provider) = &self.default_provider
            && !self
                .providers
                .iter()
                .any(|provider| provider.id == *default_provider)
        {
            return Err(ConfigError::UnknownDefaultProvider(
                default_provider.clone(),
            ));
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
            if provider
                .api_key_file
                .as_deref()
                .is_some_and(|path| path.as_os_str().is_empty())
            {
                return Err(ConfigError::InvalidProvider(format!(
                    "provider '{}' has an empty api_key_file",
                    provider.id
                )));
            }
            if let Some(path) = &provider.api_key_file
                && !path.is_absolute()
            {
                return Err(ConfigError::InvalidProvider(format!(
                    "provider '{}' api_key_file must be absolute",
                    provider.id
                )));
            }
            if provider.adapter == ProviderAdapter::Deterministic
                && (provider.api_key_env.is_some() || provider.api_key_file.is_some())
            {
                return Err(ConfigError::InvalidProvider(format!(
                    "deterministic provider '{}' cannot declare credentials",
                    provider.id
                )));
            }
        }
        Ok(())
    }

    pub fn default_provider(&self) -> Option<&ProviderConfig> {
        self.default_provider
            .as_deref()
            .and_then(|id| self.providers.iter().find(|provider| provider.id == id))
            .or_else(|| self.providers.first())
    }

    /// Resolve the v2 permission block to the runtime gate. `None` means the
    /// config predates presets and existing flag defaults apply.
    pub fn resolved_permissions(&self) -> Option<PermissionSet> {
        self.permissions.as_ref().map(|permissions| {
            permissions.preset.permission_set(
                self.workspace.as_deref(),
                &permissions.read_roots,
                permissions.allow_network,
            )
        })
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
                        let api_key = provider.read_api_key()?;
                        let model_provider = OpenAiCompatibleProvider::new(
                            provider.id.clone(),
                            provider.model.clone(),
                            base_url,
                            api_key,
                            timeout,
                        )
                        .map_err(|error| ConfigError::Provider(error.to_string()))?
                        .with_reasoning_effort(provider.reasoning_effort);
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
                let (credential_status, credential_variable) =
                    if let Some(path) = &provider.api_key_file {
                        let status = if credential_file_is_private(path) {
                            CredentialStatus::Present
                        } else {
                            CredentialStatus::Missing
                        };
                        (status, Some(path.display().to_string()))
                    } else {
                        match provider.api_key_env.as_deref() {
                            Some(name) if std::env::var(name).is_ok() => {
                                (CredentialStatus::Present, Some(name.into()))
                            }
                            Some(name) => (CredentialStatus::Missing, Some(name.into())),
                            None => (CredentialStatus::NotRequired, None),
                        }
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
                                (provider.api_key_env.is_some() || provider.api_key_file.is_some())
                                    .then(|| "readiness-placeholder".into()),
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

impl ProviderConfig {
    /// Resolve the provider credential. A configured key file wins over the
    /// environment variable so setup has one canonical 0600 store.
    pub fn read_api_key(&self) -> Result<Option<String>, ConfigError> {
        if let Some(path) = &self.api_key_file {
            if !credential_file_is_private(path) {
                return Err(ConfigError::InsecureCredentialFile(path.clone()));
            }
            let key = std::fs::read_to_string(path)
                .map_err(|_| ConfigError::MissingCredentialFile(path.clone()))?;
            let key = key.trim().to_string();
            if key.is_empty() {
                return Err(ConfigError::InvalidCredentialFile(path.clone()));
            }
            return Ok(Some(key));
        }
        self.api_key_env
            .as_deref()
            .map(|name| {
                std::env::var(name).map_err(|_| ConfigError::MissingCredential {
                    provider_id: self.id.clone(),
                    variable: name.into(),
                })
            })
            .transpose()
    }
}

pub(crate) fn credential_file_is_private(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return false;
        }
    }
    true
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
    #[error("unsupported config schema version {0}")]
    InvalidConfigVersion(u32),
    #[error("default provider '{0}' is not declared")]
    UnknownDefaultProvider(String),
    #[error("credential file is missing or unreadable: {0}")]
    MissingCredentialFile(PathBuf),
    #[error("credential file must be a private regular file (0600): {0}")]
    InsecureCredentialFile(PathBuf),
    #[error("credential file is empty: {0}")]
    InvalidCredentialFile(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AgentConfig {
        AgentConfig {
            schema_version: 1,
            default_provider: None,
            workspace: None,
            permissions: None,
            providers: vec![ProviderConfig {
                id: "deterministic".into(),
                adapter: ProviderAdapter::Deterministic,
                model: "offline-v1".into(),
                base_url: None,
                api_key_env: None,
                api_key_file: None,
                reasoning_effort: None,
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
    fn permission_presets_map_to_expected_gates() {
        let workspace = Path::new("/work");
        let read_only = PermissionPreset::ReadOnly.permission_set(Some(workspace), &[], false);
        assert!(!read_only.allow_write);
        assert!(!read_only.allow_network);
        assert_eq!(read_only.read_roots, vec![PathBuf::from("/work")]);

        let safe = PermissionPreset::Safe.permission_set(Some(workspace), &[], false);
        assert!(safe.allow_write);
        assert!(!safe.allow_network);

        let safe_network = PermissionPreset::Safe.permission_set(Some(workspace), &[], true);
        assert!(safe_network.allow_network);

        let full = PermissionPreset::FullLocal.permission_set(Some(workspace), &[], false);
        assert!(full.allow_write);
        assert!(full.allow_network);

        assert_eq!(
            PermissionPreset::parse("read-only"),
            Some(PermissionPreset::ReadOnly)
        );
        assert_eq!(
            PermissionPreset::parse("full-local"),
            Some(PermissionPreset::FullLocal)
        );
        assert_eq!(
            PermissionPreset::parse("safe"),
            Some(PermissionPreset::Safe)
        );
        assert_eq!(PermissionPreset::parse("unknown"), None);
    }

    #[test]
    fn v1_config_loads_with_defaults() {
        let config: AgentConfig = serde_json::from_str(
            r#"{"providers":[{"id":"deterministic","adapter":"deterministic","model":"offline-v1"}]}"#,
        )
        .unwrap();
        config.validate().unwrap();
        assert_eq!(config.schema_version, 1);
        assert!(config.default_provider.is_none());
        assert!(config.resolved_permissions().is_none());
    }

    #[test]
    fn v2_config_resolves_default_provider_and_permissions() {
        let config: AgentConfig = serde_json::from_str(
            r#"{
                "schema_version": 2,
                "default_provider": "deterministic",
                "workspace": "/work",
                "permissions": {"preset": "full-local", "read_roots": ["/work"]},
                "providers": [{"id":"deterministic","adapter":"deterministic","model":"offline-v1"}]
            }"#,
        )
        .unwrap();
        config.validate().unwrap();
        assert_eq!(config.default_provider().unwrap().id, "deterministic");
        let permissions = config.resolved_permissions().unwrap();
        assert!(permissions.allow_write);
        assert!(permissions.allow_network);
    }

    #[test]
    fn newer_config_schema_fails_closed() {
        let config: AgentConfig = serde_json::from_str(
            r#"{"schema_version":3,"providers":[{"id":"deterministic","adapter":"deterministic","model":"offline-v1"}]}"#,
        )
        .unwrap();
        assert!(matches!(
            config.validate(),
            Err(ConfigError::InvalidConfigVersion(3))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_private_and_readable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider.key");
        std::fs::write(&path, "sk-test\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let provider: ProviderConfig = serde_json::from_value(serde_json::json!({
            "id": "remote",
            "adapter": "open_ai_compatible",
            "model": "model",
            "base_url": "https://example.invalid/v1",
            "api_key_file": path.display().to_string(),
        }))
        .unwrap();
        assert_eq!(provider.read_api_key().unwrap().as_deref(), Some("sk-test"));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            provider.read_api_key(),
            Err(ConfigError::InsecureCredentialFile(_))
        ));
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
