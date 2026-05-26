use zentra_db::DbPool;
use zentra_stream::{EventBus, EventHistory};
use zentra_drilldown::DrilldownEngine;
use zentra_query::metric_query::MetricQueryService;
use zentra_query::log_query::LogQueryService;
use zentra_query::global_search::GlobalSearchService;
use zentra_log_engine::correlation::CorrelationEngine;
use crate::target_pool::SharedTargetPoolCache;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared application state available to all API handlers
#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub tsdb_pool: DbPool,
    pub event_bus: EventBus,
    pub event_history: Arc<EventHistory>,
    pub drilldown: Arc<DrilldownEngine>,
    pub metric_query: Arc<MetricQueryService>,
    pub log_query: Arc<LogQueryService>,
    pub global_search: Arc<GlobalSearchService>,
    pub correlation: Arc<CorrelationEngine>,
    pub org_id: uuid::Uuid,
    /// Cached OTLP API keys: hash -> (org_id, permissions)
    pub otlp_key_cache: Arc<DashMap<String, (uuid::Uuid, Vec<String>)>>,
    /// Validated JWT signing secret (single source of truth).
    pub jwt_secret: Arc<str>,
    /// Cached outbound PgPools for monitored targets (max 64, 5-min idle TTL).
    pub target_pools: SharedTargetPoolCache,
    /// Per-instance demo/live mode flag.
    demo_mode: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(pool: DbPool, tsdb_pool: DbPool, event_bus: EventBus, org_id: uuid::Uuid, jwt_secret: String) -> Self {
        // Default: demo mode OFF unless ZENTRA_DEMO_MODE=true (must be explicit opt-in)
        let demo = std::env::var("ZENTRA_DEMO_MODE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        Self {
            drilldown: Arc::new(DrilldownEngine::new(pool.clone())),
            metric_query: Arc::new(MetricQueryService::new(tsdb_pool.clone())),
            log_query: Arc::new(LogQueryService::new(tsdb_pool.clone())),
            global_search: Arc::new(GlobalSearchService::new(pool.clone())),
            correlation: Arc::new(CorrelationEngine::new(tsdb_pool.clone())),
            event_history: Arc::new(EventHistory::new(10_000)),
            target_pools: crate::target_pool::new_cache(),
            otlp_key_cache: Arc::new(DashMap::new()),
            jwt_secret: jwt_secret.into(),
            pool,
            tsdb_pool,
            event_bus,
            org_id,
            demo_mode: Arc::new(AtomicBool::new(demo)),
        }
    }

    /// Check if this instance is in demo mode.
    pub fn is_demo_mode(&self) -> bool {
        self.demo_mode.load(Ordering::Relaxed)
    }

    /// Switch demo/live mode at runtime.
    pub fn set_demo_mode(&self, demo: bool) {
        self.demo_mode.store(demo, Ordering::Relaxed);
    }
}
