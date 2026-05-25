//! SNMP Trap management API — backed by `snmp_traps` and `snmp_trap_rules` tables.

use axum::{
    extract::{State, Query, Path},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;
use axum::http::StatusCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapRecord {
    pub id: Uuid,
    pub received_at: DateTime<Utc>,
    pub source_ip: String,
    pub source_port: Option<i32>,
    pub version: String,
    pub community: Option<String>,
    pub enterprise_oid: Option<String>,
    pub trap_type: Option<String>,
    pub varbinds: serde_json::Value,
    pub severity: String,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapRule {
    pub id: Uuid,
    pub name: String,
    pub oid_pattern: Option<String>,
    pub source_ip_pattern: Option<String>,
    pub severity_map: serde_json::Value,
    pub auto_acknowledge: bool,
    pub create_alert: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTrapRuleRequest {
    pub name: String,
    pub oid_pattern: Option<String>,
    pub source_ip_pattern: Option<String>,
    pub severity_map: Option<serde_json::Value>,
    pub auto_acknowledge: Option<bool>,
    pub create_alert: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct TrapQuery {
    pub source_ip: Option<String>,
    pub severity: Option<String>,
    pub limit: Option<i64>,
}

pub async fn recent_traps(
    State(state): State<AppState>,
    Query(q): Query<TrapQuery>,
) -> Result<Json<Vec<TrapRecord>>, ApiError> {
    let limit = q.limit.unwrap_or(100);
    let rows = sqlx::query_as::<_, (Uuid, DateTime<Utc>, String, Option<i32>, String, Option<String>, Option<String>, Option<String>, Option<serde_json::Value>, String, bool)>(
        "SELECT id, received_at, source_ip::TEXT, source_port, version,
                community, enterprise_oid, trap_type,
                COALESCE(varbinds, '{}'), severity, acknowledged
         FROM snmp_traps
         WHERE ($1::TEXT IS NULL OR source_ip::TEXT = $1)
           AND ($2::TEXT IS NULL OR severity = $2)
         ORDER BY received_at DESC
         LIMIT $3",
    )
    .bind(q.source_ip)
    .bind(q.severity)
    .bind(limit)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let records = rows.into_iter().map(|(id, received_at, source_ip, source_port, version, community, enterprise_oid, trap_type, varbinds, severity, acknowledged)| {
        TrapRecord {
            id, received_at, source_ip, source_port, version,
            community, enterprise_oid, trap_type,
            varbinds: varbinds.unwrap_or(serde_json::Value::Object(Default::default())),
            severity, acknowledged,
        }
    }).collect();
    Ok(Json(records))
}

pub async fn trap_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let total_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM snmp_traps WHERE received_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let by_severity: Vec<(String, i64)> = sqlx::query_as(
        "SELECT severity, COUNT(*)::BIGINT FROM snmp_traps
         WHERE received_at >= NOW() - INTERVAL '24 hours'
         GROUP BY severity",
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let top_sources: Vec<(String, i64)> = sqlx::query_as(
        "SELECT source_ip::TEXT, COUNT(*)::BIGINT as cnt FROM snmp_traps
         WHERE received_at >= NOW() - INTERVAL '24 hours'
         GROUP BY source_ip
         ORDER BY cnt DESC
         LIMIT 10",
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(serde_json::json!({
        "total_24h": total_24h,
        "by_severity": by_severity.into_iter().map(|(k, v)| serde_json::json!({ "severity": k, "count": v })).collect::<Vec<_>>(),
        "top_sources": top_sources.into_iter().map(|(ip, c)| serde_json::json!({ "source_ip": ip, "count": c })).collect::<Vec<_>>(),
    })))
}

pub async fn acknowledge_trap(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query(
        "UPDATE snmp_traps SET acknowledged = TRUE WHERE id = $1",
    )
    .bind(id)
    .execute(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::from((StatusCode::NOT_FOUND, format!("trap {} not found", id))));
    }
    Ok(Json(serde_json::json!({ "acknowledged": id })))
}

pub async fn list_trap_rules(
    State(state): State<AppState>,
) -> Result<Json<Vec<TrapRule>>, ApiError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<String>, serde_json::Value, bool, bool, DateTime<Utc>)>(
        "SELECT id, name, oid_pattern, source_ip_pattern, severity_map,
                auto_acknowledge, create_alert, created_at
         FROM snmp_trap_rules
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let rules = rows.into_iter().map(|(id, name, oid_pattern, source_ip_pattern, severity_map, auto_acknowledge, create_alert, created_at)| {
        TrapRule { id, name, oid_pattern, source_ip_pattern, severity_map, auto_acknowledge, create_alert, created_at }
    }).collect();
    Ok(Json(rules))
}

pub async fn create_trap_rule(
    State(state): State<AppState>,
    Json(req): Json<CreateTrapRuleRequest>,
) -> Result<Json<TrapRule>, ApiError> {
    let id = Uuid::new_v4();
    let severity_map = req.severity_map.unwrap_or_else(|| serde_json::json!({}));
    let auto_acknowledge = req.auto_acknowledge.unwrap_or(false);
    let create_alert = req.create_alert.unwrap_or(true);

    sqlx::query(
        "INSERT INTO snmp_trap_rules (id, name, oid_pattern, source_ip_pattern, severity_map, auto_acknowledge, create_alert)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.oid_pattern)
    .bind(&req.source_ip_pattern)
    .bind(&severity_map)
    .bind(auto_acknowledge)
    .bind(create_alert)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(TrapRule {
        id,
        name: req.name,
        oid_pattern: req.oid_pattern,
        source_ip_pattern: req.source_ip_pattern,
        severity_map,
        auto_acknowledge,
        create_alert,
        created_at: Utc::now(),
    }))
}

pub async fn delete_trap_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("DELETE FROM snmp_trap_rules WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::from((StatusCode::NOT_FOUND, format!("trap rule {} not found", id))));
    }
    Ok(Json(serde_json::json!({ "deleted": id })))
}
