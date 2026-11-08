//! Small bounded SSE event bus for the Surface API.

use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use serde_json::Value;

use crate::surface_api::{SurfaceError, SurfaceSseEvent};

#[derive(Debug, Default)]
pub struct SurfaceEventBus {
    next_id: u64,
    subscribers: Vec<SyncSender<SurfaceSseEvent>>,
}

#[derive(Debug)]
pub struct SurfaceSubscription {
    receiver: Receiver<SurfaceSseEvent>,
}

impl SurfaceSubscription {
    pub fn receiver(&self) -> &Receiver<SurfaceSseEvent> {
        &self.receiver
    }
}

impl SurfaceEventBus {
    pub fn subscribe(&mut self, capacity: usize) -> SurfaceSubscription {
        let (sender, receiver) = sync_channel(capacity.max(1));
        self.subscribers.push(sender);
        SurfaceSubscription { receiver }
    }

    pub fn publish(
        &mut self,
        event: impl Into<String>,
        data: Value,
    ) -> Result<SurfaceSseEvent, SurfaceError> {
        self.next_id = self.next_id.saturating_add(1);
        let event = SurfaceSseEvent {
            id: self.next_id,
            event: event.into(),
            data,
        };
        event.to_sse()?;
        self.subscribers.retain(|subscriber| {
            !matches!(
                subscriber.try_send(event.clone()),
                Err(std::sync::mpsc::TrySendError::Disconnected(_))
            )
        });
        Ok(event)
    }

    pub fn subscribers(&self) -> usize {
        self.subscribers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subscribers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn subscribers_receive_ids_and_disconnects_are_pruned() {
        let mut bus = SurfaceEventBus::default();
        let first = bus.subscribe(4);
        let second = bus.subscribe(4);
        assert_eq!(bus.subscribers(), 2);
        let event = bus
            .publish("message", serde_json::json!({"text": "hello"}))
            .unwrap();
        assert_eq!(event.id, 1);
        assert_eq!(
            first
                .receiver()
                .recv_timeout(Duration::from_millis(50))
                .unwrap()
                .id,
            1
        );
        assert_eq!(
            second
                .receiver()
                .recv_timeout(Duration::from_millis(50))
                .unwrap()
                .id,
            1
        );
        drop(first);
        bus.publish("message", serde_json::json!({"text": "after"}))
            .unwrap();
        assert_eq!(bus.subscribers(), 1);
    }

    #[test]
    fn invalid_event_names_are_rejected() {
        let mut bus = SurfaceEventBus::default();
        assert_eq!(
            bus.publish("bad/event", serde_json::json!({}))
                .unwrap_err()
                .code,
            crate::surface_api::SurfaceErrorCode::InvalidRequest
        );
    }
}
