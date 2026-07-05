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
use crate::state::AppState;

const AGENT_KEY_HEADER: &str = "x-zentra-key";
const AGENT_LOG_BUFFER_CAP: usize = 500;

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub datacenter_name: Option<String>,
    pub hostname: Option<String>,
    pub ip_address: Option<String>,
    pub os: Option<String>,
    pub agent_version: Option<String>,
    pub description: Option<String>,
    pub region_label: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentKeyIdentity {
    org_id: Uuid,
    agent_id: Uuid,
    key_id: Uuid,
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
    /// Agent-reported IP — preferred over remote_addr which is 127.0.0.1 behind nginx
    pub ip_address: Option<String>,
    /// Agent-reported hostname
    pub hostname: Option<String>,
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
    key_expires_at: Option<DateTime<Utc>>,
    key_enabled: Option<bool>,
    registered_ip: Option<String>,
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
    format!("isak_{suffix}")
}

fn generate_org_enrollment_key() -> String {
    let mut rng = rand::thread_rng();
    let suffix = (0..40)
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

fn split_host_port(address: &str) -> (String, Option<i32>) {
    let trimmed = address.trim();
    if let Some(host) = trimmed.strip_prefix('[').and_then(|rest| rest.split_once(']').map(|(host, _)| host)) {
        return (host.to_string(), None);
    }
    match trimmed.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            let parsed_port = port.parse::<i32>().ok();
            (host.to_string(), parsed_port)
        }
        _ => (trimmed.to_string(), None),
    }
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

fn extract_bearer(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))
}

fn extract_request_ip(headers: &HeaderMap, remote: Option<&SocketAddr>) -> Option<String> {
    headers
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            headers
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| remote.map(|r| r.ip().to_string()))
}

