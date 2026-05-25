use zentra_core::models::event::Event;
use tokio::sync::broadcast;
use tracing::{info, debug};

const DEFAULT_CAPACITY: usize = 4096;

/// Central event bus using tokio broadcast channels.
///
/// All components (plugins, ingestion, alert engine, UI) subscribe to events.
/// Events are typed and carry full context for drill-down.
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(DEFAULT_CAPACITY);
        info!(capacity = DEFAULT_CAPACITY, "Event bus created");
        Self { tx }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        info!(capacity, "Event bus created");
        Self { tx }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: Event) {
        let event_type = event.event_type();
        match self.tx.send(event) {
            Ok(count) => {
                debug!(event_type, subscribers = count, "Event published");
            }
            Err(_) => {
                debug!(event_type, "Event published (no subscribers)");
            }
        }
    }

    /// Subscribe to the event stream
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_pubsub() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let event = Event::HealthCheck {
            component: "test".into(),
            healthy: true,
            timestamp: Utc::now(),
        };
        bus.publish(event);

        let received = rx.recv().await.unwrap();
        assert_eq!(received.event_type(), "health_check");
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(Event::PluginStarted {
            plugin_id: "test".into(),
            timestamp: Utc::now(),
        });

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.event_type(), "plugin_started");
        assert_eq!(e2.event_type(), "plugin_started");
    }
}
