//! Interactive first-run setup wizard.
//!
//! This module is a CLI surface only: it collects choices and never writes
//! config or keys. `onboarding::run_setup` stays the single writer and policy
//! path. The wizard intentionally prefers menus over free text; the API key
//! (and an explicitly chosen custom endpoint, model or path) are the only
//! values typed by hand.

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::{AgentConfig, PermissionPreset};
use crate::onboarding::{
    SetupCheck, SetupError, SetupOptions, SetupReport, is_loopback_url, stored_key,
};
use crate::provider::list_provider_models;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
/// The setup surface reuses the TUI's military-green palette:
/// mark green, panel text green, panel green and shadow green.
const ACCENT: &str = "\x1b[38;2;63;185;80m";
const ACCENT_BOLD: &str = "\x1b[1;38;2;63;185;80m";
const BODY: &str = "\x1b[38;2;94;158;94m";
const LINE: &str = "\x1b[38;2;74;120;85m";
const SHADOW: &str = "\x1b[38;2;58;92;66m";
const RED: &str = "\x1b[91m";
const YELLOW: &str = "\x1b[93m";

const LOGO: &[&str] = &[
    " ██╗ █████╗  ██████╗ ███████╗███╗   ██╗████████╗",
    "███║██╔══██╗██╔════╝ ██╔════╝████╗  ██║╚══██╔══╝",
    "╚██║███████║██║  ███╗█████╗  ██╔██╗ ██║   ██║   ",
    " ██║██╔══██║██║   ██║██╔══╝  ██║╚██╗██║   ██║   ",
    " ██║██║  ██║╚██████╔╝███████╗██║ ╚████║   ██║   ",
    " ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═══╝   ╚═╝   ",
];

const COMPACT_LOGO: &[&str] = &["1 · A G E N T"];

const PROVIDERS: &[ProviderOption] = &[
    ProviderOption {
        id: "deepseek",
        label: "DeepSeek",
        detail: "recommended · hosted",
    },
    ProviderOption {
        id: "openai-compatible",
        label: "OpenAI-compatible",
        detail: "any OpenAI /v1 endpoint",
    },
    ProviderOption {
        id: "ollama",
        label: "Ollama",
        detail: "local · no key required",
    },
];

#[derive(Debug, Clone, Copy)]
struct ProviderOption {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Provider,
    Endpoint,
    Key,
    Model,
    Permissions,
    Workspace,
    Review,
}

impl Step {
    fn title(self) -> &'static str {
        match self {
            Self::Provider => "Provider",
            Self::Endpoint => "Endpoint",
            Self::Key => "Credential",
            Self::Model => "Model",
            Self::Permissions => "Permissions",
            Self::Workspace => "Workspace",
            Self::Review => "Review",
        }
    }
}

enum StepAction {
    Next,
    Stay,
    Back,
    Cancel,
}

enum ProbeState {
    NotStarted,
    Ready(Vec<String>),
    Failed(String),
    Skipped,
}

struct WizardState {
    root: PathBuf,
    provider: String,
    base_url: Option<String>,
    /// Key just entered in this wizard run. Never rendered.
    api_key: Option<String>,
    /// Key found in the 0600 store or the provider environment. Never rendered.
    stored_key: Option<String>,
    model: Option<String>,
    permission: PermissionPreset,
    workspace: PathBuf,
    probe: ProbeState,
}

impl WizardState {
    fn new(root: &Path, existing: Option<&AgentConfig>, options: &SetupOptions) -> Self {
        let provider = options
            .provider
            .clone()
            .or_else(|| {
                existing
                    .and_then(|config| config.default_provider())
                    .map(|provider| provider.id.clone())
            })
            .unwrap_or_else(|| "deepseek".into());
        let existing_provider = existing.and_then(|config| {
            config
                .providers
                .iter()
                .find(|provider_config| provider_config.id == provider)
        });
        let base_url = options
            .base_url
            .clone()
            .or_else(|| existing_provider.and_then(|provider| provider.base_url.clone()))
            .or_else(|| default_base_url(&provider).map(str::to_string));
        let model = options
            .model
            .clone()
            .or_else(|| existing_provider.map(|provider| provider.model.clone()));
        let permission = options
            .permission_preset
            .or_else(|| {
                existing
                    .and_then(|config| config.permissions.as_ref())
                    .map(|permissions| permissions.preset)
            })
            .unwrap_or_default();
        let workspace = options
            .workspace
            .clone()
            .or_else(|| existing.and_then(|config| config.workspace.clone()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let stored_key = stored_key(root, &provider, existing);
        let api_key = options.api_key.clone();
        Self {
            root: root.to_path_buf(),
            provider,
            base_url,
            api_key,
            stored_key,
            model,
            permission,
            workspace,
            probe: ProbeState::NotStarted,
        }
    }

    fn key_present(&self) -> bool {
        self.api_key
            .as_ref()
            .or(self.stored_key.as_ref())
            .is_some_and(|key| !key.trim().is_empty())
    }

    fn probe_key(&self) -> String {
        self.api_key
            .as_deref()
            .or(self.stored_key.as_deref())
            .unwrap_or_default()
            .to_string()
    }

    fn credential_label(&self) -> &'static str {
        if !requires_key(&self.provider, self.base_url.as_deref()) {
            "not required"
        } else if self.api_key.is_some() {
            "new key (hidden)"
        } else if self.stored_key.is_some() {
            "existing key (reused)"
        } else {
            "missing"
        }
    }

    fn reset_probe(&mut self) {
        self.probe = ProbeState::NotStarted;
    }
}

pub struct WizardChoices {
    pub provider: String,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key: Option<String>,
    pub permission_preset: PermissionPreset,
    pub workspace: PathBuf,
    pub probed_models: Option<Vec<String>>,
    /// The wizard already handled a failed or skipped probe and the user chose
    /// to continue; `run_setup` must not repeat the same probe.
    pub skip_validation_probe: bool,
}

pub enum WizardOutcome {
    Ready(WizardChoices),
    Cancelled,
}

/// Runs the full-screen wizard. The caller must have already checked that
/// stdin and stdout are terminals.
pub async fn run(
    root: &Path,
    existing: Option<&AgentConfig>,
    options: &SetupOptions,
    key_from_stdin: bool,
) -> Result<WizardOutcome> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!("setup wizard requires a TTY");
    }
    let screen = Screen::enter().context("cannot start the setup wizard")?;
    let mut state = WizardState::new(root, existing, options);
    if !welcome(&screen, existing)? {
        return Ok(WizardOutcome::Cancelled);
    }

    let mut index = 0usize;
    loop {
        let steps = visible_steps(&state, options, key_from_stdin);
        if steps.is_empty() {
            break;
        }
        if index >= steps.len() {
            index = steps.len().saturating_sub(1);
        }
        let step = steps[index];
        let action = match step {
            Step::Provider => provider_step(&screen, &mut state, existing, index, steps.len())?,
            Step::Endpoint => endpoint_step(&screen, &mut state, index, steps.len())?,
            Step::Key => key_step(&screen, &mut state, index, steps.len())?,
            Step::Model => {
                model_step(
                    &screen,
                    &mut state,
                    options,
                    key_from_stdin,
                    index,
                    steps.len(),
                )
                .await?
            }
            Step::Permissions => permissions_step(&screen, &mut state, index, steps.len())?,
            Step::Workspace => workspace_step(&screen, &mut state, index, steps.len())?,
            Step::Review => review_step(&screen, &state, options, index, steps.len())?,
        };
        match action {
            StepAction::Next => {
                if step == Step::Review {
                    break;
                }
                index += 1;
            }
            StepAction::Stay => {}
            StepAction::Back => {
                if index == 0 {
                    return Ok(WizardOutcome::Cancelled);
                }
                index -= 1;
            }
            StepAction::Cancel => return Ok(WizardOutcome::Cancelled),
        }
    }

    drop(screen);
    let model = state.model.unwrap_or_default();
    if model.trim().is_empty() {
        bail!("setup wizard did not select a model");
    }
    let skip_validation_probe = matches!(state.probe, ProbeState::Failed(_) | ProbeState::Skipped);
    let probed_models = match state.probe {
        ProbeState::Ready(models) => Some(models),
        ProbeState::NotStarted | ProbeState::Failed(_) | ProbeState::Skipped => None,
    };
    Ok(WizardOutcome::Ready(WizardChoices {
        provider: state.provider,
        base_url: state.base_url,
        model,
        api_key: state.api_key,
        permission_preset: state.permission,
        workspace: state.workspace,
        probed_models,
        skip_validation_probe,
    }))
}

