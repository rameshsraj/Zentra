//! Annotations API handler for Zentra.
//! Reference: grafana-main annotations/models.go — Item with text, tags, epoch.

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

use crate::error::ApiError;

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct Annotation {
    pub annotation_id: Uuid,
    pub org_id: Uuid,
    pub title: String,
    pub text: String,
    pub annotation_type: String,
    pub tags: Vec<String>,
    pub target_id: Option<Uuid>,
    pub dashboard_id: Option<String>,
    pub panel_id: Option<String>,
    pub epoch_start: i64,
    pub epoch_end: Option<i64>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAnnotationRequest {
    pub title: String,
    pub text: String,
    pub annotation_type: Option<String>,
    pub tags: Option<Vec<String>>,
    pub target_id: Option<Uuid>,
    pub dashboard_id: Option<String>,
    pub panel_id: Option<String>,
    pub epoch_start: Option<i64>,
    pub epoch_end: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAnnotationRequest {
    pub title: Option<String>,
    pub text: Option<String>,
    pub tags: Option<Vec<String>>,
    pub epoch_end: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AnnotationQuery {
    pub annotation_type: Option<String>,
    pub tag: Option<String>,
    pub target_id: Option<Uuid>,
    pub dashboard_id: Option<String>,
    pub from_epoch: Option<i64>,
    pub to_epoch: Option<i64>,
    pub limit: Option<i64>,
}

// ============================================================================
// Handlers
// ============================================================================

pub async fn list_annotations(
    State(_state): State<AppState>,
    Query(params): Query<AnnotationQuery>,
) -> Result<Json<Vec<Annotation>>, ApiError> {
    return Ok(Json(vec![]));
}

pub async fn create_annotation(
    State(_state): State<AppState>,
    Json(req): Json<CreateAnnotationRequest>,
) -> Result<Json<Annotation>, ApiError> {
    let now = Utc::now();
    let annotation = Annotation {
        annotation_id: Uuid::now_v7(),
        org_id: Uuid::nil(),
        title: req.title,
        text: req.text,
        annotation_type: req.annotation_type.unwrap_or_else(|| "manual".into()),
        tags: req.tags.unwrap_or_default(),
        target_id: req.target_id,
        dashboard_id: req.dashboard_id,
        panel_id: req.panel_id,
        epoch_start: req.epoch_start.unwrap_or_else(|| now.timestamp_millis()),
        epoch_end: req.epoch_end,
        created_by: Some("user".into()),
        created_at: now,
        updated_at: now,
    };

    Ok(Json(annotation))
}

pub async fn get_annotation(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Annotation>, ApiError> {
    let now = Utc::now();
    Ok(Json(Annotation {
        annotation_id: id,
        org_id: Uuid::nil(),
        title: "Sample Annotation".into(),
        text: "Description".into(),
        annotation_type: "manual".into(),
        tags: vec![],
        target_id: None,
        dashboard_id: None,
        panel_id: None,
        epoch_start: now.timestamp_millis(),
        epoch_end: None,
        created_by: None,
        created_at: now,
        updated_at: now,
    }))
}

pub async fn update_annotation(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAnnotationRequest>,
) -> Result<Json<Annotation>, ApiError> {
    let now = Utc::now();
    Ok(Json(Annotation {
        annotation_id: id,
        org_id: Uuid::nil(),
        title: req.title.unwrap_or_else(|| "Updated".into()),
        text: req.text.unwrap_or_else(|| "Updated".into()),
        annotation_type: "manual".into(),
        tags: req.tags.unwrap_or_default(),
        target_id: None,
        dashboard_id: None,
        panel_id: None,
        epoch_start: now.timestamp_millis(),
        epoch_end: req.epoch_end,
        created_by: None,
        created_at: now,
        updated_at: now,
    }))
}

pub async fn delete_annotation(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "deleted": id.to_string() })))
}
