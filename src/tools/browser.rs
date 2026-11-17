//! Headless browser rendering through Chrome's `--dump-dom` mode.
//!
//! This is a read-only browser adapter, not a computer-use driver. It validates
//! the URL with the same public-HTTPS checks as `fetch_url`, runs Chrome
//! headless with a bounded time budget and returns extracted text.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Url;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::web::{html_to_text, resolve_public, validate_url};
use super::{AgentTool, ToolError, ToolOutcome};
use crate::browser::{
    ActionAuthorization, ActionCardError, ActionCardStore, ActionRequest, BrowserConfig,
    BrowserSession, ElementDescriptor, SecretStore, authorize_request, default_browser_root,
    normalize_origin, origin_of,
};
use crate::cua::ArgumentSource;
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};
use crate::runtime_home::default_home;

const ENV_CHROME: &str = "ONEAGENT_CHROME_BIN";
const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 128 * 1024;
const MAX_URL_CHARS: usize = 4096;
const TIMEOUT: Duration = Duration::from_secs(40);

fn default_chrome() -> PathBuf {
    std::env::var_os(ENV_CHROME)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("google-chrome"))
}

fn bounded(value: &str, max_chars: usize) -> (String, bool) {
    let mut bounded: String = value.chars().take(max_chars).collect();
    let truncated = value.chars().nth(max_chars).is_some();
    if truncated {
        bounded.push('…');
    }
    (bounded, truncated)
}

pub struct BrowserReadTool {
    chrome: PathBuf,
}

impl BrowserReadTool {
    pub fn new(chrome: impl Into<PathBuf>) -> Self {
        Self {
            chrome: chrome.into(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(default_chrome())
    }
}

#[async_trait]
impl AgentTool for BrowserReadTool {
    fn parallel_safe(&self) -> bool {
        true
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "browser_read".into(),
            description: "Render a public HTTPS page in headless Chrome and return bounded readable text. This runs JavaScript, unlike fetch_url. It is not a computer-use driver: there is no click or typing. Requires network permission (--allow-network).".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["url"],
                "properties": {
                    "url": {
                        "type": "string",
                        "maxLength": MAX_URL_CHARS,
                        "description": "Public https:// URL."
                    },
                    "max_chars": {
                        "type": "integer",
                        "minimum": 1000,
                        "maximum": MAX_TEXT_CHARS,
                        "description": "Maximum characters of extracted text, default 131072."
                    }
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(45)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_network {
            return Err(ToolError::PermissionDenied(
                "network permission is disabled; in the TUI run /permissions network on for the next run, or pass --allow-network for headless".into(),
            ));
        }
        let raw_url = arguments
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidArguments("url is required".into()))?;
        let max_chars = arguments
            .get("max_chars")
            .and_then(Value::as_u64)
            .unwrap_or(MAX_TEXT_CHARS as u64)
            .clamp(1000, MAX_TEXT_CHARS as u64) as usize;
        let url = Url::parse(raw_url)
            .map_err(|error| ToolError::InvalidArguments(format!("invalid URL: {error}")))?;
        validate_url(&url)?;
        resolve_public(&url).await?;

        let mut command = tokio::process::Command::new(&self.chrome);
        command
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-extensions",
                "--disable-background-networking",
                "--virtual-time-budget=8000",
                "--dump-dom",
                url.as_str(),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let run = async {
            let child = super::spawn_with_retry(&mut command).await?;
            child.wait_with_output().await
        };
        let output = tokio::time::timeout(TIMEOUT, run)
            .await
            .map_err(|_| ToolError::TimedOut)?
            .map_err(|error| {
                ToolError::Failed(format!(
                    "cannot start headless browser {}: {error}",
                    self.chrome.display()
                ))
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Failed(format!(
                "headless browser exited {}: {}",
                output.status,
                bounded(stderr.trim(), 2_000).0
            )));
        }
        let bytes = &output.stdout;
        let truncated_bytes = bytes.len() > MAX_OUTPUT_BYTES;
        let dom = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_OUTPUT_BYTES)]);
        let text = html_to_text(&dom);
        let (text, truncated_text) = bounded(&text, max_chars);
        let mut metadata = std::collections::BTreeMap::new();
        metadata.insert("final_url".into(), url.to_string());
        metadata.insert("bytes".into(), bytes.len().to_string());
        metadata.insert(
            "truncated".into(),
            (truncated_bytes || truncated_text).to_string(),
        );
        Ok(ToolOutcome {
            summary: format!(
                "无头浏览器已渲染 {}（{} 字符{}）",
                url,
                text.chars().count(),
                if truncated_bytes || truncated_text {
                    "，已截断"
                } else {
                    ""
                }
            ),
            content: text,
            metadata,
        })
    }
}