fn visible_steps(state: &WizardState, options: &SetupOptions, key_from_stdin: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    if options.provider.is_none() {
        steps.push(Step::Provider);
    }
    if state.provider == "openai-compatible" && options.base_url.is_none() {
        steps.push(Step::Endpoint);
    }
    if requires_key(&state.provider, state.base_url.as_deref())
        && options.api_key.is_none()
        && !key_from_stdin
    {
        steps.push(Step::Key);
    }
    if options.model.is_none() {
        steps.push(Step::Model);
    }
    if options.permission_preset.is_none() {
        steps.push(Step::Permissions);
    }
    if options.workspace.is_none() {
        steps.push(Step::Workspace);
    }
    steps.push(Step::Review);
    steps
}

fn requires_key(provider: &str, base_url: Option<&str>) -> bool {
    match provider {
        "deepseek" => true,
        "openai-compatible" | "openai" => {
            base_url.is_none_or(|base_url| !is_loopback_url(base_url))
        }
        _ => false,
    }
}

fn default_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "deepseek" => Some("https://api.deepseek.com/v1"),
        "ollama" => Some("http://127.0.0.1:11434/v1"),
        _ => None,
    }
}

fn provider_label(provider: &str) -> &str {
    PROVIDERS
        .iter()
        .find(|option| option.id == provider)
        .map(|option| option.label)
        .unwrap_or(provider)
}

fn permission_detail(preset: PermissionPreset) -> &'static str {
    match preset {
        PermissionPreset::ReadOnly => "read only · no writes, no network",
        PermissionPreset::Safe => "workspace writes · network/exec need approval",
        PermissionPreset::FullLocal => "write + network · irreversible actions still confirm",
    }
}

fn welcome(screen: &Screen, existing: Option<&AgentConfig>) -> Result<bool> {
    loop {
        screen.draw(&welcome_lines(
            screen.width,
            screen.height,
            screen.color,
            existing,
        ))?;
        let key = read_key()?;
        if is_cancel(&key) {
            return Ok(false);
        }
        if key.code == KeyCode::Esc {
            return Ok(false);
        }
        if key.code == KeyCode::Enter {
            return Ok(true);
        }
    }
}

fn welcome_lines(
    width: usize,
    height: usize,
    color: bool,
    existing: Option<&AgentConfig>,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push(rule(width, color));
    lines.push(String::new());
    let logo = if width >= 62 {
        LOGO
    } else if width >= 32 {
        COMPACT_LOGO
    } else {
        &["1AGENT"]
    };
    for line in logo {
        lines.push(center(&paint_logo(line, color), width));
    }
    lines.push(String::new());
    lines.push(welcome_text(
        "EVIDENCE-ACCOUNTABLE AGENT RUNTIME",
        BODY,
        width,
        color,
    ));
    lines.push(welcome_text(
        &format!("v{} · REAL WORLD", env!("CARGO_PKG_VERSION")),
        SHADOW,
        width,
        color,
    ));
    lines.push(String::new());
    if existing.is_some() {
        lines.push(welcome_text(
            "Setup is already configured. Review or change it.",
            BODY,
            width,
            color,
        ));
        if let Some(provider) = existing.and_then(|config| config.default_provider()) {
            lines.push(welcome_text(
                &format!("{} / {}", provider.id, provider.model),
                SHADOW,
                width,
                color,
            ));
        }
    } else {
        let narrow = width < 56;
        lines.push(welcome_text(
            if narrow {
                "Welcome. Let's get started."
            } else {
                "Welcome. Let's get your first task running."
            },
            BODY,
            width,
            color,
        ));
        lines.push(String::new());
        let bullets: &[&str] = if narrow {
            &[
                "Pick a provider and model",
                "Choose permissions & workspace",
                "Run a short smoke test",
            ]
        } else {
            &[
                "Pick a provider and model",
                "Choose permissions and a workspace",
                "Verify everything with a short smoke run",
            ]
        };
        for item in bullets {
            let item = fit_plain(item, width.saturating_sub(8));
            lines.push(center(
                &format!(
                    "{}  {}",
                    paint("•", ACCENT, color),
                    paint(&item, SHADOW, color)
                ),
                width,
            ));
        }
    }
    lines.push(String::new());
    if width >= 44 {
        lines.push(center(
            &format!(
                "{}   {}",
                paint("Press Enter to begin", ACCENT, color),
                paint("Ctrl-C to quit", SHADOW, color)
            ),
            width,
        ));
    } else {
        lines.push(welcome_text(
            "Enter begin · Ctrl-C quit",
            BODY,
            width,
            color,
        ));
    }
    lines.push(String::new());
    lines.push(rule(width, color));

    let top = height.saturating_sub(lines.len()) / 2;
    let mut padded = vec![String::new(); top.min(6)];
    padded.extend(lines);
    padded
}

fn provider_step(
    screen: &Screen,
    state: &mut WizardState,
    existing: Option<&AgentConfig>,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    let current = PROVIDERS
        .iter()
        .position(|option| option.id == state.provider)
        .unwrap_or(0);
    let mut menu = MenuState::new(PROVIDERS.len(), current);
    loop {
        let items = PROVIDERS
            .iter()
            .map(|option| MenuItem::new(option.label, option.detail))
            .collect::<Vec<_>>();
        let mut lines = step_header(Step::Provider, index, total, screen.width, screen.color);
        lines.extend(menu_lines(
            "How should 1Agent connect?",
            "Choose a provider. You can change it later.",
            &items,
            &menu,
            screen.width,
            screen.color,
        ));
        screen.draw(&lines)?;
        match read_key()? {
            key if is_cancel(&key) => return Ok(StepAction::Cancel),
            key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
            key if is_up(&key) => menu.up(),
            key if is_down(&key) => menu.down(),
            key if key.code == KeyCode::Enter => {
                let selected = PROVIDERS[menu.selected()].id;
                if selected != state.provider {
                    state.provider = selected.to_string();
                    state.base_url = existing
                        .and_then(|config| {
                            config
                                .providers
                                .iter()
                                .find(|provider| provider.id == state.provider)
                        })
                        .and_then(|provider| provider.base_url.clone())
                        .or_else(|| default_base_url(&state.provider).map(str::to_string));
                    state.model = existing
                        .and_then(|config| {
                            config
                                .providers
                                .iter()
                                .find(|provider| provider.id == state.provider)
                        })
                        .map(|provider| provider.model.clone());
                    state.api_key = None;
                    state.stored_key = stored_key(&state.root, &state.provider, existing);
                    state.reset_probe();
                }
                return Ok(StepAction::Next);
            }
            key => {
                if let Some(selected) = numeric_selection(&key, PROVIDERS.len()) {
                    menu.select(selected);
                }
            }
        }
    }
}

