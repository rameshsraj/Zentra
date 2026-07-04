use axum::{
    extract::{ConnectInfo, Path, State},
    http::HeaderMap,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::auth_middleware::AuthContext;
use crate::error::ApiError;
use crate::handlers::otlp::validate_otlp_key;
use crate::state::AppState;

const AGENT_KEY_HEADER: &str = "x-zentra-key";
const AGENT_LOG_BUFFER_CAP: usize = 500;

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub datacenter_name: String,
    pub description: Option<String>,
    pub region_label: Option<String>,
}

/// A single agent log line shipped with a heartbeat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogLine {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentHeartbeatRequest {
    pub agent_version: String,
    pub active_targets: i32,
    pub os_name: String,
    pub dc_name: String,
    #[serde(default)]
    pub recent_logs: Vec<AgentLogLine>,
}

/// In-memory ring buffer of recent agent logs, keyed by (org_id, datacenter_name).
/// Lost on API restart by design — diagnostic only; persisted copy goes through OTLP.
fn agent_log_store() -> &'static RwLock<HashMap<(Uuid, String), VecDeque<AgentLogLine>>> {
    static STORE: OnceLock<RwLock<HashMap<(Uuid, String), VecDeque<AgentLogLine>>>> =
        OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn push_agent_logs(org_id: Uuid, dc_name: &str, lines: Vec<AgentLogLine>) {
    if lines.is_empty() {
        return;
    }
    let key = (org_id, dc_name.to_string());
    let mut store = agent_log_store().write();
    let buf = store.entry(key).or_insert_with(VecDeque::new);
    for line in lines {
        if buf.len() >= AGENT_LOG_BUFFER_CAP {
            buf.pop_front();
        }
        buf.push_back(line);
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AgentListRow {
    agent_id: Uuid,
    datacenter_name: String,
    status: String,
    last_seen: Option<DateTime<Utc>>,
    agent_version: Option<String>,
    active_target_count: i32,
    ip_address: Option<String>,
    region_label: Option<String>,
}

fn is_agent_admin(ctx: &AuthContext) -> bool {
    matches!(ctx.role.as_str(), "Admin" | "Org Admin" | "Super Admin")
}

fn require_agent_admin(ctx: &AuthContext) -> Result<(), ApiError> {
    if is_agent_admin(ctx) {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "Only Org Admin or Super Admin users can manage agents".into(),
        ))
    }
}

fn generate_agent_api_key() -> String {
    let mut rng = rand::thread_rng();
    let suffix = (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect::<String>();
    format!("isok_{suffix}")
}

fn hash_api_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn extract_agent_key(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(AGENT_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ApiError::Unauthorized("Missing X-Zentra-Key header".into()))
}

pub async fn register_agent(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(body): Json<RegisterAgentRequest>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let datacenter_name = body.datacenter_name.trim();
    if datacenter_name.is_empty() {
        return Err(ApiError::BadRequest("datacenter_name is required".into()));
    }

    let raw_key = generate_agent_api_key();
    let key_hash = hash_api_key(&raw_key);
    let key_prefix = raw_key.chars().take(13).collect::<String>();
    let agent_id = Uuid::now_v7();

    let mut tx = state
        .tsdb_pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let otlp_api_key_id: Uuid = sqlx::query_scalar(
        "INSERT INTO otlp_api_keys
         (org_id, name, key_hash, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, enabled, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, true, NOW(), NOW())
         RETURNING id",
    )
    .bind(ctx.org_id)
    .bind(format!("agent:{datacenter_name}"))
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(vec![
        "traces".to_string(),
        "metrics".to_string(),
        "logs".to_string(),
        "profiles".to_string(),
    ])
    .bind(10_000_i32)
    .bind(5_242_880_i32)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "INSERT INTO agent_fleet
         (agent_id, org_id, otlp_api_key_id, api_key_hash, datacenter_name, description, region_label, status, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', NOW(), NOW())",
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .bind(otlp_api_key_id)
    .bind(&key_hash)
    .bind(datacenter_name)
    .bind(body.description.as_deref())
    .bind(body.region_label.as_deref())
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "agent_id": agent_id,
        "api_key": raw_key,
        "warning": "Store this key securely. It will not be shown again.",
    })))
}

