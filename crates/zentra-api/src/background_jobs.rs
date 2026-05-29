//! Background workers for automated tasks (HA-safe via Postgres advisory locks):
//! - Materialized View refresh (RED metrics)
//! - Alert rule evaluation (multi-tenant)
//! - OTLP API key cache pruning
//!
//! In a multi-node HA deployment, each worker acquires a non-blocking Postgres
//! advisory lock before executing its payload. If the lock is already held by
//! another node, the tick is silently skipped — no duplicate work occurs.

use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn, error, debug};
use crate::state::AppState;

// Unique 64-bit lock IDs for Postgres advisory locks.
// These MUST be globally unique across the Zentra schema.
const RED_METRIC_MV_LOCK_ID: i64 = 48201;
const ALERT_EVAL_LOCK_ID: i64 = 48202;
const CACHE_PRUNE_LOCK_ID: i64 = 48203;

/// Try to acquire a non-blocking session-level advisory lock.
/// Returns `true` if the lock was acquired (caller should proceed),
/// `false` if another node already holds it (skip this tick).
async fn try_advisory_lock(pool: &sqlx::PgPool, lock_id: i64) -> bool {
    match sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_lock($1)")
        .bind(lock_id)
        .fetch_one(pool)
        .await
    {
        Ok(acquired) => acquired,
        Err(e) => {
            warn!(lock_id, "Advisory lock query failed: {}", e);
            false
        }
    }
}

/// Release a session-level advisory lock.
async fn release_advisory_lock(pool: &sqlx::PgPool, lock_id: i64) {
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(lock_id)
        .execute(pool)
        .await
    {
        warn!(lock_id, "Advisory unlock failed: {}", e);
    }
}

/// Start all background workers.
pub fn start_background_jobs(state: AppState) {
    // 1. Materialized View Refresh (every 1 minute)
    let state_mv = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(60));
        tick.tick().await; // Skip first immediate tick
        info!("Background: RED metrics MV refresh worker started (HA-safe)");

        loop {
            tick.tick().await;

            if !try_advisory_lock(&state_mv.tsdb_pool, RED_METRIC_MV_LOCK_ID).await {
                debug!("Background: MV refresh skipped — another node holds the lock");
                continue;
            }

            // Try CONCURRENTLY first (requires unique index on the MV)
            let result = sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY span_red_metrics_1m")
                .execute(&state_mv.tsdb_pool)
                .await;

            if let Err(e) = result {
                // Fallback to standard (blocking) refresh if unique index is missing
                warn!("Background: CONCURRENTLY refresh failed ({}), trying standard refresh", e);
                if let Err(e2) = sqlx::query("REFRESH MATERIALIZED VIEW span_red_metrics_1m")
                    .execute(&state_mv.tsdb_pool)
                    .await
                {
                    error!("Background: MV refresh failed: {} (fallback: {})", e, e2);
                }
            }

            release_advisory_lock(&state_mv.tsdb_pool, RED_METRIC_MV_LOCK_ID).await;
        }
    });

    // 2. Alert Rule Evaluation (every 5 minutes)
    let state_alerts = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(300));
        tick.tick().await;
        info!("Background: OTel alert evaluation worker started (HA-safe)");

        loop {
            tick.tick().await;

            if !try_advisory_lock(&state_alerts.tsdb_pool, ALERT_EVAL_LOCK_ID).await {
                debug!("Background: Alert eval skipped — another node holds the lock");
                continue;
            }

            // Evaluate alert rules for the default org
            let org_id = state_alerts.org_id;
            match crate::handlers::otlp::evaluate_alert_rules_internal(&state_alerts, org_id).await {
                Ok(evals) => {
                    let firing = evals.iter().filter(|e| e["status"] == "FIRING").count();
                    if firing > 0 {
                        warn!(org_id = %org_id, firing, "Background: Alert rules firing");
                    }

                    // Write firing alerts to active_alerts table
                    for eval in &evals {
                        if eval["status"] == "FIRING" {
                            let rule_name = eval["rule"].as_str().unwrap_or("unknown");
                            let metric = eval["metric"].as_str().unwrap_or("unknown");
                            let current = eval["current"].as_f64().unwrap_or(0.0);
                            let threshold = eval["threshold"].as_f64().unwrap_or(0.0);

                            let _ = sqlx::query(
                                r#"INSERT INTO active_alerts (org_id, rule_name, metric, current_value, threshold, severity, fired_at)
                                   VALUES ($1, $2, $3, $4, $5, 'warning', NOW())
                                   ON CONFLICT (org_id, rule_name) DO UPDATE SET current_value = $4, fired_at = NOW()"#
                            )
                            .bind(org_id)
                            .bind(rule_name)
                            .bind(metric)
                            .bind(current)
                            .bind(threshold)
                            .execute(&state_alerts.tsdb_pool)
                            .await;
                        }
                    }
                }
                Err(e) => {
                    error!("Background: Alert eval failed for org {}: {}", org_id, e);
                }
            }

            release_advisory_lock(&state_alerts.tsdb_pool, ALERT_EVAL_LOCK_ID).await;
        }
    });

    // 3. API Key Cache Pruning (every 1 hour)
    let state_cache = state.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(3600));
        tick.tick().await;
        info!("Background: API key cache pruner started (HA-safe)");

        loop {
            tick.tick().await;

            if !try_advisory_lock(&state_cache.tsdb_pool, CACHE_PRUNE_LOCK_ID).await {
                debug!("Background: Cache prune skipped — another node holds the lock");
                continue;
            }

            // Clear the in-memory cache to force re-validation against DB
            state_cache.otlp_key_cache.clear();
            info!("Background: OTLP API key cache pruned ({} entries cleared)", 0);

            release_advisory_lock(&state_cache.tsdb_pool, CACHE_PRUNE_LOCK_ID).await;
        }
    });
}