fn endpoint_step(
    screen: &Screen,
    state: &mut WizardState,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    let mut items = Vec::new();
    if let Some(current) = state.base_url.as_deref() {
        items.push(MenuItem::new("Keep current endpoint", current).badge("current"));
    }
    let openai = "https://api.openai.com/v1";
    if state.base_url.as_deref() != Some(openai) {
        items.push(MenuItem::new("OpenAI", openai));
    }
    items.push(MenuItem::new("Enter another URL…", "http:// or https://"));
    let mut menu = MenuState::new(items.len(), 0);
    loop {
        let mut lines = step_header(Step::Endpoint, index, total, screen.width, screen.color);
        lines.extend(menu_lines(
            "Where is the OpenAI-compatible endpoint?",
            "Most users can keep the provider default.",
            &items,
            &menu,
            screen.width,
            screen.color,
        ));
        screen.draw(&lines)?;
        match read_key()? {
            key if is_cancel(&key) => return Ok(StepAction::Cancel),
            key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
            key if is_up(&key) => menu.up(),
            key if is_down(&key) => menu.down(),
            key if key.code == KeyCode::Enter => {
                let selected = menu.selected();
                let choice = items[selected].label.clone();
                if choice == "Enter another URL…" {
                    let initial = state.base_url.clone().unwrap_or_default();
                    let action = read_text(
                        screen,
                        &initial,
                        false,
                        |value, error| {
                            let mut lines = step_header(
                                Step::Endpoint,
                                index,
                                total,
                                screen.width,
                                screen.color,
                            );
                            lines.extend(text_field_lines(
                                "Endpoint URL",
                                "Use https:// for hosted providers; http:// is accepted for localhost.",
                                value,
                                error,
                                screen.width,
                                screen.color,
                            ));
                            lines
                        },
                        validate_endpoint,
                    )?;
                    match action {
                        TextAction::Value(value) => {
                            if state.base_url.as_deref() != Some(value.as_str()) {
                                state.base_url = Some(value);
                                state.model = None;
                                state.reset_probe();
                            }
                            return Ok(StepAction::Next);
                        }
                        TextAction::Back => continue,
                        TextAction::Cancel => return Ok(StepAction::Cancel),
                    }
                }
                let value = items[selected].detail.clone();
                if state.base_url.as_deref() != Some(value.as_str()) {
                    state.base_url = Some(value);
                    state.model = None;
                    state.reset_probe();
                }
                return Ok(StepAction::Next);
            }
            key => {
                if let Some(selected) = numeric_selection(&key, items.len()) {
                    menu.select(selected);
                }
            }
        }
    }
}

fn key_step(
    screen: &Screen,
    state: &mut WizardState,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    if state.key_present() {
        let keep_label = if state.api_key.is_some() {
            "Keep the key just entered"
        } else {
            "Keep current key"
        };
        let items = vec![
            MenuItem::new(keep_label, "stored with 0600 permissions").badge("recommended"),
            MenuItem::new("Enter a different key", "input is hidden"),
        ];
        let mut menu = MenuState::new(items.len(), 0);
        loop {
            let mut lines = step_header(Step::Key, index, total, screen.width, screen.color);
            lines.extend(menu_lines(
                &format!("{} credential", provider_label(&state.provider)),
                "The key is stored outside the model context and never printed.",
                &items,
                &menu,
                screen.width,
                screen.color,
            ));
            screen.draw(&lines)?;
            match read_key()? {
                key if is_cancel(&key) => return Ok(StepAction::Cancel),
                key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
                key if is_up(&key) => menu.up(),
                key if is_down(&key) => menu.down(),
                key if key.code == KeyCode::Enter => {
                    if menu.selected() == 0 {
                        return Ok(StepAction::Next);
                    }
                    let action = read_text(
                        screen,
                        "",
                        true,
                        |value, error| {
                            let mut lines =
                                step_header(Step::Key, index, total, screen.width, screen.color);
                            lines.extend(text_field_lines(
                                &format!("{} API key", provider_label(&state.provider)),
                                "Input is hidden. The key is stored with 0600 permissions.",
                                value,
                                error,
                                screen.width,
                                screen.color,
                            ));
                            lines
                        },
                        validate_secret,
                    )?;
                    match action {
                        TextAction::Value(value) => {
                            state.api_key = Some(value);
                            state.reset_probe();
                            return Ok(StepAction::Next);
                        }
                        TextAction::Back => continue,
                        TextAction::Cancel => return Ok(StepAction::Cancel),
                    }
                }
                _ => {}
            }
        }
    }

    let action = read_text(
        screen,
        "",
        true,
        |value, error| {
            let mut lines = step_header(Step::Key, index, total, screen.width, screen.color);
            lines.extend(text_field_lines(
                &format!("{} API key", provider_label(&state.provider)),
                "Input is hidden. The key is stored with 0600 permissions.",
                value,
                error,
                screen.width,
                screen.color,
            ));
            lines
        },
        validate_secret,
    )?;
    match action {
        TextAction::Value(value) => {
            state.api_key = Some(value);
            state.reset_probe();
            Ok(StepAction::Next)
        }
        TextAction::Back => Ok(StepAction::Back),
        TextAction::Cancel => Ok(StepAction::Cancel),
    }
}

