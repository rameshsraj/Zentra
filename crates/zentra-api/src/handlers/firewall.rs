use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::{self, Row};
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;

#[derive(Debug, Deserialize)]
pub struct FwQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub action: Option<String>,
    pub src_ip: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. GET /api/security/firewall/logs
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_firewall_logs(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<FwQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, timestamp, host(src_ip)::text as src_ip, host(dest_ip)::text as dest_ip, \
         src_port, dest_port, proto, action, rule_name, interface, bytes, geo_src, geo_dest \
         FROM firewall_logs WHERE org_id = $1 \
         AND ($2::text IS NULL OR action = $2) \
         ORDER BY timestamp DESC LIMIT $3 OFFSET $4"
    ).bind(org_id).bind(p.action.as_deref()).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "timestamp": r.get::<chrono::DateTime<chrono::Utc>, _>("timestamp"),
        "src_ip": r.get::<Option<String>, _>("src_ip"), "dest_ip": r.get::<Option<String>, _>("dest_ip"),
        "src_port": r.get::<Option<i32>, _>("src_port"), "dest_port": r.get::<Option<i32>, _>("dest_port"),
        "proto": r.get::<Option<String>, _>("proto"), "action": r.get::<String, _>("action"),
        "rule_name": r.get::<Option<String>, _>("rule_name"), "interface": r.get::<Option<String>, _>("interface"),
        "bytes": r.get::<Option<i64>, _>("bytes"),
        "geo_src": r.get::<Option<String>, _>("geo_src"), "geo_dest": r.get::<Option<String>, _>("geo_dest"),
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM firewall_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"entries": entries, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. GET /api/security/firewall/rules
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_firewall_rules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let rows = sqlx::query(
        "SELECT id, rule_name, action, proto, host(src_cidr)::text as src_cidr, host(dest_cidr)::text as dest_cidr, \
         dest_port, interface, enabled, hit_count, created_at \
         FROM firewall_rules WHERE org_id = $1 ORDER BY created_at"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let rules: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "rule_name": r.get::<String, _>("rule_name"),
        "action": r.get::<String, _>("action"), "proto": r.get::<Option<String>, _>("proto"),
        "src_cidr": r.get::<Option<String>, _>("src_cidr"), "dest_cidr": r.get::<Option<String>, _>("dest_cidr"),
        "dest_port": r.get::<Option<i32>, _>("dest_port"), "interface": r.get::<Option<String>, _>("interface"),
        "enabled": r.get::<bool, _>("enabled"), "hit_count": r.get::<i64, _>("hit_count"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
    })).collect();

    (StatusCode::OK, Json(json!({"rules": rules}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. GET /api/security/firewall/stats
// ═══════════════════════════════════════════════════════════════════════════

pub async fn firewall_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let action_dist = sqlx::query_as::<_, (String, i64)>(
        "SELECT action, COUNT(*) FROM firewall_logs WHERE org_id = $1 GROUP BY action ORDER BY COUNT(*) DESC"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let top_blocked = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT host(src_ip)::text, COUNT(*) FROM firewall_logs WHERE org_id = $1 AND action IN ('drop', 'reject') \
         GROUP BY src_ip ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM firewall_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "total_events": total,
        "action_distribution": action_dist.iter().map(|(a, c)| json!({"action": a, "count": c})).collect::<Vec<_>>(),
        "top_blocked_ips": top_blocked.iter().map(|(ip, c)| json!({"ip": ip, "count": c})).collect::<Vec<_>>(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. POST /api/security/firewall/ingest
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct IngestFwRequest {
    pub entries: Vec<serde_json::Value>,
}

pub async fn ingest_firewall(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<IngestFwRequest>,
) -> impl IntoResponse {
    let mut inserted = 0i64;
    for entry in &req.entries {
        let r = sqlx::query(
            "INSERT INTO firewall_logs (id, org_id, timestamp, src_ip, dest_ip, src_port, dest_port, proto, action, rule_name, interface, bytes) \
             VALUES ($1, $2, COALESCE($3::timestamptz, NOW()), $4::inet, $5::inet, $6, $7, $8, $9, $10, $11, $12)"
        )
        .bind(Uuid::new_v4()).bind(org_id)
        .bind(entry.get("timestamp").and_then(|v| v.as_str()))
        .bind(entry.get("src_ip").and_then(|v| v.as_str()))
        .bind(entry.get("dest_ip").and_then(|v| v.as_str()))
        .bind(entry.get("src_port").and_then(|v| v.as_i64()).map(|v| v as i32))
        .bind(entry.get("dest_port").and_then(|v| v.as_i64()).map(|v| v as i32))
        .bind(entry.get("proto").and_then(|v| v.as_str()))
        .bind(entry.get("action").and_then(|v| v.as_str()).unwrap_or("accept"))
        .bind(entry.get("rule_name").and_then(|v| v.as_str()))
        .bind(entry.get("interface").and_then(|v| v.as_str()))
        .bind(entry.get("bytes").and_then(|v| v.as_i64()))
        .execute(&state.pool).await;
        if r.is_ok() { inserted += 1; }
    }
    (StatusCode::OK, Json(json!({"inserted": inserted}))).into_response()
}
