//! Connection Pool Health & Status API
//!
//! Provides a unified view of all connection pools across the platform:
//!   - Internal PostgreSQL pools (main + TimescaleDB)
//!   - Target database pool cache (outbound PgPools)
//!   - Oracle native connection pools
//!   - SSH multiplexed connections
//!   - Shared HTTP client pool

use axum::{extract::State, Json};
use serde::Serialize;
use serde_json::{json, Value};
use crate::state::AppState;

#[derive(Debug, Serialize)]
struct PoolSummary {
    pool_name: String,
    pool_type: String,
    status: String,
    active: u32,
    idle: u32,
    max: u32,
    details: Value,
}

/// GET /api/system/pool-status — unified connection pool health
pub async fn pool_status(State(state): State<AppState>) -> Json<Value> {
    let mut pools: Vec<PoolSummary> = Vec::new();

    // 1. Internal PostgreSQL pool (main)
    {
        let sz = state.pool.size();
        let idle = state.pool.num_idle();
        pools.push(PoolSummary {
            pool_name: "main_postgres".into(),
            pool_type: "sqlx::PgPool".into(),
            status: "healthy".into(),
            active: sz - idle as u32,
            idle: idle as u32,
            max: state.pool.options().get_max_connections(),
            details: json!({
                "min_connections": state.pool.options().get_min_connections(),
                "max_connections": state.pool.options().get_max_connections(),
                "acquire_timeout_secs": state.pool.options().get_acquire_timeout().as_secs(),
                "idle_timeout_secs": state.pool.options().get_idle_timeout().map(|d| d.as_secs()),
            }),
        });
    }

    // 2. TimescaleDB pool
    {
        let sz = state.tsdb_pool.size();
        let idle = state.tsdb_pool.num_idle();
        pools.push(PoolSummary {
            pool_name: "timescaledb".into(),
            pool_type: "sqlx::PgPool".into(),
            status: "healthy".into(),
            active: sz - idle as u32,
            idle: idle as u32,
            max: state.tsdb_pool.options().get_max_connections(),
            details: json!({
                "min_connections": state.tsdb_pool.options().get_min_connections(),
                "max_connections": state.tsdb_pool.options().get_max_connections(),
            }),
        });
    }

    // 3. Target database pool cache
    {
        let count = state.target_pools.len();
        pools.push(PoolSummary {
            pool_name: "target_db_cache".into(),
            pool_type: "LRU<PgPool>".into(),
            status: if count > 0 { "active" } else { "empty" }.into(),
            active: count as u32,
            idle: 0,
            max: 64,
            details: json!({
                "cached_pools": count,
                "max_cached": 64,
                "idle_ttl_secs": 300,
            }),
        });
    }

    // 4. Oracle native connection pools
    {
        let oracle_pools = database_plugin::oracle_connection::list_pools();
        let oracle_count = oracle_pools.len();
        let connected = oracle_pools.iter().filter(|(_, s, _)| format!("{:?}", s).contains("Connected")).count();
        let pool_ids: Vec<&str> = oracle_pools.iter().map(|(id, _, _)| id.as_str()).collect();
        pools.push(PoolSummary {
            pool_name: "oracle_native".into(),
            pool_type: "OCI Pool".into(),
            status: if connected > 0 { "active" } else if oracle_count > 0 { "degraded" } else { "empty" }.into(),
            active: connected as u32,
            idle: (oracle_count - connected) as u32,
            max: oracle_count as u32,
            details: json!({
                "registered_pools": pool_ids,
                "connected": connected,
                "total": oracle_count,
            }),
        });
    }

    // 5. SSH multiplexed connections
    {
        let ssh_stats = zentra_core::ssh_pool::global_ssh_pool().stats();
        pools.push(PoolSummary {
            pool_name: "ssh_mux".into(),
            pool_type: "ControlMaster".into(),
            status: if ssh_stats.active_masters > 0 { "active" } else { "idle" }.into(),
            active: ssh_stats.active_masters as u32,
            idle: 0,
            max: 0, // no hard limit
            details: json!({
                "active_masters": ssh_stats.active_masters,
                "total_commands": ssh_stats.total_commands,
                "control_dir": ssh_stats.control_dir,
                "persist_secs": ssh_stats.persist_secs,
                "connections": ssh_stats.connections,
            }),
        });
    }

    // 6. Shared HTTP client
    {
        pools.push(PoolSummary {
            pool_name: "http_shared".into(),
            pool_type: "reqwest::Client".into(),
            status: "healthy".into(),
            active: 0, // reqwest doesn't expose active count
            idle: 0,
            max: 0,
            details: json!({
                "pool_idle_per_host": 32,
                "idle_timeout_secs": 90,
                "connect_timeout_secs": 10,
                "request_timeout_secs": 30,
                "tcp_keepalive_secs": 60,
                "description": "Single shared client for all outbound HTTP monitoring (middleware, cloud, storage APIs)",
            }),
        });
    }

    let total_active: u32 = pools.iter().map(|p| p.active).sum();
    let total_pools = pools.len();
    let all_healthy = pools.iter().all(|p| p.status != "error");

    Json(json!({
        "overall_status": if all_healthy { "healthy" } else { "degraded" },
        "total_pool_types": total_pools,
        "total_active_connections": total_active,
        "pools": pools,
    }))
}