async fn model_step(
    screen: &Screen,
    state: &mut WizardState,
    options: &SetupOptions,
    key_from_stdin: bool,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    if (options.skip_probe || key_from_stdin) && matches!(state.probe, ProbeState::NotStarted) {
        state.probe = ProbeState::Skipped;
    }
    if matches!(state.probe, ProbeState::NotStarted) {
        let mut lines = step_header(Step::Model, index, total, screen.width, screen.color);
        lines.push(format!(
            "  {}  Contacting provider…",
            paint("◌", ACCENT, screen.color)
        ));
        lines.push(format!(
            "     {}",
            paint(&endpoint_for(state), SHADOW, screen.color)
        ));
        lines.push(String::new());
        lines.push(format!(
            "  {}",
            paint(
                "This usually takes less than a second.",
                SHADOW,
                screen.color
            )
        ));
        screen.draw(&lines)?;
        state.probe = probe_models(state).await;
    }

    match &state.probe {
        ProbeState::Ready(models) => {
            let items = model_items(models, state.model.as_deref());
            let current = state
                .model
                .as_deref()
                .and_then(|model| items.iter().position(|item| item.key == model))
                .unwrap_or(0);
            let mut menu = MenuState::new(items.len(), current);
            loop {
                let mut lines = step_header(Step::Model, index, total, screen.width, screen.color);
                lines.extend(menu_lines(
                    "Choose a model",
                    "Models come from the provider's /v1/models list.",
                    &items,
                    &menu,
                    screen.width,
                    screen.color,
                ));
                screen.draw(&lines)?;
                match read_key()? {
                    key if is_cancel(&key) => return Ok(StepAction::Cancel),
                    key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
                    key if is_up(&key) => menu.up(),
                    key if is_down(&key) => menu.down(),
                    key if key.code == KeyCode::Enter => {
                        let item = &items[menu.selected()];
                        if item.key == MANUAL_MODEL {
                            match read_model(screen, state, index, total)? {
                                TextAction::Value(value) => {
                                    state.model = Some(value);
                                    return Ok(StepAction::Next);
                                }
                                TextAction::Back => continue,
                                TextAction::Cancel => return Ok(StepAction::Cancel),
                            }
                        }
                        state.model = Some(item.key.clone());
                        return Ok(StepAction::Next);
                    }
                    key => {
                        if let Some(selected) = numeric_selection(&key, items.len()) {
                            menu.select(selected);
                        }
                    }
                }
            }
        }
        ProbeState::Failed(error) => {
            let error = error.clone();
            let mut items = vec![
                MenuItem::new("Retry", "ask the provider again"),
                MenuItem::new("Enter model id manually", "use the exact provider id"),
            ];
            if state.model.is_some() {
                items.push(MenuItem::new(
                    "Keep current model",
                    state.model.as_deref().unwrap_or_default(),
                ));
            }
            let mut menu = MenuState::new(items.len(), 0);
            loop {
                let mut lines = step_header(Step::Model, index, total, screen.width, screen.color);
                lines.push(format!(
                    "  {}  {}",
                    paint("!", YELLOW, screen.color),
                    paint("Could not list models", YELLOW, screen.color)
                ));
                lines.push(format!(
                    "     {}",
                    paint(
                        &fit_plain(&error, screen.width.saturating_sub(8)),
                        SHADOW,
                        screen.color
                    )
                ));
                lines.extend(menu_lines(
                    "Choose how to continue",
                    "A manual model id will be verified by the smoke run.",
                    &items,
                    &menu,
                    screen.width,
                    screen.color,
                ));
                screen.draw(&lines)?;
                match read_key()? {
                    key if is_cancel(&key) => return Ok(StepAction::Cancel),
                    key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
                    key if is_up(&key) => menu.up(),
                    key if is_down(&key) => menu.down(),
                    key if key.code == KeyCode::Enter => match menu.selected() {
                        0 => {
                            state.reset_probe();
                            return Ok(StepAction::Stay);
                        }
                        1 => match read_model(screen, state, index, total)? {
                            TextAction::Value(value) => {
                                state.model = Some(value);
                                return Ok(StepAction::Next);
                            }
                            TextAction::Back => continue,
                            TextAction::Cancel => return Ok(StepAction::Cancel),
                        },
                        _ => {
                            if state.model.is_some() {
                                return Ok(StepAction::Next);
                            }
                        }
                    },
                    key => {
                        if let Some(selected) = numeric_selection(&key, items.len()) {
                            menu.select(selected);
                        }
                    }
                }
            }
        }
        ProbeState::Skipped => {
            let mut items = Vec::new();
            if let Some(model) = state.model.as_deref() {
                items.push(MenuItem::new(model, "current").badge("current"));
            }
            items.push(MenuItem::new(
                "Enter model id manually",
                "probe was skipped",
            ));
            let mut menu = MenuState::new(items.len(), 0);
            loop {
                let mut lines = step_header(Step::Model, index, total, screen.width, screen.color);
                lines.push(format!(
                    "  {}",
                    paint(
                        if key_from_stdin {
                            "Model probe skipped (--api-key-stdin). The smoke run still validates the key."
                        } else {
                            "Model probe skipped (--skip-probe). The smoke run still validates the key."
                        },
                        SHADOW,
                        screen.color
                    )
                ));
                lines.extend(menu_lines(
                    "Choose a model",
                    "Use the exact model id your provider expects.",
                    &items,
                    &menu,
                    screen.width,
                    screen.color,
                ));
                screen.draw(&lines)?;
                match read_key()? {
                    key if is_cancel(&key) => return Ok(StepAction::Cancel),
                    key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
                    key if is_up(&key) => menu.up(),
                    key if is_down(&key) => menu.down(),
                    key if key.code == KeyCode::Enter => {
                        if items[menu.selected()].label == "Enter model id manually" {
                            match read_model(screen, state, index, total)? {
                                TextAction::Value(value) => {
                                    state.model = Some(value);
                                    return Ok(StepAction::Next);
                                }
                                TextAction::Back => continue,
                                TextAction::Cancel => return Ok(StepAction::Cancel),
                            }
                        }
                        state.model = Some(items[menu.selected()].key.clone());
                        return Ok(StepAction::Next);
                    }
                    key => {
                        if let Some(selected) = numeric_selection(&key, items.len()) {
                            menu.select(selected);
                        }
                    }
                }
            }
        }
        ProbeState::NotStarted => unreachable!("probe state is resolved above"),
    }
}

const MANUAL_MODEL: &str = "\u{0}manual";

fn model_items(models: &[String], current: Option<&str>) -> Vec<MenuItem> {
    let mut items = Vec::new();
    if let Some(current) = current.filter(|current| !current.trim().is_empty()) {
        let advertised = models.iter().any(|model| model == current);
        let mut item = MenuItem::new(current, "configured model").badge("current");
        if !advertised {
            item = item.badge("not advertised");
        }
        items.push(item);
    }
    for model in models {
        if Some(model.as_str()) == current {
            continue;
        }
        items.push(MenuItem::new(model, ""));
    }
    items.push(MenuItem::new(MANUAL_MODEL, "Enter another model id…"));
    // The manual sentinel must render as a human label, not as its key.
    if let Some(item) = items.last_mut() {
        item.label = "Enter another model id…".into();
        item.detail = "use the exact provider id".into();
    }
    items
}

fn read_model(
    screen: &Screen,
    state: &WizardState,
    index: usize,
    total: usize,
) -> Result<TextAction> {
    let initial = state.model.clone().unwrap_or_default();
    read_text(
        screen,
        &initial,
        false,
        |value, error| {
            let mut lines = step_header(Step::Model, index, total, screen.width, screen.color);
            lines.extend(text_field_lines(
                "Model id",
                "Use the exact id from the provider's model list.",
                value,
                error,
                screen.width,
                screen.color,
            ));
            lines
        },
        validate_model,
    )
}

async fn probe_models(state: &WizardState) -> ProbeState {
    let endpoint = endpoint_for(state);
    if endpoint.trim().is_empty() {
        return ProbeState::Failed("base URL is missing".into());
    }
    match list_provider_models(&endpoint, &state.probe_key(), Duration::from_secs(5)).await {
        Ok(models) if models.is_empty() => ProbeState::Failed("provider returned no models".into()),
        Ok(models) => ProbeState::Ready(models),
        Err(error) => ProbeState::Failed(error.to_string()),
    }
}

