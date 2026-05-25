pub mod metric_ingestion;
pub mod log_ingestion;
pub mod snmp_trap_server;

use zentra_core::models::metric::MetricBatch;
use zentra_core::models::log_entry::LogBatch;
use zentra_db::DbPool;
use zentra_stream::EventBus;
use tokio::sync::mpsc;
use tracing::{info, error};

/// Main ingestion service — consumes from stream channels and writes to DB
#[derive(Clone)]
pub struct IngestionService {
    pool: DbPool,
    event_bus: EventBus,
}

impl IngestionService {
    pub fn new(pool: DbPool, event_bus: EventBus) -> Self {
        Self { pool, event_bus }
    }

    /// Start the metric ingestion loop
    pub async fn run_metric_ingestion(&self, mut rx: mpsc::Receiver<MetricBatch>) {
        info!("Metric ingestion service started");
        while let Some(batch) = rx.recv().await {
            if batch.is_empty() {
                continue;
            }
            match zentra_db::repositories::metrics::insert_metrics(&self.pool, &batch.metrics).await {
                Ok(count) => {
                    self.event_bus.publish(zentra_core::Event::MetricsCollected {
                        plugin_id: batch.source_plugin.clone(),
                        target_id: uuid::Uuid::nil(),
                        count: count as usize,
                        timestamp: chrono::Utc::now(),
                    });
                }
                Err(e) => {
                    error!(error = %e, plugin = %batch.source_plugin, "Failed to ingest metrics");
                }
            }
        }
        info!("Metric ingestion service stopped");
    }

    /// Start the log ingestion loop
    pub async fn run_log_ingestion(&self, mut rx: mpsc::Receiver<LogBatch>) {
        info!("Log ingestion service started");
        while let Some(batch) = rx.recv().await {
            if batch.is_empty() {
                continue;
            }
            match zentra_db::repositories::logs::insert_logs(&self.pool, &batch.entries).await {
                Ok(count) => {
                    self.event_bus.publish(zentra_core::Event::LogsIngested {
                        source: batch.source.clone(),
                        target_id: uuid::Uuid::nil(),
                        count: count as usize,
                        timestamp: chrono::Utc::now(),
                    });
                }
                Err(e) => {
                    error!(error = %e, source = %batch.source, "Failed to ingest logs");
                }
            }
        }
        info!("Log ingestion service stopped");
    }
}
