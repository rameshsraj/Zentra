//! Oracle Multi-DB Scrape (V5 Prompt 17)
//!
//! Parallel scrape via tokio::spawn, per-DB labels, health indicator, duplicate detection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiDbTarget {
    pub connection_id: String,
    pub name: String,
    pub host: String,
    pub service_name: String,
    pub status: DbScrapeStatus,
    pub labels: HashMap<String, String>,
    pub last_scrape: Option<String>,
    pub last_duration_ms: Option<u64>,
    pub metrics_collected: u32,
    pub errors: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DbScrapeStatus {
    Healthy,
    Degraded,
    Down,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeResult {
    pub connection_id: String,
    pub metrics: Vec<ScrapedMetric>,
    pub duration_ms: u64,
    pub errors: Vec<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapedMetric {
    pub context: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
    pub metric_type: String,
}

/// Determine health from scrape results
pub fn assess_health(result: &ScrapeResult) -> DbScrapeStatus {
    if result.errors.len() > result.metrics.len() / 2 {
        DbScrapeStatus::Down
    } else if !result.errors.is_empty() {
        DbScrapeStatus::Degraded
    } else {
        DbScrapeStatus::Healthy
    }
}

/// Check for duplicate metrics across databases
pub fn detect_duplicates(results: &[ScrapeResult]) -> Vec<String> {
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();
    let mut dupes = Vec::new();
    for r in results {
        for m in &r.metrics {
            let key = format!("{}:{:?}", m.context, m.labels);
            seen.entry(key.clone()).or_default().push(r.connection_id.clone());
            if seen[&key].len() == 2 {
                dupes.push(format!("Metric {} found in multiple DBs: {:?}", key, seen[&key]));
            }
        }
    }
    dupes
}
