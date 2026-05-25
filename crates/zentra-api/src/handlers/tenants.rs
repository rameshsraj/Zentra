use axum::{
    extract::{Path, Query, State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

// ============================================================================
// Multi-Tenancy / MSP Mode — Tenant management, isolation, billing
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub slug: String,           // URL-safe identifier
    pub plan: String,           // free, starter, professional, enterprise
    pub status: String,         // active, suspended, trial, cancelled
    pub owner_email: String,
    pub created_at: String,
    pub trial_ends_at: Option<String>,
    pub data_isolation: String, // namespace, database, cluster
    pub max_targets: u32,
    pub max_users: u32,
    pub max_dashboards: u32,
    pub max_alerts: u32,
    pub retention_days: u32,
    pub current_targets: u32,
    pub current_users: u32,
    pub features: Vec<String>,
    pub custom_branding: bool,
    pub sso_enabled: bool,
    pub api_rate_limit: u32,    // requests per minute
    pub region: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantUsage {
    pub tenant_id: String,
    pub tenant_name: String,
    pub targets_used: u32,
    pub targets_limit: u32,
    pub users_used: u32,
    pub users_limit: u32,
    pub metrics_per_day: u64,
    pub logs_per_day: u64,
    pub alerts_fired_30d: u32,
    pub storage_gb: f32,
    pub api_calls_today: u32,
    pub last_active: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: Option<String>,
    pub plan: Option<String>,
    pub owner_email: String,
    pub max_targets: Option<u32>,
    pub max_users: Option<u32>,
    pub retention_days: Option<u32>,
    pub region: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TenantListParams {
    pub status: Option<String>,
    pub plan: Option<String>,
}

struct TenantStore {
    tenants: Vec<Tenant>,
}

fn store() -> &'static RwLock<TenantStore> {
    static INSTANCE: OnceLock<RwLock<TenantStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> TenantStore {
    let now = Utc::now();
    TenantStore {
        tenants: vec![
            Tenant {
                id: "tenant-001".into(),
                name: "Acme Corporation".into(),
                slug: "acme".into(),
                plan: "enterprise".into(),
                status: "active".into(),
                owner_email: "ops@acme.com".into(),
                created_at: (now - chrono::Duration::days(180)).to_rfc3339(),
                trial_ends_at: None,
                data_isolation: "namespace".into(),
                max_targets: 500,
                max_users: 50,
                max_dashboards: 200,
                max_alerts: 1000,
                retention_days: 365,
                current_targets: 347,
                current_users: 23,
                features: vec!["sso".into(), "api_access".into(), "custom_branding".into(), "advanced_ml".into()],
                custom_branding: true,
                sso_enabled: true,
                api_rate_limit: 10000,
                region: "us-east-1".into(),
                notes: Some("Primary enterprise customer — dedicated support channel".into()),
            },
            Tenant {
                id: "tenant-002".into(),
                name: "Globex Systems".into(),
                slug: "globex".into(),
                plan: "professional".into(),
                status: "active".into(),
                owner_email: "admin@globex.io".into(),
                created_at: (now - chrono::Duration::days(90)).to_rfc3339(),
                trial_ends_at: None,
                data_isolation: "namespace".into(),
                max_targets: 100,
                max_users: 20,
                max_dashboards: 50,
                max_alerts: 200,
                retention_days: 90,
                current_targets: 67,
                current_users: 12,
                features: vec!["api_access".into(), "advanced_ml".into()],
                custom_branding: false,
                sso_enabled: false,
                api_rate_limit: 2000,
                region: "eu-west-1".into(),
                notes: None,
            },
            Tenant {
                id: "tenant-003".into(),
                name: "StartupCo".into(),
                slug: "startupco".into(),
                plan: "starter".into(),
                status: "trial".into(),
                owner_email: "dev@startupco.io".into(),
                created_at: (now - chrono::Duration::days(7)).to_rfc3339(),
                trial_ends_at: Some((now + chrono::Duration::days(7)).to_rfc3339()),
                data_isolation: "namespace".into(),
                max_targets: 10,
                max_users: 5,
                max_dashboards: 10,
                max_alerts: 50,
                retention_days: 30,
                current_targets: 3,
                current_users: 2,
                features: vec!["api_access".into()],
                custom_branding: false,
                sso_enabled: false,
                api_rate_limit: 500,
                region: "us-west-2".into(),
                notes: None,
            },
        ],
    }
}

pub async fn list_tenants(
    State(_s): State<AppState>,
    Query(params): Query<TenantListParams>,
) -> Result<Json<Vec<Tenant>>, ApiError> {
    let store = store().read();
    let result: Vec<Tenant> = store.tenants.iter()
        .filter(|t| params.status.as_ref().map(|s| &t.status == s).unwrap_or(true))
        .filter(|t| params.plan.as_ref().map(|p| &t.plan == p).unwrap_or(true))
        .cloned().collect();
    Ok(Json(result))
}

pub async fn get_tenant(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Tenant>, ApiError> {
    let store = store().read();
    store.tenants.iter().find(|t| t.id == id || t.slug == id)
        .cloned().map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Tenant not found".into())))
}

pub async fn create_tenant(
    State(_s): State<AppState>,
    Json(req): Json<CreateTenantRequest>,
) -> Result<Json<Tenant>, ApiError> {
    let slug = req.slug.unwrap_or_else(|| req.name.to_lowercase().replace(' ', "-").replace(|c: char| !c.is_alphanumeric() && c != '-', ""));
    let plan = req.plan.as_deref().unwrap_or("starter");
    let (max_targets, max_users, max_dashboards, max_alerts, retention_days, api_rate, features) = match plan {
        "enterprise" => (500u32, 50u32, 200u32, 1000u32, 365u32, 10000u32, vec!["sso".into(), "api_access".into(), "custom_branding".into(), "advanced_ml".into()]),
        "professional" => (100, 20, 50, 200, 90, 2000, vec!["api_access".into(), "advanced_ml".into()]),
        "starter" => (10, 5, 10, 50, 30, 500, vec!["api_access".into()]),
        _ => (3, 2, 5, 20, 14, 100, vec![]),
    };
    let tenant = Tenant {
        id: crate::error::short_id("tenant"),
        name: req.name,
        slug,
        plan: plan.into(),
        status: "trial".into(),
        owner_email: req.owner_email,
        created_at: Utc::now().to_rfc3339(),
        trial_ends_at: Some((Utc::now() + chrono::Duration::days(14)).to_rfc3339()),
        data_isolation: "namespace".into(),
        max_targets: req.max_targets.unwrap_or(max_targets),
        max_users: req.max_users.unwrap_or(max_users),
        max_dashboards,
        max_alerts,
        retention_days: req.retention_days.unwrap_or(retention_days),
        current_targets: 0,
        current_users: 0,
        features,
        custom_branding: plan == "enterprise",
        sso_enabled: false,
        api_rate_limit: api_rate,
        region: req.region.unwrap_or_else(|| "us-east-1".into()),
        notes: req.notes,
    };
    store().write().tenants.push(tenant.clone());
    Ok(Json(tenant))
}

pub async fn update_tenant(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<Tenant>, ApiError> {
    let mut store = store().write();
    let t = store.tenants.iter_mut().find(|t| t.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Tenant not found".into())))?;
    if let Some(v) = req.get("status").and_then(|v| v.as_str()) { t.status = v.into(); }
    if let Some(v) = req.get("plan").and_then(|v| v.as_str()) { t.plan = v.into(); }
    if let Some(v) = req.get("max_targets").and_then(|v| v.as_u64()) { t.max_targets = v as u32; }
    if let Some(v) = req.get("max_users").and_then(|v| v.as_u64()) { t.max_users = v as u32; }
    if let Some(v) = req.get("retention_days").and_then(|v| v.as_u64()) { t.retention_days = v as u32; }
    if let Some(v) = req.get("notes").and_then(|v| v.as_str()) { t.notes = Some(v.into()); }
    if let Some(v) = req.get("sso_enabled").and_then(|v| v.as_bool()) { t.sso_enabled = v; }
    if let Some(v) = req.get("custom_branding").and_then(|v| v.as_bool()) { t.custom_branding = v; }
    Ok(Json(t.clone()))
}

pub async fn delete_tenant(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.tenants.len();
    store.tenants.retain(|t| t.id != id);
    if store.tenants.len() < before {
        Ok(Json(serde_json::json!({ "deleted": id })))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Tenant not found".into())))
    }
}

pub async fn suspend_tenant(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Tenant>, ApiError> {
    let mut store = store().write();
    let t = store.tenants.iter_mut().find(|t| t.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Tenant not found".into())))?;
    t.status = "suspended".into();
    Ok(Json(t.clone()))
}

pub async fn activate_tenant(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Tenant>, ApiError> {
    let mut store = store().write();
    let t = store.tenants.iter_mut().find(|t| t.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Tenant not found".into())))?;
    t.status = "active".into();
    t.trial_ends_at = None;
    Ok(Json(t.clone()))
}

pub async fn tenant_usage(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TenantUsage>, ApiError> {
    let store = store().read();
    let t = store.tenants.iter().find(|t| t.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Tenant not found".into())))?;
    Ok(Json(TenantUsage {
        tenant_id: t.id.clone(),
        tenant_name: t.name.clone(),
        targets_used: t.current_targets,
        targets_limit: t.max_targets,
        users_used: t.current_users,
        users_limit: t.max_users,
        metrics_per_day: (t.current_targets as u64) * 8640,
        logs_per_day: (t.current_targets as u64) * 2000,
        alerts_fired_30d: (t.current_targets / 3).max(1),
        storage_gb: t.current_targets as f32 * 0.12,
        api_calls_today: t.current_targets * 48,
        last_active: (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339(),
    }))
}

pub async fn tenants_summary(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let total = store.tenants.len();
    let active = store.tenants.iter().filter(|t| t.status == "active").count();
    let trial = store.tenants.iter().filter(|t| t.status == "trial").count();
    let suspended = store.tenants.iter().filter(|t| t.status == "suspended").count();
    let total_targets: u32 = store.tenants.iter().map(|t| t.current_targets).sum();
    let total_users: u32 = store.tenants.iter().map(|t| t.current_users).sum();
    Ok(Json(serde_json::json!({
        "total_tenants": total,
        "active_tenants": active,
        "trial_tenants": trial,
        "suspended_tenants": suspended,
        "total_monitored_targets": total_targets,
        "total_users": total_users,
        "by_plan": {
            "enterprise": store.tenants.iter().filter(|t| t.plan == "enterprise").count(),
            "professional": store.tenants.iter().filter(|t| t.plan == "professional").count(),
            "starter": store.tenants.iter().filter(|t| t.plan == "starter").count(),
            "free": store.tenants.iter().filter(|t| t.plan == "free").count(),
        },
        "by_region": store.tenants.iter().fold(serde_json::Map::new(), |mut m, t| {
            let count = m.get(&t.region).and_then(|v| v.as_u64()).unwrap_or(0) + 1;
            m.insert(t.region.clone(), serde_json::json!(count));
            m
        }),
    })))
}
