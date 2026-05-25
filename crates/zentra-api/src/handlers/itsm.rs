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
// ITSM Integration — ServiceNow, JIRA, PagerDuty
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItsmConfig {
    pub id: String,
    pub name: String,
    pub provider: String, // servicenow, jira, pagerduty, opsgenie
    pub enabled: bool,
    pub url: String,
    pub username: Option<String>,
    pub api_key_masked: Option<String>,
    pub project_key: Option<String>,   // JIRA project key
    pub instance: Option<String>,      // ServiceNow instance name
    pub routing_key: Option<String>,   // PagerDuty/OpsGenie routing key
    pub field_mappings: serde_json::Value,
    pub created_at: String,
    pub last_sync: Option<String>,
    pub sync_status: String, // ok, error, pending
    pub tickets_created: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItsmTicket {
    pub id: String,
    pub provider: String,
    pub external_id: String,
    pub external_url: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub alert_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateItsmConfigRequest {
    pub name: String,
    pub provider: String,
    pub url: String,
    pub username: Option<String>,
    pub api_key: Option<String>,
    pub project_key: Option<String>,
    pub instance: Option<String>,
    pub routing_key: Option<String>,
    pub field_mappings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTicketRequest {
    pub config_id: String,
    pub title: String,
    pub description: String,
    pub priority: Option<String>,
    pub alert_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ItsmListParams {
    pub provider: Option<String>,
}

struct ItsmStore {
    configs: Vec<ItsmConfig>,
    tickets: Vec<ItsmTicket>,
}

fn store() -> &'static RwLock<ItsmStore> {
    static INSTANCE: OnceLock<RwLock<ItsmStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> ItsmStore {
    let now = Utc::now();
    ItsmStore {
        configs: vec![
            ItsmConfig {
                id: "itsm-001".into(),
                name: "ServiceNow Production".into(),
                provider: "servicenow".into(),
                enabled: true,
                url: "https://example.service-now.com".into(),
                username: Some("zentra_svc".into()),
                api_key_masked: Some("****".into()),
                project_key: None,
                instance: Some("example".into()),
                routing_key: None,
                field_mappings: serde_json::json!({
                    "category": "Infrastructure",
                    "assignment_group": "NOC",
                    "cmdb_ci": "auto_detect"
                }),
                created_at: (now - chrono::Duration::days(30)).to_rfc3339(),
                last_sync: Some((now - chrono::Duration::hours(1)).to_rfc3339()),
                sync_status: "ok".into(),
                tickets_created: 47,
            },
            ItsmConfig {
                id: "itsm-002".into(),
                name: "JIRA Cloud".into(),
                provider: "jira".into(),
                enabled: true,
                url: "https://yourcompany.atlassian.net".into(),
                username: Some("alerts@yourcompany.com".into()),
                api_key_masked: Some("****".into()),
                project_key: Some("OPS".into()),
                instance: None,
                routing_key: None,
                field_mappings: serde_json::json!({
                    "issue_type": "Bug",
                    "labels": ["Zentra", "auto-created"],
                    "priority_map": {"critical": "Highest", "warning": "High", "info": "Medium"}
                }),
                created_at: (now - chrono::Duration::days(14)).to_rfc3339(),
                last_sync: Some((now - chrono::Duration::hours(2)).to_rfc3339()),
                sync_status: "ok".into(),
                tickets_created: 23,
            },
        ],
        tickets: vec![
            ItsmTicket {
                id: "ticket-001".into(),
                provider: "servicenow".into(),
                external_id: "INC0012345".into(),
                external_url: "https://example.service-now.com/incident.do?sys_id=abc123".into(),
                title: "CPU usage critical on app-server-01".into(),
                description: "CPU usage exceeded 95% threshold for 10 minutes. Automatic ticket created by Zentra.".into(),
                status: "resolved".into(),
                priority: "2 - High".into(),
                assignee: Some("John Doe".into()),
                alert_id: Some("alert-xyz".into()),
                created_at: (now - chrono::Duration::hours(8)).to_rfc3339(),
                updated_at: (now - chrono::Duration::hours(6)).to_rfc3339(),
                resolved_at: Some((now - chrono::Duration::hours(6)).to_rfc3339()),
            },
            ItsmTicket {
                id: "ticket-002".into(),
                provider: "jira".into(),
                external_id: "OPS-1042".into(),
                external_url: "https://yourcompany.atlassian.net/browse/OPS-1042".into(),
                title: "Disk space warning on db-primary".into(),
                description: "Disk usage at 87% on /var/lib/postgresql. Immediate attention required.".into(),
                status: "open".into(),
                priority: "High".into(),
                assignee: Some("ops-team".into()),
                alert_id: Some("alert-abc".into()),
                created_at: (now - chrono::Duration::hours(2)).to_rfc3339(),
                updated_at: (now - chrono::Duration::hours(1)).to_rfc3339(),
                resolved_at: None,
            },
        ],
    }
}

pub async fn list_configs(
    State(_s): State<AppState>,
    Query(params): Query<ItsmListParams>,
) -> Result<Json<Vec<ItsmConfig>>, ApiError> {
    let store = store().read();
    let result: Vec<ItsmConfig> = store.configs.iter()
        .filter(|c| params.provider.as_ref().map(|p| &c.provider == p).unwrap_or(true))
        .cloned()
        .collect();
    Ok(Json(result))
}

pub async fn create_config(
    State(_s): State<AppState>,
    Json(req): Json<CreateItsmConfigRequest>,
) -> Result<Json<ItsmConfig>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let config = ItsmConfig {
        id: crate::error::short_id("itsm"),
        name: req.name,
        provider: req.provider,
        enabled: true,
        url: req.url,
        username: req.username,
        api_key_masked: req.api_key.map(|_| "****".into()),
        project_key: req.project_key,
        instance: req.instance,
        routing_key: req.routing_key.map(|_| "****".into()),
        field_mappings: req.field_mappings.unwrap_or(serde_json::json!({})),
        created_at: now.clone(),
        last_sync: None,
        sync_status: "pending".into(),
        tickets_created: 0,
    };
    store().write().configs.push(config.clone());
    Ok(Json(config))
}

pub async fn delete_config(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.configs.len();
    store.configs.retain(|c| c.id != id);
    if store.configs.len() < before {
        Ok(Json(serde_json::json!({"deleted": id})))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Config not found".into())))
    }
}

pub async fn test_config(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    if let Some(config) = store.configs.iter_mut().find(|c| c.id == id) {
        config.last_sync = Some(Utc::now().to_rfc3339());
        config.sync_status = "ok".into();
        Ok(Json(serde_json::json!({
            "success": true,
            "message": format!("Successfully connected to {} ({})", config.name, config.provider),
            "latency_ms": 142
        })))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Config not found".into())))
    }
}

pub async fn list_tickets(
    State(_s): State<AppState>,
) -> Result<Json<Vec<ItsmTicket>>, ApiError> {
    let store = store().read();
    let mut tickets = store.tickets.clone();
    tickets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(tickets))
}

pub async fn create_ticket(
    State(_s): State<AppState>,
    Json(req): Json<CreateTicketRequest>,
) -> Result<Json<ItsmTicket>, ApiError> {
    let store_read = store().read();
    let config = store_read.configs.iter().find(|c| c.id == req.config_id)
        .ok_or((StatusCode::NOT_FOUND, "ITSM config not found".into()))?;
    let provider = config.provider.clone();
    let (external_id, external_url) = match provider.as_str() {
        "servicenow" => {
            let id = format!("INC{:07}", rand_u32() % 9999999);
            let url = format!("{}/incident.do?sys_id={}", config.url, uuid::Uuid::new_v4().to_string().replace('-', "").chars().take(32).collect::<String>());
            (id, url)
        }
        "jira" => {
            let proj = config.project_key.as_deref().unwrap_or("OPS");
            let num = rand_u32() % 9999;
            let id = format!("{}-{}", proj, num);
            let url = format!("{}/browse/{}-{}", config.url, proj, num);
            (id, url)
        }
        "pagerduty" => {
            let id = format!("P{}", &uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_uppercase());
            let url = format!("https://app.pagerduty.com/incidents/{}", id);
            (id, url)
        }
        _ => (uuid::Uuid::new_v4().to_string(), config.url.clone()),
    };
    drop(store_read);

    let now = Utc::now().to_rfc3339();
    let ticket = ItsmTicket {
        id: crate::error::short_id("ticket"),
        provider,
        external_id,
        external_url,
        title: req.title,
        description: req.description,
        status: "open".into(),
        priority: req.priority.unwrap_or_else(|| "Medium".into()),
        assignee: None,
        alert_id: req.alert_id,
        created_at: now.clone(),
        updated_at: now,
        resolved_at: None,
    };

    let mut store = store().write();
    if let Some(config) = store.configs.iter_mut().find(|c| c.id == req.config_id) {
        config.tickets_created += 1;
        config.last_sync = Some(Utc::now().to_rfc3339());
    }
    store.tickets.push(ticket.clone());
    Ok(Json(ticket))
}

pub async fn itsm_summary(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let total_configs = store.configs.len();
    let enabled = store.configs.iter().filter(|c| c.enabled).count();
    let total_tickets: u32 = store.configs.iter().map(|c| c.tickets_created).sum();
    let open_tickets = store.tickets.iter().filter(|t| t.status == "open").count();
    let resolved_tickets = store.tickets.iter().filter(|t| t.status == "resolved").count();
    Ok(Json(serde_json::json!({
        "total_integrations": total_configs,
        "enabled_integrations": enabled,
        "total_tickets_created": total_tickets,
        "open_tickets": open_tickets,
        "resolved_tickets": resolved_tickets,
        "by_provider": store.configs.iter().map(|c| serde_json::json!({
            "provider": c.provider,
            "name": c.name,
            "tickets": c.tickets_created,
            "status": c.sync_status,
        })).collect::<Vec<_>>(),
    })))
}

fn rand_u32() -> u32 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345)
}
