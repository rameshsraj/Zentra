//! Traces & OTLP Ingestion — Distributed tracing support
//!
//! Provides trace search, span detail, service enumeration, and OTLP JSON ingestion.

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use crate::state::AppState;
use crate::tenant::OrgId;
use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub service_name: String,
    pub start_time: DateTime<Utc>,
    pub duration_us: i64,
    pub status: String,
    pub tags: HashMap<String, String>,
    pub events: Vec<SpanEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub name: String,
    pub timestamp: DateTime<Utc>,
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    pub trace_id: String,
    pub root_service: String,
    pub root_operation: String,
    pub total_duration_us: i64,
    pub span_count: usize,
    pub has_errors: bool,
    pub services: Vec<String>,
    pub started_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct TraceSearchParams {
    pub service: Option<String>,
    pub operation: Option<String>,
    pub min_duration_us: Option<i64>,
    pub max_duration_us: Option<i64>,
    pub limit: Option<usize>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// GET /api/traces — search traces
pub async fn search_traces(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<TraceSearchParams>,
) -> Result<Json<Value>, ApiError> {
    let limit = params.limit.unwrap_or(20) as i64;
    let mut where_clauses = vec!["org_id = $1".to_string()];
    
    if params.service.is_some() {
        where_clauses.push(format!("service_name = ${}", where_clauses.len() + 1));
    }
    if params.operation.is_some() {
        where_clauses.push(format!("operation_name LIKE ${}", where_clauses.len() + 1));
    }
    if params.min_duration_us.is_some() {
        where_clauses.push(format!("duration_us >= ${}", where_clauses.len() + 1));
    }

    let sql = format!(
        "SELECT DISTINCT trace_id, service_name as root_service, operation_name as root_operation, 
                duration_us as total_duration_us, started_at,
                (SELECT count(*) FROM traces s2 WHERE s2.trace_id = traces.trace_id) as span_count,
                (SELECT EXISTS(SELECT 1 FROM traces s3 WHERE s3.trace_id = traces.trace_id AND status = 'error')) as has_errors
         FROM traces 
         WHERE {} 
         AND parent_span_id IS NULL
         ORDER BY started_at DESC 
         LIMIT ${}",
        where_clauses.join(" AND "),
        where_clauses.len() + 1
    );

    let mut query = sqlx::query_as::<_, TraceSummaryRow>(&sql)
        .bind(org_id);

    if let Some(ref svc) = params.service {
        query = query.bind(svc);
    }
    if let Some(ref op) = params.operation {
        query = query.bind(format!("%{}%", op));
    }
    if let Some(min_d) = params.min_duration_us {
        query = query.bind(min_d);
    }
    query = query.bind(limit);

    let rows = query
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let summaries: Vec<TraceSummary> = rows.into_iter().map(|r| TraceSummary {
        trace_id: r.trace_id,
        root_service: r.root_service,
        root_operation: r.root_operation,
        total_duration_us: r.total_duration_us,
        span_count: r.span_count as usize,
        has_errors: r.has_errors,
        services: vec![],
        started_at: r.started_at,
    }).collect();

    Ok(Json(json!({ "traces": summaries, "count": summaries.len() })))
}

#[derive(sqlx::FromRow)]
struct TraceSummaryRow {
    trace_id: String,
    root_service: String,
    root_operation: String,
    total_duration_us: i64,
    span_count: i64,
    has_errors: bool,
    started_at: DateTime<Utc>,
}

/// GET /api/traces/:traceId — get all traces for a trace
pub async fn get_trace(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(trace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let traces = sqlx::query_as::<_, DbSpanRow>(
        "SELECT span_id, trace_id, parent_span_id, operation_name, service_name, duration_us, started_at, status, tags, events
         FROM traces WHERE trace_id = $1 AND org_id = $2 ORDER BY started_at ASC"
    )
    .bind(&trace_id)
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if traces.is_empty() {
        return Err(ApiError::NotFound(format!("Trace '{}' not found", trace_id)));
    }

    Ok(Json(json!({
        "trace_id": trace_id,
        "traces": traces,
        "span_count": traces.len(),
    })))
}

#[derive(sqlx::FromRow, Serialize)]
struct DbSpanRow {
    span_id: String,
    trace_id: String,
    parent_span_id: Option<String>,
    operation_name: String,
    service_name: String,
    duration_us: i64,
    started_at: DateTime<Utc>,
    status: String,
    tags: Value,
    events: Value,
}

/// GET /api/traces/services — list known services
pub async fn list_services(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let services = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT service_name FROM traces WHERE org_id = $1 ORDER BY service_name"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "services": services })))
}

#[derive(Debug, Deserialize)]
pub struct OperationsQuery {
    pub service: Option<String>,
}

/// GET /api/traces/operations?service=<name> — list operations for service
pub async fn list_operations(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<OperationsQuery>,
) -> Result<Json<Value>, ApiError> {
    let mut sql = "SELECT DISTINCT operation_name FROM traces WHERE org_id = $1".to_string();
    if params.service.is_some() {
        sql.push_str(" AND service_name = $2");
    }
    sql.push_str(" ORDER BY operation_name");

    let mut query = sqlx::query_scalar::<_, String>(&sql).bind(org_id);
    if let Some(ref svc) = params.service {
        query = query.bind(svc);
    }

    let ops = query
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "operations": ops })))
}

/// POST /api/v1/traces — ingest OTLP JSON traces
#[derive(Debug, Deserialize)]
pub struct OtlpIngestRequest {
    pub spans: Option<Vec<IngestSpan>>,
}

#[derive(Debug, Deserialize)]
pub struct IngestSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation_name: String,
    pub service_name: String,
    pub start_time_unix_nano: Option<i64>,
    pub duration_us: Option<i64>,
    pub status: Option<String>,
    pub tags: Option<HashMap<String, String>>,
}