pub async fn heartbeat(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<AgentHeartbeatRequest>,
) -> Result<Json<Value>, ApiError> {
    let raw_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            headers
                .get(AGENT_KEY_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;
    let (org_id, _) = validate_otlp_key(&state, &raw_key)
        .await
        .map_err(ApiError::Unauthorized)?;
    let key_hash = hash_api_key(&raw_key);

    sqlx::query(
        "INSERT INTO agent_fleet
         (org_id, otlp_api_key_id, api_key_hash, datacenter_name, agent_version, active_target_count, last_seen, ip_address, os_name, status, created_at, updated_at)
         VALUES ($1, (SELECT id FROM otlp_api_keys WHERE key_hash = $2 LIMIT 1), $2, $3, $4, $5, NOW(), $6, $7, 'active', NOW(), NOW())
         ON CONFLICT (api_key_hash) DO UPDATE
         SET datacenter_name = EXCLUDED.datacenter_name,
             agent_version = EXCLUDED.agent_version,
             active_target_count = EXCLUDED.active_target_count,
             last_seen = NOW(),
             ip_address = EXCLUDED.ip_address,
             os_name = EXCLUDED.os_name,
             status = 'active',
             updated_at = NOW()",
    )
    .bind(org_id)
    .bind(&key_hash)
    .bind(body.dc_name.trim())
    .bind(body.agent_version.trim())
    .bind(body.active_targets)
    .bind(remote_addr.ip().to_string())
    .bind(body.os_name.trim())
    .execute(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Stash recent logs (if any) so the install wizard can tail them live,
    // and also persist them in the Log Explorer store keyed by service/dc.
    let lines_for_explorer: Vec<(i64, String)> = body
        .recent_logs
        .iter()
        .map(|l| {
            let ts = l.timestamp.timestamp_nanos_opt().unwrap_or(0);
            let line = format!("[{}] {} {}", l.level, l.target, l.message);
            (ts, line)
        })
        .collect();
    let mut labels = std::collections::HashMap::new();
    labels.insert("service".to_string(), "zentra-agent".to_string());
    labels.insert("datacenter".to_string(), body.dc_name.trim().to_string());
    labels.insert("org_id".to_string(), org_id.to_string());
    crate::handlers::log_engine::push_log_lines(labels, lines_for_explorer);
    push_agent_logs(org_id, body.dc_name.trim(), body.recent_logs);

    Ok(Json(json!({
        "server_timestamp_utc": Utc::now(),
    })))
}

/// GET /api/agents/by-dc/:dc_name/logs — returns the most recent buffered
/// log lines reported by the agent for that datacenter. Used by the install
/// wizard to give the operator a live tail without needing shell access.
pub async fn agent_logs_by_dc(
    State(_state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(dc_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let key = (ctx.org_id, dc_name);
    let store = agent_log_store().read();
    let lines: Vec<AgentLogLine> = store
        .get(&key)
        .map(|buf| buf.iter().cloned().collect())
        .unwrap_or_default();
    Ok(Json(json!({ "lines": lines })))
}

pub async fn agent_targets(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    // Accept Authorization: Bearer <key> (preferred) or legacy X-Zentra-Key header
    let raw_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            headers
                .get(AGENT_KEY_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let (org_id, _) = validate_otlp_key(&state, &raw_key)
        .await
        .map_err(ApiError::Unauthorized)?;

    let targets = zentra_db::repositories::targets::list_targets(&state.pool, org_id, None, true)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_iter()
        .filter(|target| {
            !(matches!(target.status, zentra_core::models::target::TargetStatus::Critical)
                && target.consecutive_failures >= 5)
        })
        .collect::<Vec<_>>();

    Ok(Json(json!(targets)))
}

pub async fn list_agents(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let rows = sqlx::query_as::<_, AgentListRow>(
        "SELECT agent_id, datacenter_name, status, last_seen, agent_version, active_target_count, ip_address, region_label
         FROM agent_fleet
         WHERE org_id = $1
         ORDER BY datacenter_name ASC",
    )
    .bind(ctx.org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let agents = rows
        .into_iter()
        .map(|row| {
            json!({
                "agent_id": row.agent_id,
                "datacenter_name": row.datacenter_name,
                "status": row.status,
                "last_seen": row.last_seen,
                "version": row.agent_version,
                "active_target_count": row.active_target_count,
                "ip_address": row.ip_address,
                "region_label": row.region_label,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!(agents)))
}

pub async fn deregister_agent(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let row = sqlx::query(
        "SELECT api_key_hash, otlp_api_key_id
         FROM agent_fleet
         WHERE agent_id = $1 AND org_id = $2",
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Agent not found".into()))?;

    let api_key_hash: String = row.get("api_key_hash");
    let otlp_api_key_id: Uuid = row.get("otlp_api_key_id");

    let mut tx = state
        .tsdb_pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "UPDATE agent_fleet
         SET status = 'deregistered', updated_at = NOW()
         WHERE agent_id = $1 AND org_id = $2",
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "UPDATE otlp_api_keys
         SET enabled = false, updated_at = NOW()
         WHERE id = $1 AND org_id = $2",
    )
    .bind(otlp_api_key_id)
    .bind(ctx.org_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    state.otlp_key_cache.remove(&api_key_hash);

    Ok(Json(json!({
        "deregistered": true,
        "agent_id": agent_id,
    })))
}

// ============================================================================
// Agent Key Management (Feature 6 — Admin-only isok_ key CRUD)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateAgentKeyRequest {
    pub name: String,
    pub description: Option<String>,
}

pub async fn list_agent_keys(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let rows = sqlx::query(
        "SELECT k.id, k.name, k.key_prefix, k.enabled, k.created_at, k.rate_limit_rpm,
                f.agent_id, f.datacenter_name, f.status AS agent_status, f.last_seen
         FROM otlp_api_keys k
         LEFT JOIN agent_fleet f ON f.otlp_api_key_id = k.id AND f.org_id = k.org_id
         WHERE k.org_id = $1 AND k.name NOT LIKE 'system-%'
         ORDER BY k.created_at DESC",
    )
    .bind(ctx.org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let keys: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "name": r.get::<String, _>("name"),
                "key_prefix": r.get::<String, _>("key_prefix"),
                "enabled": r.get::<bool, _>("enabled"),
                "created_at": r.get::<Option<DateTime<Utc>>, _>("created_at"),
                "rate_limit_rpm": r.get::<Option<i32>, _>("rate_limit_rpm"),
                "agent_id": r.get::<Option<Uuid>, _>("agent_id"),
                "datacenter_name": r.get::<Option<String>, _>("datacenter_name"),
                "agent_status": r.get::<Option<String>, _>("agent_status"),
                "last_seen": r.get::<Option<DateTime<Utc>>, _>("last_seen"),
            })
        })
        .collect();

    Ok(Json(json!(keys)))
}

pub async fn create_agent_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(body): Json<CreateAgentKeyRequest>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }

    let raw_key = generate_agent_api_key();
    let key_hash = hash_api_key(&raw_key);
    let key_prefix = raw_key.chars().take(13).collect::<String>();
    let key_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO otlp_api_keys
         (id, org_id, name, key_hash, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, enabled, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, ARRAY['write'], 1000, 5242880, true, NOW(), NOW())",
    )
    .bind(key_id)
    .bind(ctx.org_id)
    .bind(name)
    .bind(&key_hash)
    .bind(&key_prefix)
    .execute(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "id": key_id,
        "name": name,
        "api_key": raw_key,
        "key_prefix": key_prefix,
        "warning": "Store this key securely — it will not be shown again.",
    })))
}

