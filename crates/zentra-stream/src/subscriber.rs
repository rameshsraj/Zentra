use zentra_core::models::event::Event;
use tokio::sync::broadcast;
use tracing::debug;

/// WebSocket-ready subscriber that filters events by type
pub struct FilteredSubscriber {
    rx: broadcast::Receiver<Event>,
    filter: Option<Vec<String>>,
}

impl FilteredSubscriber {
    pub fn new(rx: broadcast::Receiver<Event>, filter: Option<Vec<String>>) -> Self {
        Self { rx, filter }
    }

    /// Wait for the next event that matches the filter
    pub async fn next(&mut self) -> Option<Event> {
        loop {
            match self.rx.recv().await {
                Ok(event) => {
                    if let Some(ref filter) = self.filter {
                        if filter.contains(&event.event_type().to_string()) {
                            return Some(event);
                        }
                        continue;
                    }
                    return Some(event);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    debug!(skipped = n, "Subscriber lagged behind");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return None;
                }
            }
        }
    }
}