// ═══════════════════════════════════════════════════════════════════════════
// SW P10: TraceQL Query Language Support
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/query/traceql — execute TraceQL expression
pub async fn traceql_query(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Json(body): Json<zentra_query::traceql::TraceQlQuery>,
) -> Result<Json<Value>, ApiError> {
    let results = zentra_query::traceql::run(&body, &state.tsdb_pool)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(Json(json!({
        "traces": results,
        "count": results.len(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct TraceQlValidateQuery {
    pub expr: String,
}

/// GET /api/query/traceql/validate?expr=X — validate expression
pub async fn traceql_validate(
    Query(params): Query<TraceQlValidateQuery>,
) -> Json<Value> {
    let result = zentra_query::traceql::validate(&params.expr);
    Json(serde_json::to_value(result).unwrap_or(json!({"valid": false})))
}

// Grafana Tempo-compatible search endpoint
#[derive(Debug, Deserialize)]
pub struct TempoSearchQuery {
    pub q: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub limit: Option<u32>,
}

/// GET /tempo/api/search?q={...}&start=&end=&limit= — Tempo-compatible search
pub async fn tempo_search(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Query(params): Query<TempoSearchQuery>,
) -> Result<Json<Value>, ApiError> {
    let expr = params.q.unwrap_or_else(|| "{status=error}".into());
    let from = params.start
        .and_then(|s| DateTime::from_timestamp(s, 0))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = params.end
        .and_then(|s| DateTime::from_timestamp(s, 0))
        .unwrap_or_else(Utc::now);
    let limit = params.limit.unwrap_or(20);

    let query = zentra_query::traceql::TraceQlQuery {
        expr,
        from: Some(from),
        to: Some(to),
        limit: Some(limit),
    };

    let results = zentra_query::traceql::run(&query, &state.tsdb_pool)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    // Convert to Tempo format
    let traces: Vec<Value> = results.iter().map(|t| json!({
        "traceID": t.trace_id,
        "rootServiceName": t.root_service,
        "rootTraceName": t.root_operation,
        "durationMs": t.duration_ms,
        "spanSets": [{"spans": [], "matched": t.span_count}],
        "startTimeUnixNano": t.started_at.timestamp_nanos_opt().unwrap_or(0),
    })).collect();

    Ok(Json(json!({
        "traces": traces,
        "metrics": {},
    })))
}

/// GET /tempo/api/traces/:traceId — Tempo-compatible trace detail
pub async fn tempo_trace(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Path(trace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // Query spans from DB
    let traces = sqlx::query_as::<_, DbSpanRow>(
        "SELECT span_id, trace_id, parent_span_id, operation_name, service_name, duration_us, started_at, status, tags, events FROM traces WHERE trace_id = $1 ORDER BY started_at"
    )
    .bind(&trace_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    if traces.is_empty() {
        return Err(ApiError::NotFound(format!("Trace '{}' not found", trace_id)));
    }

    let tempo_spans: Vec<Value> = traces.iter().map(|s| json!({
        "spanID": s.span_id,
        "traceID": s.trace_id,
        "operationName": s.operation_name,
        "serviceName": s.service_name,
        "duration": s.duration_us,
        "startTime": s.started_at.timestamp_micros(),
        "tags": s.tags,
        "references": s.parent_span_id.as_ref().map(|p| vec![json!({"refType": "CHILD_OF", "spanID": p})]).unwrap_or_default(),
    })).collect();

    Ok(Json(json!({
        "traceID": trace_id,
        "spans": tempo_spans,
        "processes": {},
    })))
}

// ═══════════════════════════════════════════════════════════════════════════
// OTLP Ingest
// ═══════════════════════════════════════════════════════════════════════════

/// O19: Consolidated trace ingest — converts custom format to OTLP and enqueues
/// via the shared ingest pipeline so all traces go through one path.
pub async fn ingest_traces(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<OtlpIngestRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut count = 0;

    if let Some(spans) = &req.spans {
        // Convert custom format to OTLP-compatible resourceSpans
        let otlp_spans: Vec<Value> = spans.iter().map(|s| {
            let start_ns = s.start_time_unix_nano.unwrap_or(0);
            let dur_us = s.duration_us.unwrap_or(0);
            let end_ns = start_ns + dur_us * 1000;
            json!({
                "traceId": s.trace_id,
                "spanId": s.span_id,
                "parentSpanId": s.parent_span_id,
                "name": s.operation_name,
                "startTimeUnixNano": start_ns.to_string(),
                "endTimeUnixNano": end_ns.to_string(),
                "status": { "code": if s.status.as_deref() == Some("error") { 2 } else { 0 } },
                "attributes": s.tags.as_ref().map(|t| t.iter().map(|(k, v)| json!({"key": k, "value": {"stringValue": v}})).collect::<Vec<_>>()).unwrap_or_default(),
            })
        }).collect();

        let svc_name = spans.first().map(|s| s.service_name.clone()).unwrap_or_else(|| "unknown".into());

        let payload = json!({
            "resourceSpans": [{
                "resource": {
                    "attributes": [{"key": "service.name", "value": {"stringValue": svc_name}}]
                },
                "scopeSpans": [{
                    "spans": otlp_spans
                }]
            }]
        });

        count = spans.len();

        // Enqueue via shared ingest pipeline
        if let Some(ref queue) = state.ingest_queue {
            let batch = crate::ingest_queue::IngestBatch {
                org_id,
                signal: crate::ingest_queue::Signal::Traces,
                payload,
                raw_bytes: vec![],
            };
            if queue.try_send(batch).is_err() {
                return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
            }
        }
    }

    Ok(Json(json!({
        "partialSuccess": null,
    })))
}
