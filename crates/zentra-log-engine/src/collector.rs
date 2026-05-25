use crate::{LogSourceConfig, LogSourceType, parser::LogParser};
use zentra_core::models::log_entry::{LogBatch, LogEntry};
use tokio::sync::mpsc;
use tracing::{info, error, debug};

/// Collects logs from configured sources and pushes into ingestion pipeline
pub struct LogCollector {
    config: LogSourceConfig,
    tx: mpsc::Sender<LogBatch>,
}

impl LogCollector {
    pub fn new(config: LogSourceConfig, tx: mpsc::Sender<LogBatch>) -> Self {
        Self { config, tx }
    }

    /// Start collecting logs in a loop
    pub async fn run(&self, org_id: uuid::Uuid) {
        info!(source = %self.config.name, "Log collector started");
        let interval = tokio::time::Duration::from_secs(self.config.polling_interval_secs);

        loop {
            let entries = match self.collect_once(org_id).await {
                Ok(entries) => entries,
                Err(e) => {
                    error!(source = %self.config.name, error = %e, "Log collection failed");
                    vec![]
                }
            };

            if !entries.is_empty() {
                let batch = LogBatch::new(&self.config.name, entries);
                if let Err(e) = self.tx.send(batch).await {
                    error!(error = %e, "Failed to send log batch to pipeline");
                }
            }

            tokio::time::sleep(interval).await;
        }
    }

    async fn collect_once(&self, org_id: uuid::Uuid) -> Result<Vec<LogEntry>, String> {
        match self.config.source_type {
            LogSourceType::File => {
                let path = self.config.path.as_ref().ok_or("No file path configured")?;
                self.collect_from_file(path, org_id).await
            }
            LogSourceType::Api => {
                let endpoint = self.config.endpoint.as_ref().ok_or("No endpoint configured")?;
                self.collect_from_api(endpoint, org_id).await
            }
            _ => {
                debug!(source_type = ?self.config.source_type, "Source type not yet implemented");
                Ok(vec![])
            }
        }
    }

    async fn collect_from_file(&self, path: &str, org_id: uuid::Uuid) -> Result<Vec<LogEntry>, String> {
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| format!("Failed to read {path}: {e}"))?;

        let entries: Vec<LogEntry> = content.lines()
            .filter_map(|line| {
                LogParser::parse(
                    line,
                    &self.config.format,
                    org_id,
                    self.config.target_id,
                    &self.config.name,
                )
            })
            .collect();

        Ok(entries)
    }

    async fn collect_from_api(&self, endpoint: &str, org_id: uuid::Uuid) -> Result<Vec<LogEntry>, String> {
        let resp = reqwest::get(endpoint).await
            .map_err(|e| format!("API request failed: {e}"))?;
        let body = resp.text().await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        let entries: Vec<LogEntry> = body.lines()
            .filter_map(|line| {
                LogParser::parse(
                    line,
                    &self.config.format,
                    org_id,
                    self.config.target_id,
                    &self.config.name,
                )
            })
            .collect();

        Ok(entries)
    }
}