fn endpoint_for(state: &WizardState) -> String {
    state
        .base_url
        .clone()
        .or_else(|| default_base_url(&state.provider).map(str::to_string))
        .unwrap_or_default()
}

fn permissions_step(
    screen: &Screen,
    state: &mut WizardState,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    let presets = [
        PermissionPreset::ReadOnly,
        PermissionPreset::Safe,
        PermissionPreset::FullLocal,
    ];
    let current = presets
        .iter()
        .position(|preset| *preset == state.permission)
        .unwrap_or(1);
    let items = presets
        .iter()
        .map(|preset| {
            let mut item = MenuItem::new(preset.as_str(), permission_detail(*preset));
            if *preset == PermissionPreset::Safe {
                item = item.badge("recommended");
            }
            item
        })
        .collect::<Vec<_>>();
    let mut menu = MenuState::new(items.len(), current);
    loop {
        let mut lines = step_header(Step::Permissions, index, total, screen.width, screen.color);
        lines.extend(menu_lines(
            "How much may 1Agent do?",
            "High-risk and irreversible actions still require confirmation.",
            &items,
            &menu,
            screen.width,
            screen.color,
        ));
        screen.draw(&lines)?;
        match read_key()? {
            key if is_cancel(&key) => return Ok(StepAction::Cancel),
            key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
            key if is_up(&key) => menu.up(),
            key if is_down(&key) => menu.down(),
            key if key.code == KeyCode::Enter => {
                state.permission = presets[menu.selected()];
                return Ok(StepAction::Next);
            }
            key => {
                if let Some(selected) = numeric_selection(&key, items.len()) {
                    menu.select(selected);
                }
            }
        }
    }
}

fn workspace_step(
    screen: &Screen,
    state: &mut WizardState,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    let mut workspace_items = Vec::new();
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = home_dir();
    let mut seen = Vec::new();
    let keep_ok = state.workspace.is_dir();
    if !state.workspace.as_os_str().is_empty() {
        seen.push(state.workspace.clone());
        workspace_items.push(
            MenuItem::new("Keep current workspace", &display_path(&state.workspace))
                .badge(if keep_ok { "current" } else { "missing" }),
        );
    }
    if !seen.contains(&current_dir) {
        seen.push(current_dir.clone());
        workspace_items.push(MenuItem::new(
            "Use current directory",
            &display_path(&current_dir),
        ));
    }
    if let Some(home_path) = home
        .as_ref()
        .filter(|home_path| !seen.iter().any(|path| path == *home_path))
    {
        seen.push(home_path.clone());
        workspace_items.push(MenuItem::new(
            "Use home directory",
            &display_path(home_path),
        ));
    }
    workspace_items.push(MenuItem::new("Enter another path…", "must already exist"));
    let default = if keep_ok {
        0
    } else {
        1.min(workspace_items.len().saturating_sub(1))
    };
    let mut menu = MenuState::new(workspace_items.len(), default);
    loop {
        let mut lines = step_header(Step::Workspace, index, total, screen.width, screen.color);
        lines.extend(menu_lines(
            "Where should 1Agent work?",
            "Writes stay inside this workspace under the safe preset.",
            &workspace_items,
            &menu,
            screen.width,
            screen.color,
        ));
        screen.draw(&lines)?;
        match read_key()? {
            key if is_cancel(&key) => return Ok(StepAction::Cancel),
            key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
            key if is_up(&key) => menu.up(),
            key if is_down(&key) => menu.down(),
            key if key.code == KeyCode::Enter => {
                let selected = menu.selected();
                let item = &workspace_items[selected];
                if item.label == "Enter another path…" {
                    let initial = state.workspace.display().to_string();
                    let action = read_text(
                        screen,
                        &initial,
                        false,
                        |value, error| {
                            let mut lines = step_header(
                                Step::Workspace,
                                index,
                                total,
                                screen.width,
                                screen.color,
                            );
                            lines.extend(text_field_lines(
                                "Workspace path",
                                "Relative paths resolve from the current directory.",
                                value,
                                error,
                                screen.width,
                                screen.color,
                            ));
                            lines
                        },
                        validate_workspace,
                    )?;
                    match action {
                        TextAction::Value(value) => {
                            state.workspace = PathBuf::from(value);
                            return Ok(StepAction::Next);
                        }
                        TextAction::Back => continue,
                        TextAction::Cancel => return Ok(StepAction::Cancel),
                    }
                }
                state.workspace = if item.label == "Use current directory" {
                    current_dir.clone()
                } else if item.label == "Use home directory" {
                    home.clone().unwrap_or_else(|| state.workspace.clone())
                } else {
                    state.workspace.clone()
                };
                return Ok(StepAction::Next);
            }
            key => {
                if let Some(selected) = numeric_selection(&key, workspace_items.len()) {
                    menu.select(selected);
                }
            }
        }
    }
}

fn review_step(
    screen: &Screen,
    state: &WizardState,
    options: &SetupOptions,
    index: usize,
    total: usize,
) -> Result<StepAction> {
    let items = vec![
        MenuItem::new("Run setup", "write config and run the checks").badge("recommended"),
        MenuItem::new("Back", "change one of the choices"),
        MenuItem::new("Cancel", "leave without writing"),
    ];
    let mut menu = MenuState::new(items.len(), 0);
    loop {
        let mut lines = step_header(Step::Review, index, total, screen.width, screen.color);
        lines.extend(review_lines(state, options, screen.width, screen.color));
        lines.extend(menu_lines(
            "Ready to configure 1Agent?",
            "No config is written until you choose Run setup.",
            &items,
            &menu,
            screen.width,
            screen.color,
        ));
        screen.draw(&lines)?;
        match read_key()? {
            key if is_cancel(&key) => return Ok(StepAction::Cancel),
            key if key.code == KeyCode::Esc => return Ok(StepAction::Back),
            key if is_up(&key) => menu.up(),
            key if is_down(&key) => menu.down(),
            key if key.code == KeyCode::Enter => {
                return Ok(match menu.selected() {
                    0 => StepAction::Next,
                    1 => StepAction::Back,
                    _ => StepAction::Cancel,
                });
            }
            key => {
                if let Some(selected) = numeric_selection(&key, items.len()) {
                    menu.select(selected);
                }
            }
        }
    }
}

fn review_rows(state: &WizardState, options: &SetupOptions) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        (
            "Provider",
            format!("{} ({})", provider_label(&state.provider), state.provider),
        ),
        ("Endpoint", endpoint_for(state)),
        (
            "Model",
            state
                .model
                .clone()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| "not selected".into()),
        ),
        ("Credential", state.credential_label().to_string()),
        (
            "Permissions",
            format!(
                "{} · {}",
                state.permission.as_str(),
                permission_detail(state.permission)
            ),
        ),
        ("Workspace", display_path(&state.workspace)),
        ("Runtime home", display_path(&state.root)),
        (
            "Smoke test",
            if options.skip_smoke {
                "skipped (--skip-smoke)".into()
            } else {
                "will run (up to 30s)".into()
            },
        ),
    ];
    let probe = match &state.probe {
        ProbeState::Ready(models) => format!("{} model(s) discovered", models.len()),
        ProbeState::Failed(error) => format!("failed · {}", fit_plain(error, 40)),
        ProbeState::Skipped => "skipped (--skip-probe)".into(),
        ProbeState::NotStarted => "not run".into(),
    };
    rows.push(("Model probe", probe));
    rows
}