pub async fn revoke_agent_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(key_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let key_hash: Option<String> = sqlx::query_scalar(
        "UPDATE otlp_api_keys SET enabled = false, updated_at = NOW()
         WHERE id = $1 AND org_id = $2
         RETURNING key_hash",
    )
    .bind(key_id)
    .bind(ctx.org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(hash) = key_hash {
        state.otlp_key_cache.remove(&hash);
    }

    Ok(Json(json!({ "revoked": true, "id": key_id })))
}

// ============================================================================
// Discovered Hosts (Feature 4 — Agent network scan + user opt-in)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct DiscoveredHostEntry {
    pub ip_address: String,
    pub hostname: Option<String>,
    pub open_ports: Option<Vec<u16>>,
    pub os_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReportDiscoveredRequest {
    pub hosts: Vec<DiscoveredHostEntry>,
}

#[derive(Debug, Deserialize)]
pub struct OptInHostRequest {
    pub opted_in: Option<bool>,
    pub ignored: Option<bool>,
}

/// Called by the agent to report newly discovered hosts in its network.
/// POST /api/agents/:agent_id/discovered  (agent auth: Authorization: Bearer)
pub async fn report_discovered(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<ReportDiscoveredRequest>,
) -> Result<Json<Value>, ApiError> {
    let raw_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let (org_id, _) = validate_otlp_key(&state, &raw_key)
        .await
        .map_err(ApiError::Unauthorized)?;

    // Verify the agent_id belongs to this org
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_fleet WHERE agent_id = $1 AND org_id = $2)"
    )
    .bind(agent_id)
    .bind(org_id)
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if !exists {
        return Err(ApiError::Forbidden("Agent not found for this org".into()));
    }

    let mut upserted = 0usize;
    for host in &body.hosts {
        let ip = host.ip_address.trim();
        if ip.is_empty() {
            continue;
        }
        let ports = serde_json::to_value(host.open_ports.as_deref().unwrap_or(&[]))
            .unwrap_or(serde_json::Value::Array(vec![]));

        sqlx::query(
            "INSERT INTO discovered_hosts
             (agent_id, org_id, ip_address, hostname, open_ports, os_hint, first_seen, last_seen)
             VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
             ON CONFLICT (agent_id, ip_address) DO UPDATE
             SET hostname  = COALESCE(EXCLUDED.hostname, discovered_hosts.hostname),
                 open_ports = EXCLUDED.open_ports,
                 os_hint   = COALESCE(EXCLUDED.os_hint, discovered_hosts.os_hint),
                 last_seen  = NOW()"
        )
        .bind(agent_id)
        .bind(org_id)
        .bind(ip)
        .bind(host.hostname.as_deref())
        .bind(&ports)
        .bind(host.os_hint.as_deref())
        .execute(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        upserted += 1;
    }

    Ok(Json(json!({ "upserted": upserted })))
}

