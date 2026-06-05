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
use tracing::info;

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
    /// Shared HTTP client with connection pooling for all outbound monitoring.
    /// Configured with keep-alive, idle TTL, and per-host connection limits.
    pub http_client: reqwest::Client,
    /// Async OTLP ingest queue — handlers enqueue, background workers batch-insert.
    pub ingest_queue: Option<crate::ingest_queue::OtlpIngestQueue>,
    /// Per-key RPM counters for OTLP quota enforcement (OT03).
    pub otlp_rpm_counters: crate::otlp_quota::RpmCounters,
    /// Per-key rate limiter placeholder (OT03).
    pub otlp_rate_limiter: Option<()>,
}

impl AppState {
    pub fn new(pool: DbPool, tsdb_pool: DbPool, event_bus: EventBus, org_id: uuid::Uuid, jwt_secret: String) -> Self {
        // Build a shared reqwest::Client with connection pooling.
        // This SINGLE client is reused for ALL outbound HTTP monitoring:
        // middleware (Nginx, Apache, HAProxy, …), cloud APIs (AWS/Azure/GCP),
        // storage REST APIs (NetApp, Pure, Dell EMC), and any other HTTP targets.
        let http_client = reqwest::Client::builder()
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(32)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .user_agent("Zentra-Monitor/1.0")
            .danger_accept_invalid_certs(
                std::env::var("ZENTRA_SKIP_TLS_VERIFY")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(false)
            )
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("Failed to build shared HTTP client");

        info!(
            pool_idle_per_host = 32,
            idle_timeout_secs = 90,
            connect_timeout_secs = 10,
            "Shared HTTP connection pool initialized"
        );

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
            ingest_queue: Some(crate::ingest_queue::spawn_ingest_workers(
                pool.clone(),
                tsdb_pool.clone(),
            )),
            otlp_rpm_counters: crate::otlp_quota::new_rpm_counters(),
            otlp_rate_limiter: Some(()),
            http_client,
            pool,
            tsdb_pool,
            event_bus,
            org_id,
        }
    }
}
