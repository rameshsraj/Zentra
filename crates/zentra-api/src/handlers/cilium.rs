//! SW P02: Cilium / eBPF Network Monitoring Dashboard handlers.
//!
//! Provides status, aggregated metrics summary, and scrape target registration
//! for Cilium agent, Hubble, and operator Prometheus endpoints.

use axum::{extract::{State, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CiliumScrapeTarget {
    pub host: String,
    pub port: i32,
    pub component: String,
}

#[derive(Debug, Serialize)]
pub struct CiliumStatusResponse {
    pub scrape_targets: Vec<CiliumScrapeTarget>,
    pub last_scrape_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterCiliumTargetsRequest {
    pub agent_host: String,
    #[serde(default = "default_agent_port")]
    pub agent_port: i32,
    pub hubble_host: Option<String>,
    #[serde(default = "default_hubble_port")]
    pub hubble_port: i32,
    pub operator_host: Option<String>,
    #[serde(default = "default_operator_port")]
    pub operator_port: i32,
}

fn default_agent_port() -> i32 { 9962 }
fn default_hubble_port() -> i32 { 9965 }
fn default_operator_port() -> i32 { 9963 }

#[derive(Debug, Serialize)]
pub struct CiliumMetricsSummary {
    pub endpoint_states: serde_json::Value,
    pub drop_reasons: serde_json::Value,
    pub bpf_map_pressure: serde_json::Value,
    pub flow_verdicts: serde_json::Value,
    pub policy_import_errors: i64,
    pub identity_count: i64,
    pub hubble_drops: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct CiliumMetricsQuery {
    pub window: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/cilium/status — scrape target list + last scrape time
pub async fn cilium_status(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<CiliumStatusResponse>, ApiError> {
    let rows = sqlx::query_as::<_, CiliumTargetRow>(
        r#"SELECT host, port, COALESCE(job_name, 'cilium') AS component, last_scraped_at
           FROM scrape_targets
           WHERE org_id = $1 AND job_name IN ('cilium-agent', 'cilium-hubble', 'cilium-operator')
           ORDER BY job_name"#
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let last_scrape = rows.iter().filter_map(|r| r.last_scraped_at).max();
    let targets = rows.into_iter().map(|r| CiliumScrapeTarget {
        host: r.host,
        port: r.port as i32,
        component: r.component,
    }).collect();

    Ok(Json(CiliumStatusResponse { scrape_targets: targets, last_scrape_at: last_scrape }))
}

/// GET /api/cilium/metrics/summary — aggregated key metrics from scrape_results
pub async fn cilium_metrics_summary(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(_params): Query<CiliumMetricsQuery>,
) -> Result<Json<CiliumMetricsSummary>, ApiError> {
    // Query latest scraped cilium metrics from the metrics/scrape_results table
    let endpoint_states = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT COALESCE(jsonb_object_agg(tags->>'state', value), '{}')
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'cilium_endpoint_state'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or_else(|| json!({}));

    let drop_reasons = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT COALESCE(jsonb_object_agg(tags->>'reason', value), '{}')
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'cilium_drop_count_total'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or_else(|| json!({}));

    let bpf_map_pressure = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT COALESCE(jsonb_object_agg(tags->>'map_name', value), '{}')
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'cilium_bpf_map_pressure'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or_else(|| json!({}));

    let flow_verdicts = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT COALESCE(jsonb_object_agg(tags->>'verdict', value), '{}')
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'hubble_flows_processed_total'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or_else(|| json!({}));

    let policy_import_errors: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(value)::BIGINT, 0)
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'cilium_policy_import_errors_total'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(0);

    let identity_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(MAX(value)::BIGINT, 0)
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'cilium_identity_count'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(0);

    let hubble_drops = sqlx::query_scalar::<_, serde_json::Value>(
        r#"SELECT COALESCE(jsonb_object_agg(
               tags->>'reason' || '/' || COALESCE(tags->>'protocol',''), value
           ), '{}')
           FROM metrics
           WHERE org_id = $1 AND metric_name = 'hubble_drop_total'
             AND timestamp > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or_else(|| json!({}));

    Ok(Json(CiliumMetricsSummary {
        endpoint_states,
        drop_reasons,
        bpf_map_pressure,
        flow_verdicts,
        policy_import_errors,
        identity_count,
        hubble_drops,
    }))
}

/// POST /api/cilium/register-scrape-targets — register Cilium scrape endpoints
pub async fn register_cilium_targets(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<RegisterCiliumTargetsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut registered = Vec::new();

    // Agent
    let agent_id = Uuid::now_v7();
    sqlx::query(
        r#"INSERT INTO scrape_targets (target_id, org_id, host, port, scheme, metrics_path, job_name, scrape_interval_seconds, enabled, discovered_by)
           VALUES ($1, $2, $3, $4, 'http', '/metrics', 'cilium-agent', 15, TRUE, 'manual')
           ON CONFLICT (org_id, host, port, metrics_path) DO UPDATE SET enabled = TRUE, job_name = 'cilium-agent'"#
    )
    .bind(agent_id).bind(org_id).bind(&req.agent_host).bind(req.agent_port)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    registered.push(json!({"component": "cilium-agent", "host": req.agent_host, "port": req.agent_port}));

    // Hubble (optional)
    if let Some(ref hubble_host) = req.hubble_host {
        let hubble_id = Uuid::now_v7();
        sqlx::query(
            r#"INSERT INTO scrape_targets (target_id, org_id, host, port, scheme, metrics_path, job_name, scrape_interval_seconds, enabled, discovered_by)
               VALUES ($1, $2, $3, $4, 'http', '/metrics', 'cilium-hubble', 15, TRUE, 'manual')
               ON CONFLICT (org_id, host, port, metrics_path) DO UPDATE SET enabled = TRUE, job_name = 'cilium-hubble'"#
        )
        .bind(hubble_id).bind(org_id).bind(hubble_host).bind(req.hubble_port)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        registered.push(json!({"component": "cilium-hubble", "host": hubble_host, "port": req.hubble_port}));
    }

    // Operator (optional)
    if let Some(ref operator_host) = req.operator_host {
        let operator_id = Uuid::now_v7();
        sqlx::query(
            r#"INSERT INTO scrape_targets (target_id, org_id, host, port, scheme, metrics_path, job_name, scrape_interval_seconds, enabled, discovered_by)
               VALUES ($1, $2, $3, $4, 'http', '/metrics', 'cilium-operator', 15, TRUE, 'manual')
               ON CONFLICT (org_id, host, port, metrics_path) DO UPDATE SET enabled = TRUE, job_name = 'cilium-operator'"#
        )
        .bind(operator_id).bind(org_id).bind(operator_host).bind(req.operator_port)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
        registered.push(json!({"component": "cilium-operator", "host": operator_host, "port": req.operator_port}));
    }

    Ok(Json(json!({ "registered": registered, "count": registered.len() })))
}

// ============================================================================
// SW P06: Hubble gRPC Flow Collector — API handlers
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct HubbleFlowsQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub verdict: Option<String>,
    pub l7_type: Option<String>,
    pub limit: Option<i64>,
}

/// GET /api/hubble/status — Hubble connectivity and stats
pub async fn hubble_status(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Check if we have any Hubble-sourced flows (flow_version=100)
    let total_1h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM netflow_records WHERE flow_version = 100 AND received_at > NOW() - INTERVAL '1 hour'"
    )
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(0);

    let l7_1h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM netflow_records WHERE flow_version = 100 AND extra ? 'l7' AND received_at > NOW() - INTERVAL '1 hour'"
    )
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(0);

    let fps: f64 = sqlx::query_scalar(
        "SELECT COALESCE(COUNT(*)::FLOAT / 60.0, 0) FROM netflow_records WHERE flow_version = 100 AND received_at > NOW() - INTERVAL '1 minute'"
    )
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(0.0);

    let connected = total_1h > 0;

    Ok(Json(json!({
        "connected": connected,
        "flows_per_second": fps,
        "total_flows_1h": total_1h,
        "l7_flows_1h": l7_1h,
    })))
}

/// GET /api/hubble/flows — query L7 flows from hubble_l7_flows view
pub async fn hubble_flows(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Query(params): Query<HubbleFlowsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let from = params.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = params.to.unwrap_or_else(Utc::now);

    let mut where_clauses = vec![
        "received_at >= $1".to_string(),
        "received_at <= $2".to_string(),
    ];
    let mut bind_idx = 3;

    if let Some(ref v) = params.verdict {
        where_clauses.push(format!("verdict = ${}", bind_idx));
        bind_idx += 1;
        let _ = v;
    }
    if let Some(ref lt) = params.l7_type {
        where_clauses.push(format!("l7_type = ${}", bind_idx));
        let _ = lt;
    }

    let sql = format!(
        "SELECT received_at, src_ip, src_port, dst_ip, dst_port, protocol_name, verdict, drop_reason, l7_type, http_method, http_url, http_status, dns_query, kafka_topic FROM hubble_l7_flows WHERE {} ORDER BY received_at DESC LIMIT {}",
        where_clauses.join(" AND "),
        limit
    );

    let mut query = sqlx::query_as::<_, HubbleFlowRow>(&sql)
        .bind(from)
        .bind(to);

    if let Some(ref v) = params.verdict {
        query = query.bind(v);
    }
    if let Some(ref lt) = params.l7_type {
        query = query.bind(lt);
    }

    let rows = query
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let flows: Vec<serde_json::Value> = rows.iter().map(|r| {
        json!({
            "received_at": r.received_at,
            "src_ip": r.src_ip,
            "src_port": r.src_port,
            "dst_ip": r.dst_ip,
            "dst_port": r.dst_port,
            "protocol": r.protocol_name,
            "verdict": r.verdict,
            "drop_reason": r.drop_reason,
            "l7_type": r.l7_type,
            "http_method": r.http_method,
            "http_url": r.http_url,
            "http_status": r.http_status,
            "dns_query": r.dns_query,
            "kafka_topic": r.kafka_topic,
        })
    }).collect();

    Ok(Json(json!({ "flows": flows, "count": flows.len() })))
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct HubbleFlowRow {
    received_at: DateTime<Utc>,
    src_ip: Option<String>,
    src_port: Option<i32>,
    dst_ip: Option<String>,
    dst_port: Option<i32>,
    protocol_name: Option<String>,
    verdict: Option<String>,
    drop_reason: Option<String>,
    l7_type: Option<String>,
    http_method: Option<String>,
    http_url: Option<String>,
    http_status: Option<String>,
    dns_query: Option<String>,
    kafka_topic: Option<String>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct CiliumTargetRow {
    host: String,
    port: i32,
    component: String,
    last_scraped_at: Option<DateTime<Utc>>,
}
