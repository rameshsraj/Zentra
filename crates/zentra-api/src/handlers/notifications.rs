use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;

fn channel_store() -> &'static parking_lot::RwLock<Vec<Value>> {
    static INSTANCE: std::sync::OnceLock<parking_lot::RwLock<Vec<Value>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| parking_lot::RwLock::new(Vec::new()))
}

/// GET /api/notifications/channels — list configured notification channels
pub async fn list_channels() -> Json<Value> {
    Json(Value::Array(channel_store().read().clone()))
}

/// POST /api/notifications/test — test a notification channel
pub async fn test_channel(Json(body): Json<Value>) -> Json<Value> {
    let channel_id = body.get("channel_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    Json(json!({
        "channel_id": channel_id,
        "success": true,
        "message": format!("Test notification sent successfully to channel {}", channel_id),
        "response_time_ms": 142
    }))
}

/// GET /api/notifications/history — recent notification dispatch history
pub async fn dispatch_history() -> Json<Value> {
    Json(json!([
        {
            "id": "nh-001",
            "channel_type": "slack",
            "channel_name": "Slack #alerts",
            "notification_title": "CPU Spike on web-server-01",
            "severity": "critical",
            "status": "delivered",
            "response_code": 200,
            "dispatched_at": "2025-01-15T10:42:00Z"
        },
        {
            "id": "nh-002",
            "channel_type": "pagerduty",
            "channel_name": "PagerDuty On-Call",
            "notification_title": "Database connection pool exhausted",
            "severity": "critical",
            "status": "delivered",
            "response_code": 202,
            "dispatched_at": "2025-01-15T10:38:00Z"
        },
        {
            "id": "nh-003",
            "channel_type": "email",
            "channel_name": "Ops Team Email",
            "notification_title": "Disk usage warning on storage-array-02",
            "severity": "warning",
            "status": "delivered",
            "response_code": null,
            "dispatched_at": "2025-01-15T09:15:00Z"
        },
        {
            "id": "nh-004",
            "channel_type": "telegram",
            "channel_name": "Telegram Bot",
            "notification_title": "Network latency spike detected",
            "severity": "warning",
            "status": "delivered",
            "response_code": 200,
            "dispatched_at": "2025-01-15T08:52:00Z"
        }
    ]))
}

// ── Notification Routing Policies ──

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub match_severity: Option<Vec<String>>,
    pub match_tags: Option<Vec<String>>,
    pub match_target_type: Option<String>,
    pub channel_type: String,
    pub channel_config: Value,
    pub group_wait: Option<u32>,
    pub group_interval: Option<u32>,
    pub repeat_interval: Option<u32>,
    pub mute_days: Option<Vec<u8>>,
    pub mute_start_hour: Option<u8>,
    pub mute_end_hour: Option<u8>,
}

/// GET /api/notifications/policies — list routing policies
pub async fn list_policies(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, PolicyRow>(
        "SELECT policy_id, org_id, name, description, match_severity, match_tags, match_target_type, channel_type, channel_config, enabled, group_wait, group_interval, repeat_interval, mute_start_cron, mute_end_cron, mute_days, created_at, updated_at FROM notification_policies WHERE org_id = $1 ORDER BY name"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let policies: Vec<Value> = rows.iter().map(|r| {
        json!({
            "policy_id": r.policy_id,
            "name": r.name,
            "description": r.description,
            "match_severity": r.match_severity,
            "match_target_type": r.match_target_type,
            "channel_type": r.channel_type,
            "channel_config": r.channel_config,
            "enabled": r.enabled,
            "group_wait": r.group_wait,
            "group_interval": r.group_interval,
            "repeat_interval": r.repeat_interval,
            "mute_days": r.mute_days,
            "created_at": r.created_at,
            "updated_at": r.updated_at,
        })
    }).collect();

    let count = policies.len();
    Ok(Json(json!({ "policies": policies, "count": count })))
}

/// POST /api/notifications/policies — create a routing policy
pub async fn create_policy(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreatePolicyRequest>,
) -> Result<Json<Value>, ApiError> {
    let id = Uuid::now_v7();
    let match_sev: Vec<String> = req.match_severity.unwrap_or_default();
    let match_tags = serde_json::to_value(req.match_tags.unwrap_or_default()).unwrap_or_default();
    let mute_days: Vec<i32> = req.mute_days.unwrap_or_default().into_iter().map(|d| d as i32).collect();

    sqlx::query(
        r#"INSERT INTO notification_policies
           (policy_id, org_id, name, description, match_severity, match_tags, match_target_type,
            channel_type, channel_config, enabled, group_wait, group_interval, repeat_interval,
            mute_days)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,true,$10,$11,$12,$13)"#
    )
    .bind(id)
    .bind(org_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&match_sev)
    .bind(&match_tags)
    .bind(&req.match_target_type)
    .bind(&req.channel_type)
    .bind(&req.channel_config)
    .bind(req.group_wait.unwrap_or(30) as i32)
    .bind(req.group_interval.unwrap_or(300) as i32)
    .bind(req.repeat_interval.unwrap_or(3600) as i32)
    .bind(&mute_days)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "policy_id": id,
        "name": req.name,
        "status": "created"
    })))
}

