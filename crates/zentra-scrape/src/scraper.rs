//! HTTP pull-based metric scraper for Zentra.
//! Reference: prometheus-main/scrape/scrape.go HTTP scrape loop.
//!
//! Scrapes /metrics endpoints at configurable intervals and converts
//! Prometheus exposition format into Zentra Metric structs.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug, error};
use uuid::Uuid;
use parking_lot::Mutex;

use zentra_core::models::metric::Metric;
use crate::parser::{parse_prometheus_text, PrometheusSample};
use crate::target_discovery::ScrapeTarget;

/// Health state of a scrape target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetHealth {
    Up,
    Down,
    Unknown,
}

/// Result of a single scrape attempt.
#[derive(Debug, Clone)]
pub struct ScrapeResult {
    pub target_id: Uuid,
    pub url: String,
    pub health: TargetHealth,
    pub samples: Vec<PrometheusSample>,
    pub metrics: Vec<Metric>,
    pub scrape_duration_ms: f64,
    pub sample_count: usize,
    pub error: Option<String>,
    pub scraped_at: chrono::DateTime<Utc>,
}

/// Configuration for the scraper.
#[derive(Debug, Clone)]
pub struct ScrapeConfig {
    pub scrape_interval: Duration,
    pub scrape_timeout: Duration,
    pub honor_labels: bool,
    pub honor_timestamps: bool,
    pub metrics_path: String,
    pub scheme: String,
}

impl Default for ScrapeConfig {
    fn default() -> Self {
        Self {
            scrape_interval: Duration::from_secs(15),
            scrape_timeout: Duration::from_secs(10),
            honor_labels: false,
            honor_timestamps: true,
            metrics_path: "/metrics".into(),
            scheme: "http".into(),
        }
    }
}

/// The Zentra metric scraper.
pub struct Scraper {
    config: ScrapeConfig,
    client: reqwest::Client,
    /// Track target health (shared for parallel access)
    health_map: Arc<Mutex<HashMap<Uuid, TargetHealth>>>,
}

impl Scraper {
    pub fn new(config: ScrapeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.scrape_timeout)
            .user_agent("Zentra-Scraper/0.1")
            .build()
            .unwrap_or_default();

        Self {
            config,
            client,
            health_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(ScrapeConfig::default())
    }

    /// Scrape a single target and return results.
    pub async fn scrape_target(&self, target: &ScrapeTarget) -> ScrapeResult {
        let url = format!(
            "{}://{}:{}{}",
            target.scheme.as_deref().unwrap_or(&self.config.scheme),
            target.host,
            target.port,
            target.metrics_path.as_deref().unwrap_or(&self.config.metrics_path),
        );

        let start = std::time::Instant::now();
        let scraped_at = Utc::now();

        match self.client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let error = format!("HTTP {}", resp.status());
                    warn!(url = %url, status = %resp.status(), "Scrape failed");
                    self.health_map.lock().insert(target.target_id, TargetHealth::Down);
                    return ScrapeResult {
                        target_id: target.target_id,
                        url,
                        health: TargetHealth::Down,
                        samples: Vec::new(),
                        metrics: Vec::new(),
                        scrape_duration_ms: start.elapsed().as_millis() as f64,
                        sample_count: 0,
                        error: Some(error),
                        scraped_at,
                    };
                }

                match resp.text().await {
                    Ok(body) => {
                        let samples = parse_prometheus_text(&body);
                        let metrics = self.convert_samples(target, &samples);
                        let count = samples.len();

                        self.health_map.lock().insert(target.target_id, TargetHealth::Up);

                        debug!(
                            url = %url,
                            samples = count,
                            duration_ms = start.elapsed().as_millis(),
                            "Scrape successful"
                        );

                        ScrapeResult {
                            target_id: target.target_id,
                            url,
                            health: TargetHealth::Up,
                            samples,
                            metrics,
                            scrape_duration_ms: start.elapsed().as_millis() as f64,
                            sample_count: count,
                            error: None,
                            scraped_at,
                        }
                    }
                    Err(e) => {
                        self.health_map.lock().insert(target.target_id, TargetHealth::Down);
                        ScrapeResult {
                            target_id: target.target_id,
                            url,
                            health: TargetHealth::Down,
                            samples: Vec::new(),
                            metrics: Vec::new(),
                            scrape_duration_ms: start.elapsed().as_millis() as f64,
                            sample_count: 0,
                            error: Some(format!("Failed to read body: {e}")),
                            scraped_at,
                        }
                    }
                }
            }
            Err(e) => {
                self.health_map.lock().insert(target.target_id, TargetHealth::Down);
                ScrapeResult {
                    target_id: target.target_id,
                    url,
                    health: TargetHealth::Down,
                    samples: Vec::new(),
                    metrics: Vec::new(),
                    scrape_duration_ms: start.elapsed().as_millis() as f64,
                    sample_count: 0,
                    error: Some(format!("Connection failed: {e}")),
                    scraped_at,
                }
            }
        }
    }

    /// Convert Prometheus samples to Zentra metrics.
    fn convert_samples(&self, target: &ScrapeTarget, samples: &[PrometheusSample]) -> Vec<Metric> {
        samples
            .iter()
            .map(|s| {
                let resource = s.labels.get("instance")
                    .or(s.labels.get("job"))
                    .cloned()
                    .unwrap_or_else(|| format!("{}:{}", target.host, target.port));

                let mut m = Metric::new(&s.metric_name, s.value, &resource)
                    .with_context(target.org_id, target.target_id, "zentra-scrape")
                    .with_resource_type("scrape_target");

                // Copy labels as tags
                for (k, v) in &s.labels {
                    m = m.with_tag(k, v);
                }

                // Add scrape job metadata
                if let Some(ref job) = target.job_name {
                    m = m.with_tag("job", job);
                }

                m
            })
            .collect()
    }

    /// Scrape all targets in parallel.
    pub async fn scrape_all(&self, targets: &[ScrapeTarget]) -> Vec<ScrapeResult> {
        let futures: Vec<_> = targets.iter().map(|t| self.scrape_target(t)).collect();
        let results = futures::future::join_all(futures).await;
        info!(targets = targets.len(), "Parallel batch scrape complete");
        results
    }

    /// Get health status for a target.
    pub fn target_health(&self, target_id: Uuid) -> TargetHealth {
        self.health_map.lock().get(&target_id).copied().unwrap_or(TargetHealth::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrape_config_defaults() {
        let config = ScrapeConfig::default();
        assert_eq!(config.scrape_interval, Duration::from_secs(15));
        assert_eq!(config.metrics_path, "/metrics");
        assert_eq!(config.scheme, "http");
    }

    #[test]
    fn test_scraper_creation() {
        let scraper = Scraper::with_defaults();
        assert_eq!(scraper.target_health(Uuid::new_v4()), TargetHealth::Unknown);
    }
}