fn review_lines(
    state: &WizardState,
    options: &SetupOptions,
    width: usize,
    color: bool,
) -> Vec<String> {
    let rows = review_rows(state, options);
    let label_width = rows
        .iter()
        .map(|(label, _)| label.width())
        .max()
        .unwrap_or(12)
        .max(12);
    let panel_width = width.saturating_sub(4).min(78);
    let inner = panel_width.saturating_sub(2);
    let value_width = inner.saturating_sub(label_width + 3);
    let mut lines = Vec::new();
    lines.push(format!(
        "  {}",
        paint(&panel_top("Review", inner), LINE, color)
    ));
    for (label, value) in rows {
        lines.push(format!(
            "  {} {} {} {}",
            paint("│", LINE, color),
            paint(&pad_to(label, label_width), LINE, color),
            paint(
                &pad_to(&fit_plain(&value, value_width), value_width),
                BODY,
                color
            ),
            paint("│", LINE, color)
        ));
    }
    lines.push(format!("  {}", paint(&panel_bottom(inner), LINE, color)));
    lines.push(String::new());
    lines
}

fn step_header(step: Step, index: usize, total: usize, width: usize, color: bool) -> Vec<String> {
    let progress = progress_bar(index, total, 14);
    vec![
        format!(
            "  {}  {}",
            paint("1AGENT", ACCENT_BOLD, color),
            paint("setup", SHADOW, color)
        ),
        rule(width, color),
        String::new(),
        format!(
            "  {}  {}  {}",
            paint(&progress, ACCENT, color),
            paint(&format!("{}/{}", index + 1, total), SHADOW, color),
            paint(step.title(), BOLD, color)
        ),
        String::new(),
    ]
}

fn menu_lines(
    title: &str,
    subtitle: &str,
    items: &[MenuItem],
    menu: &MenuState,
    width: usize,
    color: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("  {}", paint(title, BOLD, color)));
    lines.push(format!(
        "  {}",
        paint(&fit_plain(subtitle, width.saturating_sub(4)), SHADOW, color)
    ));
    lines.push(String::new());

    let window = 10usize;
    let start = menu
        .selected
        .saturating_sub(window.saturating_sub(1))
        .min(items.len().saturating_sub(window));
    let end = (start + window).min(items.len());
    if start > 0 {
        lines.push(format!("     {}", paint("↑ more", SHADOW, color)));
    }
    let label_width = items
        .iter()
        .map(|item| item.label.width())
        .max()
        .unwrap_or(0)
        .min(width.saturating_sub(12).max(8));
    for (offset, item) in items[start..end].iter().enumerate() {
        let index = start + offset;
        let selected = index == menu.selected;
        let marker = if selected {
            paint("❯", ACCENT_BOLD, color)
        } else {
            " ".to_string()
        };
        let number = paint(&format!("{:>2}", index + 1), SHADOW, color);
        let label_style = if selected { ACCENT_BOLD } else { BODY };
        let label = paint(
            &fit_plain(&pad_to(&item.label, label_width), label_width),
            label_style,
            color,
        );
        let detail = paint(
            &fit_plain(&item.detail, width.saturating_sub(label_width + 10).max(8)),
            SHADOW,
            color,
        );
        let badge = item
            .badge
            .as_deref()
            .map(|badge| {
                format!(
                    "  {}",
                    paint(
                        &format!("· {badge}"),
                        if matches!(badge, "not advertised" | "missing") {
                            YELLOW
                        } else {
                            BODY
                        },
                        color
                    )
                )
            })
            .unwrap_or_default();
        lines.push(format!("  {marker} {number}  {label}  {detail}{badge}"));
    }
    if end < items.len() {
        lines.push(format!("     {}", paint("↓ more", SHADOW, color)));
    }
    lines.push(String::new());
    lines.push(format!(
        "  {}",
        paint(
            "↑/↓ move   Enter select   1-9 jump   Esc back   Ctrl-C quit",
            SHADOW,
            color
        )
    ));
    lines.push(String::new());
    lines
}

fn text_field_lines(
    title: &str,
    subtitle: &str,
    value: &str,
    error: Option<&str>,
    width: usize,
    color: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("  {}", paint(title, BOLD, color)));
    lines.push(format!(
        "  {}",
        paint(&fit_plain(subtitle, width.saturating_sub(4)), SHADOW, color)
    ));
    lines.push(String::new());
    let panel_width = width.saturating_sub(6).clamp(20, 70);
    let inner = panel_width.saturating_sub(2);
    let content_width = inner.saturating_sub(1);
    let (content, style) = if value.is_empty() {
        ("type here".to_string(), SHADOW)
    } else {
        (fit_plain(&format!("{value}▌"), content_width), BODY)
    };
    let field = paint(&pad_to(&format!(" {content}"), inner), style, color);
    lines.push(format!(
        "  {}{}{}",
        paint("┌", LINE, color),
        paint(&"─".repeat(inner), LINE, color),
        paint("┐", LINE, color)
    ));
    lines.push(format!(
        "  {}{}{}",
        paint("│", LINE, color),
        field,
        paint("│", LINE, color)
    ));
    lines.push(format!(
        "  {}{}{}",
        paint("└", LINE, color),
        paint(&"─".repeat(inner), LINE, color),
        paint("┘", LINE, color)
    ));
    if let Some(error) = error {
        lines.push(String::new());
        lines.push(format!(
            "  {}  {}",
            paint("!", RED, color),
            paint(error, RED, color)
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "  {}",
        paint(
            "Enter confirm   Backspace edit   Ctrl-U clear   Esc back   Ctrl-C quit",
            SHADOW,
            color
        )
    ));
    lines.push(String::new());
    lines
}

#[derive(Clone)]
struct MenuItem {
    key: String,
    label: String,
    detail: String,
    badge: Option<String>,
}

impl MenuItem {
    fn new(label: &str, detail: &str) -> Self {
        Self {
            key: label.to_string(),
            label: label.to_string(),
            detail: detail.to_string(),
            badge: None,
        }
    }

    fn badge(mut self, badge: &str) -> Self {
        self.badge = Some(badge.to_string());
        self
    }
}

struct MenuState {
    selected: usize,
    len: usize,
}

impl MenuState {
    fn new(len: usize, selected: usize) -> Self {
        Self {
            selected: selected.min(len.saturating_sub(1)),
            len,
        }
    }

    fn selected(&self) -> usize {
        self.selected
    }

    fn select(&mut self, selected: usize) {
        if selected < self.len {
            self.selected = selected;
        }
    }

    fn up(&mut self) {
        if self.len > 0 {
            self.selected = (self.selected + self.len - 1) % self.len;
        }
    }

    fn down(&mut self) {
        if self.len > 0 {
            self.selected = (self.selected + 1) % self.len;
        }
    }
}

enum TextAction {
    Value(String),
    Back,
    Cancel,
}