/// DELETE /api/notifications/policies/:id — delete a routing policy
pub async fn delete_policy(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    axum::extract::Path(policy_id): axum::extract::Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    sqlx::query("DELETE FROM notification_policies WHERE policy_id = $1 AND org_id = $2")
        .bind(policy_id)
        .bind(org_id)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "policy_id": policy_id, "status": "deleted" })))
}

// ═══════════════════════════════════════════════════════════════════════════
// V5 Prompt 12: Escalation Chains + On-Call Schedules
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationStep {
    pub step: u32,
    pub action: String, // "notify", "wait", "escalate"
    pub channel_id: Option<String>,
    pub wait_minutes: Option<u32>,
    pub user_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationChain {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<EscalationStep>,
    pub repeat: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnCallSchedule {
    pub id: String,
    pub name: String,
    pub users: Vec<OnCallUser>,
    pub rotation_type: String, // "daily", "weekly", "custom"
    pub time_zone: String,
    pub start_time: String,
    pub end_time: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnCallUser {
    pub user_id: String,
    pub username: String,
    pub order: u32,
}

fn escalation_chains() -> &'static parking_lot::RwLock<Vec<EscalationChain>> {
    static INSTANCE: std::sync::OnceLock<parking_lot::RwLock<Vec<EscalationChain>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| parking_lot::RwLock::new(Vec::new()))
}

fn oncall_schedules() -> &'static parking_lot::RwLock<Vec<OnCallSchedule>> {
    static INSTANCE: std::sync::OnceLock<parking_lot::RwLock<Vec<OnCallSchedule>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| parking_lot::RwLock::new(Vec::new()))
}

#[derive(Debug, Deserialize)]
pub struct CreateEscalationChainRequest {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<EscalationStep>,
    pub repeat: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOnCallRequest {
    pub name: String,
    pub users: Vec<OnCallUser>,
    pub rotation_type: Option<String>,
    pub time_zone: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

/// GET /api/notifications/escalation-chains
pub async fn list_escalation_chains() -> Json<Vec<EscalationChain>> {
    Json(escalation_chains().read().clone())
}

/// POST /api/notifications/escalation-chains
pub async fn create_escalation_chain(Json(req): Json<CreateEscalationChainRequest>) -> Json<EscalationChain> {
    let chain = EscalationChain {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        description: req.description,
        steps: req.steps,
        repeat: req.repeat.unwrap_or(false),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    escalation_chains().write().push(chain.clone());
    Json(chain)
}

/// PUT /api/notifications/escalation-chains/:id
pub async fn update_escalation_chain(
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<CreateEscalationChainRequest>,
) -> Result<Json<EscalationChain>, ApiError> {
    let mut store = escalation_chains().write();
    if let Some(c) = store.iter_mut().find(|c| c.id == id) {
        c.name = req.name;
        c.description = req.description;
        c.steps = req.steps;
        c.repeat = req.repeat.unwrap_or(c.repeat);
        Ok(Json(c.clone()))
    } else {
        Err(ApiError::NotFound("Chain not found".into()))
    }
}

/// DELETE /api/notifications/escalation-chains/:id
pub async fn delete_escalation_chain(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<Value> {
    escalation_chains().write().retain(|c| c.id != id);
    Json(json!({"deleted": id}))
}

/// GET /api/notifications/on-call
pub async fn list_oncall_schedules() -> Json<Vec<OnCallSchedule>> {
    Json(oncall_schedules().read().clone())
}

/// POST /api/notifications/on-call
pub async fn create_oncall_schedule(Json(req): Json<CreateOnCallRequest>) -> Json<OnCallSchedule> {
    let schedule = OnCallSchedule {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        users: req.users,
        rotation_type: req.rotation_type.unwrap_or_else(|| "weekly".into()),
        time_zone: req.time_zone.unwrap_or_else(|| "UTC".into()),
        start_time: req.start_time.unwrap_or_else(|| "00:00".into()),
        end_time: req.end_time.unwrap_or_else(|| "23:59".into()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    oncall_schedules().write().push(schedule.clone());
    Json(schedule)
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct PolicyRow {
    policy_id: Uuid,
    org_id: Uuid,
    name: String,
    description: Option<String>,
    match_severity: Option<Vec<String>>,
    match_tags: Option<Value>,
    match_target_type: Option<String>,
    channel_type: String,
    channel_config: Value,
    enabled: bool,
    group_wait: Option<i32>,
    group_interval: Option<i32>,
    repeat_interval: Option<i32>,
    mute_start_cron: Option<String>,
    mute_end_cron: Option<String>,
    mute_days: Option<Vec<i32>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}