/// Stateful computer-use tool. One browser session per tool instance; a new
/// `open` keeps the same profile and proxy allowlist. Every action still goes
/// through the runtime's Authority gate because the tool is write-capable.
pub struct BrowserTool {
    session: Mutex<Option<BrowserSession>>,
}

impl BrowserTool {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

fn required_string<'a>(arguments: &'a Value, field: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolError::InvalidArguments(format!("{field} is required")))
}

fn browser_tool_error(error: crate::browser::BrowserError) -> ToolError {
    ToolError::Failed(error.to_string())
}

fn url_is_loopback(url: &str) -> bool {
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .is_some_and(|host| {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            host.eq_ignore_ascii_case("localhost")
                || host == "::1"
                || host == "0.0.0.0"
                || host.starts_with("127.")
        })
}

/// Same local page for an idempotent `open`: origin and path match, while
/// query/fragment differences do not force a reload. A reload would change the
/// page instance and invalidate an approved DOM-bound action card.
fn same_loopback_page(left: &str, right: &str) -> bool {
    let (Ok(left), Ok(right)) = (Url::parse(left), Url::parse(right)) else {
        return false;
    };
    url_is_loopback(left.as_str())
        && url_is_loopback(right.as_str())
        && left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
        && left.path() == right.path()
}

fn approval_store() -> ActionCardStore {
    ActionCardStore::default_store()
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(value.as_bytes()))
}

/// Authorize one computer-use action. Irreversible/button actions create a
/// persistent card; the retry must carry the exact `approval_id` and the
/// descriptor hash is re-checked by the store before it is consumed.
#[allow(clippy::too_many_arguments)]
async fn authorize_browser_action(
    session: &BrowserSession,
    action: &str,
    target: String,
    summary: String,
    risk: RiskLevel,
    reversible: bool,
    sources: Vec<ArgumentSource>,
    approval_id: Option<&str>,
) -> Result<Option<String>, ToolError> {
    let origin = origin_of(&session.current_url().await.map_err(browser_tool_error)?)
        .ok_or_else(|| ToolError::Failed("current page has no http(s) origin".into()))?;
    let request = ActionRequest {
        action: action.into(),
        target,
        risk,
        reversible,
        argument_sources: sources,
        session_id: session.task_id().into(),
        origin,
        summary: summary.clone(),
        action_id: super::current_action_id().unwrap_or_default(),
        expected_artifact_sha256: None,
    };
    match authorize_request(&request, now_ms()) {
        ActionAuthorization::Allow => Ok(None),
        ActionAuthorization::Deny { reason } => Err(ToolError::PermissionDenied(reason)),
        ActionAuthorization::Approval { card } => {
            let Some(card_id) = approval_id else {
                let pending = approval_store()
                    .request(card)
                    .map_err(|error| ToolError::Failed(error.to_string()))?;
                return Err(ToolError::ApprovalRequired {
                    card_id: pending.card_id,
                    summary: format!(
                        "{summary}; origin={}, risk={:?}, reversible={reversible}",
                        request.origin, request.risk
                    ),
                });
            };
            approval_store()
                .take_approved(card_id, &request, now_ms())
                .map_err(|error| match error {
                    ActionCardError::NotFound(_) => ToolError::PermissionDenied(format!(
                        "approval card {card_id} not found; retry the browser action to create a fresh card"
                    )),
                    ActionCardError::NotPending(_) => ToolError::PermissionDenied(format!(
                        "approval card {card_id} is not approved or was already used; retry the browser action to create a fresh card"
                    )),
                    ActionCardError::Expired(_) => ToolError::PermissionDenied(format!(
                        "approval card {card_id} expired; retry the browser action to create a fresh card"
                    )),
                    ActionCardError::Mismatch(_) => ToolError::PermissionDenied(format!(
                        "approval card {card_id} does not match the current page or action; retry the browser action"
                    )),
                    other => ToolError::Failed(other.to_string()),
                })?;
            Ok(Some(card_id.to_string()))
        }
    }
}

