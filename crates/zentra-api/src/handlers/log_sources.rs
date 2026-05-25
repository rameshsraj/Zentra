//! Log Sources API handlers for Zentra.

use axum::{extract::{State, Path}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Serialize, Clone)]
pub struct LogSourceEntry {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub status: String,
    pub last_ingested: Option<String>,
    pub log_count: u64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLogSourceRequest {
    pub name: String,
    pub source_type: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: Option<String>,
}

/// GET /api/log-sources — list log sources
pub async fn list_log_sources(
    State(_state): State<AppState>,
) -> Result<Json<Vec<LogSourceEntry>>, ApiError> {
    return Ok(Json(vec![]));
}

/// POST /api/log-sources — create log source
pub async fn create_log_source(
    State(_state): State<AppState>,
    Json(req): Json<CreateLogSourceRequest>,
) -> Result<Json<LogSourceEntry>, ApiError> {
    let now = Utc::now();
    Ok(Json(LogSourceEntry {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        source_type: req.source_type.unwrap_or_else(|| "file".into()),
        host: req.host.unwrap_or_default(),
        port: req.port.unwrap_or(0),
        path: req.path.unwrap_or_default(),
        status: "active".into(),
        last_ingested: None,
        log_count: 0,
        created_at: now.to_rfc3339(),
    }))
}

/// DELETE /api/log-sources/:id — delete log source
pub async fn delete_log_source(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "deleted": id })))
}
