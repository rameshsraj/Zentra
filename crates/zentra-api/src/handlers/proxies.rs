use axum::{extract::{State, Path}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::state::AppState;
use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// Models
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    pub id: String,
    pub name: String,
    pub address: String,
    pub api_key: String,
    pub status: String,
    pub version: Option<String>,
    pub last_heartbeat: Option<String>,
    pub monitored_hosts: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterProxyRequest {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProxyRequest {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub version: Option<String>,
    pub monitored_hosts: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ForwardDataRequest {
    pub metrics: Vec<Value>,
}

// ═══════════════════════════════════════════════════════════════════════════
// In-memory store
// ═══════════════════════════════════════════════════════════════════════════

fn proxies() -> &'static RwLock<Vec<Proxy>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<Proxy>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![
        Proxy {
            id: "proxy-dc1".into(), name: "DC1 Proxy".into(), address: "10.0.1.100:4443".into(),
            api_key: "px-dc1-a1b2c3d4e5f6".into(), status: "online".into(), version: Some("1.2.0".into()),
            last_heartbeat: Some(Utc::now().to_rfc3339()), monitored_hosts: 45,
            created_at: (Utc::now() - chrono::Duration::days(30)).to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        },
        Proxy {
            id: "proxy-dc2".into(), name: "DC2 Proxy".into(), address: "10.0.2.100:4443".into(),
            api_key: "px-dc2-f6e5d4c3b2a1".into(), status: "online".into(), version: Some("1.2.0".into()),
            last_heartbeat: Some((Utc::now() - chrono::Duration::minutes(2)).to_rfc3339()), monitored_hosts: 32,
            created_at: (Utc::now() - chrono::Duration::days(15)).to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        },
        Proxy {
            id: "proxy-remote".into(), name: "Remote Office".into(), address: "192.168.50.10:4443".into(),
            api_key: "px-rem-112233445566".into(), status: "offline".into(), version: Some("1.1.0".into()),
            last_heartbeat: Some((Utc::now() - chrono::Duration::hours(4)).to_rfc3339()), monitored_hosts: 8,
            created_at: (Utc::now() - chrono::Duration::days(7)).to_rfc3339(),
            updated_at: (Utc::now() - chrono::Duration::hours(4)).to_rfc3339(),
        },
    ]))
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

/// GET /api/proxies
pub async fn list_proxies(State(_s): State<AppState>) -> Json<Vec<Proxy>> {
    Json(proxies().read().clone())
}

/// POST /api/proxies
pub async fn register_proxy(
    State(_s): State<AppState>,
    Json(req): Json<RegisterProxyRequest>,
) -> Json<Proxy> {
    let now = Utc::now().to_rfc3339();
    let proxy = Proxy {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        address: req.address,
        api_key: format!("px-{}", Uuid::new_v4().to_string().replace('-', "")[..16].to_string()),
        status: "pending".into(),
        version: None,
        last_heartbeat: None,
        monitored_hosts: 0,
        created_at: now.clone(),
        updated_at: now,
    };
    proxies().write().push(proxy.clone());
    Json(proxy)
}

/// GET /api/proxies/:id
pub async fn get_proxy(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Proxy>, ApiError> {
    proxies().read().iter().find(|p| p.id == id).cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("Proxy not found".into()))
}

/// PUT /api/proxies/:id
pub async fn update_proxy(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProxyRequest>,
) -> Result<Json<Proxy>, ApiError> {
    let mut store = proxies().write();
    if let Some(p) = store.iter_mut().find(|p| p.id == id) {
        if let Some(n) = req.name { p.name = n; }
        if let Some(a) = req.address { p.address = a; }
        p.updated_at = Utc::now().to_rfc3339();
        Ok(Json(p.clone()))
    } else {
        Err(ApiError::NotFound("Proxy not found".into()))
    }
}

/// DELETE /api/proxies/:id
pub async fn delete_proxy(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    proxies().write().retain(|p| p.id != id);
    Json(json!({"deleted": id}))
}

/// POST /api/proxies/:id/heartbeat
pub async fn proxy_heartbeat(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut store = proxies().write();
    if let Some(p) = store.iter_mut().find(|p| p.id == id) {
        p.status = "online".into();
        p.last_heartbeat = Some(Utc::now().to_rfc3339());
        if let Some(v) = req.version { p.version = Some(v); }
        if let Some(h) = req.monitored_hosts { p.monitored_hosts = h; }
        p.updated_at = Utc::now().to_rfc3339();
        Ok(Json(json!({"status": "ack", "server_time": Utc::now().to_rfc3339()})))
    } else {
        Err(ApiError::NotFound("Proxy not found".into()))
    }
}

/// POST /api/proxies/:id/data
pub async fn proxy_forward_data(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ForwardDataRequest>,
) -> Json<Value> {
    // In production: write metrics to TSDB
    Json(json!({"proxy_id": id, "accepted": req.metrics.len()}))
}
