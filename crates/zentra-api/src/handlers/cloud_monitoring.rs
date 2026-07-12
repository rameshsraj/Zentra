//! Cloud monitoring API — CRUD for cloud account registrations.
//! Live metrics require AWS/Azure/GCP SDK integration (deferred to P2).

use axum::{
    extract::{State, Path, Query},
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAccount {
    pub id: Uuid,
    pub name: String,
    pub provider: String,
    pub account_id: Option<String>,
    pub region: Option<String>,
    pub status: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub resource_count: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCloudAccountRequest {
    pub name: String,
    pub provider: String,
    pub account_id: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MetricQueryParams {
    pub hours: Option<i32>,
    pub metric: Option<String>,
}

pub async fn list_accounts(
    State(state): State<AppState>,
) -> Result<Json<Vec<CloudAccount>>, ApiError> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<String>, Option<String>, String, Option<DateTime<Utc>>, Option<i64>, DateTime<Utc>)>(
        "SELECT id, name, provider, account_id, region, status,
                last_sync, resource_count, created_at
         FROM cloud_accounts
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let accounts = rows.into_iter().map(|(id, name, provider, account_id, region, status, last_sync, resource_count, created_at)| {
        CloudAccount { id, name, provider, account_id, region, status, last_sync, resource_count, created_at }
    }).collect();
    Ok(Json(accounts))
}

pub async fn create_account(
    State(state): State<AppState>,
    Json(req): Json<CreateCloudAccountRequest>,
) -> Result<Json<CloudAccount>, ApiError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO cloud_accounts (id, name, provider, account_id, region, status, created_at)
         VALUES ($1, $2, $3, $4, $5, 'pending', $6)",
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.provider)
    .bind(&req.account_id)
    .bind(&req.region)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(CloudAccount {
        id, name: req.name, provider: req.provider,
        account_id: req.account_id, region: req.region,
        status: "pending".into(), last_sync: None,
        resource_count: Some(0), created_at: now,
    }))
}

pub async fn delete_account(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("DELETE FROM cloud_accounts WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::from((StatusCode::NOT_FOUND, format!("cloud account {} not found", id))));
    }
    Ok(Json(serde_json::json!({ "deleted": id })))
}

pub async fn validate_account(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Cloud SDK validation is not yet implemented; mark account as pending_validation.
    let result = sqlx::query(
        "UPDATE cloud_accounts SET status = 'pending_validation' WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::from((StatusCode::NOT_FOUND, format!("cloud account {} not found", id))));
    }
    Ok(Json(serde_json::json!({
        "id": id,
        "valid": false,
        "message": "Cloud provider SDK integration is not yet available. Validation deferred to P2 release.",
    })))
}

pub async fn get_account_metrics(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<MetricQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "account_id": id,
        "hours": q.hours.unwrap_or(24),
        "metric": q.metric.unwrap_or_else(|| "all".to_string()),
        "status": "pending_integration",
        "series": [],
        "message": "Cloud provider SDK integration is not configured for this account yet."
    })))
}

pub async fn cloud_summary(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT provider, COUNT(*)::BIGINT FROM cloud_accounts GROUP BY provider",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let total: i64 = rows.iter().map(|(_, c)| c).sum();
    let by_provider: serde_json::Value = rows.into_iter()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect::<serde_json::Map<_, _>>()
        .into();

    Ok(Json(serde_json::json!({
        "total_accounts": total,
        "by_provider": by_provider,
        "sdk_integration": "pending_p2",
    })))
}
