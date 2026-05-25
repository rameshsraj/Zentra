use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::{self, Row};
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;

#[derive(Debug, Deserialize)]
pub struct FileListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub verdict: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. GET /api/security/files — scanned file list
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_files(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<FileListQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, filename, file_size, mime_type, md5, sha1, sha256, verdict, yara_matches, \
         source, scanned_at \
         FROM file_scans WHERE org_id = $1 \
         AND ($2::text IS NULL OR verdict = $2) \
         ORDER BY scanned_at DESC LIMIT $3 OFFSET $4"
    ).bind(org_id).bind(p.verdict.as_deref()).bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let files: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "filename": r.get::<String, _>("filename"),
        "file_size": r.get::<i64, _>("file_size"), "mime_type": r.get::<Option<String>, _>("mime_type"),
        "md5": r.get::<Option<String>, _>("md5"), "sha1": r.get::<Option<String>, _>("sha1"),
        "sha256": r.get::<Option<String>, _>("sha256"), "verdict": r.get::<String, _>("verdict"),
        "yara_matches": r.get::<Option<serde_json::Value>, _>("yara_matches"),
        "source": r.get::<Option<String>, _>("source"),
        "scanned_at": r.get::<chrono::DateTime<chrono::Utc>, _>("scanned_at"),
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_scans WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"files": files, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. POST /api/security/files/scan — submit file metadata for scan
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub filename: String,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub md5: Option<String>,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub source: Option<String>,
}

pub async fn scan_file(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<ScanRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    // In production, this would trigger actual YARA scanning + VT lookup
    let verdict = "clean"; // placeholder
    let r = sqlx::query(
        "INSERT INTO file_scans (id, org_id, filename, file_size, mime_type, md5, sha1, sha256, verdict, source) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
    ).bind(id).bind(org_id).bind(&req.filename).bind(req.file_size)
    .bind(&req.mime_type).bind(&req.md5).bind(&req.sha1).bind(&req.sha256)
    .bind(verdict).bind(&req.source)
    .execute(&state.pool).await;

    match r {
        Ok(_) => (StatusCode::CREATED, Json(json!({"id": id, "verdict": verdict}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. GET /api/security/files/:id — detail
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_file(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query(
        "SELECT id, filename, file_size, mime_type, md5, sha1, sha256, verdict, yara_matches, \
         source, metadata, scanned_at \
         FROM file_scans WHERE id = $1"
    ).bind(file_id).fetch_optional(&state.pool).await;

    match row {
        Ok(Some(r)) => (StatusCode::OK, Json(json!({
            "id": r.get::<Uuid, _>("id"), "filename": r.get::<String, _>("filename"),
            "file_size": r.get::<i64, _>("file_size"), "mime_type": r.get::<Option<String>, _>("mime_type"),
            "md5": r.get::<Option<String>, _>("md5"), "sha1": r.get::<Option<String>, _>("sha1"),
            "sha256": r.get::<Option<String>, _>("sha256"), "verdict": r.get::<String, _>("verdict"),
            "yara_matches": r.get::<Option<serde_json::Value>, _>("yara_matches"),
            "source": r.get::<Option<String>, _>("source"),
            "metadata": r.get::<Option<serde_json::Value>, _>("metadata"),
            "scanned_at": r.get::<chrono::DateTime<chrono::Utc>, _>("scanned_at"),
        }))).into_response(),
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "not_found"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. GET /api/security/files/rules — YARA rules
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_yara_rules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let rows = sqlx::query(
        "SELECT id, name, description, rule_text, source, enabled, created_at \
         FROM yara_rules WHERE org_id = $1 ORDER BY created_at"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let rules: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "name": r.get::<String, _>("name"),
        "description": r.get::<Option<String>, _>("description"),
        "rule_text": r.get::<String, _>("rule_text"),
        "source": r.get::<Option<String>, _>("source"),
        "enabled": r.get::<bool, _>("enabled"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
    })).collect();

    (StatusCode::OK, Json(json!({"rules": rules}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. POST /api/security/files/rules — upload YARA rule
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct CreateYaraRule {
    pub name: String,
    pub description: Option<String>,
    pub rule_text: String,
    pub source: Option<String>,
}

pub async fn create_yara_rule(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateYaraRule>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let r = sqlx::query(
        "INSERT INTO yara_rules (id, org_id, name, description, rule_text, source) VALUES ($1, $2, $3, $4, $5, $6)"
    ).bind(id).bind(org_id).bind(&req.name).bind(&req.description).bind(&req.rule_text).bind(&req.source)
    .execute(&state.pool).await;

    match r {
        Ok(_) => (StatusCode::CREATED, Json(json!({"id": id, "message": "YARA rule created"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. GET /api/security/files/stats
// ═══════════════════════════════════════════════════════════════════════════

pub async fn file_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let verdict_dist = sqlx::query_as::<_, (String, i64)>(
        "SELECT verdict, COUNT(*) FROM file_scans WHERE org_id = $1 GROUP BY verdict ORDER BY COUNT(*) DESC"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM file_scans WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "total_files": total,
        "verdict_distribution": verdict_dist.iter().map(|(v, c)| json!({"verdict": v, "count": c})).collect::<Vec<_>>(),
    }))).into_response()
}
