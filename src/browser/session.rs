//! Per-task Chrome for Testing session.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::process::{Child, Command};
use uuid::Uuid;

use crate::browser::cdp::{CdpClient, CdpError};
use crate::browser::egress::{EgressPolicy, EgressProxy};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

const VIEWPORT_AUDIT_JS: &str = r#"(() => {
  const doc = document.documentElement;
  const small = [];
  for (const el of document.querySelectorAll('a,button')) {
    const r = el.getBoundingClientRect();
    if (r.width > 0 && r.height > 0 && (r.width < 44 || r.height < 44)) {
      small.push({
        text: (el.innerText || '').trim().slice(0, 32),
        width: Math.round(r.width),
        height: Math.round(r.height)
      });
    }
  }
  const text = document.body ? document.body.innerText : '';
  const hits = (text.match(/【|占位|待填|待替换|TODO|FIXME|lorem ipsum|example\.com/gi) || []).length;
  return {
    scrollWidth: doc.scrollWidth,
    clientWidth: doc.clientWidth,
    innerWidth: window.innerWidth,
    title: document.title,
    h1Count: document.querySelectorAll('h1').length,
    placeholderHits: hits,
    smallTapTargets: small.slice(0, 20)
  };
})()"#;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("browser: {0}")]
    Cdp(#[from] CdpError),
    #[error("browser io: {0}")]
    Io(#[from] std::io::Error),
    #[error("browser json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("browser egress: {0}")]
    Egress(#[from] crate::browser::egress::EgressError),
    #[error("Chrome executable not found; set ONEAGENT_CHROME_PATH")]
    ChromeNotFound,
    #[error("Chrome did not expose DevToolsActivePort within {0:?}")]
    StartupTimeout(Duration),
    #[error("Chrome exited during startup: {0}")]
    StartupFailed(String),
    #[error("browser action failed: {0}")]
    Action(String),
    #[error("selector not found or not visible: {0}")]
    SelectorNotFound(String),
}

#[derive(Debug, Clone)]
pub struct BrowserConfig {
    pub headless: bool,
    pub allowlist: Vec<String>,
    pub allow_loopback: bool,
    pub chrome_path: Option<PathBuf>,
    pub profile_root: Option<PathBuf>,
    pub download_dir: Option<PathBuf>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: true,
            allowlist: Vec::new(),
            allow_loopback: false,
            chrome_path: None,
            profile_root: None,
            download_dir: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ElementDescriptor {
    pub tag: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
    pub role: String,
    pub text: String,
    pub href: String,
    pub in_form: bool,
    pub inline_handler: bool,
    pub disabled: bool,
    pub reversible: bool,
    pub reason: String,
}

impl ElementDescriptor {
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("<{}>", self.tag)];
        if !self.kind.is_empty() {
            parts.push(format!("type={}", self.kind));
        }
        if !self.id.is_empty() {
            parts.push(format!("#{}", self.id));
        }
        if !self.name.is_empty() {
            parts.push(format!("name={}", self.name));
        }
        if !self.text.is_empty() {
            parts.push(format!("text={:?}", self.text));
        }
        if !self.href.is_empty() {
            parts.push(format!("href={}", self.href));
        }
        if self.in_form {
            parts.push("in-form".into());
        }
        if self.inline_handler {
            parts.push("inline-handler".into());
        }
        parts.join(" ")
    }

    /// Deterministic action target: the exact DOM descriptor, not model text.
    pub fn binding(
        &self,
        session_id: &str,
        origin: &str,
        action: &str,
        value_digest: &str,
    ) -> String {
        use sha2::{Digest, Sha256};
        let material = serde_json::to_vec(&serde_json::json!({
            "session_id": session_id,
            "origin": origin,
            "action": action,
            "descriptor": self,
            "value_digest": value_digest,
        }))
        .unwrap_or_default();
        hex::encode(Sha256::digest(material))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmallTapTarget {
    pub text: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewportAudit {
    pub width: u32,
    pub height: u32,
    pub inner_width: u32,
    pub scroll_width: u32,
    pub client_width: u32,
    pub horizontal_overflow: bool,
    pub title: String,
    pub h1_count: u64,
    pub placeholder_hits: u64,
    pub small_tap_targets: Vec<SmallTapTarget>,
    pub screenshot: String,
    pub screenshot_sha256: String,
    pub screenshot_bytes: u64,
}

pub struct BrowserSession {
    cdp: CdpClient,
    session_id: String,
    task_id: String,
    target_id: String,
    profile_dir: PathBuf,
    download_dir: PathBuf,
    runtime_dir: PathBuf,
    child: Option<Child>,
    proxy: Option<EgressProxy>,
    ephemeral: bool,
    allow_loopback: bool,
}

impl BrowserSession {
    pub async fn launch(config: BrowserConfig) -> Result<Self, BrowserError> {
        let (profile_dir, ephemeral) = match config.profile_root {
            Some(root) => (root, false),
            None => (
                default_browser_root().join(format!("profile-{}", Uuid::new_v4())),
                true,
            ),
        };
        create_private_dir(&profile_dir)?;
        let download_dir = config
            .download_dir
            .clone()
            .unwrap_or_else(|| profile_dir.join("downloads"));
        create_private_dir(&download_dir)?;
        let runtime_dir = std::env::temp_dir().join(format!(
            "1a-br-{}",
            &Uuid::new_v4().simple().to_string()[..8]
        ));
        create_private_dir(&runtime_dir)?;
        let proxy = EgressProxy::start(EgressPolicy::new(
            config.allowlist.clone(),
            config.allow_loopback,
        ))
        .await?;
        let chrome = resolve_chrome(config.chrome_path.as_deref())?;
        let log_path = profile_dir.join("chrome.log");
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let mut command = Command::new(&chrome);
        if config.headless {
            command.arg("--headless=new");
        }
        command
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-sync")
            .arg("--disable-extensions")
            .arg("--disable-component-update")
            .arg("--disable-background-networking")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-features=Translate,MediaRouter")
            .arg("--remote-allow-origins=*")
            .arg(format!("--proxy-server={}", proxy.url()))
            .arg("--proxy-bypass-list=<-loopback>")
            .arg("--window-size=1280,800")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .env_clear()
            .env("HOME", &profile_dir)
            .env("XDG_CONFIG_HOME", profile_dir.join(".config"))
            .env("XDG_CACHE_HOME", profile_dir.join(".cache"))
            .env("TMPDIR", &runtime_dir)
            .env(
                "PATH",
                std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()),
            );
        #[cfg(unix)]
        {
            // New process group so close() can terminate Chrome's children too.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        let mut child = command.spawn()?;
        let (port, path) = match wait_for_devtools(&profile_dir, &mut child).await {
            Ok(value) => value,
            Err(error) => {
                kill_process_group(&mut child);
                return Err(error);
            }
        };
        let ws_url = format!("ws://127.0.0.1:{port}{path}");
        let cdp = match CdpClient::connect(&ws_url).await {
            Ok(cdp) => cdp,
            Err(error) => {
                kill_process_group(&mut child);
                return Err(error.into());
            }
        };
        let target = cdp
            .send("Target.createTarget", json!({"url": "about:blank"}), None)
            .await?;
        let target_id = target
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserError::Action("Target.createTarget returned no targetId".into()))?
            .to_string();
        let attached = cdp
            .send(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )
            .await?;
        let session_id = attached
            .get("sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserError::Action("attachToTarget returned no sessionId".into()))?
            .to_string();
        for domain in [
            "Page.enable",
            "Runtime.enable",
            "Network.enable",
            "Accessibility.enable",
        ] {
            cdp.send(domain, json!({}), Some(&session_id)).await?;
        }
        cdp.send(
            "Browser.setDownloadBehavior",
            json!({
                "behavior": "allow",
                "downloadPath": download_dir.display().to_string(),
                "eventsEnabled": true,
            }),
            None,
        )
        .await?;
        Ok(Self {
            cdp,
            session_id,
            task_id: Uuid::new_v4().to_string(),
            target_id,
            profile_dir,
            download_dir,
            runtime_dir,
            child: Some(child),
            proxy: Some(proxy),
            ephemeral,
            allow_loopback: config.allow_loopback,
        })
    }

    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Stable per-launch task id used for action-card binding.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }

    pub fn allow_loopback(&self) -> bool {
        self.allow_loopback
    }

    pub fn profile_dir(&self) -> &Path {
        &self.profile_dir
    }

    pub fn proxy_url(&self) -> Option<String> {
        self.proxy.as_ref().map(EgressProxy::url)
    }

    pub async fn navigate(&self, url: &str) -> Result<(), BrowserError> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|error| BrowserError::Action(format!("invalid URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(BrowserError::Action(
                "browser navigation only supports http/https".into(),
            ));
        }
        let mut events = self.cdp.subscribe();
        let result = self
            .cdp
            .send("Page.navigate", json!({"url": url}), Some(&self.session_id))
            .await?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str)
            && !error.is_empty()
        {
            return Err(BrowserError::Action(error.to_string()));
        }
        let loaded = async {
            loop {
                match events.recv().await {
                    Ok(value)
                        if value.get("method").and_then(Value::as_str)
                            == Some("Page.loadEventFired") =>
                    {
                        return Ok(());
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(BrowserError::Action("CDP event stream closed".into()));
                    }
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(15), loaded).await;
        self.wait_for_ready(Duration::from_secs(15)).await
    }

    async fn wait_for_ready(&self, timeout: Duration) -> Result<(), BrowserError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let state = self
                .evaluate("document.readyState")
                .await
                .unwrap_or_else(|_| Value::String(String::new()));
            if state.as_str() == Some("complete") || state.as_str() == Some("interactive") {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(BrowserError::Action("page did not become ready".into()));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn evaluate(&self, expression: &str) -> Result<Value, BrowserError> {
        let result = self
            .cdp
            .send(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
                Some(&self.session_id),
            )
            .await?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(BrowserError::Action(format!(
                "JavaScript exception: {exception}"
            )));
        }
        Ok(result
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    pub async fn title(&self) -> Result<String, BrowserError> {
        Ok(self
            .evaluate("document.title")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    pub async fn visible_text(&self) -> Result<String, BrowserError> {
        Ok(self
            .evaluate("document.body ? document.body.innerText : ''")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    pub async fn accessibility_tree(&self) -> Result<Value, BrowserError> {
        Ok(self
            .cdp
            .send(
                "Accessibility.getFullAXTree",
                json!({}),
                Some(&self.session_id),
            )
            .await?)
    }

    pub async fn screenshot(&self, path: &Path) -> Result<u64, BrowserError> {
        let result = self
            .cdp
            .send(
                "Page.captureScreenshot",
                json!({"format": "png", "fromSurface": true}),
                Some(&self.session_id),
            )
            .await?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| BrowserError::Action("screenshot returned no data".into()))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|error| BrowserError::Action(format!("screenshot decode: {error}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &bytes)?;
        Ok(bytes.len() as u64)
    }

    /// Capture each viewport's real layout metrics and a PNG. This is the
    /// local design-review evidence: no network, no terminal session.
    pub async fn audit_viewports(
        &self,
        widths: &[u32],
    ) -> Result<Vec<ViewportAudit>, BrowserError> {
        let mut audits = Vec::new();
        for &width in widths {
            let height = 900u32;
            self.cdp
                .send(
                    "Emulation.setDeviceMetricsOverride",
                    json!({
                        "width": width,
                        "height": height,
                        "deviceScaleFactor": 1,
                        "mobile": width < 768
                    }),
                    Some(&self.session_id),
                )
                .await?;
            tokio::time::sleep(Duration::from_millis(250)).await;
            let metrics = self.evaluate(VIEWPORT_AUDIT_JS).await?;
            let scroll_width = metrics
                .get("scrollWidth")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            let client_width = metrics
                .get("clientWidth")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            let inner_width = metrics
                .get("innerWidth")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            let title = metrics
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let h1_count = metrics.get("h1Count").and_then(Value::as_u64).unwrap_or(0);
            let placeholder_hits = metrics
                .get("placeholderHits")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let small_tap_targets = serde_json::from_value::<Vec<SmallTapTarget>>(
                metrics
                    .get("smallTapTargets")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            )
            .unwrap_or_default();
            let screenshot = self
                .download_dir
                .join(format!("audit-{}-{width}x{height}.png", self.task_id));
            let screenshot_bytes = self.screenshot(&screenshot).await?;
            let bytes = std::fs::read(&screenshot)?;
            let screenshot_sha256 = hex::encode(Sha256::digest(&bytes));
            audits.push(ViewportAudit {
                width,
                height,
                inner_width,
                scroll_width,
                client_width,
                horizontal_overflow: scroll_width > client_width,
                title,
                h1_count,
                placeholder_hits,
                small_tap_targets,
                screenshot: screenshot.display().to_string(),
                screenshot_sha256,
                screenshot_bytes,
            });
        }
        let _ = self
            .cdp
            .send(
                "Emulation.clearDeviceMetricsOverride",
                json!({}),
                Some(&self.session_id),
            )
            .await;
        Ok(audits)
    }

    pub async fn current_url(&self) -> Result<String, BrowserError> {
        Ok(self
            .evaluate("location.href")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    /// Build a ground-truth descriptor for a selector before an action. The
    /// classification is deliberately conservative: only links and plain form
    /// focus are treated as reversible; every button/submit/unknown element
    /// requires an approval card.
    pub async fn describe_selector(
        &self,
        selector: &str,
    ) -> Result<ElementDescriptor, BrowserError> {
        let selector_json = serde_json::to_string(selector)?;
        let expression = format!(
            r#"(() => {{
                const el = document.querySelector({selector_json});
                if (!el) return null;
                const tag = el.tagName ? el.tagName.toLowerCase() : '';
                const type = (el.getAttribute('type') || (tag === 'input' ? 'text' : '')).toLowerCase();
                const text = String(el.innerText || el.value || el.getAttribute('aria-label') || el.getAttribute('title') || '').trim().replace(/\s+/g, ' ').slice(0, 120);
                const href = tag === 'a' && el.href ? String(el.href).slice(0, 2048) : '';
                const inForm = !!el.closest('form');
                const inline = !!(el.getAttribute('onclick') || el.getAttribute('oninput') || el.getAttribute('onchange') || el.getAttribute('onsubmit'));
                const inputReversible = ['text','search','email','password','number','tel','url','checkbox','radio','range','color','date','time','month','week','datetime-local'].includes(type);
                let reversible = false;
                let reason = 'unknown element; fail closed';
                if (el.disabled) {{
                    reason = 'disabled element';
                }} else if (tag === 'a' && /^https?:/i.test(href) && !el.hasAttribute('download') && !inline) {{
                    reversible = true;
                    reason = 'http link navigation';
                }} else if (tag === 'label' || tag === 'summary' || tag === 'option' || tag === 'select' || tag === 'textarea') {{
                    reversible = true;
                    reason = 'form focus without submit';
                }} else if (tag === 'input' && inputReversible) {{
                    reversible = true;
                    reason = 'form focus without submit';
                }}
                return {{
                    tag,
                    type,
                    id: String(el.id || '').slice(0, 128),
                    name: String(el.getAttribute('name') || '').slice(0, 128),
                    role: String(el.getAttribute('role') || '').slice(0, 64),
                    text,
                    href,
                    in_form: inForm,
                    inline_handler: inline,
                    disabled: !!el.disabled,
                    reversible,
                    reason
                }};
            }})()"#
        );
        let value = self.evaluate(&expression).await?;
        if value.is_null() {
            return Err(BrowserError::SelectorNotFound(selector.into()));
        }
        serde_json::from_value(value)
            .map_err(|error| BrowserError::Action(format!("element descriptor: {error}")))
    }

    pub async fn click_selector(&self, selector: &str) -> Result<(), BrowserError> {
        let selector_json = serde_json::to_string(selector)?;
        let expression = format!(
            "(() => {{ const el = document.querySelector({selector_json}); if (!el) return null; const r = el.getBoundingClientRect(); return {{x: r.left + r.width / 2, y: r.top + r.height / 2, visible: r.width > 0 && r.height > 0}}; }})()"
        );
        let rect = self.evaluate(&expression).await?;
        let Some(x) = rect.get("x").and_then(Value::as_f64) else {
            return Err(BrowserError::SelectorNotFound(selector.into()));
        };
        let y = rect
            .get("y")
            .and_then(Value::as_f64)
            .ok_or_else(|| BrowserError::SelectorNotFound(selector.into()))?;
        if rect.get("visible").and_then(Value::as_bool) != Some(true) {
            return Err(BrowserError::SelectorNotFound(selector.into()));
        }
        self.cdp
            .send(
                "Input.dispatchMouseEvent",
                json!({"type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1}),
                Some(&self.session_id),
            )
            .await?;
        self.cdp
            .send(
                "Input.dispatchMouseEvent",
                json!({"type": "mouseReleased", "x": x, "y": y, "button": "left", "clickCount": 1}),
                Some(&self.session_id),
            )
            .await?;
        tokio::time::sleep(Duration::from_millis(150)).await;
        Ok(())
    }

    pub async fn type_selector(&self, selector: &str, text: &str) -> Result<(), BrowserError> {
        let selector_json = serde_json::to_string(selector)?;
        let focused = self
            .evaluate(&format!(
                "(() => {{ const el = document.querySelector({selector_json}); if (!el) return false; el.focus(); return true; }})()"
            ))
            .await?;
        if focused.as_bool() != Some(true) {
            return Err(BrowserError::SelectorNotFound(selector.into()));
        }
        self.cdp
            .send(
                "Input.insertText",
                json!({"text": text}),
                Some(&self.session_id),
            )
            .await?;
        Ok(())
    }

    /// Mask a field's value for screenshots after a secret has been typed.
    /// This is visual best-effort; page JavaScript can still read the value.
    pub async fn mask_selector(&self, selector: &str) -> Result<(), BrowserError> {
        let selector_json = serde_json::to_string(selector)?;
        let masked = self
            .evaluate(&format!(
                "(() => {{ const el = document.querySelector({selector_json}); if (!el) return false; el.style.setProperty('-webkit-text-security', 'disc'); return true; }})()"
            ))
            .await?;
        if masked.as_bool() != Some(true) {
            return Err(BrowserError::SelectorNotFound(selector.into()));
        }
        Ok(())
    }

    pub async fn press_key(&self, key: &str) -> Result<(), BrowserError> {
        let (code, key_code, text) = match key {
            "Enter" => ("Enter", 13, "\r"),
            "Tab" => ("Tab", 9, "\t"),
            "Escape" => ("Escape", 27, ""),
            other => {
                return Err(BrowserError::Action(format!("unsupported key '{other}'")));
            }
        };
        for event_type in ["keyDown", "keyUp"] {
            self.cdp
                .send(
                    "Input.dispatchKeyEvent",
                    json!({
                        "type": event_type,
                        "key": key,
                        "code": code,
                        "windowsVirtualKeyCode": key_code,
                        "nativeVirtualKeyCode": key_code,
                        "text": if event_type == "keyDown" { text } else { "" },
                    }),
                    Some(&self.session_id),
                )
                .await?;
        }
        Ok(())
    }

    pub async fn scroll(&self, delta_y: f64) -> Result<(), BrowserError> {
        self.cdp
            .send(
                "Input.dispatchMouseEvent",
                json!({"type": "mouseWheel", "x": 640, "y": 400, "deltaX": 0, "deltaY": delta_y}),
                Some(&self.session_id),
            )
            .await?;
        Ok(())
    }

    /// Capture a bounded CDP screencast as numbered JPEG frames plus a small
    /// JSONL index. This is intentionally not a video encoder; evidence can be
    /// replayed frame-by-frame without an ffmpeg dependency.
    pub async fn record_screencast(
        &self,
        directory: &Path,
        duration: Duration,
    ) -> Result<u32, BrowserError> {
        std::fs::create_dir_all(directory)?;
        let mut events = self.cdp.subscribe();
        self.cdp
            .send(
                "Page.startScreencast",
                json!({"format": "jpeg", "quality": 60, "everyNthFrame": 1}),
                Some(&self.session_id),
            )
            .await?;
        let deadline = tokio::time::Instant::now() + duration;
        let mut frames = 0u32;
        let mut index = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let event = match tokio::time::timeout(remaining, events.recv()).await {
                Ok(Ok(event)) => event,
                _ => break,
            };
            if event.get("method").and_then(Value::as_str) != Some("Page.screencastFrame") {
                continue;
            }
            let params = event.get("params").cloned().unwrap_or(Value::Null);
            let data = params
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let frame_session = params
                .get("sessionId")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            let metadata = params.get("metadata").cloned().unwrap_or(Value::Null);
            let bytes = match base64::engine::general_purpose::STANDARD.decode(data) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            frames += 1;
            let path = directory.join(format!("frame-{frames:05}.jpg"));
            std::fs::write(&path, &bytes)?;
            index.push(json!({
                "frame": frames,
                "path": path.display().to_string(),
                "bytes": bytes.len(),
                "metadata": metadata,
            }));
            let _ = self
                .cdp
                .send(
                    "Page.screencastFrameAck",
                    json!({"sessionId": frame_session}),
                    Some(&self.session_id),
                )
                .await;
        }
        let _ = self
            .cdp
            .send("Page.stopScreencast", json!({}), Some(&self.session_id))
            .await;
        let index_bytes = serde_json::to_vec_pretty(&index)?;
        std::fs::write(directory.join("index.json"), index_bytes)?;
        Ok(frames)
    }

    pub async fn wait_for_download(
        &self,
        file_name: &str,
        timeout: Duration,
    ) -> Result<PathBuf, BrowserError> {
        let path = self.download_dir.join(file_name);
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if path.is_file() {
                return Ok(path);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(BrowserError::Action(format!(
                    "download did not appear: {}",
                    path.display()
                )));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn close(mut self) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            self.cdp.send("Browser.close", json!({}), None),
        )
        .await;
        if let Some(mut child) = self.child.take() {
            kill_process_group(&mut child);
            let _ = child.wait().await;
        }
        self.proxy.take();
        let _ = std::fs::remove_dir_all(&self.runtime_dir);
        if self.ephemeral {
            let _ = std::fs::remove_dir_all(&self.profile_dir);
        }
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            kill_process_group(child);
        }
    }
}

async fn wait_for_devtools(
    profile_dir: &Path,
    child: &mut Child,
) -> Result<(u16, String), BrowserError> {
    let path = profile_dir.join("DevToolsActivePort");
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Ok(content) = std::fs::read_to_string(&path) {
            let mut lines = content.lines();
            if let (Some(port), Some(ws_path)) = (lines.next(), lines.next())
                && let Ok(port) = port.trim().parse::<u16>()
            {
                let ws_path = ws_path.trim();
                if ws_path.starts_with("/devtools/browser/") {
                    return Ok((port, ws_path.to_string()));
                }
            }
        }
        if let Some(status) = child.try_wait()? {
            let log = std::fs::read_to_string(profile_dir.join("chrome.log")).unwrap_or_default();
            let tail = log
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            return Err(BrowserError::StartupFailed(format!(
                "Chrome exited with {status} before DevToolsActivePort appeared\n{tail}"
            )));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(BrowserError::StartupTimeout(STARTUP_TIMEOUT));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn resolve_chrome(explicit: Option<&Path>) -> Result<PathBuf, BrowserError> {
    if let Some(path) = explicit {
        return executable(path)
            .then(|| path.to_path_buf())
            .ok_or(BrowserError::ChromeNotFound);
    }
    if let Ok(path) = std::env::var("ONEAGENT_CHROME_PATH") {
        let path = PathBuf::from(path);
        if executable(&path) {
            return Ok(path);
        }
    }
    for candidate in [
        "google-chrome-stable",
        "google-chrome",
        "chromium",
        "chromium-browser",
        "chrome",
    ] {
        if let Some(path) = find_in_path(candidate)
            && executable(&path)
        {
            return Ok(path);
        }
    }
    Err(BrowserError::ChromeNotFound)
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn create_private_dir(path: &Path) -> Result<(), BrowserError> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn kill_process_group(child: &mut Child) {
    if child.id().is_some() {
        #[cfg(unix)]
        if let Some(pid) = child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
        }
        let _ = child.start_kill();
    }
}

pub fn default_browser_root() -> PathBuf {
    crate::runtime_home::default_home().join("browser")
}