fn action_id(arguments: &Value) -> Option<&str> {
    arguments
        .get("approval_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn click_target(
    descriptor: &ElementDescriptor,
    session: &BrowserSession,
    current_url: &str,
    origin: &str,
) -> (String, RiskLevel, bool) {
    let mut reversible = descriptor.reversible;
    if descriptor.tag == "a" && !descriptor.href.is_empty() {
        let same_host = Url::parse(&descriptor.href)
            .ok()
            .zip(Url::parse(current_url).ok())
            .is_some_and(|(target, current)| target.host_str() == current.host_str());
        if !same_host {
            reversible = false;
        }
    }
    let risk = if reversible {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    };
    (
        descriptor.binding(session.task_id(), origin, "click", ""),
        risk,
        reversible,
    )
}

#[async_trait]
impl AgentTool for BrowserTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "browser".into(),
            description: "Computer-use browser sandbox. Supports open/navigate/read/click/type/press/screenshot/audit/close. `audit` returns real viewport metrics (scrollWidth/clientWidth, placeholder hits, tap-target sizes) and PNG paths for local design review; use it with `allow_loopback=true` on a preview_file URL and no network permission. Every http(s) host, including subresources, must be explicitly allowlisted; loopback requires allow_loopback. The browser cannot run in read-only mode. Buttons, submits, cross-origin links, password entry and Enter create a persistent action card: ask the human to run `1agent browser approve <card_id>`, then retry with approval_id. After `approval_required`, keep the current browser session: do not call open or navigate again, because a reload invalidates the DOM-bound card; retry the exact same selector/action with approval_id. Cards last 10 minutes; if expired, regenerate. Password fields require secret_ref, never plaintext.".into(),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["action"],
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["open", "navigate", "read", "click", "type", "press", "screenshot", "audit", "close"]
                    },
                    "widths": {
                        "type": "array",
                        "maxItems": 5,
                        "items": {"type": "integer", "minimum": 320, "maximum": 3840},
                        "description": "Viewport widths for action=audit; default 390,768,1440."
                    },
                    "url": {"type": "string", "maxLength": 4096},
                    "selector": {"type": "string", "maxLength": 1024},
                    "text": {"type": "string", "maxLength": 8192},
                    "secret_ref": {"type": "string", "maxLength": 256},
                    "origin": {"type": "string", "maxLength": 2048},
                    "approval_id": {"type": "string", "maxLength": 128},
                    "key": {"type": "string", "enum": ["Enter", "Tab", "Escape"]},
                    "allow": {"type": "array", "items": {"type": "string"}, "maxItems": 32},
                    "allow_loopback": {"type": "boolean"}
                }
            }),
        }
    }

    fn risk(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn requires_write(&self) -> bool {
        true
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(120)
    }

    async fn execute(
        &self,
        arguments: Value,
        permissions: &PermissionSet,
        _cancellation: &CancellationToken,
    ) -> Result<ToolOutcome, ToolError> {
        if !permissions.allow_write {
            return Err(ToolError::PermissionDenied(
                "browser is write-capable; a writable permission preset is required".into(),
            ));
        }
        let action = required_string(&arguments, "action")?
            .trim()
            .to_ascii_lowercase();
        if !permissions.allow_network {
            let loopback_open = matches!(action.as_str(), "open" | "navigate")
                && arguments
                    .get("allow_loopback")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                && arguments
                    .get("url")
                    .and_then(Value::as_str)
                    .is_some_and(url_is_loopback);
            if matches!(action.as_str(), "open" | "navigate") && !loopback_open {
                return Err(ToolError::PermissionDenied(
                    "browser requires network permission; local pages need allow_loopback=true and a loopback URL"
                        .into(),
                ));
            }
        }
        let mut guard = self.session.lock().await;
        if !permissions.allow_network
            && !matches!(action.as_str(), "open" | "navigate")
            && let Some(session) = guard.as_ref()
            && !session.allow_loopback()
        {
            return Err(ToolError::PermissionDenied(
                "browser session is not loopback-only; in the TUI run /permissions network on for the next run, or pass --allow-network for headless external pages"
                    .into(),
            ));
        }
        match action.as_str() {
            "open" | "navigate" => {
                let url = required_string(&arguments, "url")?;
                if action == "open" && guard.is_none() {
                    let allow = arguments
                        .get("allow")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let mut allowlist = allow;
                    if allowlist.is_empty() {
                        let parsed = Url::parse(url).map_err(|error| {
                            ToolError::InvalidArguments(format!("invalid URL: {error}"))
                        })?;
                        let host = parsed
                            .host_str()
                            .ok_or_else(|| ToolError::InvalidArguments("URL has no host".into()))?;
                        allowlist.push(host.to_string());
                    }
                    let config = BrowserConfig {
                        headless: true,
                        allowlist,
                        allow_loopback: arguments
                            .get("allow_loopback")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        chrome_path: None,
                        profile_root: None,
                        download_dir: None,
                    };
                    let session = BrowserSession::launch(config)
                        .await
                        .map_err(browser_tool_error)?;
                    session.navigate(url).await.map_err(browser_tool_error)?;
                    *guard = Some(session);
                } else {
                    let session = guard.as_ref().ok_or_else(|| {
                        ToolError::Failed("browser is not open; use action=open".into())
                    })?;
                    let current = session.current_url().await.map_err(browser_tool_error)?;
                    let already_open =
                        current == url || (action == "open" && same_loopback_page(&current, url));
                    if !already_open {
                        session.navigate(url).await.map_err(browser_tool_error)?;
                    }
                }
                browser_read_outcome(guard.as_ref().expect("session exists"), "open").await
            }
            "read" => {
                let session = guard.as_ref().ok_or_else(|| {
                    ToolError::Failed("browser is not open; use action=open".into())
                })?;
                browser_read_outcome(session, "read").await
            }
            "click" => {
                let selector = required_string(&arguments, "selector")?;
                let session = guard.as_ref().ok_or_else(|| {
                    ToolError::Failed("browser is not open; use action=open".into())
                })?;
                let current = session.current_url().await.map_err(browser_tool_error)?;
                let origin = origin_of(&current).ok_or_else(|| {
                    ToolError::Failed("current page has no http(s) origin".into())
                })?;
                let descriptor = session
                    .describe_selector(selector)
                    .await
                    .map_err(browser_tool_error)?;
                let (target, risk, reversible) =
                    click_target(&descriptor, session, &current, &origin);
                let summary = format!("click {} at {origin}", descriptor.summary());
                let approved = authorize_browser_action(
                    session,
                    "click",
                    target,
                    summary,
                    risk,
                    reversible,
                    if reversible {
                        vec![ArgumentSource::TrustedObservation]
                    } else {
                        vec![ArgumentSource::UntrustedObservation]
                    },
                    action_id(&arguments),
                )
                .await?;
                session
                    .click_selector(selector)
                    .await
                    .map_err(browser_tool_error)?;
                let mut outcome = browser_read_outcome(session, "click").await?;
                if let Some(card_id) = approved {
                    outcome.metadata.insert("approval_id".into(), card_id);
                }
                Ok(outcome)
            }
            "type" => {
                let selector = required_string(&arguments, "selector")?;
                let text = arguments.get("text").and_then(Value::as_str);
                let secret_ref = arguments.get("secret_ref").and_then(Value::as_str);
                if text.is_some() == secret_ref.is_some() {
                    return Err(ToolError::InvalidArguments(
                        "type requires exactly one of text or secret_ref".into(),
                    ));
                }
                let session = guard.as_ref().ok_or_else(|| {
                    ToolError::Failed("browser is not open; use action=open".into())
                })?;
                let current = session.current_url().await.map_err(browser_tool_error)?;
                let current_origin = origin_of(&current).ok_or_else(|| {
                    ToolError::Failed("current page has no http(s) origin".into())
                })?;
                let descriptor = session
                    .describe_selector(selector)
                    .await
                    .map_err(browser_tool_error)?;
                let (value, secret_label, approved) = if let Some(reference) = secret_ref {
                    let expected = required_string(&arguments, "origin")?;
                    let expected = normalize_origin(expected)
                        .map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
                    if expected != current_origin {
                        return Err(ToolError::PermissionDenied(format!(
                            "secret origin {expected} does not match current origin {current_origin}"
                        )));
                    }
                    let value_digest = digest(&format!("secret:{reference}:{current_origin}"));
                    let target = descriptor.binding(
                        session.task_id(),
                        &current_origin,
                        "type-secret",
                        &value_digest,
                    );
                    let summary = format!(
                        "enter secret {reference} into {} at {current_origin}",
                        descriptor.summary()
                    );
                    let approved = authorize_browser_action(
                        session,
                        "type-secret",
                        target,
                        summary,
                        RiskLevel::Medium,
                        false,
                        vec![ArgumentSource::UntrustedObservation],
                        action_id(&arguments),
                    )
                    .await?;
                    let store = SecretStore::open(default_home().join("secrets.json"))
                        .map_err(|error| ToolError::Failed(error.to_string()))?;
                    let value = store
                        .resolve(reference, &current_origin)
                        .map_err(|error| ToolError::Failed(error.to_string()))?;
                    (value, Some(reference.to_string()), approved)
                } else {
                    if descriptor.kind == "password" {
                        return Err(ToolError::PermissionDenied(
                            "password fields require secret_ref; plaintext passwords are refused"
                                .into(),
                        ));
                    }
                    let text = text.unwrap_or_default().to_string();
                    let target = descriptor.binding(
                        session.task_id(),
                        &current_origin,
                        "type",
                        &digest(&text),
                    );
                    let summary = format!("type into {} at {current_origin}", descriptor.summary());
                    let approved = authorize_browser_action(
                        session,
                        "type",
                        target,
                        summary,
                        RiskLevel::Low,
                        true,
                        vec![ArgumentSource::TrustedObservation],
                        action_id(&arguments),
                    )
                    .await?;
                    (text, None, approved)
                };
                session
                    .type_selector(selector, &value)
                    .await
                    .map_err(browser_tool_error)?;
                let mask_failed = if secret_label.is_some() {
                    session.mask_selector(selector).await.is_err()
                } else {
                    false
                };
                let mut outcome = browser_read_outcome(session, "type").await?;
                if let Some(label) = secret_label {
                    outcome.metadata.insert("secret_ref".into(), label);
                    outcome
                        .metadata
                        .insert("secret_value".into(), "redacted".into());
                    outcome.metadata.insert(
                        "screenshot_mask".into(),
                        if mask_failed { "failed" } else { "applied" }.into(),
                    );
                }
                if let Some(card_id) = approved {
                    outcome.metadata.insert("approval_id".into(), card_id);
                }
                Ok(outcome)
            }
            "press" => {
                let key = required_string(&arguments, "key")?;
                let session = guard.as_ref().ok_or_else(|| {
                    ToolError::Failed("browser is not open; use action=open".into())
                })?;
                let approved = if key == "Enter" {
                    let current = session.current_url().await.map_err(browser_tool_error)?;
                    let origin = origin_of(&current).ok_or_else(|| {
                        ToolError::Failed("current page has no http(s) origin".into())
                    })?;
                    let target = digest(&format!("{}|{origin}|press:Enter", session.task_id()));
                    authorize_browser_action(
                        session,
                        "press",
                        target,
                        format!("press Enter at {origin}"),
                        RiskLevel::Medium,
                        false,
                        vec![ArgumentSource::TrustedObservation],
                        action_id(&arguments),
                    )
                    .await?
                } else {
                    None
                };
                session.press_key(key).await.map_err(browser_tool_error)?;
                let mut outcome = browser_read_outcome(session, "press").await?;
                if let Some(card_id) = approved {
                    outcome.metadata.insert("approval_id".into(), card_id);
                }
                Ok(outcome)
            }
            "audit" => {
                let session = guard.as_ref().ok_or_else(|| {
                    ToolError::Failed("browser is not open; use action=open".into())
                })?;
                let widths = arguments
                    .get("widths")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_u64)
                            .map(|width| width as u32)
                            .collect::<Vec<_>>()
                    })
                    .filter(|widths| !widths.is_empty())
                    .unwrap_or_else(|| vec![390, 768, 1440]);
                if widths.len() > 5 || widths.iter().any(|width| !(320..=3840).contains(width)) {
                    return Err(ToolError::InvalidArguments(
                        "widths must be 1-5 integers in 320..=3840".into(),
                    ));
                }
                let audits = session
                    .audit_viewports(&widths)
                    .await
                    .map_err(browser_tool_error)?;
                let overflow = audits
                    .iter()
                    .filter(|audit| audit.horizontal_overflow)
                    .count();
                let placeholders = audits
                    .iter()
                    .map(|audit| audit.placeholder_hits)
                    .sum::<u64>();
                let small_targets = audits
                    .iter()
                    .map(|audit| audit.small_tap_targets.len())
                    .sum::<usize>();
                let all_pass = overflow == 0
                    && placeholders == 0
                    && audits.iter().all(|audit| audit.h1_count == 1)
                    && small_targets == 0;
                let url = session.current_url().await.map_err(browser_tool_error)?;
                let content = json!({
                    "url": url,
                    "viewports": audits,
                    "all_pass": all_pass,
                    "overflow_count": overflow,
                    "placeholder_hits": placeholders,
                    "small_tap_targets": small_targets
                })
                .to_string();
                let mut metadata = std::collections::BTreeMap::new();
                metadata.insert("action".into(), "audit".into());
                metadata.insert("all_pass".into(), all_pass.to_string());
                metadata.insert("viewports".into(), audits.len().to_string());
                metadata.insert("overflow".into(), overflow.to_string());
                metadata.insert("placeholders".into(), placeholders.to_string());
                metadata.insert("small_tap_targets".into(), small_targets.to_string());
                Ok(ToolOutcome {
                    summary: format!(
                        "page audit: {} viewports · overflow {} · placeholders {} · small tap targets {}",
                        audits.len(),
                        overflow,
                        placeholders,
                        small_targets
                    ),
                    content,
                    metadata,
                })
            }
            "screenshot" => {
                let session = guard.as_ref().ok_or_else(|| {
                    ToolError::Failed("browser is not open; use action=open".into())
                })?;
                let path = default_browser_root()
                    .join("screenshots")
                    .join(format!("{}.png", Uuid::new_v4()));
                let bytes = session
                    .screenshot(&path)
                    .await
                    .map_err(browser_tool_error)?;
                let mut metadata = std::collections::BTreeMap::new();
                metadata.insert("action".into(), "screenshot".into());
                metadata.insert("path".into(), path.display().to_string());
                metadata.insert("bytes".into(), bytes.to_string());
                Ok(ToolOutcome {
                    summary: format!("浏览器截图已保存（{bytes} 字节）"),
                    content: path.display().to_string(),
                    metadata,
                })
            }
            "close" => {
                if let Some(session) = guard.take() {
                    session.close().await;
                }
                let mut metadata = std::collections::BTreeMap::new();
                metadata.insert("action".into(), "close".into());
                Ok(ToolOutcome {
                    summary: "浏览器会话已关闭".into(),
                    content: "closed".into(),
                    metadata,
                })
            }
            other => Err(ToolError::InvalidArguments(format!(
                "unknown browser action '{other}'"
            ))),
        }
    }
}

