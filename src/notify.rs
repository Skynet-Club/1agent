//! One-way inbox notifications to a chat webhook.
//!
//! This is a presence entry, not a full chat transport: it posts the current
//! open inbox items to a generic/Slack/Discord webhook. It does not read
//! replies, change item state or store the webhook URL.

use std::collections::BTreeMap;
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
pub struct NotifyReceipt {
    pub sent_at_ms: u128,
    pub open: usize,
    pub new_items: usize,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DigestNotifyReceipt {
    pub sent_at_ms: u128,
    pub runs: usize,
    pub alerts: usize,
    pub inbox_open: usize,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotifyOutcome {
    pub sent: bool,
    pub open: usize,
    pub new_items: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sent_ms: Option<u128>,
    #[serde(default)]
    pub dry_run: bool,
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
            .redirect(reqwest::redirect::Policy::none())
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

    pub fn format_name(&self) -> &'static str {
        match self.format {
            WebhookFormat::Generic => "generic",
            WebhookFormat::Slack => "slack",
            WebhookFormat::Discord => "discord",
        }
    }

    async fn send(&self, items: &[InboxItem]) -> Result<(), NotifyError> {
        self.post(&self.payload(items)?).await
    }

    pub async fn send_text(&self, text: &str) -> Result<(), NotifyError> {
        if text.trim().is_empty() {
            return Err(NotifyError::Invalid("notification text is empty".into()));
        }
        let payload = match self.format {
            WebhookFormat::Generic | WebhookFormat::Slack => json!({"text": text}),
            WebhookFormat::Discord => json!({"content": text}),
        };
        self.post(&payload).await
    }

    async fn post(&self, payload: &serde_json::Value) -> Result<(), NotifyError> {
        let response = self
            .client
            .post(&self.url)
            .json(payload)
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
/// stored watermark, then advances the in-memory watermark. A later target
/// failing stops the fan-out; the caller must not persist the watermark, so
/// already-delivered targets can receive one duplicate on retry.
pub async fn notify_inbox_if_new_to(
    notifiers: &[InboxNotifier],
    items: &[InboxItem],
    state: &mut NotifyState,
    now_ms: u128,
) -> Result<NotifyOutcome, NotifyError> {
    if notifiers.is_empty() {
        return Err(NotifyError::Invalid("no notification targets".into()));
    }
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
            dry_run: false,
        });
    }
    for notifier in notifiers {
        notifier.send(items).await?;
    }
    state.last_sent_ms = now_ms;
    Ok(NotifyOutcome {
        sent: true,
        open: items.len(),
        new_items,
        last_sent_ms: Some(state.last_sent_ms),
        dry_run: false,
    })
}

/// Single-target convenience wrapper around [`notify_inbox_if_new_to`].
pub async fn notify_inbox_if_new(
    notifier: &InboxNotifier,
    items: &[InboxItem],
    state: &mut NotifyState,
    now_ms: u128,
) -> Result<NotifyOutcome, NotifyError> {
    notify_inbox_if_new_to(std::slice::from_ref(notifier), items, state, now_ms).await
}

fn append_json_line<T: Serialize>(path: &Path, value: &T) -> Result<(), NotifyError> {
    use std::io::Write;
    let line = serde_json::to_vec(value).map_err(|error| NotifyError::Json(error.to_string()))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(&line)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

pub fn append_receipt(path: &Path, receipt: &NotifyReceipt) -> Result<(), NotifyError> {
    append_json_line(path, receipt)
}

pub fn append_digest_receipt(
    path: &Path,
    receipt: &DigestNotifyReceipt,
) -> Result<(), NotifyError> {
    append_json_line(path, receipt)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptSummary {
    pub total: usize,
    pub inbox: usize,
    pub digest: usize,
    pub last_sent_ms: u128,
    pub formats: BTreeMap<String, usize>,
}

/// Reads a receipt JSONL log. Unknown shapes and malformed lines fail closed;
/// a missing file is an empty log. Returns the parsed receipts for rendering.
pub fn read_receipts(path: &Path) -> Result<(ReceiptSummary, Vec<serde_json::Value>), NotifyError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let mut summary = ReceiptSummary::default();
    let mut receipts = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| NotifyError::State(format!("line {}: {error}", index + 1)))?;
        let is_inbox = value.get("open").is_some() && value.get("new_items").is_some();
        let is_digest = value.get("inbox_open").is_some() && value.get("runs").is_some();
        if !is_inbox && !is_digest {
            return Err(NotifyError::State(format!(
                "line {} is not a notification receipt",
                index + 1
            )));
        }
        summary.total += 1;
        if is_inbox {
            summary.inbox += 1;
        } else {
            summary.digest += 1;
        }
        if let Some(sent_at_ms) = value.get("sent_at_ms").and_then(serde_json::Value::as_u64) {
            summary.last_sent_ms = summary.last_sent_ms.max(u128::from(sent_at_ms));
        }
        if let Some(format) = value.get("format").and_then(serde_json::Value::as_str) {
            *summary.formats.entry(format.to_string()).or_default() += 1;
        }
        receipts.push(value);
    }
    Ok((summary, receipts))
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

    #[tokio::test]
    async fn redirects_are_not_followed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let _ = socket.read(&mut buffer).await;
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://{address}/elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .ok();
            let followed =
                tokio::time::timeout(std::time::Duration::from_millis(300), listener.accept())
                    .await
                    .is_ok();
            let _ = sender.send(followed);
        });
        let notifier = InboxNotifier::new(format!("http://{address}/hook")).unwrap();
        assert!(notifier.send_text("hello").await.is_err());
        assert!(!receiver.await.unwrap());
    }

    #[tokio::test]
    async fn empty_notification_text_is_rejected() {
        let notifier = InboxNotifier::new("http://127.0.0.1:9/hook").unwrap();
        assert!(notifier.send_text("   ").await.is_err());
    }

    #[test]
    fn insecure_remote_and_embedded_credentials_are_rejected() {
        assert!(InboxNotifier::new("http://example.com/hook").is_err());
        assert!(InboxNotifier::new("https://user:pass@example.com/hook").is_err());
        assert!(InboxNotifier::new("http://localhost:8080/hook").is_ok());
    }

    #[test]
    fn receipts_append_as_jsonl() {
        let path = std::env::temp_dir().join(format!(
            "1agent-notify-receipt-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let receipt = NotifyReceipt {
            sent_at_ms: 10,
            open: 2,
            new_items: 1,
            format: "slack".into(),
        };
        append_receipt(&path, &receipt).unwrap();
        append_receipt(&path, &receipt).unwrap();
        let lines = std::fs::read_to_string(&path).unwrap();
        assert_eq!(lines.lines().count(), 2);
        let parsed: NotifyReceipt = serde_json::from_str(lines.lines().next().unwrap()).unwrap();
        assert_eq!(parsed, receipt);
        std::fs::remove_file(&path).ok();
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
