use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use sqlx;
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;

// ═══════════════════════════════════════════════════════════════════════════
// Query params
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct ListRulesQuery {
    pub category: Option<String>,
    pub severity: Option<String>,
    pub enabled: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ListAlertsQuery {
    pub severity: Option<String>,
    pub status: Option<String>,
    pub src_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRuleRequest {
    pub sid: i32,
    pub category: Option<String>,
    pub severity: Option<String>,
    pub message: String,
    pub rule_text: String,
    pub enabled: Option<bool>,
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRuleRequest {
    pub category: Option<String>,
    pub severity: Option<String>,
    pub message: Option<String>,
    pub rule_text: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BulkRuleAction {
    pub action: String,  // "enable" or "disable"
    pub rule_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertStatusRequest {
    pub status: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ids/rules
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_rules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ListRulesQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (Uuid, i32, String, String, String, String, bool, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, sid, category, severity, message, rule_text, enabled, source, created_at \
         FROM ids_rules WHERE org_id = $1 \
         AND ($2::text IS NULL OR category ILIKE '%' || $2 || '%') \
         AND ($3::text IS NULL OR severity = $3) \
         AND ($4::bool IS NULL OR enabled = $4) \
         AND ($5::text IS NULL OR message ILIKE '%' || $5 || '%') \
         ORDER BY sid LIMIT $6 OFFSET $7"
    )
    .bind(org_id)
    .bind(params.category.as_deref())
    .bind(params.severity.as_deref())
    .bind(params.enabled)
    .bind(params.search.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let rules: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.0, "sid": r.1, "category": r.2, "severity": r.3,
        "message": r.4, "rule_text": r.5, "enabled": r.6, "source": r.7, "created_at": r.8,
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ids_rules WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"rules": rules, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/security/ids/rules
// ═══════════════════════════════════════════════════════════════════════════

pub async fn create_rule(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO ids_rules (id, org_id, sid, category, severity, message, rule_text, enabled, source) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(id).bind(org_id).bind(req.sid)
    .bind(req.category.as_deref().unwrap_or("uncategorized"))
    .bind(req.severity.as_deref().unwrap_or("medium"))
    .bind(&req.message).bind(&req.rule_text)
    .bind(req.enabled.unwrap_or(true))
    .bind(req.source.as_deref().unwrap_or("custom"))
    .execute(&state.pool).await;

    match result {
        Ok(_) => (StatusCode::CREATED, Json(json!({"id": id, "message": "Rule created"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/security/ids/rules/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
    Json(req): Json<UpdateRuleRequest>,
) -> impl IntoResponse {
    let _ = sqlx::query(
        "UPDATE ids_rules SET \
         category = COALESCE($1, category), severity = COALESCE($2, severity), \
         message = COALESCE($3, message), rule_text = COALESCE($4, rule_text), \
         enabled = COALESCE($5, enabled), updated_at = NOW() WHERE id = $6"
    )
    .bind(&req.category).bind(&req.severity)
    .bind(&req.message).bind(&req.rule_text)
    .bind(req.enabled).bind(rule_id)
    .execute(&state.pool).await;

    (StatusCode::OK, Json(json!({"message": "Rule updated"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/security/ids/rules/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn delete_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = sqlx::query("DELETE FROM ids_rules WHERE id = $1").bind(rule_id).execute(&state.pool).await;
    (StatusCode::OK, Json(json!({"message": "Rule deleted"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/security/ids/rules/bulk
// ═══════════════════════════════════════════════════════════════════════════

pub async fn bulk_rules(
    State(state): State<AppState>,
    Json(req): Json<BulkRuleAction>,
) -> impl IntoResponse {
    let enabled = req.action == "enable";
    let mut affected = 0i64;
    for id in &req.rule_ids {
        let r = sqlx::query("UPDATE ids_rules SET enabled = $1, updated_at = NOW() WHERE id = $2")
            .bind(enabled).bind(id).execute(&state.pool).await;
        if let Ok(r) = r { affected += r.rows_affected() as i64; }
    }
    (StatusCode::OK, Json(json!({"message": format!("Bulk {} done", req.action), "affected": affected}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/security/ids/rules/import — import rules from text
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct ImportRulesRequest {
    pub rules_text: String,
    pub source: Option<String>,
}

pub async fn import_rules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<ImportRulesRequest>,
) -> impl IntoResponse {
    let mut imported = 0i64;
    let source = req.source.as_deref().unwrap_or("import");

    for line in req.rules_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        // Extract SID from rule text (e.g., "sid:2001234;")
        let sid = extract_sid(line);
        if sid == 0 { continue; }

        let msg = extract_msg(line).unwrap_or_else(|| format!("Rule SID {}", sid));

        let id = Uuid::new_v4();
        let r = sqlx::query(
            "INSERT INTO ids_rules (id, org_id, sid, message, rule_text, source) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (sid) DO NOTHING"
        )
        .bind(id).bind(org_id).bind(sid).bind(&msg).bind(line).bind(source)
        .execute(&state.pool).await;
        if let Ok(r) = r { imported += r.rows_affected() as i64; }
    }

    (StatusCode::OK, Json(json!({"message": "Import complete", "imported": imported}))).into_response()
}

fn extract_sid(rule: &str) -> i32 {
    if let Some(pos) = rule.find("sid:") {
        let after = &rule[pos + 4..];
        let end = after.find(';').unwrap_or(after.len());
        after[..end].trim().parse().unwrap_or(0)
    } else { 0 }
}

fn extract_msg(rule: &str) -> Option<String> {
    if let Some(pos) = rule.find("msg:\"") {
        let after = &rule[pos + 5..];
        let end = after.find('"')?;
        Some(after[..end].to_string())
    } else { None }
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ids/alerts
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_alerts(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ListAlertsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        Uuid, Option<Uuid>, chrono::DateTime<chrono::Utc>, Option<String>, Option<i32>,
        Option<String>, Option<i32>, Option<String>, Option<String>, String, String, Option<String>,
    )>(
        "SELECT a.id, a.rule_id, a.timestamp, host(a.src_ip)::text, a.src_port, \
         host(a.dest_ip)::text, a.dest_port, a.protocol, a.payload, a.severity, a.status, a.community_id \
         FROM ids_alerts a WHERE a.org_id = $1 \
         AND ($2::text IS NULL OR a.severity = $2) \
         AND ($3::text IS NULL OR a.status = $3) \
         ORDER BY a.timestamp DESC LIMIT $4 OFFSET $5"
    )
    .bind(org_id)
    .bind(params.severity.as_deref())
    .bind(params.status.as_deref())
    .bind(limit).bind(offset)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let alerts: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "id": r.0, "rule_id": r.1, "timestamp": r.2,
        "src_ip": r.3, "src_port": r.4, "dest_ip": r.5, "dest_port": r.6,
        "protocol": r.7, "payload": r.8, "severity": r.9, "status": r.10, "community_id": r.11,
    })).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ids_alerts WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({"alerts": alerts, "total": total}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ids/alerts/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_alert(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, (
        Uuid, Option<Uuid>, chrono::DateTime<chrono::Utc>, Option<String>, Option<i32>,
        Option<String>, Option<i32>, Option<String>, Option<String>, String, String, Option<String>, Option<Uuid>,
    )>(
        "SELECT a.id, a.rule_id, a.timestamp, host(a.src_ip)::text, a.src_port, \
         host(a.dest_ip)::text, a.dest_port, a.protocol, a.payload, a.severity, a.status, a.community_id, a.assigned_to \
         FROM ids_alerts a WHERE a.id = $1"
    ).bind(alert_id).fetch_optional(&state.pool).await;

    match row {
        Ok(Some(r)) => {
            let mut alert = json!({
                "id": r.0, "rule_id": r.1, "timestamp": r.2,
                "src_ip": r.3, "src_port": r.4, "dest_ip": r.5, "dest_port": r.6,
                "protocol": r.7, "payload": r.8, "severity": r.9, "status": r.10,
                "community_id": r.11, "assigned_to": r.12,
            });

            // Get associated rule details
            if let Some(rule_id) = r.1 {
                let rule = sqlx::query_as::<_, (i32, String, String)>(
                    "SELECT sid, message, category FROM ids_rules WHERE id = $1"
                ).bind(rule_id).fetch_optional(&state.pool).await.unwrap_or(None);
                if let Some(rule) = rule {
                    alert["rule_sid"] = json!(rule.0);
                    alert["rule_message"] = json!(rule.1);
                    alert["rule_category"] = json!(rule.2);
                }
            }

            (StatusCode::OK, Json(alert)).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "alert_not_found"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/security/ids/alerts/:id/status
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_alert_status(
    State(state): State<AppState>,
    Path(alert_id): Path<Uuid>,
    Json(req): Json<UpdateAlertStatusRequest>,
) -> impl IntoResponse {
    let _ = sqlx::query("UPDATE ids_alerts SET status = $1 WHERE id = $2")
        .bind(&req.status).bind(alert_id).execute(&state.pool).await;
    (StatusCode::OK, Json(json!({"message": "Alert status updated"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/security/ids/stats
// ═══════════════════════════════════════════════════════════════════════════

pub async fn ids_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let total_rules: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ids_rules WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
    let enabled_rules: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ids_rules WHERE org_id = $1 AND enabled = true")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    let total_alerts: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ids_alerts WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
    let alerts_24h: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ids_alerts WHERE org_id = $1 AND timestamp >= NOW() - INTERVAL '24 hours'"
    ).bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    let sev_dist = sqlx::query_as::<_, (String, i64)>(
        "SELECT severity, COUNT(*) FROM ids_alerts WHERE org_id = $1 AND timestamp >= NOW() - INTERVAL '24 hours' GROUP BY severity"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let top_categories = sqlx::query_as::<_, (String, i64)>(
        "SELECT r.category, COUNT(*) FROM ids_alerts a JOIN ids_rules r ON r.id = a.rule_id \
         WHERE a.org_id = $1 AND a.timestamp >= NOW() - INTERVAL '24 hours' GROUP BY r.category ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let top_sources = sqlx::query_as::<_, (String, i64)>(
        "SELECT host(src_ip)::text, COUNT(*) FROM ids_alerts WHERE org_id = $1 AND timestamp >= NOW() - INTERVAL '24 hours' AND src_ip IS NOT NULL GROUP BY src_ip ORDER BY COUNT(*) DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "total_rules": total_rules, "enabled_rules": enabled_rules,
        "total_alerts": total_alerts, "alerts_24h": alerts_24h,
        "severity_distribution": sev_dist.iter().map(|(s, c)| json!({"severity": s, "count": c})).collect::<Vec<_>>(),
        "top_categories": top_categories.iter().map(|(c, n)| json!({"category": c, "count": n})).collect::<Vec<_>>(),
        "top_sources": top_sources.iter().map(|(ip, n)| json!({"ip": ip, "count": n})).collect::<Vec<_>>(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/security/ids/ingest — EVE JSON ingestion
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub events: Vec<serde_json::Value>,
}

pub async fn ingest_eve(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<IngestRequest>,
) -> impl IntoResponse {
    let mut ingested = 0i64;

    for event in &req.events {
        let src_ip = event.get("src_ip").and_then(|v| v.as_str()).unwrap_or("0.0.0.0");
        let dest_ip = event.get("dest_ip").and_then(|v| v.as_str()).unwrap_or("0.0.0.0");
        let src_port = event.get("src_port").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let dest_port = event.get("dest_port").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let proto = event.get("proto").and_then(|v| v.as_str()).unwrap_or("unknown");

        if let Some(alert) = event.get("alert") {
            let sid = alert.get("signature_id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let severity_num = alert.get("severity").and_then(|v| v.as_i64()).unwrap_or(3);
            let severity = match severity_num { 1 => "critical", 2 => "high", 3 => "medium", _ => "low" };

            // Find matching rule
            let rule_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM ids_rules WHERE sid = $1")
                .bind(sid).fetch_optional(&state.pool).await.unwrap_or(None);

            let id = Uuid::new_v4();
            let r = sqlx::query(
                "INSERT INTO ids_alerts (id, org_id, rule_id, src_ip, src_port, dest_ip, dest_port, protocol, severity, community_id) \
                 VALUES ($1, $2, $3, $4::inet, $5, $6::inet, $7, $8, $9, $10)"
            )
            .bind(id).bind(org_id).bind(rule_id)
            .bind(src_ip).bind(src_port)
            .bind(dest_ip).bind(dest_port)
            .bind(proto).bind(severity)
            .bind(event.get("community_id").and_then(|v| v.as_str()))
            .execute(&state.pool).await;

            if r.is_ok() { ingested += 1; }
        }
    }

    (StatusCode::OK, Json(json!({"message": "Ingestion complete", "ingested": ingested}))).into_response()
}
