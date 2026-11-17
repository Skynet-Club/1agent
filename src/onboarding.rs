//! 0.98 first-run setup and status projection.
//!
//! This module deliberately orchestrates existing primitives (`runtime_home`,
//! provider discovery, 0600 key files, `doctor`) instead of adding a second
//! runtime or policy path.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::config::{
    AgentConfig, PermissionConfig, PermissionPreset, ProviderAdapter, ProviderConfig,
};
use crate::domain::{PermissionSet, StopReason};
use crate::journal::{JournalEventKind, read_events};
use crate::provider::{
    ModelProvider, OpenAiCompatibleProvider, ProviderErrorCode, list_provider_models,
};
use crate::runtime_home::{DoctorStatus, default_home, doctor, init, is_initialized};

pub const SETUP_SCHEMA: &str = "1agent.setup/v1";
pub const SETUP_STATE_SCHEMA: &str = "1agent.setup.state.v1";
pub const STATUS_SCHEMA: &str = "1agent.status/v1";
pub const CLI_SCHEMA: &str = "1agent.cli/v1";

const PROVIDER_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const SMOKE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct SetupOptions {
    pub root: PathBuf,
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub permission_preset: Option<PermissionPreset>,
    pub workspace: Option<PathBuf>,
    pub read_roots: Vec<PathBuf>,
    /// Models already discovered by the interactive wizard. When present,
    /// `run_setup` reuses them instead of issuing a second probe.
    pub probed_models: Option<Vec<String>>,
    pub check: bool,
    pub probe: bool,
    pub non_interactive: bool,
    pub yes: bool,
    pub resume: bool,
    pub force: bool,
    pub skip_probe: bool,
    pub skip_smoke: bool,
    pub json: bool,
}

