//! Append-only local alert log for operator-visible health events.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AlertError {
    #[error("alert log io: {0}")]
    Io(#[from] std::io::Error),
    #[error("alert log json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("alert log record is invalid: {0}")]
    Invalid(String),
    #[error("alert webhook failed: {0}")]
    Webhook(String),
}

/// Delivers an alert to an operator webhook. HTTPS is required except for a
/// loopback HTTP endpoint used by local integrations and tests. Delivery errors
/// are reported to the caller but never block the local alert log.
pub struct AlertWebhook {
    client: reqwest::Client,
    url: String,
}

impl std::fmt::Debug for AlertWebhook {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AlertWebhook")
            .field("url", &"<redacted>")
            .finish()
    }
}

impl AlertWebhook {
    pub fn new(url: impl Into<String>) -> Result<Self, AlertError> {
        let url = url.into();
        let parsed = reqwest::Url::parse(&url)
            .map_err(|_| AlertError::Invalid("invalid alert webhook URL".into()))?;
        let loopback = parsed.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .map(|address| address.is_loopback())
                    .unwrap_or(false)
        });
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            return Err(AlertError::Invalid(
                "alert webhook must use https (http is allowed only for loopback)".into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| AlertError::Webhook(error.without_url().to_string()))?;
        Ok(Self { client, url })
    }

    pub async fn send(&self, record: &AlertRecord) -> Result<(), AlertError> {
        validate(record)?;
        let response = self
            .client
            .post(&self.url)
            .json(record)
            .send()
            .await
            .map_err(|error| AlertError::Webhook(error.without_url().to_string()))?;
        if !response.status().is_success() {
            return Err(AlertError::Webhook(format!(
                "endpoint returned status {}",
                response.status().as_u16()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AlertRecord {
    pub alert_id: String,
    pub at_ms: u128,
    pub kind: String,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub observed: u64,
    pub threshold: u64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AlertLog {
    path: PathBuf,
}

impl AlertLog {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, AlertError> {
        let path = path.into();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            crate::jsonl::ensure_parent_private(&path)?;
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, record: &AlertRecord) -> Result<(), AlertError> {
        validate(record)?;
        let line = serde_json::to_string(record)?;
        crate::jsonl::append_batch(&self.path, &format!("{line}\n"))?;
        Ok(())
    }

    pub fn read(&self) -> Result<Vec<AlertRecord>, AlertError> {
        let mut records = Vec::new();
        if !self.path.exists() {
            return Ok(records);
        }
        let file = std::fs::File::open(&self.path)?;
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: AlertRecord = serde_json::from_str(&line)?;
            validate(&record)?;
            records.push(record);
        }
        Ok(records)
    }
}

pub fn provider_health_alert(
    provider_id: &str,
    model: &str,
    at_ms: u128,
    failures: u32,
    threshold: u32,
) -> AlertRecord {
    AlertRecord {
        alert_id: Uuid::new_v4().to_string(),
        at_ms,
        kind: "provider_health".into(),
        provider_id: Some(provider_id.into()),
        model: Some(model.into()),
        observed: u64::from(failures),
        threshold: u64::from(threshold),
        message: format!(
            "provider {provider_id}/{model} reached {failures} consecutive failures (threshold {threshold})"
        ),
    }
}

fn validate(record: &AlertRecord) -> Result<(), AlertError> {
    if record.alert_id.trim().is_empty()
        || record.kind.trim().is_empty()
        || record.message.trim().is_empty()
        || record.message.chars().count() > 2_000
    {
        return Err(AlertError::Invalid("empty or oversized alert field".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_log_appends_and_reads_valid_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alerts.jsonl");
        let log = AlertLog::open(&path).unwrap();
        log.append(&provider_health_alert("p", "m", 10, 2, 2))
            .unwrap();
        log.append(&provider_health_alert("p", "m", 20, 3, 2))
            .unwrap();
        let records = log.read().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].observed, 2);
        assert_eq!(records[1].at_ms, 20);
    }

    #[test]
    fn webhook_requires_https_except_for_loopback() {
        assert!(AlertWebhook::new("http://example.com/alerts").is_err());
        assert!(AlertWebhook::new("ftp://127.0.0.1/alerts").is_err());
        assert!(AlertWebhook::new("http://127.0.0.1:8787/alerts").is_ok());
        assert!(AlertWebhook::new("http://localhost:8787/alerts").is_ok());
        assert!(AlertWebhook::new("https://example.com/alerts").is_ok());
        assert!(
            !format!(
                "{:?}",
                AlertWebhook::new("https://example.com/alerts").unwrap()
            )
            .contains("example.com")
        );
    }

    #[tokio::test]
    async fn webhook_posts_the_alert_record() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    socket.read(&mut chunk),
                )
                .await
                .unwrap()
                .unwrap();
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);
                if let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers =
                        String::from_utf8_lossy(&buffer[..header_end]).to_ascii_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if buffer.len() >= header_end + 4 + length {
                        break;
                    }
                }
            }
            let text = String::from_utf8_lossy(&buffer);
            let body = text.split("\r\n\r\n").nth(1).unwrap_or("");
            let record: AlertRecord = serde_json::from_str(body).unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            record
        });

        let webhook = AlertWebhook::new(format!("http://{address}/alerts")).unwrap();
        let record = provider_health_alert("provider-1", "model-1", 10, 2, 2);
        webhook.send(&record).await.unwrap();
        assert_eq!(server.await.unwrap(), record);
    }

    #[test]
    fn invalid_and_malformed_alerts_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alerts.jsonl");
        let log = AlertLog::open(&path).unwrap();
        let mut record = provider_health_alert("p", "m", 10, 2, 2);
        record.message.clear();
        assert!(matches!(log.append(&record), Err(AlertError::Invalid(_))));

        std::fs::write(&path, b"{\"alert_id\":\"x\"}\n").unwrap();
        assert!(matches!(log.read(), Err(AlertError::Json(_))));
    }
}