fn read_text<F, V>(
    screen: &Screen,
    initial: &str,
    masked: bool,
    mut draw: F,
    validate: V,
) -> Result<TextAction>
where
    F: FnMut(&str, Option<&str>) -> Vec<String>,
    V: Fn(&str) -> std::result::Result<String, String>,
{
    let mut value = initial.to_string();
    let mut error: Option<String> = None;
    loop {
        let display = if masked {
            "•".repeat(value.chars().count())
        } else {
            value.clone()
        };
        screen.draw(&draw(&display, error.as_deref()))?;
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                KeyCode::Enter => match validate(&value) {
                    Ok(value) => return Ok(TextAction::Value(value)),
                    Err(message) => error = Some(message),
                },
                KeyCode::Esc => return Ok(TextAction::Back),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(TextAction::Cancel);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    value.clear();
                    error = None;
                }
                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    while value.ends_with(' ') {
                        value.pop();
                    }
                    while value.chars().last().is_some_and(|ch| !ch.is_whitespace()) {
                        value.pop();
                    }
                    error = None;
                }
                KeyCode::Backspace => {
                    value.pop();
                    error = None;
                }
                KeyCode::Char(character)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                        && value.chars().count() < 4096 =>
                {
                    value.push(character);
                    error = None;
                }
                _ => {}
            },
            Event::Paste(text) => {
                let text = text.replace(['\r', '\n'], "").trim().to_string();
                if !text.is_empty() && value.chars().count() + text.chars().count() <= 4096 {
                    value.push_str(&text);
                    error = None;
                }
            }
            _ => {}
        }
    }
}

fn validate_endpoint(value: &str) -> std::result::Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("Endpoint cannot be empty.".into());
    }
    let url = reqwest::Url::parse(&value).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Use http:// or https://.".into());
    }
    Ok(value)
}

fn validate_secret(value: &str) -> std::result::Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("Key cannot be empty.".into());
    }
    Ok(value)
}

fn validate_model(value: &str) -> std::result::Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("Model id cannot be empty.".into());
    }
    if value.chars().any(char::is_whitespace) {
        return Err("Model id cannot contain spaces.".into());
    }
    Ok(value)
}

fn validate_workspace(value: &str) -> std::result::Result<String, String> {
    let value = expand_tilde(value.trim());
    if value.as_os_str().is_empty() {
        return Err("Workspace cannot be empty.".into());
    }
    let path = if value.is_absolute() {
        value
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&value))
            .unwrap_or(value)
    };
    if !path.is_dir() {
        return Err(format!("Not a directory: {}", display_path(&path)));
    }
    Ok(path.display().to_string())
}

fn expand_tilde(value: &str) -> PathBuf {
    if value == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(value)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn display_path(path: &Path) -> String {
    if let Some(home) = home_dir()
        && let Ok(rest) = path.strip_prefix(&home)
    {
        if rest.as_os_str().is_empty() {
            return "~".into();
        }
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

fn is_cancel(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_up(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Up) || key.code == KeyCode::Char('k')
}

fn is_down(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Down) || key.code == KeyCode::Char('j')
}

fn numeric_selection(key: &KeyEvent, len: usize) -> Option<usize> {
    let KeyCode::Char(character) = key.code else {
        return None;
    };
    let digit = character.to_digit(10)? as usize;
    (1..=len).contains(&digit).then_some(digit - 1)
}

fn read_key() -> io::Result<KeyEvent> {
    loop {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => return Ok(key),
            _ => {}
        }
    }
}

fn progress_bar(done: usize, total: usize, width: usize) -> String {
    let total = total.max(1);
    let filled = (done.min(total) * width) / total;
    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

fn panel_top(title: &str, width: usize) -> String {
    let prefix = format!("─ {title} ");
    let used = visible_width(&prefix);
    format!("┌{prefix}{}┐", "─".repeat(width.saturating_sub(used)))
}

fn panel_bottom(width: usize) -> String {
    format!("└{}┘", "─".repeat(width))
}

fn rule(width: usize, color: bool) -> String {
    format!(
        "  {}",
        paint(&"─".repeat(width.saturating_sub(4)), LINE, color)
    )
}

fn center(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(visible_width(text)) / 2;
    format!("{}{text}", " ".repeat(padding))
}

fn welcome_text(text: &str, style: &str, width: usize, color: bool) -> String {
    let fitted = fit_plain(text, width.saturating_sub(4));
    center(&paint(&fitted, style, color), width)
}

fn paint_logo(line: &str, color: bool) -> String {
    if !color {
        return line.to_string();
    }
    let mut prefix = String::new();
    let mut suffix = String::new();
    let mut width = 0;
    for character in line.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width < 5 {
            prefix.push(character);
        } else {
            suffix.push(character);
        }
        width += character_width;
    }
    format!("{ACCENT}{prefix}{RESET}{BODY}{suffix}{RESET}")
}

fn paint(text: &str, style: &str, color: bool) -> String {
    if color {
        format!("{style}{text}{RESET}")
    } else {
        text.to_string()
    }
}

fn fit_plain(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used + character_width + 1 > width {
            break;
        }
        out.push(character);
        used += character_width;
    }
    out.push('…');
    out
}

fn pad_to(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    let current = visible_width(&out);
    if current < width {
        out.push_str(&" ".repeat(width - current));
    }
    out
}

fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        width += UnicodeWidthChar::width(character).unwrap_or(0);
    }
    width
}

fn truncate_ansi(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    let mut styled = false;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\x1b' && chars.peek() == Some(&'[') {
            out.push(character);
            out.push(chars.next().unwrap_or('['));
            for code in chars.by_ref() {
                out.push(code);
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
            styled = true;
            continue;
        }
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used + character_width > width {
            break;
        }
        out.push(character);
        used += character_width;
    }
    if styled {
        out.push_str(RESET);
    }
    out
}

struct Screen {
    width: usize,
    height: usize,
    color: bool,
}

impl Screen {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            Hide
        )?;
        let (width, height) = crate::tui_terminal::terminal_dimensions();
        Ok(Self {
            width,
            height,
            color: colors_enabled(),
        })
    }

    fn draw(&self, lines: &[String]) -> io::Result<()> {
        let mut stdout = io::stdout();
        queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        for (row, line) in lines.iter().take(self.height).enumerate() {
            queue!(stdout, MoveTo(0, row as u16))?;
            write!(stdout, "{}", truncate_ansi(line, self.width))?;
        }
        stdout.flush()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableBracketedPaste,
            LeaveAlternateScreen,
            Show
        );
        let _ = terminal::disable_raw_mode();
    }
}

fn colors_enabled() -> bool {
    io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").map_or(true, |term| term != "dumb")
}

fn colors_enabled_stderr() -> bool {
    io::stderr().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").map_or(true, |term| term != "dumb")
}

pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    pub fn start(message: &str) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        if !io::stderr().is_terminal() {
            return Self { stop, handle: None };
        }
        let flag = Arc::clone(&stop);
        let message = message.to_string();
        let handle = std::thread::spawn(move || {
            let frames = ["◐", "◓", "◑", "◒"];
            let mut index = 0usize;
            while !flag.load(Ordering::Relaxed) {
                let mut stderr = io::stderr();
                let _ = write!(
                    stderr,
                    "\r\x1b[2K  {}  {message}",
                    frames[index % frames.len()]
                );
                let _ = stderr.flush();
                index += 1;
                std::thread::sleep(Duration::from_millis(120));
            }
            let mut stderr = io::stderr();
            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn print_success(report: &SetupReport) {
    let color = colors_enabled();
    let rows = report
        .checks
        .iter()
        .map(|check| (check, check_label(&check.id)))
        .collect::<Vec<_>>();
    let label_width = rows
        .iter()
        .map(|(_, label)| label.width())
        .max()
        .unwrap_or(10)
        .max(10);
    let panel_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(80)
        .saturating_sub(6)
        .clamp(40, 78);
    let inner = panel_width.saturating_sub(2);
    // The row is `│ <icon> <label> <value> │`; the icon and three spaces
    // consume five columns of the interior.
    let value_width = inner.saturating_sub(label_width + 5);
    println!();
    // The borders stay in the military-green line colour; only the title is
    // accent-green.
    println!(
        "  {}",
        paint(&panel_top("Ready", inner), ACCENT_BOLD, color)
    );
    for (check, label) in rows {
        let icon = match check.status {
            crate::onboarding::SetupCheckStatus::Ok => paint("✓", ACCENT, color),
            crate::onboarding::SetupCheckStatus::Warn => paint("!", YELLOW, color),
            crate::onboarding::SetupCheckStatus::Skipped => paint("-", SHADOW, color),
            crate::onboarding::SetupCheckStatus::Missing => paint("!", YELLOW, color),
            crate::onboarding::SetupCheckStatus::Fail => paint("×", RED, color),
        };
        println!(
            "  {} {} {} {} {}",
            paint("│", LINE, color),
            icon,
            paint(&pad_to(label, label_width), LINE, color),
            paint(
                &pad_to(&fit_plain(&check.detail, value_width), value_width),
                BODY,
                color
            ),
            paint("│", LINE, color)
        );
    }
    println!("  {}", paint(&panel_bottom(inner), LINE, color));
    if let Some(provider) = &report.provider {
        println!();
        println!(
            "  {}  {}",
            paint("Provider", BODY, color),
            paint(
                &format!(
                    "{} / {} · credential {}",
                    provider.id, provider.model, provider.credential
                ),
                SHADOW,
                color
            )
        );
    }
    println!();
    println!("  {}", paint("Next", BOLD, color));
    if let Some(next) = &report.next {
        println!(
            "    {}  {}",
            paint("→", ACCENT, color),
            paint(next, BODY, color)
        );
    }
    println!(
        "    {}  {}",
        paint("→", ACCENT, color),
        paint("1agent tui", BODY, color)
    );
    println!();
}

pub fn print_failure(error: &SetupError, checks: &[SetupCheck]) {
    let color = colors_enabled_stderr();
    let panel_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(80)
        .saturating_sub(6)
        .clamp(40, 78);
    let inner = panel_width.saturating_sub(2);
    // `│ <×> <check id> <detail> │`: the marker and three spaces plus the
    // 16-column check id consume 21 columns of the interior.
    let detail_width = inner.saturating_sub(21);
    eprintln!();
    eprintln!("  {}", paint(&panel_top("Setup failed", inner), RED, color));
    for check in checks
        .iter()
        .filter(|check| !matches!(check.status, crate::onboarding::SetupCheckStatus::Ok))
    {
        eprintln!(
            "  {} {} {} {} {}",
            paint("│", LINE, color),
            paint("×", RED, color),
            paint(&pad_to(&check.id, 16), LINE, color),
            paint(
                &pad_to(&fit_plain(&check.detail, detail_width), detail_width),
                BODY,
                color
            ),
            paint("│", LINE, color)
        );
    }
    eprintln!("  {}", paint(&panel_bottom(inner), LINE, color));
    eprintln!();
    eprintln!("  {}  {}", paint("error:", RED, color), error.message);
    eprintln!("  {}   {}", paint("next:", BODY, color), error.next);
    eprintln!();
}

fn check_label(id: &str) -> &'static str {
    match id {
        "home" => "Runtime home",
        "provider" => "Provider",
        "credentials" => "Credential",
        "provider_probe" => "Model list",
        "model" => "Model",
        "permissions" => "Permissions",
        "workspace" => "Workspace",
        "smoke" => "Smoke test",
        "doctor" => "Doctor",
        _ => "Check",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> SetupOptions {
        SetupOptions::new(PathBuf::from("/tmp/1agent-wizard-test"))
    }

    fn state() -> WizardState {
        WizardState::new(Path::new("/tmp/1agent-wizard-test"), None, &options())
    }

    #[test]
    fn welcome_lines_fit_common_widths() {
        for width in [40, 60, 80, 120] {
            let lines = welcome_lines(width, 30, false, None);
            for line in &lines {
                assert!(
                    visible_width(line) <= width,
                    "line wider than {width}: {line:?}"
                );
            }
        }
    }

    #[test]
    fn narrow_terminal_uses_compact_logo() {
        let lines = welcome_lines(40, 24, false, None);
        let text = lines.join("\n");
        assert!(text.contains("1 · A G E N T"));
        assert!(!text.contains("█████╗"));
    }

    #[test]
    fn model_items_keep_unadvertised_current_model_visible() {
        let items = model_items(&["model-a".into(), "model-b".into()], Some("old-model"));
        assert_eq!(items[0].label, "old-model");
        assert_eq!(items[0].badge.as_deref(), Some("not advertised"));
        assert!(items.iter().any(|item| item.key == "model-a"));
        assert_eq!(
            items.last().map(|item| item.key.as_str()),
            Some(MANUAL_MODEL)
        );
    }

    #[test]
    fn model_items_do_not_duplicate_the_current_model() {
        let items = model_items(&["model-a".into(), "model-b".into()], Some("model-a"));
        assert_eq!(items.iter().filter(|item| item.key == "model-a").count(), 1);
    }

    #[test]
    fn menu_navigation_wraps() {
        let mut menu = MenuState::new(3, 0);
        menu.up();
        assert_eq!(menu.selected(), 2);
        menu.down();
        assert_eq!(menu.selected(), 0);
    }

    #[test]
    fn review_never_contains_the_secret() {
        let mut state = state();
        state.api_key = Some("sk-super-secret".into());
        state.model = Some("model-a".into());
        let rows = review_rows(&state, &options());
        let text = rows
            .iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains("sk-super-secret"));
        assert!(text.contains("new key (hidden)"));
    }

    #[test]
    fn progress_bar_scales() {
        assert_eq!(progress_bar(0, 4, 4), "[░░░░]");
        assert_eq!(progress_bar(2, 4, 4), "[██░░]");
        assert_eq!(progress_bar(4, 4, 4), "[████]");
    }

    #[test]
    fn fit_plain_respects_width() {
        assert_eq!(fit_plain("abcdef", 6), "abcdef");
        assert_eq!(fit_plain("abcdef", 3), "ab…");
        assert!(fit_plain("abcdef", 3).width() <= 3);
    }

    #[test]
    fn workspace_validation_rejects_missing_paths() {
        let error = validate_workspace("/definitely/not/here").unwrap_err();
        assert!(error.contains("Not a directory"));
    }

    #[test]
    fn endpoint_validation_requires_http() {
        assert!(validate_endpoint("https://example.com/v1").is_ok());
        assert!(validate_endpoint("ftp://example.com").is_err());
        assert!(validate_endpoint("example.com").is_err());
    }
}