async fn browser_read_outcome(
    session: &BrowserSession,
    action: &str,
) -> Result<ToolOutcome, ToolError> {
    let title = session.title().await.map_err(browser_tool_error)?;
    let text = session.visible_text().await.map_err(browser_tool_error)?;
    let bounded: String = text.chars().take(8_000).collect();
    let mut metadata = std::collections::BTreeMap::new();
    metadata.insert("action".into(), action.into());
    metadata.insert("title".into(), title.clone());
    metadata.insert(
        "truncated".into(),
        (text.chars().count() > 8_000).to_string(),
    );
    Ok(ToolOutcome {
        summary: format!("浏览器：{title}"),
        content: bounded,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[cfg(unix)]
    fn fake_chrome(dir: &std::path::Path) -> PathBuf {
        let path = dir.join("fake-chrome");
        let script = r#"#!/bin/sh
printf '<html><head><style>.x{}</style><script>bad()</script></head><body><h1>Rendered</h1><p>Hello browser</p></body></html>\n'
"#;
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn renders_dom_and_extracts_text() {
        let dir = tempfile::tempdir().unwrap();
        let tool = BrowserReadTool::new(fake_chrome(dir.path()));
        let outcome = tool
            .execute(
                json!({"url": "https://1.1.1.1/"}),
                &PermissionSet {
                    allow_network: true,
                    ..PermissionSet::default()
                },
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(outcome.content.contains("Rendered"));
        assert!(outcome.content.contains("Hello browser"));
        assert!(!outcome.content.contains("bad()"));
    }

    #[tokio::test]
    async fn network_permission_is_required() {
        let error = BrowserReadTool::new("unused")
            .execute(
                json!({"url": "https://1.1.1.1/"}),
                &PermissionSet::default(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::PermissionDenied(_)));
    }

    #[test]
    fn loopback_urls_are_recognized_for_local_page_audit() {
        assert!(url_is_loopback("http://127.0.0.1:8765/"));
        assert!(url_is_loopback("http://127.1.2.3:8765/"));
        assert!(url_is_loopback("http://localhost:8765/"));
        assert!(url_is_loopback("http://[::1]:8765/"));
        assert!(!url_is_loopback("https://example.com/"));
        assert!(!url_is_loopback("not a url"));
        assert!(same_loopback_page(
            "http://127.0.0.1:40593/?v=2",
            "http://127.0.0.1:40593/"
        ));
        assert!(!same_loopback_page(
            "http://127.0.0.1:40593/a",
            "http://127.0.0.1:40593/b"
        ));
        assert!(!same_loopback_page(
            "https://example.com/a",
            "https://example.com/a"
        ));
    }

    #[tokio::test]
    async fn non_https_urls_are_rejected() {
        let error = BrowserReadTool::new("unused")
            .execute(
                json!({"url": "http://1.1.1.1/"}),
                &PermissionSet {
                    allow_network: true,
                    ..PermissionSet::default()
                },
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ToolError::InvalidArguments(_)));
    }
}
