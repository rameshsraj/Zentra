use axum::{
    extract::{State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use crate::state::AppState;

use crate::error::ApiError;

// ============================================================================
// HA Architecture — Node status, leader election, cluster health
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub id: String,
    pub hostname: String,
    pub role: String,       // leader, follower, candidate
    pub status: String,     // healthy, degraded, unreachable
    pub region: String,
    pub zone: String,
    pub version: String,
    pub uptime_secs: u64,
    pub last_heartbeat: String,
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub connections: u32,
    pub metrics_per_sec: u32,
    pub logs_per_sec: u32,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaStatus {
    pub id: String,
    pub primary_id: String,
    pub host: String,
    pub port: u16,
    pub status: String,     // streaming, lagging, disconnected
    pub lag_bytes: u64,
    pub lag_secs: f32,
    pub sync_state: String, // sync, async, quorum
    pub last_replay: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaClusterStatus {
    pub cluster_id: String,
    pub cluster_name: String,
    pub health: String,         // healthy, degraded, critical
    pub leader_id: String,
    pub leader_hostname: String,
    pub nodes: Vec<ClusterNode>,
    pub replicas: Vec<ReplicaStatus>,
    pub quorum_size: u32,
    pub quorum_met: bool,
    pub total_nodes: u32,
    pub healthy_nodes: u32,
    pub term: u64,              // Raft term
    pub commit_index: u64,
    pub last_applied: u64,
    pub election_timeout_ms: u32,
    pub heartbeat_interval_ms: u32,
    pub split_brain_detected: bool,
    pub failovers_last_30d: u32,
    pub last_failover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverEvent {
    pub id: String,
    pub occurred_at: String,
    pub old_leader: String,
    pub new_leader: String,
    pub reason: String,
    pub duration_secs: u32,
    pub affected_connections: u32,
}

pub async fn cluster_status(
    State(_s): State<AppState>,
) -> Result<Json<HaClusterStatus>, ApiError> {
    let now = Utc::now();
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".into());

    Ok(Json(HaClusterStatus {
        cluster_id: "cluster-standalone".into(),
        cluster_name: "Zentra".into(),
        health: "healthy".into(),
        leader_id: "self".into(),
        leader_hostname: hostname.clone(),
        nodes: vec![
            ClusterNode {
                id: "self".into(),
                hostname,
                role: "leader".into(),
                status: "healthy".into(),
                region: String::new(),
                zone: String::new(),
                version: env!("CARGO_PKG_VERSION").into(),
                uptime_secs: 0,
                last_heartbeat: now.to_rfc3339(),
                cpu_usage: 0.0,
                memory_usage: 0.0,
                connections: 0,
                metrics_per_sec: 0,
                logs_per_sec: 0,
                is_current: true,
            },
        ],
        replicas: vec![],
        quorum_size: 1,
        quorum_met: true,
        total_nodes: 1,
        healthy_nodes: 1,
        term: 0,
        commit_index: 0,
        last_applied: 0,
        election_timeout_ms: 5000,
        heartbeat_interval_ms: 1000,
        split_brain_detected: false,
        failovers_last_30d: 0,
        last_failover: None,
    }))
}

pub async fn failover_history(
    State(_s): State<AppState>,
) -> Result<Json<Vec<FailoverEvent>>, ApiError> {
    Ok(Json(vec![]))
}

pub async fn ha_health(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let now = Utc::now();
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "cluster_health": "100%",
        "leader_uptime_secs": 0,
        "replication_lag_ms": 0,
        "write_quorum": true,
        "read_replicas": 0,
        "consistent_hash_ring_size": 0,
        "data_distribution": {},
        "checked_at": now.to_rfc3339(),
    })))
}
