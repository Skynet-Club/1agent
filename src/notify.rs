//! One-way inbox notifications to a chat webhook.
//!
//! This is a presence entry, not a full chat transport: it posts the current
//! open inbox items to a generic/Slack/Discord webhook. It does not read
//! replies, change item state or store the webhook URL.

use std::net::IpAddr;
use std::time::Duration;

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
        InboxItem {
            item_id: "i1".into(),
            kind: InboxKind::Review,
            title: title.into(),
            why_now: "manual".into(),
            source: "test".into(),
            created_ms: 1,
            updated_ms: 1,
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
}
