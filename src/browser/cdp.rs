//! Minimal Chrome DevTools Protocol client.
//!
//! This is deliberately a thin JSON-RPC layer rather than a generated binding:
//! we only need Page/Runtime/Input/Network/Accessibility/Browser/Target, and a
//! small surface is easier to pin and audit.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Error)]
pub enum CdpError {
    #[error("CDP websocket: {0}")]
    WebSocket(String),
    #[error("CDP json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("CDP timeout waiting for {0}")]
    Timeout(String),
    #[error("CDP connection closed")]
    Closed,
    #[error("CDP remote error {code}: {message}")]
    Remote { code: i64, message: String },
    #[error("CDP command failed: {0}")]
    Command(String),
}

#[derive(Clone)]
pub struct CdpClient {
    inner: Arc<Inner>,
}

struct Inner {
    tx: mpsc::Sender<Message>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>,
    events: broadcast::Sender<Value>,
    next_id: AtomicU64,
}

impl CdpClient {
    pub async fn connect(url: &str) -> Result<Self, CdpError> {
        let (ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|error| CdpError::WebSocket(error.to_string()))?;
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::channel::<Message>(64);
        let (events, _) = broadcast::channel(256);
        let inner = Arc::new(Inner {
            tx,
            pending: Mutex::new(HashMap::new()),
            events,
            next_id: AtomicU64::new(1),
        });
        let writer = inner.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if sink.send(message).await.is_err() {
                    break;
                }
            }
            let _ = writer.tx.send(Message::Close(None)).await;
        });
        let reader = inner.clone();
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                let Ok(message) = message else { break };
                match message {
                    Message::Text(text) => {
                        let Ok(value) = serde_json::from_str::<Value>(&text) else {
                            continue;
                        };
                        if let Some(id) = value.get("id").and_then(Value::as_u64) {
                            let pending = reader.pending.lock().await.remove(&id);
                            if let Some(sender) = pending {
                                let result = if let Some(error) = value.get("error") {
                                    Err(CdpError::Remote {
                                        code: error
                                            .get("code")
                                            .and_then(Value::as_i64)
                                            .unwrap_or(-1),
                                        message: error
                                            .get("message")
                                            .and_then(Value::as_str)
                                            .unwrap_or("unknown")
                                            .to_string(),
                                    })
                                } else {
                                    Ok(value.get("result").cloned().unwrap_or(Value::Null))
                                };
                                let _ = sender.send(result);
                            }
                        } else if value.get("method").and_then(Value::as_str).is_some() {
                            let _ = reader.events.send(value);
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            let mut pending = reader.pending.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(CdpError::Closed));
            }
        });
        Ok(Self { inner })
    }

    pub async fn send(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);
        let mut message = json!({"id": id, "method": method, "params": params});
        if let Some(session_id) = session_id {
            message["sessionId"] = Value::String(session_id.to_string());
        }
        let text = serde_json::to_string(&message)?;
        if self
            .inner
            .tx
            .send(Message::Text(text.into()))
            .await
            .is_err()
        {
            self.inner.pending.lock().await.remove(&id);
            return Err(CdpError::Closed);
        }
        match tokio::time::timeout(Duration::from_secs(20), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CdpError::Closed),
            Err(_) => {
                self.inner.pending.lock().await.remove(&id);
                Err(CdpError::Timeout(method.to_string()))
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.inner.events.subscribe()
    }

    pub async fn wait_for_event(&self, method: &str, timeout: Duration) -> Result<Value, CdpError> {
        let mut events = self.subscribe();
        let wait = async {
            loop {
                match events.recv().await {
                    Ok(value) if value.get("method").and_then(Value::as_str) == Some(method) => {
                        return Ok(value);
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return Err(CdpError::Closed),
                }
            }
        };
        tokio::time::timeout(timeout, wait)
            .await
            .map_err(|_| CdpError::Timeout(method.to_string()))?
    }
}
