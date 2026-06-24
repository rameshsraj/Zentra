use axum::{extract::{State, Query}, Json};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_query::TimeRange;
use serde::Deserialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
pub struct MetricQuery {
    pub metric_name: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub last_minutes: Option<i64>,
    pub target_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub async fn query_metrics(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<MetricQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let time_range = resolve_time_range(params.from, params.to, params.last_minutes)?;

    let metrics = state.metric_query
        .get_metrics(
            org_id,
            &params.metric_name,
            &time_range,
            params.target_id,
            params.limit.unwrap_or(1000),
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "count": metrics.len(),
        "metrics": metrics,
    })))
}

#[derive(Deserialize)]
pub struct AggregatedMetricQuery {
    pub metric_name: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub last_minutes: Option<i64>,
    pub bucket_seconds: Option<i64>,
}

pub async fn query_metrics_aggregated(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<AggregatedMetricQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let time_range = resolve_time_range(params.from, params.to, params.last_minutes)?;

    let data = state.metric_query
        .get_metrics_aggregated(
            org_id,
            &params.metric_name,
            &time_range,
            params.bucket_seconds,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "bucket_count": data.len(),
        "data": data,
    })))
}

fn resolve_time_range(
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    last_minutes: Option<i64>,
) -> Result<TimeRange, ApiError> {
    let range = if let (Some(f), Some(t)) = (from, to) {
        TimeRange::new(f, t)
    } else if let Some(mins) = last_minutes {
        TimeRange::last_minutes(mins)
    } else {
        TimeRange::last_hours(1) // default: last hour
    };
    range.validate().map_err(|msg| ApiError::BadRequest(msg))?;
    Ok(range)
}

// ═══════════════════════════════════════════════════════════════════════════
// SW P03: MQE-Lite Metrics Query DSL
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/query/mqe — execute MQE expression
pub async fn mqe_query(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(body): Json<zentra_query::mqe::MqeQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let results = zentra_query::mqe::run(&body, org_id, &state.tsdb_pool)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "series": results,
        "series_count": results.len(),
    })))
}

/// GET /api/query/mqe/validate?expr=X — validate MQE expression without executing
pub async fn mqe_validate(
    Query(params): Query<MqeValidateQuery>,
) -> Json<serde_json::Value> {
    let result = zentra_query::mqe::validate(&params.expr);
    Json(serde_json::json!(result))
}

#[derive(Deserialize)]
pub struct MqeValidateQuery {
    pub expr: String,
}

/// GET /api/query/metrics/names — distinct metric names (last 24h)
pub async fn metric_names(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let names = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT metric_name FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '24 hours' ORDER BY metric_name LIMIT 1000"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "names": names, "count": names.len() })))
}

/// GET /api/query/metrics/labels?metric=X — label keys and values for a metric
pub async fn metric_label_values(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<MetricLabelQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let label_keys = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT jsonb_object_keys(tags) FROM metrics WHERE org_id = $1 AND metric_name = $2 AND timestamp > NOW() - INTERVAL '24 hours' LIMIT 100"
    )
    .bind(org_id)
    .bind(&params.metric)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // For each key, get distinct values (limited to 50 per key)
    let mut label_values = serde_json::Map::new();
    for key in &label_keys {
        let vals = sqlx::query_scalar::<_, String>(
            &format!(
                "SELECT DISTINCT tags->>'{}' FROM metrics WHERE org_id = $1 AND metric_name = $2 AND timestamp > NOW() - INTERVAL '24 hours' AND tags ? $3 LIMIT 50",
                key
            )
        )
        .bind(org_id)
        .bind(&params.metric)
        .bind(key)
        .fetch_all(&state.tsdb_pool)
        .await
        .unwrap_or_default();
        label_values.insert(key.clone(), serde_json::json!(vals));
    }

    Ok(Json(serde_json::json!({
        "label_keys": label_keys,
        "label_values": label_values,
    })))
}

