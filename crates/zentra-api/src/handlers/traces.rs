//! Traces & OTLP Ingestion — Distributed tracing support
//!
//! Provides trace search, span detail, service enumeration, and OTLP JSON ingestion.

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::state::AppState;

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

// In-memory trace store (MVP — replace with DB in production)
fn trace_store() -> &'static RwLock<Vec<Span>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<Span>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(Vec::new()))
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
    Query(params): Query<TraceSearchParams>,
) -> Json<Value> {
    let store = trace_store().read();
    let limit = params.limit.unwrap_or(20);

    // Group spans by trace_id
    let mut trace_map: HashMap<String, Vec<&Span>> = HashMap::new();
    for span in store.iter() {
        trace_map.entry(span.trace_id.clone()).or_default().push(span);
    }

    let mut summaries: Vec<TraceSummary> = trace_map.into_iter().filter_map(|(trace_id, spans)| {
        let root = spans.iter().find(|s| s.parent_span_id.is_none())?;

        // Apply filters
        if let Some(ref svc) = params.service {
            if !spans.iter().any(|s| s.service_name.eq_ignore_ascii_case(svc)) { return None; }
        }
        if let Some(ref op) = params.operation {
            if !spans.iter().any(|s| s.operation_name.contains(op.as_str())) { return None; }
        }
        if let Some(min_d) = params.min_duration_us {
            if root.duration_us < min_d { return None; }
        }
        if let Some(max_d) = params.max_duration_us {
            if root.duration_us > max_d { return None; }
        }

        let services: Vec<String> = spans.iter().map(|s| s.service_name.clone()).collect::<std::collections::HashSet<_>>().into_iter().collect();
        let has_errors = spans.iter().any(|s| s.status == "ERROR" || s.tags.get("error").map_or(false, |v| v == "true"));

        Some(TraceSummary {
            trace_id,
            root_service: root.service_name.clone(),
            root_operation: root.operation_name.clone(),
            total_duration_us: root.duration_us,
            span_count: spans.len(),
            has_errors,
            services,
            started_at: root.start_time,
        })
    }).collect();

    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    summaries.truncate(limit);

    Json(json!({ "traces": summaries, "count": summaries.len() }))
}

/// GET /api/traces/:traceId — get all spans for a trace
pub async fn get_trace(
    Path(trace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = trace_store().read();
    let spans: Vec<&Span> = store.iter().filter(|s| s.trace_id == trace_id).collect();

    if spans.is_empty() {
        return Err(ApiError::NotFound(format!("Trace '{}' not found", trace_id)));
    }

    Ok(Json(json!({
        "trace_id": trace_id,
        "spans": spans,
        "span_count": spans.len(),
    })))
}

/// GET /api/traces/services — list known services
pub async fn list_services() -> Json<Value> {
    let store = trace_store().read();
    let services: Vec<String> = store.iter().map(|s| s.service_name.clone())
        .collect::<std::collections::HashSet<_>>().into_iter().collect();
    Json(json!({ "services": services }))
}

/// GET /api/traces/operations?service=<name> — list operations for service
#[derive(Debug, Deserialize)]
pub struct OperationsQuery {
    pub service: Option<String>,
}

pub async fn list_operations(
    Query(params): Query<OperationsQuery>,
) -> Json<Value> {
    let store = trace_store().read();
    let ops: Vec<String> = store.iter()
        .filter(|s| params.service.as_ref().map_or(true, |svc| s.service_name.eq_ignore_ascii_case(svc)))
        .map(|s| s.operation_name.clone())
        .collect::<std::collections::HashSet<_>>().into_iter().collect();
    Json(json!({ "operations": ops }))
}

/// POST /api/v1/traces — ingest OTLP JSON traces
#[derive(Debug, Deserialize)]
pub struct OtlpIngestRequest {
    pub resource_spans: Option<Vec<Value>>,
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

/// GET /tempo/api/traces/:traceID — Tempo-compatible trace detail
pub async fn tempo_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    // Query spans from DB
    let spans = sqlx::query_as::<_, TempoSpanRow>(
        "SELECT span_id, trace_id, parent_span_id, operation_name, service_name, duration_us, started_at, status, tags FROM spans WHERE trace_id = $1 ORDER BY started_at"
    )
    .bind(&trace_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    if spans.is_empty() {
        // Fall back to in-memory store
        let store = trace_store().read();
        let mem_spans: Vec<&Span> = store.iter().filter(|s| s.trace_id == trace_id).collect();
        if mem_spans.is_empty() {
            return Err(ApiError::NotFound(format!("Trace '{}' not found", trace_id)));
        }
        return Ok(Json(json!({
            "traceID": trace_id,
            "spans": mem_spans,
        })));
    }

    let tempo_spans: Vec<Value> = spans.iter().map(|s| json!({
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

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TempoSpanRow {
    span_id: String,
    trace_id: String,
    parent_span_id: Option<String>,
    operation_name: String,
    service_name: String,
    duration_us: i64,
    started_at: DateTime<Utc>,
    status: String,
    tags: Option<serde_json::Value>,
}

// ═══════════════════════════════════════════════════════════════════════════
// OTLP Ingest
// ═══════════════════════════════════════════════════════════════════════════

pub async fn ingest_traces(
    Json(req): Json<OtlpIngestRequest>,
) -> Json<Value> {
    let mut count = 0;

    if let Some(spans) = req.spans {
        let mut store = trace_store().write();
        for s in spans {
            let start = s.start_time_unix_nano
                .and_then(|ns| DateTime::from_timestamp_nanos(ns).into())
                .unwrap_or_else(Utc::now);
            store.push(Span {
                trace_id: s.trace_id,
                span_id: s.span_id,
                parent_span_id: s.parent_span_id,
                operation_name: s.operation_name,
                service_name: s.service_name,
                start_time: start,
                duration_us: s.duration_us.unwrap_or(0),
                status: s.status.unwrap_or_else(|| "OK".into()),
                tags: s.tags.unwrap_or_default(),
                events: vec![],
            });
            count += 1;
        }
    }

    Json(json!({
        "status": "success",
        "spans_ingested": count,
        "timestamp": Utc::now().to_rfc3339()
    }))
}