async fn validate_agent_key(
    state: &AppState,
    headers: &HeaderMap,
    remote: Option<&SocketAddr>,
) -> Result<AgentKeyIdentity, ApiError> {
    let raw_key = extract_bearer(headers)
        .or_else(|_| extract_agent_key(headers))?;
    if !raw_key.starts_with("isak_") {
        return Err(ApiError::Forbidden("Agent operational key (isak_) required".into()));
    }

    let key_hash = hash_api_key(&raw_key);
    let request_ip = extract_request_ip(headers, remote);

    let row = sqlx::query(
        "SELECT id, org_id, agent_id, ip_locked, registered_ip::text AS registered_ip
         FROM agent_api_keys
         WHERE key_hash = $1
           AND enabled = true
           AND (expires_at > NOW() OR (grace_until IS NOT NULL AND grace_until > NOW()))
           AND revoked_at IS NULL
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(&key_hash)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::Unauthorized("Invalid, revoked, or expired agent key".into()))?;

    let key_id: Uuid = row.get("id");
    let org_id: Uuid = row.get("org_id");
    let agent_id: Uuid = row.get("agent_id");
    let ip_locked: bool = row.get("ip_locked");
    let registered_ip: Option<String> = row.get("registered_ip");

    if ip_locked {
        if let (Some(expected), Some(actual)) = (registered_ip.as_deref(), request_ip.as_deref()) {
            if expected != actual {
                return Err(ApiError::Forbidden("ip mismatch".into()));
            }
        }
    }

    let pool = state.tsdb_pool.clone();
    let update_key_id = key_id;
    tokio::spawn(async move {
        let _ = sqlx::query("UPDATE agent_api_keys SET last_used = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(update_key_id)
            .execute(&pool)
            .await;
    });

    Ok(AgentKeyIdentity {
        org_id,
        agent_id,
        key_id,
    })
}

pub async fn register_agent(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    maybe_ctx: Option<Extension<AuthContext>>,
    Json(body): Json<RegisterAgentRequest>,
) -> Result<Json<Value>, ApiError> {
    let bearer = extract_bearer(&headers).ok();

    if let Some(raw_org_key) = bearer.filter(|k| k.starts_with("isok_")) {
        let request_ip = body
            .ip_address
            .clone()
            .or_else(|| extract_request_ip(&headers, Some(&remote_addr)))
            .ok_or_else(|| ApiError::BadRequest("ip_address is required".into()))?;

        let datacenter_name = body
            .datacenter_name
            .as_deref()
            .or(body.hostname.as_deref())
            .unwrap_or("agent")
            .trim()
            .to_string();
        if datacenter_name.is_empty() {
            return Err(ApiError::BadRequest("datacenter_name or hostname is required".into()));
        }
        let hostname = body.hostname.clone().unwrap_or_else(|| datacenter_name.clone());

        let org_key_hash = hash_api_key(&raw_org_key);
        let row = sqlx::query(
            "SELECT id, org_id, max_registrations_per_hour, allowed_cidr::text AS allowed_cidr, require_approval
             FROM org_enrollment_keys
             WHERE key_hash = $1
               AND enabled = true
               AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(&org_key_hash)
        .fetch_optional(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::Unauthorized("Invalid or expired org enrollment key".into()))?;

        let enrollment_key_id: Uuid = row.get("id");
        let org_id: Uuid = row.get("org_id");
        let max_registrations_per_hour: i32 = row.get("max_registrations_per_hour");
        let allowed_cidr: Option<String> = row.get("allowed_cidr");
        let require_approval: bool = row.get("require_approval");

        if let Some(cidr) = allowed_cidr.as_deref() {
            let ip_allowed: bool = sqlx::query_scalar("SELECT $1::inet <<= $2::cidr")
                .bind(&request_ip)
                .bind(cidr)
                .fetch_one(&state.tsdb_pool)
                .await
                .unwrap_or(false);
            if !ip_allowed {
                return Err(ApiError::Forbidden("Registration request IP not in allowed CIDR".into()));
            }
        }

        let recent_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_registration_audit
             WHERE enrollment_key_id = $1
               AND created_at >= NOW() - INTERVAL '1 hour'
               AND status IN ('registered', 'pending_approval')",
        )
        .bind(enrollment_key_id)
        .fetch_one(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        if recent_count >= max_registrations_per_hour as i64 {
            return Err(ApiError::TooManyRequests("Registration rate limit exceeded for this enrollment key".into()));
        }

        let raw_agent_key = generate_agent_api_key();
        let agent_key_hash = hash_api_key(&raw_agent_key);
        let agent_key_prefix = raw_agent_key.chars().take(18).collect::<String>();
        let agent_id = Uuid::now_v7();
        let status = if require_approval { "pending_approval" } else { "active" };

        let mut tx = state
            .tsdb_pool
            .begin()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let otlp_api_key_id: Uuid = sqlx::query_scalar(
            "INSERT INTO otlp_api_keys
             (org_id, name, key_hash, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, enabled, created_at, updated_at)
             VALUES ($1, $2, $3, $4, ARRAY['metrics','logs'], 10000, 5242880, $5, NOW(), NOW())
             RETURNING id",
        )
        .bind(org_id)
        .bind(format!("agent:{datacenter_name}"))
        .bind(&agent_key_hash)
        .bind(&agent_key_prefix)
        .bind(!require_approval)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO agent_fleet
             (agent_id, org_id, otlp_api_key_id, api_key_hash, datacenter_name, hostname,
              description, region_label, agent_version, ip_address, os_name, status,
              enrollment_key_id, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), NOW())",
        )
        .bind(agent_id)
        .bind(org_id)
        .bind(otlp_api_key_id)
        .bind(&agent_key_hash)
        .bind(&datacenter_name)
        .bind(&hostname)
        .bind(body.description.as_deref())
        .bind(body.region_label.as_deref())
        .bind(body.agent_version.as_deref())
        .bind(&request_ip)
        .bind(body.os.as_deref())
        .bind(status)
        .bind(enrollment_key_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO agent_api_keys
             (org_id, agent_id, key_hash, display_prefix, enabled, registered_ip, ip_locked,
              expires_at, last_rotated_at, enrollment_key_id, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6::inet, true, NOW() + INTERVAL '1 year', NOW(), $7, NOW(), NOW())",
        )
        .bind(org_id)
        .bind(agent_id)
        .bind(&agent_key_hash)
        .bind(&agent_key_prefix)
        .bind(!require_approval)
        .bind(&request_ip)
        .bind(enrollment_key_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        sqlx::query(
            "UPDATE org_enrollment_keys
             SET last_used_at = NOW(), registration_count = registration_count + 1
             WHERE id = $1",
        )
        .bind(enrollment_key_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO agent_registration_audit
             (org_id, enrollment_key_id, agent_id, request_ip, hostname, status, payload)
             VALUES ($1, $2, $3, $4::inet, $5, $6, $7::jsonb)",
        )
        .bind(org_id)
        .bind(enrollment_key_id)
        .bind(agent_id)
        .bind(&request_ip)
        .bind(&hostname)
        .bind(if require_approval { "pending_approval" } else { "registered" })
        .bind(json!({
            "datacenter_name": datacenter_name,
            "agent_version": body.agent_version,
            "os": body.os,
        }))
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        if require_approval {
            return Ok(Json(json!({
                "agent_id": agent_id,
                "status": "pending_approval",
            })));
        }

        return Ok(Json(json!({
            "agent_id": agent_id,
            "agent_key": raw_agent_key,
            "expires_at": Utc::now() + chrono::Duration::days(365),
        })));
    }

    let ctx = maybe_ctx
        .map(|e| e.0)
        .ok_or_else(|| ApiError::Unauthorized("Missing authentication context".into()))?;
    require_agent_admin(&ctx)?;

    let datacenter_name = body
        .datacenter_name
        .as_deref()
        .unwrap_or("")
        .trim();
    if datacenter_name.is_empty() {
        return Err(ApiError::BadRequest("datacenter_name is required".into()));
    }

    let raw_key = generate_org_enrollment_key();
    let key_hash = hash_api_key(&raw_key);
    let key_prefix = raw_key.chars().take(13).collect::<String>();
    let key_id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO org_enrollment_keys
         (id, org_id, key_hash, display_prefix, enabled, max_registrations_per_hour, created_by, created_at)
         VALUES ($1, $2, $3, $4, true, 5, $5, NOW())",
    )
    .bind(key_id)
    .bind(ctx.org_id)
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(ctx.user_id)
    .execute(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "id": key_id,
        "org_key": raw_key,
        "warning": "Store this key securely. It will not be shown again.",
    })))
}

