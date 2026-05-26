//! Background workers for automated tasks:
//! - Materialized View refresh (RED metrics)
//! - Alert rule evaluation
//! - OTLP API key cache pruning

use std::time::Duration;
use tokio::time::interval;
use tracing::{info, error};
use crate::state::AppState;

/// Start all background workers.
pub fn start_background_jobs(state: AppState) {
    // 1. Materialized View Refresh (every 1 minute)
    let state_mv = state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
        interval.tick().await; // Skip first tick
        info!("Background: RED metrics MV refresh worker started");
        
        loop {
            interval.tick().await;
            if let Err(e) = sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY span_red_metrics_1m")
                .execute(&state_mv.tsdb_pool)
                .await
            {
                // Note: CONCURRENTLY requires a unique index on the MV. 
                // If it fails, fallback to standard refresh if index missing.
                if let Err(e2) = sqlx::query("REFRESH MATERIALIZED VIEW span_red_metrics_1m")
                    .execute(&state_mv.tsdb_pool)
                    .await {
                    error!("Background: Failed to refresh RED metrics MV: {} (fallback also failed: {})", e, e2);
                }
            }
        }
    });

    // 2. Alert Rule Evaluation (every 5 minutes)
    let state_alerts = state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(300));
        interval.tick().await;
        info!("Background: OTel alert evaluation worker started");

        loop {
            interval.tick().await;
            // For now, we call the existing evaluation logic for each org.
            // In production, this would be a more complex multi-tenant worker.
            // We use the default org_id for now as an MVP.
            let org_id = state_alerts.org_id;
            if let Err(e) = crate::handlers::otlp::evaluate_alert_rules_internal(&state_alerts, org_id).await {
                error!("Background: Failed to evaluate alert rules for org {}: {}", org_id, e);
            }
        }
    });

    // 3. API Key Cache Pruning (every 1 hour)
    let state_cache = state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(3600));
        interval.tick().await;
        info!("Background: API key cache pruner started");

        loop {
            interval.tick().await;
            // Clear the cache to force re-validation against DB (handles revocation/expiry)
            state_cache.otlp_key_cache.clear();
            info!("Background: OTLP API key cache pruned");
        }
    });
}
