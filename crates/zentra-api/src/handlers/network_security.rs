use axum::{
    extract::{State, Path, Query},
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
pub struct PaginatedQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub src_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/connections — paginated connection log
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_connections(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, timestamp, uid, host(src_ip)::text as src_ip, src_port, host(dest_ip)::text as dest_ip, dest_port, \
         proto, service, duration, bytes_orig, bytes_resp, pkts_orig, pkts_resp, conn_state, \
         community_id, geo_src, geo_dest \
         FROM connection_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2 OFFSET $3"
    ).bind(org_id).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "timestamp": r.get::<chrono::DateTime<chrono::Utc>, _>("timestamp"),
        "uid": r.get::<Option<String>, _>("uid"), "src_ip": r.get::<Option<String>, _>("src_ip"),
        "src_port": r.get::<Option<i32>, _>("src_port"), "dest_ip": r.get::<Option<String>, _>("dest_ip"),
        "dest_port": r.get::<Option<i32>, _>("dest_port"), "proto": r.get::<Option<String>, _>("proto"),
        "service": r.get::<Option<String>, _>("service"), "duration": r.get::<Option<f64>, _>("duration"),
        "bytes_orig": r.get::<i64, _>("bytes_orig"), "bytes_resp": r.get::<i64, _>("bytes_resp"),
        "pkts_orig": r.get::<i64, _>("pkts_orig"), "pkts_resp": r.get::<i64, _>("pkts_resp"),
        "conn_state": r.get::<Option<String>, _>("conn_state"),
        "community_id": r.get::<Option<String>, _>("community_id"),
        "geo_src": r.get::<Option<String>, _>("geo_src"), "geo_dest": r.get::<Option<String>, _>("geo_dest"),
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM connection_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"connections": entries, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/connections/:id — detail with linked protocol logs
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_connection(
    State(state): State<AppState>,
    Path(conn_id): Path<Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, (
        Uuid, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>, Option<i32>,
        Option<String>, Option<i32>, Option<String>, Option<String>, Option<f64>,
        i64, i64, Option<String>, Option<String>,
    )>(
        "SELECT id, timestamp, uid, host(src_ip)::text, src_port, host(dest_ip)::text, dest_port, \
         proto, service, duration, bytes_orig, bytes_resp, community_id, conn_state \
         FROM connection_logs WHERE id = $1"
    ).bind(conn_id).fetch_optional(&state.pool).await;

    match row {
        Ok(Some(r)) => {
            let conn = json!({
                "id": r.0, "timestamp": r.1, "uid": r.2, "src_ip": r.3, "src_port": r.4,
                "dest_ip": r.5, "dest_port": r.6, "proto": r.7, "service": r.8,
                "duration": r.9, "bytes_orig": r.10, "bytes_resp": r.11,
                "community_id": r.12, "conn_state": r.13,
            });
            (StatusCode::OK, Json(conn)).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "not_found"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/connections/stats — top talkers, protocol breakdown
// ═══════════════════════════════════════════════════════════════════════════

pub async fn connection_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let proto_dist = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT proto, COUNT(*) FROM connection_logs WHERE org_id = $1 GROUP BY proto ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let top_talkers = sqlx::query_as::<_, (Option<String>, i64, i64)>(
        "SELECT host(src_ip)::text, COUNT(*), SUM(bytes_orig + bytes_resp) FROM connection_logs \
         WHERE org_id = $1 GROUP BY src_ip ORDER BY SUM(bytes_orig + bytes_resp) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM connection_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "total_connections": total,
        "protocol_distribution": proto_dist.iter().map(|(p, c)| json!({"proto": p, "count": c})).collect::<Vec<_>>(),
        "top_talkers": top_talkers.iter().map(|(ip, c, b)| json!({"ip": ip, "connections": c, "bytes": b})).collect::<Vec<_>>(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/dns — DNS query log
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_dns(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        Uuid, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, serde_json::Value, Option<i32>, bool,
    )>(
        "SELECT id, timestamp, uid, host(src_ip)::text, query, qtype, rcode, \
         COALESCE(answers, '[]'), ttl, rejected \
         FROM dns_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2 OFFSET $3"
    ).bind(org_id).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.0, "timestamp": r.1, "uid": r.2, "src_ip": r.3,
        "query": r.4, "qtype": r.5, "rcode": r.6, "answers": r.7, "ttl": r.8, "rejected": r.9,
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dns_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"entries": entries, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/dns/stats
// ═══════════════════════════════════════════════════════════════════════════

pub async fn dns_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dns_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
    let nxdomain: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM dns_logs WHERE org_id = $1 AND rcode = 'NXDOMAIN'"
    ).bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    let top_domains = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT query, COUNT(*) FROM dns_logs WHERE org_id = $1 GROUP BY query ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "total_queries": total, "nxdomain_count": nxdomain,
        "top_domains": top_domains.iter().map(|(d, c)| json!({"domain": d, "count": c})).collect::<Vec<_>>(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/http — HTTP log
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_http(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        Uuid, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<i32>, Option<String>,
    )>(
        "SELECT id, timestamp, host(src_ip)::text, host(dest_ip)::text, method, host, uri, \
         user_agent, status_code, mime_type \
         FROM http_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2 OFFSET $3"
    ).bind(org_id).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.0, "timestamp": r.1, "src_ip": r.2, "dest_ip": r.3,
        "method": r.4, "host": r.5, "uri": r.6, "user_agent": r.7,
        "status_code": r.8, "mime_type": r.9,
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM http_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"entries": entries, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/http/stats
// ═══════════════════════════════════════════════════════════════════════════

pub async fn http_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let status_dist = sqlx::query_as::<_, (Option<i32>, i64)>(
        "SELECT status_code, COUNT(*) FROM http_logs WHERE org_id = $1 GROUP BY status_code ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let top_hosts = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT host, COUNT(*) FROM http_logs WHERE org_id = $1 GROUP BY host ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "status_distribution": status_dist.iter().map(|(s, c)| json!({"status_code": s, "count": c})).collect::<Vec<_>>(),
        "top_hosts": top_hosts.iter().map(|(h, c)| json!({"host": h, "count": c})).collect::<Vec<_>>(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ssl — SSL/TLS log with JA3
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_ssl(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        Uuid, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>,
    )>(
        "SELECT id, timestamp, host(src_ip)::text, host(dest_ip)::text, server_name, \
         version, cipher, ja3, ja3s, validation_status, not_valid_before, not_valid_after \
         FROM ssl_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2 OFFSET $3"
    ).bind(org_id).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.0, "timestamp": r.1, "src_ip": r.2, "dest_ip": r.3,
        "server_name": r.4, "version": r.5, "cipher": r.6,
        "ja3": r.7, "ja3s": r.8, "validation_status": r.9,
        "not_valid_before": r.10, "not_valid_after": r.11,
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ssl_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"entries": entries, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ssl/stats
// ═══════════════════════════════════════════════════════════════════════════

pub async fn ssl_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let version_dist = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT version, COUNT(*) FROM ssl_logs WHERE org_id = $1 GROUP BY version ORDER BY COUNT(*) DESC"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let cipher_dist = sqlx::query_as::<_, (Option<String>, i64)>(
        "SELECT cipher, COUNT(*) FROM ssl_logs WHERE org_id = $1 GROUP BY cipher ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let expiring = sqlx::query_as::<_, (Option<String>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT server_name, not_valid_after FROM ssl_logs WHERE org_id = $1 AND not_valid_after IS NOT NULL AND not_valid_after < NOW() + INTERVAL '30 days' ORDER BY not_valid_after LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "version_distribution": version_dist.iter().map(|(v, c)| json!({"version": v, "count": c})).collect::<Vec<_>>(),
        "cipher_distribution": cipher_dist.iter().map(|(ci, c)| json!({"cipher": ci, "count": c})).collect::<Vec<_>>(),
        "expiring_certs": expiring.iter().map(|(s, d)| json!({"server_name": s, "expires": d})).collect::<Vec<_>>(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ssh — SSH log with HASSH
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_ssh(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<PaginatedQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        Uuid, chrono::DateTime<chrono::Utc>, Option<String>, Option<String>,
        Option<i32>, Option<String>, Option<String>, Option<String>, bool,
    )>(
        "SELECT id, timestamp, host(src_ip)::text, host(dest_ip)::text, auth_attempts, \
         client, server, hassh, success \
         FROM ssh_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2 OFFSET $3"
    ).bind(org_id).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.0, "timestamp": r.1, "src_ip": r.2, "dest_ip": r.3,
        "auth_attempts": r.4, "client": r.5, "server": r.6, "hassh": r.7, "success": r.8,
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ssh_logs WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"entries": entries, "total": total}))).into_response()
}
