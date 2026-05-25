//! Oracle Scrape Scheduler
//!
//! Real background task that iterates all registered Oracle connections,
//! executes active metric definitions, stores results in cache,
//! tracks scrape meta-metrics, and supports per-metric intervals.
//!
//! Control via start/stop/force_scrape. Status exposed via API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::RwLock;
use lazy_static::lazy_static;
use tracing::{info, debug};

use crate::oracle_connection::{list_pools, PoolStatus};
use crate::oracle_metric_executor::execute_metric;
use crate::oracle_metrics_loader::{load_default_metrics, filter_for_connection};
use crate::oracle_cache;

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub global_interval_secs: u64,
    pub max_concurrent: usize,
    pub retry_attempts: u32,
    pub retry_delay_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            global_interval_secs: 60,
            max_concurrent: 5,
            retry_attempts: 3,
            retry_delay_secs: 5,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Status types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStatus {
    pub running: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub connections_scraped: u32,
    pub metrics_collected: u64,
    pub errors_total: u64,
    pub avg_duration_ms: u64,
    pub per_connection: HashMap<String, ConnectionScrapeStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionScrapeStatus {
    pub connection_id: String,
    pub name: String,
    pub last_scrape: Option<String>,
    pub duration_ms: u64,
    pub metrics_count: u32,
    pub error_count: u32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeMetaMetrics {
    pub total_scrapes: u64,
    pub successful_scrapes: u64,
    pub failed_scrapes: u64,
    pub avg_scrape_duration_ms: u64,
    pub max_scrape_duration_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub by_connection: HashMap<String, u64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Global state
// ═══════════════════════════════════════════════════════════════════════════

lazy_static! {
    static ref SCHEDULER_STATE: Arc<RwLock<SchedulerState>> =
        Arc::new(RwLock::new(SchedulerState::default()));
    static ref SCHEDULER_RUNNING: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref FORCE_SCRAPE_FLAG: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

#[derive(Debug, Default)]
struct SchedulerState {
    config: SchedulerConfig,
    last_run: Option<String>,
    total_scrapes: u64,
    successful_scrapes: u64,
    failed_scrapes: u64,
    total_duration_ms: u64,
    max_duration_ms: u64,
    cache_hits: u64,
    cache_misses: u64,
    per_connection: HashMap<String, ConnectionScrapeStatus>,
    /// Track last scrape time per connection+metric context
    last_scraped: HashMap<String, chrono::DateTime<chrono::Utc>>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// Start the background scrape loop. Spawns a tokio task.
pub fn start_scheduler(config: Option<SchedulerConfig>) {
    if SCHEDULER_RUNNING.load(Ordering::Relaxed) {
        info!("Scheduler already running, skipping start");
        return;
    }

    if let Some(cfg) = config {
        SCHEDULER_STATE.write().config = cfg;
    }

    SCHEDULER_RUNNING.store(true, Ordering::Relaxed);
    info!("Starting Oracle metric scheduler");

    tokio::spawn(async move {
        scheduler_loop().await;
    });
}

/// Stop the background scrape loop.
pub fn stop_scheduler() {
    info!("Stopping Oracle metric scheduler");
    SCHEDULER_RUNNING.store(false, Ordering::Relaxed);
}

/// Force an immediate scrape on next tick.
pub fn force_scrape() {
    info!("Force scrape requested");
    FORCE_SCRAPE_FLAG.store(true, Ordering::Relaxed);
}

/// Get current scheduler status.
pub fn get_status() -> SchedulerStatus {
    let state = SCHEDULER_STATE.read();
    let running = SCHEDULER_RUNNING.load(Ordering::Relaxed);
    let interval = state.config.global_interval_secs;
    let total = state.total_scrapes;
    let avg = if total > 0 { state.total_duration_ms / total } else { 0 };

    SchedulerStatus {
        running,
        last_run: state.last_run.clone(),
        next_run: if running {
            state.last_run.as_ref().and_then(|lr| {
                chrono::DateTime::parse_from_rfc3339(lr).ok().map(|dt| {
                    (dt + chrono::Duration::seconds(interval as i64)).to_rfc3339()
                })
            })
        } else {
            None
        },
        connections_scraped: state.per_connection.len() as u32,
        metrics_collected: state.successful_scrapes,
        errors_total: state.failed_scrapes,
        avg_duration_ms: avg,
        per_connection: state.per_connection.clone(),
    }
}

/// Get meta-metrics for monitoring the scraper itself.
pub fn get_meta_metrics() -> ScrapeMetaMetrics {
    let state = SCHEDULER_STATE.read();
    let total = state.total_scrapes;
    let avg = if total > 0 { state.total_duration_ms / total } else { 0 };
    let by_conn: HashMap<String, u64> = state.per_connection.iter()
        .map(|(k, v)| (k.clone(), v.metrics_count as u64))
        .collect();

    ScrapeMetaMetrics {
        total_scrapes: state.total_scrapes,
        successful_scrapes: state.successful_scrapes,
        failed_scrapes: state.failed_scrapes,
        avg_scrape_duration_ms: avg,
        max_scrape_duration_ms: state.max_duration_ms,
        cache_hits: state.cache_hits,
        cache_misses: state.cache_misses,
        by_connection: by_conn,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Scheduler loop
// ═══════════════════════════════════════════════════════════════════════════

async fn scheduler_loop() {
    let interval_secs = SCHEDULER_STATE.read().config.global_interval_secs;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));

    info!(interval = interval_secs, "Scheduler loop started");

    loop {
        interval.tick().await;

        if !SCHEDULER_RUNNING.load(Ordering::Relaxed) {
            info!("Scheduler stopped");
            break;
        }

        let forced = FORCE_SCRAPE_FLAG.swap(false, Ordering::Relaxed);
        if forced {
            debug!("Executing forced scrape");
        }

        run_scrape_cycle(forced).await;
    }
}

async fn run_scrape_cycle(force: bool) {
    let cycle_start = std::time::Instant::now();
    let now = chrono::Utc::now();

    // Get all connected pools
    let pools = list_pools();
    let connected: Vec<_> = pools.iter()
        .filter(|(_, status, _)| *status == PoolStatus::Connected)
        .collect();

    if connected.is_empty() {
        debug!("No connected pools, skipping scrape cycle");
        return;
    }

    // Load metric definitions
    let metrics = load_default_metrics();
    if metrics.is_empty() {
        debug!("No metric definitions loaded, skipping scrape cycle");
        return;
    }

    debug!(pools = connected.len(), metrics = metrics.len(), "Starting scrape cycle");

    let global_interval = SCHEDULER_STATE.read().config.global_interval_secs;
    let mut total_results = 0u64;
    let mut total_errors = 0u64;

    for (pool_id, _, _) in &connected {
        let conn_start = std::time::Instant::now();
        let mut conn_metrics = 0u32;
        let mut conn_errors = 0u32;

        // P4: filter to metrics scoped for this connection
        let conn_metrics_defs = filter_for_connection(&metrics, pool_id);

        for metric_def in &conn_metrics_defs {
            let scrape_key = format!("{}:{}", pool_id, metric_def.context);

            // Check per-metric interval
            if !force {
                let metric_interval = metric_def.scrape_interval.unwrap_or(global_interval as u32) as i64;
                let skip = {
                    let state = SCHEDULER_STATE.read();
                    state.last_scraped.get(&scrape_key)
                        .map(|last| (now - *last).num_seconds() < metric_interval)
                        .unwrap_or(false)
                }; // read guard dropped here
                if skip {
                    SCHEDULER_STATE.write().cache_hits += 1;
                    continue;
                }
            }

            // Execute metric
            let results = execute_metric(metric_def, pool_id);

            if results.is_empty() || results.iter().all(|r| r.error.is_some()) {
                conn_errors += 1;
                total_errors += 1;
            } else {
                let count = results.len() as u32;
                conn_metrics += count;
                total_results += count as u64;

                // Store in cache
                for result in &results {
                    oracle_cache::set_cached(pool_id, &metric_def.context, result);
                }
            }

            // Update last scraped time
            {
                let mut state = SCHEDULER_STATE.write();
                state.last_scraped.insert(scrape_key, now);
                state.cache_misses += 1;
            }
        }

        let conn_duration = conn_start.elapsed().as_millis() as u64;
        let conn_status = if conn_errors == 0 {
            "healthy".into()
        } else if conn_errors < conn_metrics {
            "degraded".into()
        } else {
            "error".into()
        };

        SCHEDULER_STATE.write().per_connection.insert(pool_id.clone(), ConnectionScrapeStatus {
            connection_id: pool_id.clone(),
            name: pool_id.clone(),
            last_scrape: Some(now.to_rfc3339()),
            duration_ms: conn_duration,
            metrics_count: conn_metrics,
            error_count: conn_errors,
            status: conn_status,
        });
    }

    let cycle_duration = cycle_start.elapsed().as_millis() as u64;
    let mut state = SCHEDULER_STATE.write();
    state.last_run = Some(now.to_rfc3339());
    state.total_scrapes += 1;
    state.successful_scrapes += total_results;
    state.failed_scrapes += total_errors;
    state.total_duration_ms += cycle_duration;
    if cycle_duration > state.max_duration_ms {
        state.max_duration_ms = cycle_duration;
    }

    debug!(
        pools = connected.len(),
        metrics = total_results,
        errors = total_errors,
        duration_ms = cycle_duration,
        "Scrape cycle complete"
    );
}