/// Returns discovered hosts for an agent (user-facing).
/// GET /api/agents/:agent_id/discovered  (user session auth)
pub async fn list_discovered(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let rows = sqlx::query(
        "SELECT id, ip_address, hostname, open_ports, os_hint, first_seen, last_seen,
                opted_in, ignored, target_id
         FROM discovered_hosts
         WHERE agent_id = $1 AND org_id = $2
         ORDER BY opted_in DESC, last_seen DESC"
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let hosts: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id":          r.get::<Uuid, _>("id"),
            "ip_address":  r.get::<String, _>("ip_address"),
            "hostname":    r.get::<Option<String>, _>("hostname"),
            "open_ports":  r.get::<serde_json::Value, _>("open_ports"),
            "os_hint":     r.get::<Option<String>, _>("os_hint"),
            "first_seen":  r.get::<DateTime<Utc>, _>("first_seen"),
            "last_seen":   r.get::<DateTime<Utc>, _>("last_seen"),
            "opted_in":    r.get::<bool, _>("opted_in"),
            "ignored":     r.get::<bool, _>("ignored"),
            "target_id":   r.get::<Option<Uuid>, _>("target_id"),
        })
    }).collect();

    Ok(Json(json!(hosts)))
}

/// User opts in or ignores a discovered host.
/// PATCH /api/agents/:agent_id/discovered/:host_id
pub async fn update_discovered_host(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path((agent_id, host_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<OptInHostRequest>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    // Build update dynamically — only set fields that were provided
    let opted_in = body.opted_in;
    let ignored = body.ignored;

    let updated: bool = sqlx::query_scalar(
        "UPDATE discovered_hosts
         SET opted_in = COALESCE($3, opted_in),
             ignored  = COALESCE($4, ignored)
         WHERE id = $1 AND agent_id = $2
           AND org_id = (SELECT org_id FROM agent_fleet WHERE agent_id = $2 AND org_id = $5 LIMIT 1)
         RETURNING TRUE"
    )
    .bind(host_id)
    .bind(agent_id)
    .bind(opted_in)
    .bind(ignored)
    .bind(ctx.org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .unwrap_or(false);

    if !updated {
        return Err(ApiError::NotFound("Discovered host not found".into()));
    }

    Ok(Json(json!({ "updated": true, "id": host_id })))
}

// ============================================================================
// Local Component Discovery (Feature 7 — agent self-inspection)
// Agent reports which services are running on its own host; user confirms
// selection → targets are auto-created with the agent_id set so no SSH needed.
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ReportComponentEntry {
    pub component_type: String,  // "linux","postgres","mysql","redis","nginx","apache","mongodb","oracle"
    pub component_name: String,
    pub address: String,
    pub port: Option<i32>,
    pub version: Option<String>,
    #[serde(default)]
    pub extra: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ReportComponentsRequest {
    pub components: Vec<ReportComponentEntry>,
}

/// POST /api/agents/by-dc/:dc_name/components — called by the agent to report
/// locally-detected services. Uses Bearer auth (same as heartbeat).
pub async fn report_components(
    State(state): State<AppState>,
    Path(dc_name): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ReportComponentsRequest>,
) -> Result<Json<Value>, ApiError> {
    let raw_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let (org_id, _) = validate_otlp_key(&state, &raw_key)
        .await
        .map_err(ApiError::Unauthorized)?;

    // Resolve the agent_id for this dc_name + org
    let agent_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT agent_id FROM agent_fleet WHERE org_id = $1 AND datacenter_name = $2 AND status = 'active' LIMIT 1"
    )
    .bind(org_id)
    .bind(dc_name.trim())
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let agent_id = agent_id.ok_or_else(|| ApiError::NotFound("No active agent for this datacenter".into()))?;

    let mut upserted = 0usize;
    for comp in &body.components {
        let extra = if comp.extra.is_null() { serde_json::Value::Object(Default::default()) } else { comp.extra.clone() };
        sqlx::query(
            "INSERT INTO discovered_components
             (agent_id, org_id, component_type, component_name, address, port, version, extra, first_seen, last_seen)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
             ON CONFLICT (agent_id, component_type, port) DO UPDATE
             SET component_name = EXCLUDED.component_name,
                 version        = COALESCE(EXCLUDED.version, discovered_components.version),
                 extra          = EXCLUDED.extra,
                 last_seen      = NOW()"
        )
        .bind(agent_id)
        .bind(org_id)
        .bind(&comp.component_type)
        .bind(&comp.component_name)
        .bind(&comp.address)
        .bind(comp.port)
        .bind(comp.version.as_deref())
        .bind(&extra)
        .execute(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        upserted += 1;
    }

    Ok(Json(json!({ "upserted": upserted, "agent_id": agent_id })))
}

/// GET /api/agents/by-dc/:dc_name/components — user-facing; lists discovered
/// components for the given datacenter. Used by the Install Wizard Step 4.
pub async fn list_components(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(dc_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    let rows = sqlx::query(
        "SELECT dc.id, dc.component_type, dc.component_name, dc.address, dc.port,
                dc.version, dc.extra, dc.confirmed, dc.ignored, dc.target_id,
                dc.first_seen, dc.last_seen,
                af.agent_id, af.status AS agent_status
         FROM discovered_components dc
         JOIN agent_fleet af ON af.agent_id = dc.agent_id
         WHERE af.org_id = $1 AND af.datacenter_name = $2 AND dc.ignored = FALSE
         ORDER BY dc.confirmed ASC, dc.last_seen DESC"
    )
    .bind(ctx.org_id)
    .bind(dc_name.trim())
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let components: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id":             r.get::<Uuid, _>("id"),
            "component_type": r.get::<String, _>("component_type"),
            "component_name": r.get::<String, _>("component_name"),
            "address":        r.get::<String, _>("address"),
            "port":           r.get::<Option<i32>, _>("port"),
            "version":        r.get::<Option<String>, _>("version"),
            "extra":          r.get::<serde_json::Value, _>("extra"),
            "confirmed":      r.get::<bool, _>("confirmed"),
            "target_id":      r.get::<Option<Uuid>, _>("target_id"),
            "first_seen":     r.get::<DateTime<Utc>, _>("first_seen"),
            "last_seen":      r.get::<DateTime<Utc>, _>("last_seen"),
            "agent_id":       r.get::<Uuid, _>("agent_id"),
            "agent_status":   r.get::<String, _>("agent_status"),
        })
    }).collect();

    Ok(Json(json!({
        "components": components,
        "datacenter": dc_name.trim(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmComponentsRequest {
    /// IDs of discovered_components rows the user selected
    pub component_ids: Vec<Uuid>,
    /// The datacenter name (used to set agent_id on created targets)
    pub datacenter_name: String,
}

/// POST /api/agents/components/confirm — user selects components to monitor;
/// we auto-create targets for each one and mark them confirmed.
pub async fn confirm_components(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(body): Json<ConfirmComponentsRequest>,
) -> Result<Json<Value>, ApiError> {
    require_agent_admin(&ctx)?;

    if body.component_ids.is_empty() {
        return Err(ApiError::BadRequest("component_ids must not be empty".into()));
    }

    // Fetch the agent_id for this datacenter
    let agent_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT agent_id FROM agent_fleet WHERE org_id = $1 AND datacenter_name = $2 AND status = 'active' LIMIT 1"
    )
    .bind(ctx.org_id)
    .bind(body.datacenter_name.trim())
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let agent_id = agent_id.ok_or_else(|| ApiError::NotFound("No active agent for this datacenter".into()))?;

    // Fetch the selected components that belong to this org and aren't already confirmed
    let rows = sqlx::query(
        "SELECT dc.id, dc.component_type, dc.component_name, dc.address, dc.port, dc.version
         FROM discovered_components dc
         JOIN agent_fleet af ON af.agent_id = dc.agent_id
         WHERE dc.id = ANY($1) AND af.org_id = $2 AND dc.confirmed = FALSE"
    )
    .bind(&body.component_ids)
    .bind(ctx.org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut created_targets: Vec<Value> = Vec::new();

    for row in &rows {
        let comp_id: Uuid = row.get("id");
        let comp_type: String = row.get("component_type");
        let comp_name: String = row.get("component_name");
        let address: String = row.get("address");
        let port: Option<i32> = row.get("port");
        let version: Option<String> = row.get("version");

        // Map component_type → target_subtype string
        let subtype = match comp_type.as_str() {
            "linux" | "server"  => "linux",
            "windows"           => "windows",
            "postgres"          => "postgres",
            "mysql"             => "mysql",
            "oracle"            => "oracle",
            "redis"             => "redis",
            "mongodb"           => "mongodb",
            "nginx" | "apache" | "haproxy" | "tomcat" | "phpfpm" => "linux",
            _                   => "linux",
        };

        let display_name = if let Some(v) = &version {
            format!("{} ({})", comp_name, v)
        } else {
            comp_name.clone()
        };
        let default_port: Option<u16> = port.map(|p| p as u16);

        let target_id = Uuid::now_v7();
        let now = Utc::now();

        // Insert a minimal target — no SSH credentials needed; agent_id is set
        // so the remote agent will collect metrics directly.
        let insert_ok = sqlx::query(
            "INSERT INTO targets
             (target_id, org_id, name, target_type, target_subtype, host, port,
              agent_id, status, consecutive_failures, created_at, updated_at)
             VALUES ($1, $2, $3, 'server', $4, $5, $6, $7, 'active', 0, $8, $8)
             ON CONFLICT DO NOTHING"
        )
        .bind(target_id)
        .bind(ctx.org_id)
        .bind(&display_name)
        .bind(subtype)
        .bind(&address)
        .bind(default_port.map(|p| p as i32))
        .bind(agent_id)
        .bind(now)
        .execute(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        if insert_ok.rows_affected() > 0 {
            // Mark component as confirmed and link target_id
            sqlx::query(
                "UPDATE discovered_components SET confirmed = TRUE, target_id = $2 WHERE id = $1"
            )
            .bind(comp_id)
            .bind(target_id)
            .execute(&state.tsdb_pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            created_targets.push(json!({
                "target_id":    target_id,
                "name":         display_name,
                "host":         address,
                "subtype":      subtype,
                "component_id": comp_id,
            }));
        }
    }

    Ok(Json(json!({
        "created": created_targets.len(),
        "targets": created_targets,
    })))
}