impl SetupOptions {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            provider: None,
            base_url: None,
            model: None,
            api_key: None,
            permission_preset: None,
            workspace: None,
            read_roots: Vec::new(),
            probed_models: None,
            check: false,
            probe: false,
            non_interactive: false,
            yes: false,
            resume: false,
            force: false,
            skip_probe: false,
            skip_smoke: false,
            json: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetupError {
    pub code: &'static str,
    pub message: String,
    pub next: String,
    pub details: Option<Value>,
}

impl SetupError {
    pub fn new(code: &'static str, message: impl Into<String>, next: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            next: next.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SetupError {}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetupCheckStatus {
    Ok,
    Missing,
    Warn,
    Skipped,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupCheck {
    pub id: String,
    pub status: SetupCheckStatus,
    pub detail: String,
}

impl SetupCheck {
    fn new(id: &str, status: SetupCheckStatus, detail: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            status,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderSummary {
    pub id: String,
    pub model: String,
    pub adapter: String,
    pub credential: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionSummary {
    pub preset: String,
    pub allow_write: bool,
    pub allow_network: bool,
    pub read_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetupReport {
    pub ok: bool,
    pub root: PathBuf,
    pub config: PathBuf,
    pub provider: Option<ProviderSummary>,
    pub permissions: Option<PermissionSummary>,
    pub checks: Vec<SetupCheck>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub model: String,
    pub adapter: String,
    pub credential: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionStatus {
    pub preset: String,
    pub allow_write: bool,
    pub allow_network: bool,
    pub read_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LastRunStatus {
    pub journal: String,
    pub status: String,
    pub finished_at_unix_ms: Option<u64>,
    pub cost_micros: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusReport {
    pub schema: &'static str,
    pub ok: bool,
    pub version: String,
    pub platform: PlatformInfo,
    pub home: PathBuf,
    pub config: PathBuf,
    pub provider: Option<ProviderStatus>,
    pub permissions: Option<PermissionStatus>,
    pub last_run: Option<LastRunStatus>,
    pub next: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupState {
    schema: String,
    current_step: String,
    #[serde(default)]
    completed: Vec<String>,
    #[serde(default)]
    config_hash: Option<String>,
    #[serde(default)]
    updated_at_unix_ms: u64,
    #[serde(default)]
    last_error: Option<String>,
}

impl SetupState {
    fn new() -> Self {
        Self {
            schema: SETUP_STATE_SCHEMA.into(),
            current_step: "preflight".into(),
            completed: Vec::new(),
            config_hash: None,
            updated_at_unix_ms: now_ms(),
            last_error: None,
        }
    }

    fn mark(&mut self, step: &str) {
        if !self.completed.iter().any(|existing| existing == step) {
            self.completed.push(step.into());
        }
        self.current_step = step.into();
        self.updated_at_unix_ms = now_ms();
    }

    fn has(&self, step: &str) -> bool {
        self.completed.iter().any(|existing| existing == step)
    }
}

struct ProviderSpec {
    id: String,
    adapter: ProviderAdapter,
    base_url: Option<String>,
    model: Option<String>,
    key_required: bool,
    key_env: Option<&'static str>,
    deterministic: bool,
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join("config.json")
}

pub fn setup_state_path(root: &Path) -> PathBuf {
    root.join("setup.json")
}

pub fn provider_key_path(root: &Path, provider_id: &str) -> PathBuf {
    let safe = provider_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    root.join("keys").join(format!("provider-{safe}.key"))
}

pub fn load_config(root: &Path) -> Result<Option<AgentConfig>, SetupError> {
    let path = config_path(root);
    match fs::read_to_string(&path) {
        Ok(content) => {
            let config: AgentConfig = serde_json::from_str(&content).map_err(|error| {
                SetupError::new(
                    "setup.config_invalid",
                    format!("{} is not a valid 1Agent config: {error}", path.display()),
                    "1agent setup --force",
                )
            })?;
            config.validate().map_err(|error| {
                SetupError::new(
                    "setup.config_invalid",
                    format!("{} failed validation: {error}", path.display()),
                    "1agent setup --force",
                )
            })?;
            Ok(Some(config))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(SetupError::new(
            "setup.config_unreadable",
            format!("cannot read {}: {error}", path.display()),
            "check file permissions, then run `1agent setup --check`",
        )),
    }
}

pub fn save_config(root: &Path, config: &AgentConfig) -> Result<(), SetupError> {
    config.validate().map_err(|error| {
        SetupError::new(
            "setup.config_invalid",
            format!("config failed validation: {error}"),
            "1agent setup --force",
        )
    })?;
    let bytes = serde_json::to_vec_pretty(config).map_err(|error| {
        SetupError::new(
            "setup.config_invalid",
            format!("cannot serialize config: {error}"),
            "1agent setup --force",
        )
    })?;
    write_private_atomic(&config_path(root), &bytes)
}

pub async fn run_setup(options: SetupOptions) -> Result<SetupReport, SetupError> {
    let root = options.root.clone();
    let state_path = setup_state_path(&root);
    let existing_state = load_state(&state_path);
    let existing_config = load_config(&root)?;
    let mut checks = Vec::new();

    if !is_initialized(&root) && options.check {
        checks.push(SetupCheck::new(
            "home",
            SetupCheckStatus::Missing,
            format!("{} is not initialized", root.display()),
        ));
        return Ok(not_ready(root, checks));
    }

    let spec = resolve_provider_spec(&options, existing_config.as_ref())?;
    checks.push(SetupCheck::new(
        "provider",
        SetupCheckStatus::Ok,
        format!(
            "{} ({})",
            spec.id,
            spec.model.as_deref().unwrap_or("<model pending>")
        ),
    ));

    if is_initialized(&root) {
        checks.push(SetupCheck::new(
            "home",
            SetupCheckStatus::Ok,
            "runtime home exists",
        ));
    } else {
        init(&root, now_ms() as u128).map_err(|error| {
            SetupError::new(
                "setup.home_init_failed",
                format!("cannot initialize {}: {error}", root.display()),
                "check the path and parent permissions",
            )
        })?;
        checks.push(SetupCheck::new(
            "home",
            SetupCheckStatus::Ok,
            "runtime home initialized",
        ));
    }

    let key_path = provider_key_path(&root, &spec.id);
    let (key, key_source) = resolve_key(&options, existing_config.as_ref(), &spec, &key_path)?;
    let key_present = key.as_ref().is_some_and(|value| !value.trim().is_empty());
    if !spec.key_required {
        checks.push(SetupCheck::new(
            "credentials",
            SetupCheckStatus::Ok,
            "provider does not require a key",
        ));
    } else if key_present {
        checks.push(SetupCheck::new(
            "credentials",
            SetupCheckStatus::Ok,
            format!("credential present ({key_source})"),
        ));
    } else if options.check {
        checks.push(SetupCheck::new(
            "credentials",
            SetupCheckStatus::Missing,
            "provider key is missing",
        ));
    } else {
        return Err(SetupError::new(
            "setup.missing_api_key",
            format!("provider '{}' requires an API key", spec.id),
            "1agent setup --resume --api-key-stdin",
        ));
    }

    let wizard_probe = options.probed_models.is_some();
    let mut probed_models = options.probed_models.clone().unwrap_or_default();
    if wizard_probe {
        checks.push(SetupCheck::new(
            "provider_probe",
            SetupCheckStatus::Ok,
            format!(
                "{} model(s) advertised (verified by setup wizard)",
                probed_models.len()
            ),
        ));
    } else if !spec.deterministic
        && !options.skip_probe
        && (!options.check || options.probe)
        && (key_present || !spec.key_required)
    {
        match list_provider_models(
            spec.base_url.as_deref().unwrap_or_default(),
            key.as_deref().unwrap_or_default(),
            PROVIDER_PROBE_TIMEOUT,
        )
        .await
        {
            Ok(models) => probed_models = models,
            Err(error) if options.check => checks.push(SetupCheck::new(
                "provider_probe",
                SetupCheckStatus::Missing,
                format!("model discovery failed: {error}"),
            )),
            Err(error) => {
                let (code, next) = match error.code {
                    ProviderErrorCode::Authentication => (
                        "setup.provider_auth",
                        "re-enter the key with `1agent setup --resume`",
                    ),
                    ProviderErrorCode::Timeout | ProviderErrorCode::Network => (
                        "setup.provider_unreachable",
                        "check the endpoint, then run `1agent model test`",
                    ),
                    _ => (
                        "setup.provider_probe_failed",
                        "check the base URL, then run `1agent setup --resume`",
                    ),
                };
                return Err(SetupError::new(
                    code,
                    format!("model discovery failed: {error}"),
                    next,
                ));
            }
        }
        if !probed_models.is_empty() {
            checks.push(SetupCheck::new(
                "provider_probe",
                SetupCheckStatus::Ok,
                format!("{} model(s) advertised", probed_models.len()),
            ));
        }
    } else {
        checks.push(SetupCheck::new(
            "provider_probe",
            SetupCheckStatus::Skipped,
            if spec.deterministic {
                "deterministic provider"
            } else if options.skip_probe {
                "--skip-probe"
            } else {
                "check mode"
            },
        ));
    }

    let configured_model = existing_config
        .as_ref()
        .and_then(|config| config.default_provider())
        .filter(|provider| provider.id == spec.id)
        .map(|provider| provider.model.clone());
    let model = if spec.deterministic {
        // The deterministic provider advertises exactly one identity; do not
        // let a guessed `--model` produce a config that fails at run time.
        spec.model.clone().or_else(|| Some("offline-v1".into()))
    } else {
        resolve_setup_model(
            options.model.clone(),
            configured_model,
            spec.model.clone(),
            &probed_models,
        )
    };
    let Some(model) = model.filter(|value| !value.trim().is_empty()) else {
        if options.check {
            checks.push(SetupCheck::new(
                "model",
                SetupCheckStatus::Missing,
                "no model selected",
            ));
            return Ok(not_ready(root, checks));
        }
        return Err(SetupError::new(
            "setup.model_required",
            "no model selected",
            "pass `--model <id>` or run with the provider reachable",
        ));
    };
    if !probed_models.is_empty() && !probed_models.iter().any(|advertised| advertised == &model) {
        let detail = format!("{model} is not advertised by the provider");
        if options.check {
            checks.push(SetupCheck::new("model", SetupCheckStatus::Missing, detail));
            return Ok(not_ready(root, checks));
        }
        checks.push(SetupCheck::new("model", SetupCheckStatus::Warn, detail));
    } else {
        checks.push(SetupCheck::new(
            "model",
            SetupCheckStatus::Ok,
            model.clone(),
        ));
    }

    let workspace = options
        .workspace
        .clone()
        .or_else(|| {
            existing_config
                .as_ref()
                .and_then(|config| config.workspace.clone())
        })
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| {
            SetupError::new(
                "setup.workspace_missing",
                "cannot determine the current workspace",
                "pass `--workspace <path>`",
            )
        })?;
    let read_roots = if options.read_roots.is_empty() {
        existing_config
            .as_ref()
            .and_then(|config| config.permissions.as_ref())
            .map(|permissions| permissions.read_roots.clone())
            .filter(|roots| !roots.is_empty())
            .unwrap_or_else(|| vec![workspace.clone()])
    } else {
        options.read_roots.clone()
    };
    let preset = options
        .permission_preset
        .or_else(|| {
            existing_config
                .as_ref()
                .and_then(|config| config.permissions.as_ref())
                .map(|permissions| permissions.preset)
        })
        .unwrap_or_default();
    let allow_network = existing_config
        .as_ref()
        .and_then(|config| config.permissions.as_ref())
        .map(|permissions| permissions.allow_network)
        .unwrap_or(true);
    let permissions = PermissionConfig {
        preset,
        read_roots: read_roots.clone(),
        allow_network,
    };
    let resolved = permissions
        .preset
        .permission_set(Some(&workspace), &read_roots, allow_network);
    checks.push(SetupCheck::new(
        "permissions",
        SetupCheckStatus::Ok,
        format!(
            "{} (write: {}, network: {})",
            preset.as_str(),
            resolved.allow_write,
            resolved.allow_network
        ),
    ));

    if workspace.is_dir() {
        checks.push(SetupCheck::new(
            "workspace",
            SetupCheckStatus::Ok,
            workspace.display().to_string(),
        ));
    } else if options.check {
        checks.push(SetupCheck::new(
            "workspace",
            SetupCheckStatus::Missing,
            format!("{} is not a directory", workspace.display()),
        ));
    } else {
        return Err(SetupError::new(
            "setup.workspace_missing",
            format!("workspace {} is not a directory", workspace.display()),
            "pass `--workspace <existing path>`",
        ));
    }
    for root in &read_roots {
        if !root.is_dir() {
            checks.push(SetupCheck::new(
                "read_root",
                SetupCheckStatus::Missing,
                format!("{} is not a directory", root.display()),
            ));
        }
    }

    let workspace = absolute_existing(&workspace)?;
    let read_roots = read_roots
        .iter()
        .map(|path| absolute_existing(path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut config = existing_config.unwrap_or_else(|| AgentConfig {
        schema_version: 3,
        default_provider: None,
        workspace: None,
        permissions: None,
        appearance: None,
        providers: Vec::new(),
    });
    config.schema_version = 3;
    config.default_provider = Some(spec.id.clone());
    config.workspace = Some(workspace.clone());
    config.permissions = Some(PermissionConfig {
        preset,
        read_roots: read_roots.clone(),
        allow_network,
    });
    let provider_config = build_provider_config(&spec, &model, &key_path, key_present)?;
    if let Some(position) = config
        .providers
        .iter()
        .position(|provider| provider.id == spec.id)
    {
        config.providers[position] = provider_config;
    } else {
        config.providers.push(provider_config);
    }
    config.validate().map_err(|error| {
        SetupError::new(
            "setup.config_invalid",
            format!("generated config failed validation: {error}"),
            "1agent setup --force",
        )
    })?;
    let config_bytes = serde_json::to_vec_pretty(&config).map_err(|error| {
        SetupError::new(
            "setup.config_invalid",
            format!("cannot serialize config: {error}"),
            "1agent setup --force",
        )
    })?;
    let config_hash = hash_bytes(&config_bytes);
    let existing_bytes = fs::read(config_path(&root)).ok();
    let config_changed = existing_bytes.as_deref() != Some(config_bytes.as_slice());

    if options.check {
        if config_changed {
            checks.push(SetupCheck::new(
                "config",
                SetupCheckStatus::Missing,
                "config.json needs to be written or updated",
            ));
        } else {
            checks.push(SetupCheck::new(
                "config",
                SetupCheckStatus::Ok,
                config_path(&root).display().to_string(),
            ));
        }
        if existing_state
            .as_ref()
            .is_some_and(|state| state.has("smoke"))
        {
            checks.push(SetupCheck::new(
                "smoke",
                SetupCheckStatus::Ok,
                "recorded in setup.json",
            ));
        } else {
            checks.push(SetupCheck::new(
                "smoke",
                SetupCheckStatus::Warn,
                "smoke task is unverified",
            ));
        }
        let doctor_report = doctor(&root).map_err(|error| {
            SetupError::new(
                "setup.doctor_failed",
                format!("doctor failed: {error}"),
                "1agent doctor",
            )
        })?;
        checks.push(SetupCheck::new(
            "doctor",
            doctor_check_status(doctor_report.status),
            format!("overall: {:?}", doctor_report.status),
        ));
        let report = not_ready_or_ready(root, checks);
        return Ok(report);
    }

    if let Some(key) = key.as_ref().filter(|_| spec.key_required) {
        let existing_key = read_private_key(&key_path).ok().flatten();
        if existing_key.as_deref() != Some(key.as_str()) {
            write_private_atomic(&key_path, key.as_bytes())?;
        }
    }
    if config_changed {
        if options.force && config_path(&root).exists() {
            let backup = config_path(&root).with_extension(format!("bak-{}", now_ms()));
            let _ = fs::copy(config_path(&root), &backup);
        }
        write_private_atomic(&config_path(&root), &config_bytes)?;
    }

    let mut state = existing_state.unwrap_or_else(SetupState::new);
    state.mark("preflight");
    state.mark("provider");
    state.mark("credentials");
    state.mark("model");
    state.mark("permissions");
    state.mark("workspace");

    let mut smoke_ok = state.has("smoke");
    if !smoke_ok && !options.skip_smoke && !spec.deterministic {
        run_smoke(&spec, key.as_deref()).await?;
        state.mark("smoke");
        smoke_ok = true;
        checks.push(SetupCheck::new(
            "smoke",
            SetupCheckStatus::Ok,
            "Reply with OK",
        ));
    } else {
        let detail = if options.skip_smoke {
            "--skip-smoke"
        } else if spec.deterministic {
            "deterministic provider"
        } else {
            "already recorded in setup.json"
        };
        checks.push(SetupCheck::new("smoke", SetupCheckStatus::Skipped, detail));
    }
    let _ = smoke_ok;

    let doctor_report = doctor(&root).map_err(|error| {
        SetupError::new(
            "setup.doctor_failed",
            format!("doctor failed: {error}"),
            "1agent doctor",
        )
    })?;
    checks.push(SetupCheck::new(
        "doctor",
        doctor_check_status(doctor_report.status),
        format!("overall: {:?}", doctor_report.status),
    ));
    state.mark("doctor");
    state.current_step = "done".into();
    state.config_hash = Some(config_hash);
    state.last_error = None;
    state.updated_at_unix_ms = now_ms();
    write_private_atomic(
        &state_path,
        &serde_json::to_vec_pretty(&state).map_err(|error| {
            SetupError::new(
                "setup.state_invalid",
                format!("cannot serialize setup state: {error}"),
                "1agent setup --resume",
            )
        })?,
    )?;

    Ok(SetupReport {
        ok: true,
        root: root.clone(),
        config: config_path(&root),
        provider: Some(provider_summary(
            &spec.id,
            &model,
            spec.adapter,
            if !spec.key_required {
                "not_required"
            } else if key_present {
                "present"
            } else {
                "missing"
            },
        )),
        permissions: Some(permission_summary(preset, &resolved)),
        checks,
        next: Some("1agent run --task \"...\"".into()),
    })
}

/// Pick the model id in the operator's order of preference: explicit `--model`,
/// then the already configured model, then whatever the provider advertises
/// first. Never lets an advertised model replace an existing choice.
fn select_model(
    explicit: Option<String>,
    configured: Option<String>,
    advertised: &[String],
) -> Option<String> {
    explicit
        .or(configured)
        .or_else(|| advertised.first().cloned())
}

/// The provider preset has a fallback model id (for example
/// `deepseek-chat`), but that is not a user choice. It must lose to the
/// provider's advertised list; otherwise a fresh setup would pin a stale
/// default instead of selecting an available model.
fn resolve_setup_model(
    explicit: Option<String>,
    configured: Option<String>,
    provider_default: Option<String>,
    advertised: &[String],
) -> Option<String> {
    select_model(explicit, configured, advertised).or(provider_default)
}

fn not_ready(root: PathBuf, checks: Vec<SetupCheck>) -> SetupReport {
    not_ready_or_ready(root, checks)
}

fn not_ready_or_ready(root: PathBuf, checks: Vec<SetupCheck>) -> SetupReport {
    let ok = checks.iter().all(|check| {
        !matches!(
            check.status,
            SetupCheckStatus::Missing | SetupCheckStatus::Fail
        )
    });
    let next = if ok {
        Some("1agent run --task \"...\"".into())
    } else {
        Some("1agent setup --resume".into())
    };
    SetupReport {
        ok,
        root: root.clone(),
        config: config_path(&root),
        provider: None,
        permissions: None,
        checks,
        next,
    }
}

fn resolve_provider_spec(
    options: &SetupOptions,
    existing: Option<&AgentConfig>,
) -> Result<ProviderSpec, SetupError> {
    let requested = options
        .provider
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase());
    let existing_provider = existing.and_then(|config| config.default_provider());
    let name = match requested.as_deref() {
        Some(name) => name.to_string(),
        None => existing_provider
            .map(|provider| provider.id.clone())
            .ok_or_else(|| {
                SetupError::new(
                    "setup.missing_provider",
                    "setup needs a provider in non-interactive mode",
                    "1agent setup --non-interactive --provider deepseek --model deepseek-chat",
                )
            })?,
    };
    let base_url = options.base_url.clone().or_else(|| {
        existing_provider
            .filter(|provider| provider.id == name)
            .and_then(|provider| provider.base_url.clone())
    });
    let model = options.model.clone().or_else(|| {
        existing_provider
            .filter(|provider| provider.id == name)
            .map(|provider| provider.model.clone())
    });
    let (id, adapter, default_base_url, default_model, key_required, key_env, deterministic) =
        match name.as_str() {
            "deepseek" => (
                "deepseek",
                ProviderAdapter::OpenAiCompatible,
                Some("https://api.deepseek.com/v1"),
                Some("deepseek-chat"),
                true,
                Some("DEEPSEEK_API_KEY"),
                false,
            ),
            "openai-compatible" | "openai" => {
                let base = base_url.clone().ok_or_else(|| {
                    SetupError::new(
                        "setup.missing_base_url",
                        "openai-compatible requires --base-url",
                        "1agent setup --non-interactive --provider openai-compatible --base-url https://... --model ...",
                    )
                })?;
                let required = !is_loopback_url(&base);
                (
                    "openai-compatible",
                    ProviderAdapter::OpenAiCompatible,
                    None,
                    None,
                    required,
                    Some("AGENT_API_KEY"),
                    false,
                )
            }
            "ollama" => (
                "ollama",
                ProviderAdapter::OpenAiCompatible,
                Some("http://127.0.0.1:11434/v1"),
                None,
                false,
                None,
                false,
            ),
            "deterministic" | "mock" => (
                "deterministic",
                ProviderAdapter::Deterministic,
                None,
                Some("offline-v1"),
                false,
                None,
                true,
            ),
            other => {
                return Err(SetupError::new(
                    "setup.unknown_provider",
                    format!("unknown provider '{other}'"),
                    "use deepseek, openai-compatible, ollama, or deterministic (tests only)",
                ));
            }
        };
    let _ = name;
    let model = if deterministic {
        Some("offline-v1".to_string())
    } else {
        model.or_else(|| default_model.map(str::to_string))
    };
    Ok(ProviderSpec {
        id: id.into(),
        adapter,
        base_url: base_url.or_else(|| default_base_url.map(str::to_string)),
        model,
        key_required,
        key_env,
        deterministic,
    })
}

fn resolve_key(
    options: &SetupOptions,
    existing: Option<&AgentConfig>,
    spec: &ProviderSpec,
    key_path: &Path,
) -> Result<(Option<String>, &'static str), SetupError> {
    if let Some(key) = options.api_key.as_ref() {
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err(SetupError::new(
                "setup.invalid_api_key",
                "API key from stdin is empty",
                "1agent setup --resume --api-key-stdin",
            ));
        }
        return Ok((Some(key), "stdin"));
    }
    if let Some(provider) = existing
        .and_then(|config| config.default_provider())
        .filter(|provider| provider.id == spec.id)
    {
        if let Some(path) = &provider.api_key_file
            && let Ok(Some(key)) = read_private_key(path)
        {
            return Ok((Some(key), "existing key file"));
        }
        if let Some(name) = &provider.api_key_env
            && let Ok(key) = std::env::var(name)
            && !key.trim().is_empty()
        {
            return Ok((Some(key.trim().to_string()), "environment"));
        }
    }
    if let Ok(Some(key)) = read_private_key(key_path) {
        return Ok((Some(key), "existing key file"));
    }
    if let Some(name) = spec.key_env
        && let Ok(key) = std::env::var(name)
        && !key.trim().is_empty()
    {
        return Ok((Some(key.trim().to_string()), "environment"));
    }
    Ok((None, "missing"))
}

fn build_provider_config(
    spec: &ProviderSpec,
    model: &str,
    key_path: &Path,
    key_present: bool,
) -> Result<ProviderConfig, SetupError> {
    let adapter = match spec.adapter {
        ProviderAdapter::Deterministic => "deterministic",
        ProviderAdapter::OpenAiCompatible => "open_ai_compatible",
    };
    let value = serde_json::json!({
        "id": spec.id,
        "adapter": adapter,
        "model": model,
        "base_url": spec.base_url,
        "api_key_env": spec.key_env,
        "api_key_file": if key_present { Some(key_path.display().to_string()) } else { None },
    });
    serde_json::from_value(value).map_err(|error| {
        SetupError::new(
            "setup.config_invalid",
            format!("cannot build provider config: {error}"),
            "1agent setup --resume",
        )
    })
}

async fn run_smoke(spec: &ProviderSpec, api_key: Option<&str>) -> Result<(), SetupError> {
    let base_url = spec.base_url.as_deref().ok_or_else(|| {
        SetupError::new(
            "setup.smoke_failed",
            "provider has no base URL",
            "1agent setup --resume",
        )
    })?;
    let provider = OpenAiCompatibleProvider::new(
        spec.id.clone(),
        spec.model.clone().unwrap_or_else(|| "smoke".into()),
        base_url,
        api_key.map(str::to_string),
        SMOKE_TIMEOUT,
    )
    .map_err(|error| {
        SetupError::new(
            "setup.smoke_failed",
            format!("cannot construct provider: {error}"),
            "1agent model test",
        )
    })?;
    let cancellation = CancellationToken::new();
    let result = tokio::time::timeout(
        SMOKE_TIMEOUT,
        provider.complete_text(
            "You are a 1Agent setup smoke test. Reply with the single word OK.",
            "Reply with OK.",
            &cancellation,
        ),
    )
    .await;
    match result {
        Ok(Ok(text)) if !text.trim().is_empty() => Ok(()),
        Ok(Ok(_)) => Err(SetupError::new(
            "setup.smoke_failed",
            "provider returned an empty answer",
            "1agent model test",
        )),
        Ok(Err(error)) => Err(SetupError::new(
            "setup.smoke_failed",
            format!("provider call failed: {error}"),
            "1agent model test",
        )),
        Err(_) => Err(SetupError::new(
            "setup.smoke_failed",
            "provider call timed out after 30s",
            "1agent model test",
        )),
    }
}

pub fn status(root: &Path) -> StatusReport {
    let config_path = config_path(root);
    let (config, error) = match load_config(root) {
        Ok(Some(config)) => (Some(config), None),
        Ok(None) => (None, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let provider = config.as_ref().and_then(|config| {
        config.default_provider().map(|provider| ProviderStatus {
            id: provider.id.clone(),
            model: provider.model.clone(),
            adapter: adapter_label(provider.adapter),
            credential: credential_label(provider),
        })
    });
    let permissions = config.as_ref().and_then(|config| {
        let resolved = config.resolved_permissions()?;
        Some(PermissionStatus {
            preset: config
                .permissions
                .as_ref()
                .map(|permissions| permissions.preset.as_str().to_string())
                .unwrap_or_else(|| "default".into()),
            allow_write: resolved.allow_write,
            allow_network: resolved.allow_network,
            read_roots: resolved.read_roots,
        })
    });
    let ready = is_initialized(root)
        && config.is_some()
        && provider
            .as_ref()
            .is_some_and(|provider| provider.credential != "missing");
    let next = if ready {
        Some("1agent run --task \"...\"".to_string())
    } else {
        Some("1agent setup".to_string())
    };
    StatusReport {
        schema: STATUS_SCHEMA,
        ok: ready,
        version: env!("CARGO_PKG_VERSION").into(),
        platform: PlatformInfo {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
        },
        home: root.to_path_buf(),
        config: config_path,
        provider,
        permissions,
        last_run: last_run(root),
        next,
        error,
    }
}

fn last_run(root: &Path) -> Option<LastRunStatus> {
    let journal = newest_journal(&root.join("journals"))?;
    let events = read_events(&journal).ok()?;
    let last = events.last()?;
    let status = events
        .iter()
        .rev()
        .find_map(|event| match &event.event {
            JournalEventKind::RunStopped { reason } => Some(stop_reason_label(*reason)),
            _ => None,
        })
        .unwrap_or("unknown");
    Some(LastRunStatus {
        journal: journal.display().to_string(),
        status: status.into(),
        finished_at_unix_ms: Some(last.timestamp_unix_ms as u64),
        cost_micros: None,
    })
}

fn newest_journal(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("jsonl"))
        .max_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        })
}

fn stop_reason_label(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Completed => "completed",
        StopReason::NeedsEvidence => "needs_evidence",
        StopReason::NeedsAuthorization => "needs_authorization",
        StopReason::BudgetExhausted => "budget_exhausted",
        StopReason::Cancelled => "cancelled",
        StopReason::ProviderError => "provider_error",
        StopReason::ToolFailure => "tool_failure",
        StopReason::InvalidOutput => "invalid_output",
        StopReason::InternalError => "internal_error",
    }
}

pub fn credential_label(provider: &ProviderConfig) -> String {
    if provider.adapter == ProviderAdapter::Deterministic {
        return "not_required".into();
    }
    if provider
        .api_key_file
        .as_deref()
        .is_some_and(crate::config::credential_file_is_private)
    {
        return "present".into();
    }
    if provider
        .api_key_env
        .as_deref()
        .is_some_and(|name| std::env::var(name).is_ok())
    {
        return "present".into();
    }
    if provider.id == "ollama" || provider.base_url.as_deref().is_some_and(is_loopback_url) {
        return "not_required".into();
    }
    "missing".into()
}

fn provider_summary(
    id: &str,
    model: &str,
    adapter: ProviderAdapter,
    credential: &str,
) -> ProviderSummary {
    ProviderSummary {
        id: id.into(),
        model: model.into(),
        adapter: adapter_label(adapter),
        credential: credential.into(),
    }
}

fn permission_summary(preset: PermissionPreset, resolved: &PermissionSet) -> PermissionSummary {
    PermissionSummary {
        preset: preset.as_str().into(),
        allow_write: resolved.allow_write,
        allow_network: resolved.allow_network,
        read_roots: resolved.read_roots.clone(),
    }
}

fn adapter_label(adapter: ProviderAdapter) -> String {
    match adapter {
        ProviderAdapter::Deterministic => "deterministic".into(),
        ProviderAdapter::OpenAiCompatible => "open_ai_compatible".into(),
    }
}

fn doctor_check_status(status: DoctorStatus) -> SetupCheckStatus {
    match status {
        DoctorStatus::Ok => SetupCheckStatus::Ok,
        DoctorStatus::Warn => SetupCheckStatus::Warn,
        DoctorStatus::Fail => SetupCheckStatus::Fail,
    }
}

fn load_state(path: &Path) -> Option<SetupState> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_private_key(path: &Path) -> Result<Option<String>, SetupError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(SetupError::new(
                "setup.key_unreadable",
                format!("cannot read {}: {error}", path.display()),
                "check file permissions",
            ));
        }
    };
    if !metadata.file_type().is_file() {
        return Err(SetupError::new(
            "setup.key_insecure",
            format!("{} is not a regular file", path.display()),
            "remove the symlink and re-enter the key",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(SetupError::new(
                "setup.key_insecure",
                format!("{} is not private (expected 0600)", path.display()),
                "chmod 600 the key file or re-enter the key",
            ));
        }
    }
    let key = fs::read_to_string(path).map_err(|error| {
        SetupError::new(
            "setup.key_unreadable",
            format!("cannot read {}: {error}", path.display()),
            "check file permissions",
        )
    })?;
    let key = key.trim().to_string();
    Ok((!key.is_empty()).then_some(key))
}

/// Returns the key that setup would reuse for this provider, without ever
/// exposing it to the model or printing it. The interactive wizard uses this
/// only to offer "keep existing key" and to run the model probe.
pub(crate) fn stored_key(
    root: &Path,
    provider_id: &str,
    existing: Option<&AgentConfig>,
) -> Option<String> {
    let provider = existing.and_then(|config| {
        config
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
    });
    if let Some(path) = provider.and_then(|provider| provider.api_key_file.as_ref())
        && let Ok(Some(key)) = read_private_key(path)
    {
        return Some(key);
    }
    if let Some(name) = provider.and_then(|provider| provider.api_key_env.as_ref())
        && let Ok(key) = std::env::var(name)
        && !key.trim().is_empty()
    {
        return Some(key.trim().to_string());
    }
    if let Ok(Some(key)) = read_private_key(&provider_key_path(root, provider_id)) {
        return Some(key);
    }
    let env_name = match provider_id {
        "deepseek" => "DEEPSEEK_API_KEY",
        "openai-compatible" | "openai" => "AGENT_API_KEY",
        _ => return None,
    };
    std::env::var(env_name)
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), SetupError> {
    let parent = path.parent().ok_or_else(|| {
        SetupError::new(
            "setup.write_failed",
            format!("{} has no parent directory", path.display()),
            "check the path",
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        SetupError::new(
            "setup.write_failed",
            format!("cannot create {}: {error}", parent.display()),
            "check directory permissions",
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|error| {
            SetupError::new(
                "setup.write_failed",
                format!("cannot set permissions on {}: {error}", parent.display()),
                "check directory permissions",
            )
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("1agent.tmp");
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let mut file = private_options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary)
        .map_err(|error| {
            SetupError::new(
                "setup.write_failed",
                format!("cannot create {}: {error}", temporary.display()),
                "check directory permissions",
            )
        })?;
    file.write_all(bytes).map_err(|error| {
        SetupError::new(
            "setup.write_failed",
            format!("cannot write {}: {error}", temporary.display()),
            "check disk space and permissions",
        )
    })?;
    file.sync_all().map_err(|error| {
        SetupError::new(
            "setup.write_failed",
            format!("cannot fsync {}: {error}", temporary.display()),
            "check disk health",
        )
    })?;
    fs::rename(&temporary, path).map_err(|error| {
        SetupError::new(
            "setup.write_failed",
            format!("cannot replace {}: {error}", path.display()),
            "check directory permissions",
        )
    })?;
    Ok(())
}

fn private_options() -> fs::OpenOptions {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = fs::OpenOptions::new();
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
        options
    }
    #[cfg(not(unix))]
    {
        fs::OpenOptions::new()
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn absolute_existing(path: &Path) -> Result<PathBuf, SetupError> {
    fs::canonicalize(path).map_err(|error| {
        SetupError::new(
            "setup.workspace_missing",
            format!("cannot resolve {}: {error}", path.display()),
            "pass an existing absolute path",
        )
    })
}

pub(crate) fn is_loopback_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    let host = url.host_str().unwrap_or_default();
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn default_root() -> PathBuf {
    default_home()
}

#[cfg(test)]
mod tests {
    use super::{resolve_setup_model, select_model};

    fn advertised() -> Vec<String> {
        vec!["fake-model-a".into(), "fake-model-b".into()]
    }

    #[test]
    fn explicit_model_wins_over_configured_and_advertised() {
        assert_eq!(
            select_model(
                Some("explicit-model".into()),
                Some("configured-model".into()),
                &advertised()
            ),
            Some("explicit-model".into())
        );
    }

    #[test]
    fn configured_model_wins_over_first_advertised() {
        assert_eq!(
            select_model(None, Some("configured-model".into()), &advertised()),
            Some("configured-model".into())
        );
    }

    #[test]
    fn first_advertised_model_is_used_without_explicit_or_configured() {
        assert_eq!(
            select_model(None, None, &advertised()),
            Some("fake-model-a".into())
        );
    }

    #[test]
    fn no_model_when_nothing_is_known() {
        assert_eq!(select_model(None, None, &[]), None);
    }

    #[test]
    fn fresh_provider_prefers_advertised_over_provider_default() {
        assert_eq!(
            resolve_setup_model(None, None, Some("deepseek-chat".into()), &advertised()),
            Some("fake-model-a".into())
        );
    }

    #[test]
    fn provider_default_is_used_only_without_advertised_models() {
        assert_eq!(
            resolve_setup_model(None, None, Some("deepseek-chat".into()), &[]),
            Some("deepseek-chat".into())
        );
    }

    #[test]
    fn configured_model_still_wins_in_resolve() {
        assert_eq!(
            resolve_setup_model(
                None,
                Some("configured-model".into()),
                Some("deepseek-chat".into()),
                &advertised()
            ),
            Some("configured-model".into())
        );
    }
}
