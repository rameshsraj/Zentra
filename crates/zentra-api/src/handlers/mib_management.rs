//! MIB file management API — backed by `loaded_mibs` table.

use axum::{
    extract::{State, Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;
use axum::http::StatusCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MibRecord {
    pub id: Uuid,
    pub name: String,
    pub filename: Option<String>,
    pub object_count: i32,
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct OidQuery {
    pub oid: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadMibRequest {
    pub name: String,
    pub filename: Option<String>,
    pub content: Option<String>,
}

fn resolve_well_known(oid: &str) -> Option<(&'static str, &'static str)> {
    match oid {
        "1.3.6.1.2.1.1.1"    => Some(("sysDescr",     "System description")),
        "1.3.6.1.2.1.1.3"    => Some(("sysUpTime",    "System uptime in hundredths of a second")),
        "1.3.6.1.2.1.1.5"    => Some(("sysName",      "System name")),
        "1.3.6.1.2.1.2.1"    => Some(("ifNumber",     "Number of network interfaces")),
        "1.3.6.1.2.1.25.1.1" => Some(("hrSystemUptime", "Host uptime")),
        "1.3.6.1.4.1.2021.11.9"  => Some(("ssCpuUser",   "User CPU percentage (UCD-SNMP)")),
        "1.3.6.1.4.1.2021.11.10" => Some(("ssCpuSystem", "System CPU percentage (UCD-SNMP)")),
        "1.3.6.1.4.1.2021.11.11" => Some(("ssCpuIdle",   "Idle CPU percentage (UCD-SNMP)")),
        "1.3.6.1.4.1.2021.4.5"   => Some(("memTotalReal", "Total real memory (UCD-SNMP)")),
        "1.3.6.1.4.1.2021.4.6"   => Some(("memAvailReal", "Available real memory (UCD-SNMP)")),
        _ => None,
    }
}

pub async fn list_mibs(
    State(state): State<AppState>,
) -> Result<Json<Vec<MibRecord>>, ApiError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, Option<i32>, DateTime<Utc>)>(
        "SELECT id, name, file_name, object_count, loaded_at
         FROM loaded_mibs
         ORDER BY loaded_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let mibs = rows.into_iter().map(|(id, name, filename, object_count, uploaded_at)| {
        MibRecord { id, name, filename, object_count: object_count.unwrap_or(0), uploaded_at }
    }).collect();
    Ok(Json(mibs))
}

pub async fn upload_mib(
    State(state): State<AppState>,
    Json(req): Json<UploadMibRequest>,
) -> Result<Json<MibRecord>, ApiError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO loaded_mibs (id, name, file_name, content, loaded_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.filename)
    .bind(&req.content)
    .bind(now)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(MibRecord { id, name: req.name, filename: req.filename, object_count: 0, uploaded_at: now }))
}

pub async fn delete_mib(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("DELETE FROM loaded_mibs WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::from((StatusCode::NOT_FOUND, format!("MIB {} not found", id))));
    }
    Ok(Json(serde_json::json!({ "deleted": id })))
}

pub async fn resolve_oid(
    State(_state): State<AppState>,
    Query(q): Query<OidQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if let Some((name, desc)) = resolve_well_known(&q.oid) {
        return Ok(Json(serde_json::json!({
            "oid": q.oid,
            "name": name,
            "description": desc,
        })));
    }
    Ok(Json(serde_json::json!({
        "oid": q.oid,
        "name": null,
        "description": "OID not found in loaded MIBs",
    })))
}