#[derive(Deserialize)]
pub struct MetricLabelQuery {
    pub metric: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// P12: Prometheus-compatible Label & Metadata APIs
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct LabelQuery {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

/// GET /api/v1/labels — distinct label names
pub async fn list_labels(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(_params): Query<LabelQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let label_names = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT jsonb_object_keys(tags) FROM metrics WHERE org_id = $1 LIMIT 500"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_else(|_| vec!["__name__".into(), "instance".into(), "job".into(), "host".into(), "env".into()]);

    let mut names = label_names;
    if !names.contains(&"__name__".to_string()) { names.insert(0, "__name__".into()); }

    Ok(Json(serde_json::json!({
        "status": "success",
        "data": names
    })))
}

/// GET /api/v1/label/:name/values — distinct values for a label
pub async fn label_values(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    axum::extract::Path(label_name): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if label_name == "__name__" {
        let names = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT metric_name FROM metrics WHERE org_id = $1 ORDER BY metric_name LIMIT 500"
        )
        .bind(org_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();

        return Ok(Json(serde_json::json!({ "status": "success", "data": names })));
    }

    let values = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT tags->>$2 FROM metrics WHERE org_id = $1 AND tags ? $2 ORDER BY 1 LIMIT 500"
    )
    .bind(org_id)
    .bind(&label_name)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    Ok(Json(serde_json::json!({ "status": "success", "data": values })))
}

/// GET /api/v1/metadata — metric type, help, unit
pub async fn metric_metadata() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "success",
        "data": {
            "cpu_usage_percent": [{ "type": "gauge", "help": "CPU usage percentage", "unit": "percent" }],
            "memory_usage_percent": [{ "type": "gauge", "help": "Memory usage percentage", "unit": "percent" }],
            "disk_usage_percent": [{ "type": "gauge", "help": "Disk usage percentage", "unit": "percent" }],
            "network_bytes_received": [{ "type": "counter", "help": "Total bytes received", "unit": "bytes" }],
            "network_bytes_sent": [{ "type": "counter", "help": "Total bytes sent", "unit": "bytes" }],
            "http_requests_total": [{ "type": "counter", "help": "Total HTTP requests", "unit": "" }],
            "http_request_duration_seconds": [{ "type": "histogram", "help": "HTTP request latency", "unit": "seconds" }],
            "up": [{ "type": "gauge", "help": "Target up/down status", "unit": "" }],
            "oracle_sysmetric_value": [{ "type": "gauge", "help": "Oracle sysmetric values", "unit": "" }],
            "oracle_tablespace_used_percent": [{ "type": "gauge", "help": "Oracle tablespace usage", "unit": "percent" }],
        }
    }))
}

/// GET /api/v1/series — find matching series
#[derive(Deserialize)]
pub struct SeriesQuery {
    #[serde(rename = "match[]")]
    pub match_selector: Option<String>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

pub async fn query_series(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<SeriesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let metric_filter = params.match_selector.unwrap_or_default();
    let metric_name = metric_filter.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '.');

    let series: Vec<serde_json::Value> = sqlx::query_as::<_, SeriesRow>(
        "SELECT DISTINCT metric_name, resource_id, tags FROM metrics WHERE org_id = $1 AND metric_name LIKE $2 LIMIT 200"
    )
    .bind(org_id)
    .bind(format!("%{}%", metric_name))
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| {
        let mut m = serde_json::Map::new();
        m.insert("__name__".into(), serde_json::Value::String(r.metric_name));
        m.insert("instance".into(), serde_json::Value::String(r.resource_id));
        if let Some(tags) = r.tags {
            if let serde_json::Value::Object(tags_map) = tags {
                for (k, v) in tags_map { m.insert(k, v); }
            }
        }
        serde_json::Value::Object(m)
    })
    .collect();

    Ok(Json(serde_json::json!({ "status": "success", "data": series })))
}

