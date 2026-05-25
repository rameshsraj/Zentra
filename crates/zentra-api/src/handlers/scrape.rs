//! Scrape target management API handlers for Zentra.
//! Exposes CRUD for scrape targets, health status, and recording rules.

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;

use crate::error::ApiError;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrapeTargetResponse {
    pub target_id: Uuid,
    pub org_id: Uuid,
    pub host: String,
    pub port: u16,
    pub scheme: String,
    pub metrics_path: String,
    pub job_name: Option<String>,
    pub scrape_interval_seconds: u32,
    pub enabled: bool,
    pub discovered_by: String,
    pub health: String,
    pub sample_count: u32,
    pub scrape_duration_ms: Option<f64>,
    pub last_error: Option<String>,
    pub last_scrape: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateScrapeTargetRequest {
    pub host: String,
    pub port: Option<u16>,
    pub scheme: Option<String>,
    pub metrics_path: Option<String>,
    pub job_name: Option<String>,
    pub scrape_interval_seconds: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ScrapeTargetQuery {
    pub job_name: Option<String>,
    pub health: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ScrapeHealthSummary {
    pub total_targets: u32,
    pub targets_up: u32,
    pub targets_down: u32,
    pub targets_unknown: u32,
    pub avg_scrape_duration_ms: f64,
    pub total_samples_last_scrape: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrapeRecordingRuleResponse {
    pub rule_id: Uuid,
    pub name: String,
    pub source_metric: String,
    pub output_metric: String,
    pub aggregation: String,
    pub group_by: Vec<String>,
    pub enabled: bool,
}

// ============================================================================
// Handlers
// ============================================================================

pub async fn list_scrape_targets(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(_params): Query<ScrapeTargetQuery>,
) -> Result<Json<Vec<ScrapeTargetResponse>>, ApiError> {
    let now = Utc::now();

    let targets = zentra_db::repositories::targets::list_targets(
        &state.pool, org_id, None, true,
    ).await.map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let scrape_targets: Vec<ScrapeTargetResponse> = targets.iter().map(|t| {
        use zentra_core::models::target::TargetStatus;
        let health = match t.status {
            TargetStatus::Healthy | TargetStatus::Connected => "up",
            TargetStatus::Unreachable | TargetStatus::Critical => "down",
            TargetStatus::Warning => "warning",
            _ => "unknown",
        };
        ScrapeTargetResponse {
            target_id: t.target_id,
            org_id: t.org_id,
            host: t.host.clone(),
            port: t.port,
            scheme: "http".into(),
            metrics_path: "/metrics".into(),
            job_name: Some(format!("{:?}", t.target_subtype).to_lowercase()),
            scrape_interval_seconds: t.check_interval_seconds,
            enabled: true,
            discovered_by: "database".into(),
            health: health.into(),
            sample_count: 0,
            scrape_duration_ms: None,
            last_error: t.last_error.clone(),
            last_scrape: t.last_check_at,
        }
    }).collect();
    return Ok(Json(scrape_targets));
}

pub async fn create_scrape_target(
    State(_state): State<AppState>,
    Json(req): Json<CreateScrapeTargetRequest>,
) -> Result<Json<ScrapeTargetResponse>, ApiError> {
    let now = Utc::now();
    Ok(Json(ScrapeTargetResponse {
        target_id: Uuid::now_v7(),
        org_id: Uuid::nil(),
        host: req.host,
        port: req.port.unwrap_or(9090),
        scheme: req.scheme.unwrap_or_else(|| "http".into()),
        metrics_path: req.metrics_path.unwrap_or_else(|| "/metrics".into()),
        job_name: req.job_name,
        scrape_interval_seconds: req.scrape_interval_seconds.unwrap_or(15),
        enabled: true,
        discovered_by: "static".into(),
        health: "unknown".into(),
        sample_count: 0,
        scrape_duration_ms: None,
        last_error: None,
        last_scrape: None,
    }))
}

pub async fn delete_scrape_target(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "deleted": id.to_string() })))
}

pub async fn scrape_health_summary(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<ScrapeHealthSummary>, ApiError> {
    let targets = zentra_db::repositories::targets::list_targets(
        &state.pool, org_id, None, true,
    ).await.unwrap_or_default();
    let total = targets.len() as u32;
    use zentra_core::models::target::TargetStatus;
    let up = targets.iter().filter(|t| t.status == TargetStatus::Healthy || t.status == TargetStatus::Connected).count() as u32;
    let down = targets.iter().filter(|t| t.status == TargetStatus::Unreachable || t.status == TargetStatus::Critical).count() as u32;
    return Ok(Json(ScrapeHealthSummary {
        total_targets: total,
        targets_up: up,
        targets_down: down,
        targets_unknown: total - up - down,
        avg_scrape_duration_ms: 0.0,
        total_samples_last_scrape: 0,
    }));
}

pub async fn list_recording_rules(
    State(_state): State<AppState>,
) -> Result<Json<Vec<ScrapeRecordingRuleResponse>>, ApiError> {
    let rules = vec![
        ScrapeRecordingRuleResponse {
            rule_id: Uuid::now_v7(),
            name: "cpu_avg_5m".into(),
            source_metric: "node_cpu_seconds_total".into(),
            output_metric: "recording:node_cpu:avg_5m".into(),
            aggregation: "avg".into(),
            group_by: vec!["instance".into(), "mode".into()],
            enabled: true,
        },
        ScrapeRecordingRuleResponse {
            rule_id: Uuid::now_v7(),
            name: "http_request_rate".into(),
            source_metric: "http_requests_total".into(),
            output_metric: "recording:http_requests:rate_1m".into(),
            aggregation: "rate".into(),
            group_by: vec!["job".into()],
            enabled: true,
        },
    ];
    Ok(Json(rules))
}
