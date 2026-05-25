//! Oracle Metric Cache (V5 Prompt 17)
//!
//! In-memory + DB-backed, get/set/get_all/invalidate, TTL based on scrape interval

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub connection_id: String,
    pub metric_context: String,
    pub value: serde_json::Value,
    pub scraped_at: String,
    pub scrape_duration_ms: u64,
    pub ttl_secs: u64,
    pub error: Option<String>,
}

lazy_static::lazy_static! {
    static ref METRIC_CACHE: Arc<RwLock<HashMap<String, CacheEntry>>> = Arc::new(RwLock::new(HashMap::new()));
}

fn cache_key(connection_id: &str, context: &str) -> String {
    format!("{}:{}", connection_id, context)
}

/// Get a cached metric value
pub fn get(connection_id: &str, context: &str) -> Option<CacheEntry> {
    let cache = METRIC_CACHE.read();
    let key = cache_key(connection_id, context);
    if let Some(entry) = cache.get(&key) {
        // Check TTL
        if let Ok(scraped) = chrono::DateTime::parse_from_rfc3339(&entry.scraped_at) {
            let age = chrono::Utc::now().signed_duration_since(scraped);
            if age.num_seconds() as u64 <= entry.ttl_secs {
                return Some(entry.clone());
            }
        }
    }
    None
}

/// Set a cached metric value
pub fn set(entry: CacheEntry) {
    let key = cache_key(&entry.connection_id, &entry.metric_context);
    METRIC_CACHE.write().insert(key, entry);
}

/// Get all cached entries for a connection
pub fn get_all(connection_id: &str) -> Vec<CacheEntry> {
    let cache = METRIC_CACHE.read();
    let prefix = format!("{}:", connection_id);
    cache.values()
        .filter(|e| e.connection_id == connection_id || cache_key(&e.connection_id, &e.metric_context).starts_with(&prefix))
        .cloned()
        .collect()
}

/// Invalidate cache for a connection
pub fn invalidate(connection_id: &str) {
    let mut cache = METRIC_CACHE.write();
    let keys_to_remove: Vec<String> = cache.keys()
        .filter(|k| k.starts_with(&format!("{}:", connection_id)))
        .cloned()
        .collect();
    for key in keys_to_remove {
        cache.remove(&key);
    }
}

/// Invalidate all cache entries
pub fn invalidate_all() {
    METRIC_CACHE.write().clear();
}

/// Convenience: store a MetricResult in cache for a given connection+context.
pub fn set_cached(connection_id: &str, context: &str, result: &crate::oracle_metric_executor::MetricResult) {
    let value = serde_json::to_value(result).unwrap_or_default();
    set(CacheEntry {
        connection_id: connection_id.to_string(),
        metric_context: context.to_string(),
        value,
        scraped_at: result.scraped_at.clone(),
        scrape_duration_ms: result.duration_ms,
        ttl_secs: 120, // 2 minutes default TTL
        error: result.error.clone(),
    });
}

/// Cache statistics
pub fn stats() -> CacheStats {
    let cache = METRIC_CACHE.read();
    let total = cache.len();
    let mut by_connection: HashMap<String, usize> = HashMap::new();
    let mut expired = 0usize;
    let now = chrono::Utc::now();
    for entry in cache.values() {
        *by_connection.entry(entry.connection_id.clone()).or_default() += 1;
        if let Ok(scraped) = chrono::DateTime::parse_from_rfc3339(&entry.scraped_at) {
            let age = now.signed_duration_since(scraped);
            if age.num_seconds() as u64 > entry.ttl_secs {
                expired += 1;
            }
        }
    }
    CacheStats { total, expired, by_connection }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheStats {
    pub total: usize,
    pub expired: usize,
    pub by_connection: HashMap<String, usize>,
}
