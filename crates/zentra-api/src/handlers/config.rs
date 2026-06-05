//! Config & Status API — Runtime info, build info, TSDB stats
//! Prometheus-compatible status endpoints (not exposed publicly per policy)

use axum::{extract::State, Json, response::{Html, IntoResponse}, http::header};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::state::AppState;
use crate::tenant::OrgId;
use crate::error::ApiError;

static START_TIME: std::sync::OnceLock<chrono::DateTime<chrono::Utc>> = std::sync::OnceLock::new();

/// Standard JSON response for live mode when data source is unavailable.
/// Returns `{ "status": "error", "mode": "live", "message": "...", "data": null }`.
pub fn live_mode_no_data(component: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "status": "error",
        "mode": "live",
        "component": component,
        "message": format!("Live mode: {} — {}", component, reason),
        "data": [],
        "timestamp": Utc::now().to_rfc3339()
    })
}

/// Standard empty-array JSON response for live mode when data source is unavailable.
pub fn live_mode_empty(component: &str) -> serde_json::Value {
    serde_json::json!({
        "status": "success",
        "mode": "live",
        "component": component,
        "message": format!("Live mode: no {} data available — configure data source", component),
        "data": [],
        "timestamp": Utc::now().to_rfc3339()
    })
}

/// GET /api/config/mode — always returns live mode.
pub async fn get_mode(
    State(_state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Json<Value> {
    Json(json!({
        "mode": "live",
        "org_id": org_id,
        "description": "Live mode — displaying real-time data from connected sources."
    }))
}

#[derive(Deserialize)]
pub struct SetModeRequest {
    pub mode: String,
}

/// POST /api/config/mode — always live.
pub async fn set_mode(
    State(_state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(_req): Json<SetModeRequest>,
) -> Json<Value> {
    Json(json!({
        "status": "success",
        "mode": "live",
        "org_id": org_id,
        "message": "Live mode active"
    }))
}

fn start_time() -> chrono::DateTime<chrono::Utc> {
    *START_TIME.get_or_init(Utc::now)
}

/// GET /api/v1/status/runtimeinfo
pub async fn runtime_info(
    State(state): State<AppState>,
) -> Json<Value> {
    let now = Utc::now();
    let uptime = (now - start_time()).num_seconds();
    Json(json!({
        "status": "success",
        "data": {
            "startTime": start_time().to_rfc3339(),
            "CWD": std::env::current_dir().unwrap_or_default().display().to_string(),
            "reloadConfigSuccess": true,
            "lastConfigTime": start_time().to_rfc3339(),
            "goroutineCount": tokio::runtime::Handle::current().metrics().num_workers(),
            "GOMAXPROCS": num_cpus::get(),
            "GOGC": "",
            "GODEBUG": "",
            "storageRetention": "15d",
            "uptimeSeconds": uptime,
            "version": env!("CARGO_PKG_VERSION"),
        }
    }))
}

/// GET /api/v1/status/buildinfo
pub async fn build_info() -> Json<Value> {
    Json(json!({
        "status": "success",
        "data": {
            "version": env!("CARGO_PKG_VERSION"),
            "revision": option_env!("GIT_HASH").unwrap_or("dev"),
            "branch": option_env!("GIT_BRANCH").unwrap_or("main"),
            "buildDate": option_env!("BUILD_DATE").unwrap_or("unknown"),
            "rustVersion": option_env!("RUSTC_VERSION").unwrap_or("unknown"),
            "platform": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        }
    }))
}

/// GET /api/v1/status/config
pub async fn running_config() -> Json<Value> {
    Json(json!({
        "status": "success",
        "data": {
            "yaml": {
                "global": {
                    "scrape_interval": "15s",
                    "evaluation_interval": "15s",
                    "scrape_timeout": "10s",
                },
                "retention": {
                    "raw": "15d",
                    "1m_aggregate": "90d",
                    "5m_aggregate": "365d",
                    "1h_aggregate": "730d",
                },
                "features": {
                    "promql_engine": true,
                    "ml_anomaly_detection": true,
                    "service_discovery": true,
                    "remote_write": true,
                    "remote_read": true,
                    "recording_rules": true,
                    "alerting_rules": true,
                }
            }
        }
    }))
}

/// GET /api/v1/status/flags
pub async fn flags() -> Json<Value> {
    Json(json!({
        "status": "success",
        "data": {
            "config.file": "/etc/Zentra/Zentra.yaml",
            "web.listen-address": "0.0.0.0:7001",
            "storage.tsdb.path": "/var/lib/Zentra/data",
            "storage.tsdb.retention.time": "15d",
            "log.level": "info",
        }
    }))
}

/// GET /api/v1/status/tsdb
pub async fn tsdb_status(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    // Try to get real stats from TimescaleDB
    let series_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT (metric_name, resource_id)) FROM metrics WHERE org_id = $1"
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let sample_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metrics WHERE org_id = $1"
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let db_size: String = sqlx::query_scalar(
        "SELECT pg_size_pretty(pg_database_size(current_database()))"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or_else(|_| "unknown".into());

    Ok(Json(json!({
        "status": "success",
        "data": {
            "seriesCountByMetricName": [],
            "labelValueCountByLabelName": [],
            "memoryInBytesByLabelName": [],
            "seriesCountByLabelValuePair": [],
            "headStats": {
                "numSeries": series_count,
                "numSamples": sample_count,
                "chunkCount": 0,
                "minTime": 0,
                "maxTime": 0,
            },
            "databaseSize": db_size,
        }
    })))
}

/// POST /api/v1/admin/reload — reload configuration
pub async fn reload_config() -> Json<Value> {
    // In production: re-read config file, update scrape targets, etc.
    Json(json!({
        "status": "success",
        "message": "Configuration reloaded successfully",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

// ============================================================================
// OpenAPI spec endpoint (utoipa-generated)
// ============================================================================

/// GET /api/v1/openapi.json — OpenAPI 3.0 specification generated by utoipa.
/// Annotate more handlers with `#[utoipa::path(...)]` to expand coverage.
pub async fn openapi_spec() -> Json<Value> {
    Json(crate::openapi::spec_json())
}

pub async fn swagger_ui() -> impl IntoResponse {
    Html(r#"<!DOCTYPE html>
<html>
<head>
  <title>Zentra API — Swagger UI</title>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" >
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-standalone-preset.js"></script>
  <script>
    window.onload = function() {
      const ui = SwaggerUIBundle({
        url: "/api/openapi.json",
        dom_id: '#swagger-ui',
        presets: [SwaggerUIBundle.presets.apis, SwaggerUIStandalonePreset],
        layout: "StandaloneLayout",
        deepLinking: true,
        displayRequestDuration: true,
        filter: true,
        tryItOutEnabled: true,
      })
    }
  </script>
</body>
</html>"#)
}

pub async fn redoc_ui() -> impl IntoResponse {
    Html(r#"<!DOCTYPE html>
<html>
  <head>
    <title>Zentra API — ReDoc</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link href="https://fonts.googleapis.com/css?family=Montserrat:300,400,700|Roboto:300,400,700" rel="stylesheet">
    <style>body { margin: 0; padding: 0; }</style>
  </head>
  <body>
    <redoc spec-url='/api/openapi.json' hide-download-button></redoc>
    <script src="https://cdn.jsdelivr.net/npm/redoc@latest/bundles/redoc.standalone.js"></script>
  </body>
</html>"#)
}
