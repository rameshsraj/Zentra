use axum::{extract::{State, Query}, Json};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_log_engine::correlation::CorrelatedEvent;
use serde::Deserialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
pub struct CorrelationQuery {
    pub target_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub window_minutes: Option<i64>,
}

pub async fn logs_for_metric(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<CorrelationQuery>,
) -> Result<Json<Vec<CorrelatedEvent>>, ApiError> {
    let results = state.correlation
        .logs_for_metric(
            org_id,
            params.target_id,
            params.timestamp,
            params.window_minutes.unwrap_or(5),
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(results))
}

pub async fn metrics_for_log(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<CorrelationQuery>,
) -> Result<Json<Vec<CorrelatedEvent>>, ApiError> {
    let results = state.correlation
        .metrics_for_log(
            org_id,
            params.target_id,
            params.timestamp,
            params.window_minutes.unwrap_or(5),
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(results))
}