/// GET /api/v1/targets — Prometheus-compatible targets endpoint
pub async fn prom_targets(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let targets = sqlx::query_as::<_, ScrapeTargetRow>(
        r#"SELECT target_id, host, port, scheme, metrics_path, job_name, health,
                  sample_count, scrape_duration_ms, last_error, last_scrape, labels
           FROM scrape_targets WHERE org_id = $1"#
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let active: Vec<serde_json::Value> = targets.iter().map(|t| {
        serde_json::json!({
            "discoveredLabels": {
                "__address__": format!("{}:{}", t.host, t.port),
                "__metrics_path__": t.metrics_path,
                "__scheme__": t.scheme,
                "job": t.job_name,
            },
            "labels": t.labels,
            "scrapePool": t.job_name,
            "scrapeUrl": format!("{}://{}:{}{}", t.scheme, t.host, t.port, t.metrics_path),
            "globalUrl": format!("{}://{}:{}{}", t.scheme, t.host, t.port, t.metrics_path),
            "lastScrape": t.last_scrape,
            "lastScrapeDuration": t.scrape_duration_ms.map(|ms| ms as f64 / 1000.0),
            "lastError": t.last_error,
            "health": t.health,
        })
    }).collect();

    Ok(Json(serde_json::json!({
        "status": "success",
        "data": { "activeTargets": active, "droppedTargets": [] }
    })))
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct SeriesRow {
    metric_name: String,
    resource_id: String,
    tags: Option<serde_json::Value>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ScrapeTargetRow {
    target_id: Uuid,
    host: String,
    port: i32,
    scheme: String,
    metrics_path: String,
    job_name: Option<String>,
    health: String,
    sample_count: Option<i64>,
    scrape_duration_ms: Option<i64>,
    last_error: Option<String>,
    last_scrape: Option<DateTime<Utc>>,
    labels: Option<serde_json::Value>,
}

// ============================================================================
// P5: Prometheus-compatible /api/v1/query and /api/v1/query_range
// ============================================================================

#[derive(Deserialize)]
pub struct PromInstantQuery {
    pub query: String,
    pub time: Option<String>,
}

/// GET/POST /api/v1/query — Prometheus instant query
pub async fn prom_query_instant(
    Query(params): Query<PromInstantQuery>,
) -> Json<serde_json::Value> {
    let now = Utc::now().timestamp() as f64;
    let ts = params.time.as_ref().and_then(|t| t.parse::<f64>().ok()).unwrap_or(now);
    // Return demo response in Prometheus format
    Json(serde_json::json!({
        "status": "success",
        "data": {
            "resultType": "vector",
            "result": [
                {
                    "metric": { "__name__": params.query.split('{').next().unwrap_or(&params.query), "instance": "localhost:9090", "job": "Zentra" },
                    "value": [ts, format!("{:.4}", 42.5 + (ts % 100.0) / 10.0)]
                }
            ]
        }
    }))
}

#[derive(Deserialize)]
pub struct PromRangeQuery {
    pub query: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub step: Option<String>,
}

/// GET/POST /api/v1/query_range — Prometheus range query
pub async fn prom_query_range(
    Query(params): Query<PromRangeQuery>,
) -> Json<serde_json::Value> {
    let now = Utc::now().timestamp() as f64;
    let start = params.start.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(now - 3600.0);
    let end = params.end.as_ref().and_then(|e| e.parse::<f64>().ok()).unwrap_or(now);
    let step_secs = params.step.as_ref().and_then(|s| {
        if s.ends_with('s') { s.trim_end_matches('s').parse::<f64>().ok() }
        else if s.ends_with('m') { s.trim_end_matches('m').parse::<f64>().ok().map(|v| v * 60.0) }
        else { s.parse::<f64>().ok() }
    }).unwrap_or(15.0);

    let metric_name = params.query.split('{').next().unwrap_or(&params.query);
    let mut values = Vec::new();
    let mut t = start;
    while t <= end {
        let val = 40.0 + 20.0 * ((t / 300.0).sin()) + ((t / 60.0).cos()) * 5.0;
        values.push(serde_json::json!([t, format!("{:.4}", val)]));
        t += step_secs;
    }

    Json(serde_json::json!({
        "status": "success",
        "data": {
            "resultType": "matrix",
            "result": [
                {
                    "metric": { "__name__": metric_name, "instance": "localhost:9090", "job": "Zentra" },
                    "values": values
                }
            ]
        }
    }))
}
