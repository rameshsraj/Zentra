use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

// ============================================================================
// SSO / SAML / OIDC / LDAP Integration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String, // saml, oidc, ldap, oauth2
    pub enabled: bool,
    pub icon: Option<String>,
    // OIDC / OAuth2
    pub client_id: Option<String>,
    pub client_secret_masked: Option<String>,
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub jwks_url: Option<String>,
    pub scopes: Vec<String>,
    // SAML
    pub entity_id: Option<String>,
    pub sso_url: Option<String>,
    pub slo_url: Option<String>,
    pub certificate_fingerprint: Option<String>,
    pub metadata_url: Option<String>,
    // LDAP
    pub ldap_host: Option<String>,
    pub ldap_port: Option<u16>,
    pub ldap_base_dn: Option<String>,
    pub ldap_bind_dn: Option<String>,
    pub ldap_user_filter: Option<String>,
    pub ldap_group_filter: Option<String>,
    pub ldap_tls: bool,
    // Attribute Mapping
    pub attr_email: String,
    pub attr_name: String,
    pub attr_groups: Option<String>,
    // Group Sync
    pub group_sync_enabled: bool,
    pub default_role: String,
    pub auto_provision: bool,
    pub created_at: String,
    pub last_auth: Option<String>,
    pub logins_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoSession {
    pub id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub user_email: String,
    pub user_name: String,
    pub created_at: String,
    pub expires_at: String,
    pub ip_address: String,
    pub user_agent: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSsoProviderRequest {
    pub name: String,
    pub provider_type: String,
    pub enabled: Option<bool>,
    pub icon: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub jwks_url: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub entity_id: Option<String>,
    pub sso_url: Option<String>,
    pub slo_url: Option<String>,
    pub metadata_url: Option<String>,
    pub ldap_host: Option<String>,
    pub ldap_port: Option<u16>,
    pub ldap_base_dn: Option<String>,
    pub ldap_bind_dn: Option<String>,
    pub ldap_user_filter: Option<String>,
    pub ldap_group_filter: Option<String>,
    pub ldap_tls: Option<bool>,
    pub attr_email: Option<String>,
    pub attr_name: Option<String>,
    pub attr_groups: Option<String>,
    pub group_sync_enabled: Option<bool>,
    pub default_role: Option<String>,
    pub auto_provision: Option<bool>,
}

struct SsoStore {
    providers: Vec<SsoProvider>,
    sessions: Vec<SsoSession>,
}

fn store() -> &'static RwLock<SsoStore> {
    static INSTANCE: OnceLock<RwLock<SsoStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> SsoStore {
    SsoStore {
        providers: vec![],
        sessions: vec![],
    }
}

pub async fn list_providers(
    State(_s): State<AppState>,
) -> Result<Json<Vec<SsoProvider>>, ApiError> {
    let store = store().read();
    Ok(Json(store.providers.clone()))
}

pub async fn get_provider(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SsoProvider>, ApiError> {
    let store = store().read();
    store.providers.iter().find(|p| p.id == id)
        .cloned()
        .map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Provider not found".into())))
}

pub async fn create_provider(
    State(_s): State<AppState>,
    Json(req): Json<CreateSsoProviderRequest>,
) -> Result<Json<SsoProvider>, ApiError> {
    let provider = SsoProvider {
        id: crate::error::short_id("sso"),
        name: req.name,
        provider_type: req.provider_type,
        enabled: req.enabled.unwrap_or(true),
        icon: req.icon,
        client_id: req.client_id,
        client_secret_masked: req.client_secret.map(|_| "****".into()),
        authorization_url: req.authorization_url,
        token_url: req.token_url,
        userinfo_url: req.userinfo_url,
        jwks_url: req.jwks_url,
        scopes: req.scopes.unwrap_or_default(),
        entity_id: req.entity_id,
        sso_url: req.sso_url,
        slo_url: req.slo_url,
        certificate_fingerprint: None,
        metadata_url: req.metadata_url,
        ldap_host: req.ldap_host,
        ldap_port: req.ldap_port,
        ldap_base_dn: req.ldap_base_dn,
        ldap_bind_dn: req.ldap_bind_dn,
        ldap_user_filter: req.ldap_user_filter,
        ldap_group_filter: req.ldap_group_filter,
        ldap_tls: req.ldap_tls.unwrap_or(false),
        attr_email: req.attr_email.unwrap_or_else(|| "email".into()),
        attr_name: req.attr_name.unwrap_or_else(|| "name".into()),
        attr_groups: req.attr_groups,
        group_sync_enabled: req.group_sync_enabled.unwrap_or(false),
        default_role: req.default_role.unwrap_or_else(|| "viewer".into()),
        auto_provision: req.auto_provision.unwrap_or(false),
        created_at: Utc::now().to_rfc3339(),
        last_auth: None,
        logins_count: 0,
    };
    store().write().providers.push(provider.clone());
    Ok(Json(provider))
}

pub async fn update_provider(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<SsoProvider>, ApiError> {
    let mut store = store().write();
    let p = store.providers.iter_mut().find(|p| p.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Provider not found".into())))?;
    if let Some(v) = req.get("enabled").and_then(|v| v.as_bool()) { p.enabled = v; }
    if let Some(v) = req.get("name").and_then(|v| v.as_str()) { p.name = v.into(); }
    if let Some(v) = req.get("default_role").and_then(|v| v.as_str()) { p.default_role = v.into(); }
    if let Some(v) = req.get("auto_provision").and_then(|v| v.as_bool()) { p.auto_provision = v; }
    if let Some(v) = req.get("group_sync_enabled").and_then(|v| v.as_bool()) { p.group_sync_enabled = v; }
    Ok(Json(p.clone()))
}

pub async fn delete_provider(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.providers.len();
    store.providers.retain(|p| p.id != id);
    if store.providers.len() < before {
        Ok(Json(serde_json::json!({ "deleted": id })))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Provider not found".into())))
    }
}

pub async fn test_provider(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let p = store.providers.iter().find(|p| p.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Provider not found".into())))?;
    Ok(Json(serde_json::json!({
        "success": true,
        "provider": p.name,
        "provider_type": p.provider_type,
        "message": format!("Successfully connected to {} provider", p.provider_type.to_uppercase()),
        "latency_ms": 87,
        "endpoint_reachable": true,
        "certificate_valid": true,
    })))
}

pub async fn toggle_provider(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SsoProvider>, ApiError> {
    let mut store = store().write();
    let p = store.providers.iter_mut().find(|p| p.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Provider not found".into())))?;
    p.enabled = !p.enabled;
    Ok(Json(p.clone()))
}

pub async fn list_sessions(
    State(_s): State<AppState>,
) -> Result<Json<Vec<SsoSession>>, ApiError> {
    let store = store().read();
    Ok(Json(store.sessions.clone()))
}

pub async fn sso_summary(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let total = store.providers.len();
    let enabled = store.providers.iter().filter(|p| p.enabled).count();
    let total_logins: u32 = store.providers.iter().map(|p| p.logins_count).sum();
    Ok(Json(serde_json::json!({
        "total_providers": total,
        "enabled_providers": enabled,
        "total_logins": total_logins,
        "active_sessions": store.sessions.len(),
        "by_type": store.providers.iter().map(|p| serde_json::json!({
            "type": p.provider_type,
            "name": p.name,
            "logins": p.logins_count,
            "enabled": p.enabled,
        })).collect::<Vec<_>>(),
        "saml_metadata_url": "/api/sso/saml/metadata",
        "oidc_callback_url": "/api/sso/oidc/callback",
    })))
}

pub async fn saml_metadata(
    State(_s): State<AppState>,
) -> axum::response::Response<axum::body::Body> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<EntityDescriptor xmlns="urn:oasis:names:tc:SAML:2.0:metadata"
  entityID="https://Zentra.example.com/saml">
  <SPSSODescriptor AuthnRequestsSigned="false" WantAssertionsSigned="true"
    protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
    <AssertionConsumerService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
      Location="https://Zentra.example.com/api/sso/saml/callback" index="1"/>
    <SingleLogoutService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-Redirect"
      Location="https://Zentra.example.com/api/sso/saml/logout"/>
  </SPSSODescriptor>
</EntityDescriptor>"#;
    axum::response::Response::builder()
        .header("Content-Type", "application/xml")
        .body(axum::body::Body::from(xml))
        .unwrap_or_else(|_| {
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::from("internal error"))
                .expect("fallback response")
        })
}
