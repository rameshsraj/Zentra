use zentra_core::models::event::Event;
use zentra_core::models::metric::MetricBatch;
use zentra_core::models::log_entry::LogBatch;
use tokio::sync::mpsc;
use tracing::{info, debug};

/// Channels for the streaming pipeline:
/// Plugins → Ingestion → Storage + WebSocket broadcast
pub struct StreamChannels {
    pub metric_tx: mpsc::Sender<MetricBatch>,
    pub metric_rx: mpsc::Receiver<MetricBatch>,
    pub log_tx: mpsc::Sender<LogBatch>,
    pub log_rx: mpsc::Receiver<LogBatch>,
}

impl StreamChannels {
    pub fn new(buffer_size: usize) -> Self {
        let (metric_tx, metric_rx) = mpsc::channel(buffer_size);
        let (log_tx, log_rx) = mpsc::channel(buffer_size);
        Self {
            metric_tx,
            metric_rx,
            log_tx,
            log_rx,
        }
    }
}

/// The stream processor consumes metric and log batches from channels,
/// writes them to storage, and forwards to the event bus for real-time UI.
pub struct StreamProcessor {
    event_bus: super::EventBus,
}

impl StreamProcessor {
    pub fn new(event_bus: super::EventBus) -> Self {
        Self { event_bus }
    }

    /// Process incoming metric batches
    pub async fn process_metrics(&self, mut rx: mpsc::Receiver<MetricBatch>) {
        info!("Metric stream processor started");
        while let Some(batch) = rx.recv().await {
            let count = batch.len();
            let plugin = batch.source_plugin.clone();
            debug!(plugin = %plugin, count, "Processing metric batch");

            // Publish event to bus for real-time subscribers
            self.event_bus.publish(Event::MetricsCollected {
                plugin_id: plugin,
                target_id: uuid::Uuid::nil(), // will be enriched upstream
                count,
                timestamp: chrono::Utc::now(),
            });

            // Storage write happens in the ingestion crate
        }
        info!("Metric stream processor stopped");
    }

    /// Process incoming log batches
    pub async fn process_logs(&self, mut rx: mpsc::Receiver<LogBatch>) {
        info!("Log stream processor started");
        while let Some(batch) = rx.recv().await {
            let count = batch.len();
            let source = batch.source.clone();
            debug!(source = %source, count, "Processing log batch");

            self.event_bus.publish(Event::LogsIngested {
                source,
                target_id: uuid::Uuid::nil(),
                count,
                timestamp: chrono::Utc::now(),
            });
        }
        info!("Log stream processor stopped");
    }
}