pub async fn heartbeat(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<AgentHeartbeatRequest>,
) -> Result<Json<Value>, ApiError> {
    let auth = validate_agent_key(&state, &headers, Some(&remote_addr)).await?;
    let org_id = auth.org_id;

    // Use X-Real-IP (set by nginx to the real client IP) as the authoritative source.
    // The agent-reported IP is unreliable: UDP routing picks whichever interface the
    // OS happens to route through, which may be an internal/bridge IP (e.g. 172.16.x.x).
    // X-Real-IP reflects the actual TCP connection source as seen by the reverse proxy.
    let effective_ip = headers
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && *s != "127.0.0.1")
        .map(|s| s.to_string())
        .or_else(|| {
            // Fallback: first IP in X-Forwarded-For
            headers
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != "127.0.0.1")
        })
        .unwrap_or_else(|| remote_addr.ip().to_string());

    sqlx::query(
        "UPDATE agent_fleet
         SET datacenter_name = $3,
             agent_version = $4,
             active_target_count = $5,
             last_seen = NOW(),
             ip_address = $6,
             os_name = $7,
             hostname = COALESCE($8, hostname),
             status = CASE WHEN status = 'deregistered' THEN status ELSE 'active' END,
             updated_at = NOW()
         WHERE org_id = $1 AND agent_id = $2",
    )
    .bind(org_id)
    .bind(auth.agent_id)
    .bind(body.dc_name.trim())
    .bind(body.agent_version.trim())
    .bind(body.active_targets)
    .bind(&effective_ip)
    .bind(body.os_name.trim())
    .bind(body.hostname.as_deref())
    .execute(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // On every heartbeat, refresh ALL discovered_components addresses for this agent
    // that are still set to 127.0.0.1 (stale from initial discovery before real IP was known).
    // Also sync targets.host so monitoring uses the correct IP for every component type.
    if effective_ip != "127.0.0.1" {
        let aid = auth.agent_id;
            // Update component addresses: replace 127.0.0.1 (bare or with port) with real IP.
            // For components with a port (e.g. "127.0.0.1:80"), preserve the port suffix.
            let _ = sqlx::query(
                "UPDATE discovered_components
                 SET address = CASE
                   WHEN address = '127.0.0.1' THEN $1
                   WHEN address LIKE '127.0.0.1:%' THEN $1 || substr(address, 10)
                   ELSE address
                 END
                 WHERE agent_id = $2
                   AND (address = '127.0.0.1' OR address LIKE '127.0.0.1:%')"
            )
            .bind(&effective_ip)
            .bind(aid)
            .execute(&state.tsdb_pool)
            .await;

            // Also update the linux host component unconditionally to real IP
            let _ = sqlx::query(
                "UPDATE discovered_components
                 SET address = $1
                 WHERE agent_id = $2 AND component_type = 'linux'"
            )
            .bind(&effective_ip)
            .bind(aid)
            .execute(&state.tsdb_pool)
            .await;

            // Sync targets: fix bare 127.0.0.1 hosts
            let _ = sqlx::query(
                "UPDATE targets SET host = $1
                 WHERE agent_id = $2 AND host = '127.0.0.1'"
            )
            .bind(&effective_ip)
            .bind(aid)
            .execute(&state.tsdb_pool)
            .await;

            // Sync targets: fix host:port style (e.g. "127.0.0.1:80" → "real_ip:80")
            let _ = sqlx::query(
                "UPDATE targets SET host = $1 || substr(host, 10)
                 WHERE agent_id = $2 AND host LIKE '127.0.0.1:%'"
            )
            .bind(&effective_ip)
            .bind(aid)
            .execute(&state.tsdb_pool)
            .await;
    }

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
        "agent_id": auth.agent_id,
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
    let auth = validate_agent_key(&state, &headers, None).await?;
    let org_id = auth.org_id;
    let agent_id = auth.agent_id;

    let targets = zentra_db::repositories::targets::list_targets(&state.pool, org_id, None, true)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .into_iter()
        .filter(|target| target.agent_id == Some(agent_id))
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
        "SELECT af.agent_id, af.datacenter_name, af.status, af.last_seen, af.agent_version,
            af.active_target_count, af.ip_address, af.region_label,
            ak.expires_at AS key_expires_at, ak.enabled AS key_enabled,
            ak.registered_ip::text AS registered_ip
         FROM agent_fleet af
         LEFT JOIN LATERAL (
            SELECT expires_at, enabled, registered_ip
            FROM agent_api_keys
            WHERE agent_id = af.agent_id
            ORDER BY created_at DESC
            LIMIT 1
         ) ak ON TRUE
         WHERE af.org_id = $1
         ORDER BY datacenter_name ASC",
    )
    .bind(ctx.org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let agents = rows
        .into_iter()
        .map(|row| {
            let key_status = match (row.key_enabled.unwrap_or(false), row.key_expires_at) {
                (false, _) => "revoked",
                (true, Some(expires_at)) if expires_at <= Utc::now() => "expired",
                (true, Some(expires_at)) if expires_at <= Utc::now() + chrono::Duration::days(30) => "expiring_soon",
                (true, Some(_)) => "active",
                (true, None) => "active",
            };
            json!({
                "agent_id": row.agent_id,
                "datacenter_name": row.datacenter_name,
                "status": row.status,
                "last_seen": row.last_seen,
                "agent_version": row.agent_version,
                "active_target_count": row.active_target_count,
                "ip_address": row.ip_address,
                "region_label": row.region_label,
                "key_status": key_status,
                "key_expires_at": row.key_expires_at,
                "registered_ip": row.registered_ip,
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

    // Hard delete: remove the agent, its discovered components, its targets,
    // and any metrics tied to those targets so the datacenter truly disappears.
    sqlx::query(
        "DELETE FROM metrics
         WHERE target_id IN (
             SELECT target_id FROM targets WHERE agent_id = $1 AND org_id = $2
         )"
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "DELETE FROM targets
         WHERE agent_id = $1 AND org_id = $2"
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "DELETE FROM discovered_components
         WHERE agent_id = $1"
    )
    .bind(agent_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "DELETE FROM agent_fleet
         WHERE agent_id = $1 AND org_id = $2"
    )
    .bind(agent_id)
    .bind(ctx.org_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "DELETE FROM otlp_api_keys
         WHERE id = $1 AND org_id = $2"
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
        "deleted": true,
        "agent_id": agent_id,
    })))
}

pub async fn rotate_agent_key(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let auth = validate_agent_key(&state, &headers, None).await?;
    if auth.agent_id != agent_id {
        return Err(ApiError::Forbidden("Agent key cannot rotate another agent".into()));
    }

    let raw_new_key = generate_agent_api_key();
    let new_hash = hash_api_key(&raw_new_key);
    let new_prefix = raw_new_key.chars().take(18).collect::<String>();

    let mut tx = state
        .tsdb_pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let old_row = sqlx::query(
        "SELECT key_hash, registered_ip::text AS registered_ip, enrollment_key_id
         FROM agent_api_keys
         WHERE id = $1 AND org_id = $2",
    )
    .bind(auth.key_id)
    .bind(auth.org_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::Unauthorized("Current agent key not found".into()))?;

    let old_hash: String = old_row.get("key_hash");
    let registered_ip: Option<String> = old_row.get("registered_ip");
    let enrollment_key_id: Option<Uuid> = old_row.get("enrollment_key_id");

    let new_key_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_api_keys
         (org_id, agent_id, key_hash, display_prefix, enabled, registered_ip, ip_locked,
          expires_at, last_rotated_at, enrollment_key_id, created_at, updated_at)
         VALUES ($1, $2, $3, $4, true, $5::inet, true, NOW() + INTERVAL '1 year', NOW(), $6, NOW(), NOW())
         RETURNING id",
    )
    .bind(auth.org_id)
    .bind(agent_id)
    .bind(&new_hash)
    .bind(&new_prefix)
    .bind(registered_ip.as_deref())
    .bind(enrollment_key_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "UPDATE agent_api_keys
         SET replaced_by_key_id = $2,
             grace_until = NOW() + INTERVAL '7 days',
             last_rotated_at = NOW(),
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(auth.key_id)
    .bind(new_key_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let new_otlp_key_id: Uuid = sqlx::query_scalar(
        "INSERT INTO otlp_api_keys
         (org_id, name, key_hash, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, enabled, created_at, updated_at)
         VALUES ($1, $2, $3, $4, ARRAY['metrics','logs'], 10000, 5242880, true, NOW(), NOW())
         RETURNING id",
    )
    .bind(auth.org_id)
    .bind(format!("agent:{}", agent_id))
    .bind(&new_hash)
    .bind(&new_prefix)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "UPDATE otlp_api_keys
         SET expires_at = NOW() + INTERVAL '7 days', updated_at = NOW()
         WHERE org_id = $1 AND key_hash = $2",
    )
    .bind(auth.org_id)
    .bind(&old_hash)
    .execute(&mut *tx)
    .await
    .ok();

    sqlx::query(
        "UPDATE agent_fleet
         SET api_key_hash = $3,
             otlp_api_key_id = $4,
             updated_at = NOW()
         WHERE org_id = $1 AND agent_id = $2",
    )
    .bind(auth.org_id)
    .bind(agent_id)
    .bind(&new_hash)
    .bind(new_otlp_key_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "agent_id": agent_id,
        "agent_key": raw_new_key,
        "expires_at": Utc::now() + chrono::Duration::days(365),
        "old_key_grace_until": Utc::now() + chrono::Duration::days(7),
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
    let auth = validate_agent_key(&state, &headers, None).await?;
    let org_id = auth.org_id;
    if auth.agent_id != agent_id {
        return Err(ApiError::Forbidden("Agent key cannot report for another agent_id".into()));
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
// Agent binary download — GET /dl/:filename
// Serves pre-built agent binaries from ZENTRA_DL_DIR (default: /store/Zentra/bin/dl).
// Nginx proxies /dl/ to this handler so the SPA fallback is never served.
// ============================================================================

pub async fn download_agent(
    Path(filename): Path<String>,
) -> impl axum::response::IntoResponse {
    use axum::http::{header, StatusCode};
    use axum::response::Response;
    use axum::body::Body;

    // Reject path traversal attempts
    if filename.contains('/') || filename.contains("..") || filename.starts_with('.') {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid filename"))
            .unwrap();
    }

    // Only allow known agent binary names
    let allowed = [
        "zentra-agent-linux-x86_64",
        "zentra-agent-linux-aarch64",
        "zentra-agent-windows-x86_64.exe",
        "zentra-collector-linux-x86_64",
        "zentra-collector-linux-aarch64",
    ];
    if !allowed.contains(&filename.as_str()) {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Agent binary not found"))
            .unwrap();
    }

    let dl_dir = std::env::var("ZENTRA_DL_DIR")
        .unwrap_or_else(|_| "/store/Zentra/bin/dl".to_string());
    let path = std::path::Path::new(&dl_dir).join(&filename);

    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let content_type = if filename.ends_with(".exe") {
                "application/vnd.microsoft.portable-executable"
            } else {
                "application/octet-stream"
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .header(header::CONTENT_LENGTH, bytes.len().to_string())
                .header("cache-control", "no-store")
                .body(Body::from(bytes))
                .unwrap()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!(file = %path.display(), "Agent binary not found on disk");
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!(
                    "Agent binary '{}' is not available on this server yet. \
                     Place it at {} and try again.",
                    filename,
                    path.display()
                )))
                .unwrap()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to read agent binary");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to read binary"))
                .unwrap()
        }
    }
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
    let auth = validate_agent_key(&state, &headers, None).await?;
    let org_id = auth.org_id;
    let agent_id = auth.agent_id;

    // Update datacenter name from agent report to keep Fleet labels current.
    let _ = sqlx::query(
        "UPDATE agent_fleet SET datacenter_name = $3, updated_at = NOW() WHERE org_id = $1 AND agent_id = $2"
    )
    .bind(org_id)
    .bind(agent_id)
    .bind(dc_name.trim())
    .execute(&state.tsdb_pool)
    .await;

    let mut upserted = 0usize;
    for comp in &body.components {
        let extra = if comp.extra.is_null() { serde_json::Value::Object(Default::default()) } else { comp.extra.clone() };
        sqlx::query(
            "INSERT INTO discovered_components
             (agent_id, org_id, component_type, component_name, address, port, version, extra, first_seen, last_seen)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
             ON CONFLICT (agent_id, component_type, COALESCE(port, -1)) DO UPDATE
             SET component_name = EXCLUDED.component_name,
                 address        = EXCLUDED.address,
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
         WHERE dc.id = ANY($1) AND af.org_id = $2"
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

        // network_interface and log_directory are metadata only — no target row needed.
        if comp_type == "network_interface" || comp_type == "log_directory" {
            sqlx::query("UPDATE discovered_components SET confirmed = TRUE WHERE id = $1")
                .bind(comp_id)
                .execute(&state.tsdb_pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
            created_targets.push(json!({
                "component_id": comp_id,
                "name":         comp_name,
                "type":         comp_type,
                "acknowledged": true,
            }));
            continue;
        }

        // Map component_type → (target_type, target_subtype, default_port)
        // target_type MUST match what each UI section queries:
        //   "server"     → Servers page, load_middleware_from_db skips these
        //   "middleware"  → Middleware pages (nginx/apache/haproxy/tomcat/etc)
        //   "database"   → Database pages (oracle/redis/postgres/mysql)
        //   "storage"    → Storage pages
        let (ttype, subtype, fallback_port): (&str, &str, i32) = match comp_type.as_str() {
            "linux" | "server"  => ("server",     "linux",          22),
            "windows"           => ("server",     "windows",       3389),
            "oracle"            => ("database",   "oracle",        1521),
            "postgresql"        => ("database",   "postgresql",    5432),
            "mysql"             => ("database",   "mysql",         3306),
            "mariadb"           => ("database",   "mariadb",       3306),
            "mongodb"           => ("database",   "mongodb",      27017),
            "redis"             => ("database",   "redis",         6379),
            "nginx"             => ("middleware", "nginx",           80),
            "apache"            => ("middleware", "apache",          80),
            "haproxy"           => ("middleware", "haproxy",         80),
            "tomcat"            => ("middleware", "tomcat",        8080),
            "phpfpm"            => ("middleware", "phpfpm",        9000),
            "elasticsearch"     => ("middleware", "elasticsearch", 9200),
            "kafka"             => ("middleware", "kafka",         9092),
            "rabbitmq"          => ("middleware", "rabbitmq",      5672),
            "memcached"         => ("middleware", "memcached",    11211),
            _                   => ("server",     "linux",           22),
        };
        let (host, address_port) = split_host_port(&address);
        let effective_port = port.or(address_port).unwrap_or(fallback_port);

        let display_name = if let Some(v) = &version {
            format!("{} ({})", comp_name, v)
        } else {
            comp_name.clone()
        };

        let now = Utc::now();

        // Check if a target already exists for this agent + host + subtype.
        // Handles idempotent re-confirms (already-confirmed components).
        let existing_target_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT target_id FROM targets
             WHERE org_id = $1 AND agent_id = $2 AND host = $3 AND target_subtype = $4
             LIMIT 1"
        )
        .bind(ctx.org_id)
        .bind(agent_id)
        .bind(&host)
        .bind(subtype)
        .fetch_optional(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        let target_id = if let Some(existing) = existing_target_id {
            // Target already exists — just re-confirm the component
            existing
        } else {
            // Create new target row with agent_id set.
            // agent_id != NULL means the server NEVER tries SSH — metrics come via OTLP.
            let new_id = Uuid::now_v7();
            sqlx::query(
                "INSERT INTO targets
                 (target_id, org_id, name, target_type, target_subtype, host, port,
                  agent_id, status, consecutive_failures, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'active', 0, $9, $9)
                 ON CONFLICT (target_id) DO NOTHING"
            )
            .bind(new_id)        // $1 target_id
            .bind(ctx.org_id)    // $2 org_id
            .bind(&display_name) // $3 name
            .bind(ttype)         // $4 target_type
            .bind(subtype)       // $5 target_subtype
            .bind(&host)         // $6 host
            .bind(effective_port)// $7 port
            .bind(agent_id)      // $8 agent_id
            .bind(now)           // $9 created_at / updated_at
            .execute(&state.tsdb_pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
            new_id
        };

        sqlx::query(
            "UPDATE discovered_components SET confirmed = TRUE, target_id = $2 WHERE id = $1"
        )
        .bind(comp_id)
        .bind(target_id)
        .execute(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        // For middleware types, also upsert into middleware_instances so the middleware
        // UI pages (Nginx, Apache, HAProxy, Redis, etc.) show this instance automatically.
        let mw_type = match comp_type.as_str() {
            "nginx"         => Some("nginx"),
            "apache"        => Some("apache"),
            "haproxy"       => Some("haproxy"),
            "redis"         => Some("redis"),
            "memcached"     => Some("memcached"),
            "elasticsearch" => Some("elasticsearch"),
            "kafka"         => Some("kafka"),
            "rabbitmq"      => Some("rabbitmq"),
            "tomcat"        => Some("tomcat"),
            "phpfpm"        => Some("phpfpm"),
            _               => None,
        };
        if let Some(mw) = mw_type {
            let mw_id = format!("{}-{}", mw, &target_id.to_string()[..8]);
            let extra = serde_json::json!({
                "agent_id":  agent_id,
                "target_id": target_id,
                "stub_status_path": "/nginx_status",
            });
            let _ = sqlx::query(
                "INSERT INTO middleware_instances (id, instance_type, name, host, port, extra)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (id) DO UPDATE
                 SET name = EXCLUDED.name, host = EXCLUDED.host, port = EXCLUDED.port"
            )
            .bind(&mw_id)
            .bind(mw)
            .bind(&display_name)
            .bind(&host)
            .bind(effective_port)
            .bind(&extra)
            .execute(&state.pool)
            .await;
        }

        created_targets.push(json!({
            "target_id":    target_id,
            "name":         display_name,
            "host":         host,
            "port":         effective_port,
            "subtype":      subtype,
            "component_id": comp_id,
        }));
    }

    Ok(Json(json!({
        "created": created_targets.len(),
        "targets": created_targets,
    })))
}

/// GET /api/agents/log-dirs
/// Agent calls this to discover which log directories it should tail.
/// Returns confirmed log_directory components for the agent identified by its API key.
/// Auth: Bearer <agent api key> (isok_ prefix)
pub async fn agent_log_dirs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let auth = validate_agent_key(&state, &headers, None).await?;
    let org_id = auth.org_id;

    // The agent may pass its own agent_id as a query param for precise filtering.
    // If not provided, return all confirmed log dirs for the org.
    let agent_id_param: Option<Uuid> = headers
        .get("x-zentra-agent-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    if let Some(aid) = agent_id_param {
        if aid != auth.agent_id {
            return Err(ApiError::Forbidden("agent_id header does not match authenticated key".into()));
        }
    }

    let rows = if let Some(aid) = agent_id_param {
        sqlx::query(
            "SELECT id, component_name, extra
             FROM discovered_components
             WHERE agent_id = $1 AND org_id = $2
               AND component_type = 'log_directory'
               AND confirmed = TRUE AND ignored = FALSE
             ORDER BY first_seen ASC"
        )
        .bind(aid)
        .bind(org_id)
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        sqlx::query(
            "SELECT id, component_name, extra
             FROM discovered_components
             WHERE org_id = $1
               AND component_type = 'log_directory'
               AND confirmed = TRUE AND ignored = FALSE
             ORDER BY first_seen ASC"
        )
        .bind(org_id)
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    let dirs: Vec<Value> = rows.iter().map(|r| {
        let id: Uuid = r.get("id");
        let name: String = r.get("component_name");
        let extra: Value = r.get("extra");
        let path = extra.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        json!({ "id": id, "name": name, "path": path })
    }).collect();

    Ok(Json(json!({ "log_dirs": dirs })))
}
