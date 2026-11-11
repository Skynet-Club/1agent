//! One-way inbox notifications to a chat webhook.
//!
//! This is a presence entry, not a full chat transport: it posts the current
//! open inbox items to a generic/Slack/Discord webhook. It does not read
//! replies, change item state or store the webhook URL.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::alerts::WebhookFormat;
use crate::inbox::InboxItem;

const MAX_ITEMS: usize = 50;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("notification webhook is invalid: {0}")]
    Invalid(String),
    #[error("notification webhook failed: {0}")]
    Http(String),
    #[error("notification payload failed: {0}")]
    Json(String),
    #[error("notification state I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("notification state JSON failed: {0}")]
    State(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotifyState {
    #[serde(default)]
    pub last_sent_ms: u128,
}

impl NotifyState {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, NotifyError> {
        match std::fs::read(path.into()) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| NotifyError::State(error.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), NotifyError> {
        use std::io::Write;
        let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
        let bytes =
            serde_json::to_vec(self).map_err(|error| NotifyError::State(error.to_string()))?;
        {
            let mut file = std::fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotifyOutcome {
    pub sent: bool,
    pub open: usize,
    pub new_items: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sent_ms: Option<u128>,
}

pub struct InboxNotifier {
    client: reqwest::Client,
    url: String,
    format: WebhookFormat,
}

impl InboxNotifier {
    pub fn new(url: impl Into<String>) -> Result<Self, NotifyError> {
        let url = url.into();
        let parsed =
            reqwest::Url::parse(&url).map_err(|_| NotifyError::Invalid("invalid URL".into()))?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(NotifyError::Invalid(
                "URL must not embed credentials".into(),
            ));
        }
        let loopback = parsed.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .map(|address| address.is_loopback())
                    .unwrap_or(false)
        });
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            return Err(NotifyError::Invalid(
                "webhook must use https (http is allowed only for loopback)".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| NotifyError::Http(error.without_url().to_string()))?;
        Ok(Self {
            client,
            url,
            format: WebhookFormat::Generic,
        })
    }

    pub fn with_format(mut self, format: WebhookFormat) -> Self {
        self.format = format;
        self
    }

    async fn send(&self, items: &[InboxItem]) -> Result<(), NotifyError> {
        let payload = self.payload(items)?;
        let response = self
            .client
            .post(&self.url)
            .json(&payload)
            .send()
            .await
            .map_err(|error| NotifyError::Http(error.without_url().to_string()))?;
        if !response.status().is_success() {
            return Err(NotifyError::Http(format!(
                "endpoint returned status {}",
                response.status().as_u16()
            )));
        }
        Ok(())
    }

    fn payload(&self, items: &[InboxItem]) -> Result<serde_json::Value, NotifyError> {
        if items.is_empty() {
            return Err(NotifyError::Invalid("inbox is empty".into()));
        }
        let bounded = items.iter().take(MAX_ITEMS).collect::<Vec<_>>();
        let lines = bounded
            .iter()
            .map(|item| {
                format!(
                    "[{}] {} ({})",
                    item.kind.as_str(),
                    truncate(&item.title, MAX_TITLE_CHARS),
                    item.item_id
                )
            })
            .collect::<Vec<_>>();
        let text = format!("1Agent Inbox: {} open\n{}", items.len(), lines.join("\n"));
        Ok(match self.format {
            WebhookFormat::Generic => json!({
                "title": "1Agent Inbox",
                "open": items.len(),
                "items": bounded.iter().map(|item| json!({
                    "item_id": item.item_id,
                    "kind": item.kind.as_str(),
                    "title": truncate(&item.title, MAX_TITLE_CHARS),
                    "due_ms": item.due_ms,
                })).collect::<Vec<_>>(),
            }),
            WebhookFormat::Slack => json!({"text": text}),
            WebhookFormat::Discord => json!({"content": text}),
        })
    }
}

/// Sends the open items and returns whether anything was sent.
pub async fn notify_inbox(
    notifier: &InboxNotifier,
    items: &[InboxItem],
) -> Result<bool, NotifyError> {
    if items.is_empty() {
        return Ok(false);
    }
    notifier.send(items).await?;
    Ok(true)
}

/// Sends the full open queue only when at least one item is newer than the
/// stored watermark, then advances the in-memory watermark.
pub async fn notify_inbox_if_new(
    notifier: &InboxNotifier,
    items: &[InboxItem],
    state: &mut NotifyState,
    now_ms: u128,
) -> Result<NotifyOutcome, NotifyError> {
    let new_items = items
        .iter()
        .filter(|item| item.updated_ms > state.last_sent_ms)
        .count();
    if new_items == 0 {
        return Ok(NotifyOutcome {
            sent: false,
            open: items.len(),
            new_items: 0,
            last_sent_ms: Some(state.last_sent_ms),
        });
    }
    notifier.send(items).await?;
    state.last_sent_ms = now_ms;
    Ok(NotifyOutcome {
        sent: true,
        open: items.len(),
        new_items,
        last_sent_ms: Some(state.last_sent_ms),
    })
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut bounded: String = value.chars().take(max_chars).collect();
    if value.chars().nth(max_chars).is_some() {
        bounded.push('…');
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{InboxKind, InboxStatus};

    fn item(title: &str) -> InboxItem {
        item_at(title, 1)
    }

    fn item_at(title: &str, updated_ms: u128) -> InboxItem {
        InboxItem {
            item_id: format!("i-{updated_ms}"),
            kind: InboxKind::Review,
            title: title.into(),
            why_now: "manual".into(),
            source: "test".into(),
            created_ms: updated_ms,
            updated_ms,
            due_ms: None,
            status: InboxStatus::Open,
            snoozed_until_ms: None,
            resolution: None,
        }
    }

    #[test]
    fn payloads_are_bounded_and_formatted() {
        let notifier = InboxNotifier::new("http://127.0.0.1:9/hook").unwrap();
        let long = item(&"x".repeat(500));
        let payload = notifier.payload(&[long]).unwrap();
        assert_eq!(payload["open"], 1);
        assert_eq!(payload["items"][0]["kind"], "review");
        assert_eq!(
            payload["items"][0]["title"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            MAX_TITLE_CHARS + 1
        );

        let slack = notifier
            .with_format(WebhookFormat::Slack)
            .payload(&[item("Approve the deploy")])
            .unwrap();
        assert!(
            slack["text"]
                .as_str()
                .unwrap()
                .contains("Approve the deploy")
        );
    }

    #[test]
    fn insecure_remote_and_embedded_credentials_are_rejected() {
        assert!(InboxNotifier::new("http://example.com/hook").is_err());
        assert!(InboxNotifier::new("https://user:pass@example.com/hook").is_err());
        assert!(InboxNotifier::new("http://localhost:8080/hook").is_ok());
    }

    #[tokio::test]
    async fn watermark_suppresses_unchanged_items_and_survives_save() {
        let path = std::env::temp_dir().join(format!(
            "1agent-notify-state-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let notifier = InboxNotifier::new("http://127.0.0.1:9/hook").unwrap();
        let mut state = NotifyState { last_sent_ms: 100 };
        let unchanged = notify_inbox_if_new(&notifier, &[item_at("old", 10)], &mut state, 200)
            .await
            .unwrap();
        assert!(!unchanged.sent);
        assert_eq!(unchanged.new_items, 0);

        state.save(&path).unwrap();
        let reopened = NotifyState::open(&path).unwrap();
        assert_eq!(reopened.last_sent_ms, 100);

        let failed = notify_inbox_if_new(&notifier, &[item_at("new", 101)], &mut state, 300).await;
        assert!(failed.is_err());
        assert_eq!(state.last_sent_ms, 100);
        std::fs::remove_file(&path).ok();
    }
}
