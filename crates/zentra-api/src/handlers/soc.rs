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
pub struct ListCasesQuery {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCaseRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCaseRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct AddEventRequest {
    pub event_type: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AddIocRequest {
    pub list_name: String,
    pub ioc_type: String,
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckIocRequest {
    pub indicator: String,
}

#[derive(Debug, Deserialize)]
pub struct HuntQuery {
    pub q: Option<String>,
    pub src_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. GET /api/security/cases
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_cases(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<ListCasesQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(50).min(500);
    let offset = p.offset.unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, title, description, status, priority, assignee, alert_count, created_at, updated_at \
         FROM soc_cases WHERE org_id = $1 \
         AND ($2::text IS NULL OR status = $2) \
         AND ($3::text IS NULL OR priority = $3) \
         ORDER BY updated_at DESC LIMIT $4 OFFSET $5"
    ).bind(org_id).bind(p.status.as_deref()).bind(p.priority.as_deref())
    .bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let cases: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "title": r.get::<String, _>("title"),
        "description": r.get::<Option<String>, _>("description"),
        "status": r.get::<String, _>("status"), "priority": r.get::<String, _>("priority"),
        "assignee": r.get::<Option<Uuid>, _>("assignee"),
        "alert_count": r.get::<Option<i32>, _>("alert_count"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "updated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM soc_cases WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"cases": cases, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. POST /api/security/cases
// ═══════════════════════════════════════════════════════════════════════════

pub async fn create_case(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateCaseRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let r = sqlx::query(
        "INSERT INTO soc_cases (id, org_id, title, description, priority, assignee) VALUES ($1, $2, $3, $4, $5, $6)"
    ).bind(id).bind(org_id).bind(&req.title).bind(&req.description)
    .bind(req.priority.as_deref().unwrap_or("medium")).bind(req.assignee)
    .execute(&state.pool).await;

    match r {
        Ok(_) => (StatusCode::CREATED, Json(json!({"id": id, "message": "Case created"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. GET /api/security/cases/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
) -> impl IntoResponse {
    let case = sqlx::query(
        "SELECT id, title, description, status, priority, assignee, alert_count, created_at, updated_at \
         FROM soc_cases WHERE id = $1"
    ).bind(case_id).fetch_optional(&state.pool).await;

    match case {
        Ok(Some(r)) => {
            let events = sqlx::query(
                "SELECT id, event_type, content, user_id, created_at FROM case_events WHERE case_id = $1 ORDER BY created_at"
            ).bind(case_id).fetch_all(&state.pool).await.unwrap_or_default();

            let ev: Vec<serde_json::Value> = events.iter().map(|e| json!({
                "id": e.get::<Uuid, _>("id"), "event_type": e.get::<String, _>("event_type"),
                "content": e.get::<serde_json::Value, _>("content"),
                "user_id": e.get::<Option<Uuid>, _>("user_id"),
                "created_at": e.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            })).collect();

            (StatusCode::OK, Json(json!({
                "id": r.get::<Uuid, _>("id"), "title": r.get::<String, _>("title"),
                "description": r.get::<Option<String>, _>("description"),
                "status": r.get::<String, _>("status"), "priority": r.get::<String, _>("priority"),
                "assignee": r.get::<Option<Uuid>, _>("assignee"),
                "alert_count": r.get::<Option<i32>, _>("alert_count"),
                "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                "updated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
                "events": ev,
            }))).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "not_found"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. PUT /api/security/cases/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_case(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Json(req): Json<UpdateCaseRequest>,
) -> impl IntoResponse {
    let _ = sqlx::query(
        "UPDATE soc_cases SET title = COALESCE($1, title), description = COALESCE($2, description), \
         status = COALESCE($3, status), priority = COALESCE($4, priority), \
         assignee = COALESCE($5, assignee), updated_at = NOW() WHERE id = $6"
    ).bind(&req.title).bind(&req.description).bind(&req.status)
    .bind(&req.priority).bind(req.assignee).bind(case_id)
    .execute(&state.pool).await;

    (StatusCode::OK, Json(json!({"message": "Case updated"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. POST /api/security/cases/:id/events
// ═══════════════════════════════════════════════════════════════════════════

pub async fn add_case_event(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Json(req): Json<AddEventRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let _ = sqlx::query(
        "INSERT INTO case_events (id, case_id, event_type, content) VALUES ($1, $2, $3, $4)"
    ).bind(id).bind(case_id).bind(&req.event_type).bind(&req.content)
    .execute(&state.pool).await;

    (StatusCode::CREATED, Json(json!({"id": id, "message": "Event added"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. POST /api/security/alerts/:id/escalate
// ═══════════════════════════════════════════════════════════════════════════

pub async fn escalate_alert(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(alert_id): Path<Uuid>,
) -> impl IntoResponse {
    // Create a case from the alert
    let case_id = Uuid::new_v4();
    let alert_info = sqlx::query(
        "SELECT severity, host(src_ip)::text as src_ip, host(dest_ip)::text as dest_ip FROM ids_alerts WHERE id = $1"
    ).bind(alert_id).fetch_optional(&state.pool).await;

    let title = match &alert_info {
        Ok(Some(r)) => format!("Escalated alert: {} → {}", r.get::<Option<String>, _>("src_ip").unwrap_or_default(), r.get::<Option<String>, _>("dest_ip").unwrap_or_default()),
        _ => "Escalated alert".to_string(),
    };

    let _ = sqlx::query(
        "INSERT INTO soc_cases (id, org_id, title, priority, alert_count) VALUES ($1, $2, $3, 'high', 1)"
    ).bind(case_id).bind(org_id).bind(&title).execute(&state.pool).await;

    let _ = sqlx::query("UPDATE ids_alerts SET status = 'acknowledged' WHERE id = $1")
        .bind(alert_id).execute(&state.pool).await;

    (StatusCode::CREATED, Json(json!({"case_id": case_id, "message": "Alert escalated to case"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. GET /api/security/hunt — unified search
// ═══════════════════════════════════════════════════════════════════════════

pub async fn hunt_search(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(p): Query<HuntQuery>,
) -> impl IntoResponse {
    let limit = p.limit.unwrap_or(100).min(500);
    let q = p.q.unwrap_or_default();

    let mut results: Vec<serde_json::Value> = Vec::new();

    // Search IDS alerts
    if q.is_empty() || q.contains("alert") || q.contains("ids") {
        let rows = sqlx::query(
            "SELECT id, timestamp, host(src_ip)::text as src_ip, host(dest_ip)::text as dest_ip, severity, status \
             FROM ids_alerts WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2"
        ).bind(org_id).bind(limit / 4).fetch_all(&state.pool).await.unwrap_or_default();
        for r in &rows {
            results.push(json!({"type": "ids_alert", "id": r.get::<Uuid, _>("id"),
                "timestamp": r.get::<chrono::DateTime<chrono::Utc>, _>("timestamp"),
                "src_ip": r.get::<Option<String>, _>("src_ip"), "dest_ip": r.get::<Option<String>, _>("dest_ip"),
                "severity": r.get::<String, _>("severity")}));
        }
    }

    // Search connection logs
    if q.is_empty() || q.contains("conn") {
        let rows = sqlx::query(
            "SELECT id, timestamp, host(src_ip)::text as src_ip, host(dest_ip)::text as dest_ip, proto, service \
             FROM connection_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2"
        ).bind(org_id).bind(limit / 4).fetch_all(&state.pool).await.unwrap_or_default();
        for r in &rows {
            results.push(json!({"type": "connection", "id": r.get::<Uuid, _>("id"),
                "timestamp": r.get::<chrono::DateTime<chrono::Utc>, _>("timestamp"),
                "src_ip": r.get::<Option<String>, _>("src_ip"), "dest_ip": r.get::<Option<String>, _>("dest_ip"),
                "proto": r.get::<Option<String>, _>("proto")}));
        }
    }

    // Search DNS logs
    if q.is_empty() || q.contains("dns") {
        let rows = sqlx::query(
            "SELECT id, timestamp, host(src_ip)::text as src_ip, query, rcode \
             FROM dns_logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2"
        ).bind(org_id).bind(limit / 4).fetch_all(&state.pool).await.unwrap_or_default();
        for r in &rows {
            results.push(json!({"type": "dns", "id": r.get::<Uuid, _>("id"),
                "timestamp": r.get::<chrono::DateTime<chrono::Utc>, _>("timestamp"),
                "src_ip": r.get::<Option<String>, _>("src_ip"),
                "query": r.get::<Option<String>, _>("query")}));
        }
    }

    (StatusCode::OK, Json(json!({"results": results, "total": results.len()}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. GET /api/security/ioc — IOC lists
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_ioc(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let lists = sqlx::query(
        "SELECT l.id, l.name, l.type, l.description, l.created_at, \
         (SELECT COUNT(*) FROM ioc_entries e WHERE e.list_id = l.id) as entry_count \
         FROM ioc_lists l WHERE l.org_id = $1 ORDER BY l.created_at DESC"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let items: Vec<serde_json::Value> = lists.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "name": r.get::<String, _>("name"),
        "type": r.get::<String, _>("type"), "description": r.get::<Option<String>, _>("description"),
        "entry_count": r.get::<Option<i64>, _>("entry_count"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
    })).collect();

    (StatusCode::OK, Json(json!({"lists": items}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. POST /api/security/ioc
// ═══════════════════════════════════════════════════════════════════════════

pub async fn add_ioc(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<AddIocRequest>,
) -> impl IntoResponse {
    // Find or create the list
    let list_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM ioc_lists WHERE org_id = $1 AND name = $2"
    ).bind(org_id).bind(&req.list_name).fetch_optional(&state.pool).await.unwrap_or(None);

    let list_id = match list_id {
        Some(id) => id,
        None => {
            let id = Uuid::new_v4();
            let _ = sqlx::query("INSERT INTO ioc_lists (id, org_id, name, type, description) VALUES ($1, $2, $3, $4, $5)")
                .bind(id).bind(org_id).bind(&req.list_name).bind(&req.ioc_type).bind(&req.description)
                .execute(&state.pool).await;
            id
        }
    };

    let entry_id = Uuid::new_v4();
    let _ = sqlx::query("INSERT INTO ioc_entries (id, list_id, value) VALUES ($1, $2, $3)")
        .bind(entry_id).bind(list_id).bind(&req.value).execute(&state.pool).await;

    (StatusCode::CREATED, Json(json!({"id": entry_id, "list_id": list_id, "message": "IOC added"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. DELETE /api/security/ioc/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn delete_ioc(
    State(_state): State<AppState>,
    Path(entry_id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = sqlx::query("DELETE FROM ioc_entries WHERE id = $1").bind(entry_id).execute(&_state.pool).await;
    (StatusCode::OK, Json(json!({"message": "IOC entry deleted"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. POST /api/security/ioc/check
// ═══════════════════════════════════════════════════════════════════════════

pub async fn check_ioc(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CheckIocRequest>,
) -> impl IntoResponse {
    let found = sqlx::query(
        "SELECT e.id, e.value, l.name as list_name, l.type as list_type, e.added_at \
         FROM ioc_entries e JOIN ioc_lists l ON l.id = e.list_id \
         WHERE l.org_id = $1 AND e.value = $2"
    ).bind(org_id).bind(&req.indicator).fetch_all(&state.pool).await.unwrap_or_default();

    let matches: Vec<serde_json::Value> = found.iter().map(|r| json!({
        "id": r.get::<Uuid, _>("id"), "value": r.get::<String, _>("value"),
        "list_name": r.get::<String, _>("list_name"), "list_type": r.get::<String, _>("list_type"),
        "added_at": r.get::<chrono::DateTime<chrono::Utc>, _>("added_at"),
    })).collect();

    (StatusCode::OK, Json(json!({"indicator": req.indicator, "found": !matches.is_empty(), "matches": matches}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. GET /api/security/threat-intel/:indicator
// ═══════════════════════════════════════════════════════════════════════════

pub async fn threat_intel_lookup(
    State(state): State<AppState>,
    Path(indicator): Path<String>,
) -> impl IntoResponse {
    // Check cache first
    let cached = sqlx::query(
        "SELECT result, source, queried_at FROM threat_intel_cache WHERE indicator = $1 AND (expires_at IS NULL OR expires_at > NOW()) LIMIT 1"
    ).bind(&indicator).fetch_optional(&state.pool).await;

    if let Ok(Some(r)) = cached {
        return (StatusCode::OK, Json(json!({
            "indicator": indicator, "cached": true,
            "result": r.get::<serde_json::Value, _>("result"),
            "source": r.get::<String, _>("source"),
            "queried_at": r.get::<chrono::DateTime<chrono::Utc>, _>("queried_at"),
        }))).into_response();
    }

    // In production this would call external APIs (VirusTotal, AbuseIPDB, etc.)
    // For now return empty with cache miss
    (StatusCode::OK, Json(json!({
        "indicator": indicator, "cached": false,
        "result": null, "message": "No cached result. External lookup not configured."
    }))).into_response()
}
