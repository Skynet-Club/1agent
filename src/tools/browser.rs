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
use tokio_util::sync::CancellationToken;

use super::web::{html_to_text, resolve_public, validate_url};
use super::{AgentTool, ToolError, ToolOutcome};
use crate::domain::{PermissionSet, RiskLevel, ToolSpec};

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
                "network permission is disabled; pass --allow-network".into(),
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
