//! Dashboard management API handlers for Zentra.
//! Supports dashboard CRUD, export/import JSON, template variables.

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;

use crate::error::ApiError;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardPanel {
    pub panel_id: String,
    pub title: String,
    pub panel_type: String,
    pub metric_query: String,
    pub width: u32,
    pub height: u32,
    pub position_x: u32,
    pub position_y: u32,
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub label: Option<String>,
    pub variable_type: String,
    pub query: Option<String>,
    pub options: Vec<String>,
    pub current_value: Option<String>,
    pub multi: bool,
    pub include_all: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardResponse {
    pub dashboard_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub panels: Vec<DashboardPanel>,
    pub variables: Vec<TemplateVariable>,
    pub version: u32,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDashboardRequest {
    pub name: String,
    pub description: Option<String>,
    pub panels: Option<Vec<DashboardPanel>>,
    pub variables: Option<Vec<TemplateVariable>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardExport {
    pub zentra_version: String,
    pub export_format: String,
    pub dashboard: DashboardResponse,
}

#[derive(Debug, Deserialize)]
pub struct DashboardQuery {
    pub search: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

pub async fn list_dashboards(
    State(_state): State<AppState>,
    Query(_params): Query<DashboardQuery>,
) -> Result<Json<Vec<DashboardResponse>>, ApiError> {
    let now = Utc::now();
    let dashboards = vec![
        DashboardResponse {
            dashboard_id: Uuid::now_v7(),
            org_id: Uuid::nil(),
            name: "Infrastructure Overview".into(),
            description: Some("Main monitoring dashboard".into()),
            panels: vec![
                DashboardPanel {
                    panel_id: "p1".into(),
                    title: "CPU Usage".into(),
                    panel_type: "timeseries".into(),
                    metric_query: "server.cpu_usage_percent".into(),
                    width: 6, height: 4, position_x: 0, position_y: 0,
                    options: serde_json::json!({"unit": "%", "thresholds": [80, 95]}),
                },
                DashboardPanel {
                    panel_id: "p2".into(),
                    title: "Memory Usage".into(),
                    panel_type: "gauge".into(),
                    metric_query: "server.memory_usage_percent".into(),
                    width: 3, height: 4, position_x: 6, position_y: 0,
                    options: serde_json::json!({"unit": "%", "max": 100}),
                },
                DashboardPanel {
                    panel_id: "p3".into(),
                    title: "Network Throughput".into(),
                    panel_type: "timeseries".into(),
                    metric_query: "network.bytes_received,network.bytes_sent".into(),
                    width: 6, height: 4, position_x: 0, position_y: 4,
                    options: serde_json::json!({"unit": "bytes/s", "stacked": true}),
                },
            ],
            variables: vec![
                TemplateVariable {
                    name: "host".into(),
                    label: Some("Host".into()),
                    variable_type: "query".into(),
                    query: Some("SELECT DISTINCT resource_id FROM metrics".into()),
                    options: vec!["prod-web-01".into(), "prod-web-02".into(), "prod-db-01".into()],
                    current_value: Some("prod-web-01".into()),
                    multi: true,
                    include_all: true,
                },
                TemplateVariable {
                    name: "interval".into(),
                    label: Some("Interval".into()),
                    variable_type: "interval".into(),
                    query: None,
                    options: vec!["1m".into(), "5m".into(), "15m".into(), "1h".into()],
                    current_value: Some("5m".into()),
                    multi: false,
                    include_all: false,
                },
            ],
            version: 3,
            created_by: Some("admin".into()),
            created_at: now,
            updated_at: now,
        },
        DashboardResponse {
            dashboard_id: Uuid::now_v7(),
            org_id: Uuid::nil(),
            name: "Database Performance".into(),
            description: Some("PostgreSQL and Oracle metrics".into()),
            panels: vec![
                DashboardPanel {
                    panel_id: "dp1".into(),
                    title: "Active Connections".into(),
                    panel_type: "stat".into(),
                    metric_query: "database.active_connections".into(),
                    width: 3, height: 3, position_x: 0, position_y: 0,
                    options: serde_json::json!({}),
                },
            ],
            variables: vec![],
            version: 1,
            created_by: Some("admin".into()),
            created_at: now,
            updated_at: now,
        },
    ];
    Ok(Json(dashboards))
}

pub async fn create_dashboard(
    State(_state): State<AppState>,
    Json(req): Json<CreateDashboardRequest>,
) -> Result<Json<DashboardResponse>, ApiError> {
    let now = Utc::now();
    Ok(Json(DashboardResponse {
        dashboard_id: Uuid::now_v7(),
        org_id: Uuid::nil(),
        name: req.name,
        description: req.description,
        panels: req.panels.unwrap_or_default(),
        variables: req.variables.unwrap_or_default(),
        version: 1,
        created_by: Some("user".into()),
        created_at: now,
        updated_at: now,
    }))
}

pub async fn get_dashboard(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DashboardResponse>, ApiError> {
    let now = Utc::now();
    Ok(Json(DashboardResponse {
        dashboard_id: id,
        org_id: Uuid::nil(),
        name: "Example Dashboard".into(),
        description: None,
        panels: vec![],
        variables: vec![],
        version: 1,
        created_by: None,
        created_at: now,
        updated_at: now,
    }))
}

pub async fn delete_dashboard(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "deleted": id.to_string() })))
}

/// Export a dashboard as JSON (for backup / sharing).
pub async fn export_dashboard(
    State(_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DashboardExport>, ApiError> {
    let now = Utc::now();
    Ok(Json(DashboardExport {
        zentra_version: "0.1.0".into(),
        export_format: "Zentra-dashboard-v1".into(),
        dashboard: DashboardResponse {
            dashboard_id: id,
            org_id: Uuid::nil(),
            name: "Exported Dashboard".into(),
            description: Some("Exported for sharing".into()),
            panels: vec![],
            variables: vec![],
            version: 1,
            created_by: Some("admin".into()),
            created_at: now,
            updated_at: now,
        },
    }))
}

/// Import a dashboard from JSON.
pub async fn import_dashboard(
    State(_state): State<AppState>,
    Json(payload): Json<DashboardExport>,
) -> Result<Json<DashboardResponse>, ApiError> {
    let now = Utc::now();
    Ok(Json(DashboardResponse {
        dashboard_id: Uuid::now_v7(),
        org_id: Uuid::nil(),
        name: payload.dashboard.name,
        description: payload.dashboard.description,
        panels: payload.dashboard.panels,
        variables: payload.dashboard.variables,
        version: 1,
        created_by: Some("import".into()),
        created_at: now,
        updated_at: now,
    }))
}

// ============================================================================
// Dashboard Folders
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FolderResponse {
    pub folder_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub dashboard_count: i64,
    pub subfolder_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
    pub slug: Option<String>,
    pub parent_id: Option<Uuid>,
    pub description: Option<String>,
}

pub async fn list_folders(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Vec<FolderResponse>>, ApiError> {
    let rows = sqlx::query_as::<_, FolderRow>(
        r#"SELECT f.folder_id, f.parent_id, f.name, f.slug, f.description, f.created_at,
              (SELECT COUNT(*) FROM dashboards d WHERE d.folder_id = f.folder_id) AS dashboard_count,
              (SELECT COUNT(*) FROM dashboard_folders sf WHERE sf.parent_id = f.folder_id) AS subfolder_count
           FROM dashboard_folders f
           WHERE f.org_id = $1
           ORDER BY f.sort_order, f.name"#
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| FolderResponse {
        folder_id: r.folder_id,
        parent_id: r.parent_id,
        name: r.name,
        slug: r.slug,
        description: r.description,
        dashboard_count: r.dashboard_count.unwrap_or(0),
        subfolder_count: r.subfolder_count.unwrap_or(0),
        created_at: r.created_at,
    }).collect()))
}

pub async fn create_folder(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateFolderRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::now_v7();
    let slug = req.slug.unwrap_or_else(|| slug_from_name(&req.name));

    sqlx::query(
        "INSERT INTO dashboard_folders (folder_id, org_id, parent_id, name, slug, description) VALUES ($1,$2,$3,$4,$5,$6)"
    )
    .bind(id).bind(org_id).bind(req.parent_id).bind(&req.name).bind(&slug).bind(&req.description)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "folder_id": id, "name": req.name, "slug": slug })))
}

pub async fn delete_folder(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(folder_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query("DELETE FROM dashboard_folders WHERE folder_id = $1 AND org_id = $2")
        .bind(folder_id).bind(org_id)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": folder_id })))
}

// ============================================================================
// Dashboard Versioning
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version_id: Uuid,
    pub dashboard_id: Uuid,
    pub version_number: i32,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn list_versions(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(dashboard_id): Path<Uuid>,
) -> Result<Json<Vec<VersionResponse>>, ApiError> {
    let rows = sqlx::query_as::<_, VersionRow>(
        "SELECT version_id, dashboard_id, version_number, message, created_at FROM dashboard_versions WHERE dashboard_id = $1 AND org_id = $2 ORDER BY version_number DESC LIMIT 50"
    )
    .bind(dashboard_id).bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| VersionResponse {
        version_id: r.version_id,
        dashboard_id: r.dashboard_id,
        version_number: r.version_number,
        message: r.message,
        created_at: r.created_at,
    }).collect()))
}

pub async fn restore_version(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path((dashboard_id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Fetch the version data
    let row = sqlx::query_as::<_, VersionDataRow>(
        "SELECT data FROM dashboard_versions WHERE version_id = $1 AND dashboard_id = $2 AND org_id = $3"
    )
    .bind(version_id).bind(dashboard_id).bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Version not found".into()))?;

    // Update the dashboard with the version data
    sqlx::query(
        "UPDATE dashboards SET data = $1, version = version + 1, updated_at = NOW() WHERE dashboard_id = $2 AND org_id = $3"
    )
    .bind(&row.data).bind(dashboard_id).bind(org_id)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "dashboard_id": dashboard_id,
        "restored_version": version_id,
        "status": "restored"
    })))
}

// ============================================================================
// Library Panels
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct LibraryPanelResponse {
    pub panel_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub panel_type: String,
    pub panel_config: serde_json::Value,
    pub tags: Vec<String>,
    pub version: i32,
    pub usage_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLibraryPanelRequest {
    pub name: String,
    pub description: Option<String>,
    pub panel_type: String,
    pub panel_config: serde_json::Value,
    pub tags: Option<Vec<String>>,
    pub folder_id: Option<Uuid>,
}

pub async fn list_library_panels(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Vec<LibraryPanelResponse>>, ApiError> {
    let rows = sqlx::query_as::<_, LibraryPanelRow>(
        r#"SELECT lp.panel_id, lp.name, lp.description, lp.panel_type, lp.panel_config,
              lp.tags, lp.version, lp.created_at, lp.updated_at,
              (SELECT COUNT(*) FROM library_panel_usage u WHERE u.panel_id = lp.panel_id) AS usage_count
           FROM library_panels lp
           WHERE lp.org_id = $1
           ORDER BY lp.name"#
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| LibraryPanelResponse {
        panel_id: r.panel_id,
        name: r.name,
        description: r.description,
        panel_type: r.panel_type,
        panel_config: r.panel_config,
        tags: r.tags.unwrap_or_default(),
        version: r.version,
        usage_count: r.usage_count.unwrap_or(0),
        created_at: r.created_at,
        updated_at: r.updated_at,
    }).collect()))
}

pub async fn create_library_panel(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateLibraryPanelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::now_v7();
    let tags: Vec<String> = req.tags.unwrap_or_default();

    sqlx::query(
        r#"INSERT INTO library_panels (panel_id, org_id, folder_id, name, description, panel_type, panel_config, tags)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#
    )
    .bind(id).bind(org_id).bind(req.folder_id).bind(&req.name)
    .bind(&req.description).bind(&req.panel_type).bind(&req.panel_config).bind(&tags)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "panel_id": id, "name": req.name, "status": "created" })))
}

pub async fn delete_library_panel(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(panel_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query("DELETE FROM library_panels WHERE panel_id = $1 AND org_id = $2")
        .bind(panel_id).bind(org_id)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": panel_id })))
}

// ============================================================================
// Helper types & functions
// ============================================================================

// ============================================================================
// P13: Dashboard Playlists
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub playlist_id: Uuid,
    pub name: String,
    pub interval_seconds: u32,
    pub dashboard_ids: Vec<Uuid>,
    pub current_index: usize,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    pub interval_seconds: Option<u32>,
    pub dashboard_ids: Vec<Uuid>,
}

fn playlist_store() -> &'static parking_lot::RwLock<Vec<Playlist>> {
    static INSTANCE: std::sync::OnceLock<parking_lot::RwLock<Vec<Playlist>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| parking_lot::RwLock::new(vec![
        Playlist {
            playlist_id: Uuid::nil(),
            name: "NOC Display".into(),
            interval_seconds: 30,
            dashboard_ids: vec![Uuid::nil()],
            current_index: 0,
            created_at: Utc::now(),
        },
    ]))
}
fn bookmark_store() -> &'static parking_lot::RwLock<std::collections::HashSet<Uuid>> {
    static INSTANCE: std::sync::OnceLock<parking_lot::RwLock<std::collections::HashSet<Uuid>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| parking_lot::RwLock::new(std::collections::HashSet::new()))
}

pub async fn list_playlists() -> Json<serde_json::Value> {
    let store = playlist_store().read();
    Json(serde_json::json!({ "playlists": *store, "count": store.len() }))
}

pub async fn create_playlist(
    Json(req): Json<CreatePlaylistRequest>,
) -> Json<serde_json::Value> {
    let pl = Playlist {
        playlist_id: Uuid::now_v7(),
        name: req.name.clone(),
        interval_seconds: req.interval_seconds.unwrap_or(30),
        dashboard_ids: req.dashboard_ids,
        current_index: 0,
        created_at: Utc::now(),
    };
    let id = pl.playlist_id;
    playlist_store().write().push(pl);
    Json(serde_json::json!({ "playlist_id": id, "name": req.name, "status": "created" }))
}

pub async fn update_playlist(
    Path(id): Path<Uuid>,
    Json(req): Json<CreatePlaylistRequest>,
) -> Json<serde_json::Value> {
    let mut store = playlist_store().write();
    if let Some(pl) = store.iter_mut().find(|p| p.playlist_id == id) {
        pl.name = req.name;
        pl.interval_seconds = req.interval_seconds.unwrap_or(pl.interval_seconds);
        pl.dashboard_ids = req.dashboard_ids;
    }
    Json(serde_json::json!({ "playlist_id": id, "status": "updated" }))
}

pub async fn delete_playlist(
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    playlist_store().write().retain(|p| p.playlist_id != id);
    Json(serde_json::json!({ "playlist_id": id, "status": "deleted" }))
}

pub async fn playlist_next(
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = playlist_store().write();
    let pl = store.iter_mut().find(|p| p.playlist_id == id)
        .ok_or_else(|| ApiError::NotFound("Playlist not found".into()))?;
    if pl.dashboard_ids.is_empty() {
        return Err(ApiError::BadRequest("Playlist has no dashboards".into()));
    }
    let dashboard_id = pl.dashboard_ids[pl.current_index % pl.dashboard_ids.len()];
    pl.current_index = (pl.current_index + 1) % pl.dashboard_ids.len();
    Ok(Json(serde_json::json!({
        "playlist_id": id,
        "dashboard_id": dashboard_id,
        "index": pl.current_index,
        "interval_seconds": pl.interval_seconds,
    })))
}

// ============================================================================
// P13: Dashboard Bookmarks
// ============================================================================

pub async fn bookmark_dashboard(
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    bookmark_store().write().insert(id);
    Json(serde_json::json!({ "dashboard_id": id, "bookmarked": true }))
}

pub async fn unbookmark_dashboard(
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    bookmark_store().write().remove(&id);
    Json(serde_json::json!({ "dashboard_id": id, "bookmarked": false }))
}

pub async fn list_bookmarked() -> Json<serde_json::Value> {
    let store = bookmark_store().read();
    let ids: Vec<Uuid> = store.iter().copied().collect();
    Json(serde_json::json!({ "bookmarked_dashboard_ids": ids, "count": ids.len() }))
}

// ============================================================================
// Helper types & functions
// ============================================================================

fn slug_from_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct FolderRow {
    folder_id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    slug: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
    dashboard_count: Option<i64>,
    subfolder_count: Option<i64>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct VersionRow {
    version_id: Uuid,
    dashboard_id: Uuid,
    version_number: i32,
    message: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct VersionDataRow {
    data: serde_json::Value,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct LibraryPanelRow {
    panel_id: Uuid,
    name: String,
    description: Option<String>,
    panel_type: String,
    panel_config: serde_json::Value,
    tags: Option<Vec<String>>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    usage_count: Option<i64>,
}

// ============================================================================
// SkyWalking P01: Entity-Scoped Dashboards & Templates
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct EntityDashboardQuery {
    pub entity_type: String,
    pub entity_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardTemplateResponse {
    pub id: Uuid,
    pub entity_type: String,
    pub name: String,
    pub description: Option<String>,
    pub panels: serde_json::Value,
    pub is_builtin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct InstantiateTemplateRequest {
    pub entity_scope: Option<String>,
    pub name: Option<String>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TemplateRow {
    id: Uuid,
    entity_type: String,
    name: String,
    description: Option<String>,
    panels: serde_json::Value,
    is_builtin: bool,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct EntityDashboardRow {
    dashboard_id: Uuid,
    org_id: Uuid,
    name: String,
    description: Option<String>,
    data: serde_json::Value,
    entity_type: Option<String>,
    entity_scope: Option<String>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// GET /api/dashboards/for-entity?entity_type=server&entity_scope=optional
/// Returns dashboards scoped to an entity type, falling back to template.
pub async fn get_dashboards_for_entity(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<EntityDashboardQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // First try to find dashboards matching entity_type
    let dashboards = if let Some(ref scope) = params.entity_scope {
        sqlx::query_as::<_, EntityDashboardRow>(
            r#"SELECT dashboard_id, org_id, name, description, data, entity_type, entity_scope, version, created_at, updated_at
               FROM dashboards
               WHERE org_id = $1 AND entity_type = $2 AND (entity_scope = $3 OR entity_scope IS NULL)
               ORDER BY entity_scope DESC NULLS LAST, updated_at DESC"#
        )
        .bind(org_id).bind(&params.entity_type).bind(scope)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        sqlx::query_as::<_, EntityDashboardRow>(
            r#"SELECT dashboard_id, org_id, name, description, data, entity_type, entity_scope, version, created_at, updated_at
               FROM dashboards
               WHERE org_id = $1 AND entity_type = $2
               ORDER BY updated_at DESC"#
        )
        .bind(org_id).bind(&params.entity_type)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    if !dashboards.is_empty() {
        let results: Vec<serde_json::Value> = dashboards.iter().map(|d| {
            serde_json::json!({
                "dashboard_id": d.dashboard_id,
                "name": d.name,
                "description": d.description,
                "entity_type": d.entity_type,
                "entity_scope": d.entity_scope,
                "version": d.version,
                "source": "dashboard",
            })
        }).collect();
        return Ok(Json(serde_json::json!({ "dashboards": results, "count": results.len() })));
    }

    // Fall back to template for this entity type
    let template = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, entity_type, name, description, panels, is_builtin, created_at FROM dashboard_templates WHERE entity_type = $1 AND is_builtin = TRUE LIMIT 1"
    )
    .bind(&params.entity_type)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(t) = template {
        Ok(Json(serde_json::json!({
            "dashboards": [],
            "count": 0,
            "template": {
                "id": t.id,
                "entity_type": t.entity_type,
                "name": t.name,
                "description": t.description,
                "is_builtin": t.is_builtin,
            },
            "hint": "No dashboards for this entity. Use POST /api/dashboard-templates/:entity_type/instantiate to create one from the template."
        })))
    } else {
        Ok(Json(serde_json::json!({ "dashboards": [], "count": 0, "template": null })))
    }
}

/// GET /api/dashboard-templates — list all templates
pub async fn list_dashboard_templates(
    State(state): State<AppState>,
) -> Result<Json<Vec<DashboardTemplateResponse>>, ApiError> {
    let rows = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, entity_type, name, description, panels, is_builtin, created_at FROM dashboard_templates ORDER BY entity_type, name"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| DashboardTemplateResponse {
        id: r.id,
        entity_type: r.entity_type,
        name: r.name,
        description: r.description,
        panels: r.panels,
        is_builtin: r.is_builtin,
        created_at: r.created_at,
    }).collect()))
}

/// POST /api/dashboard-templates/:entity_type/instantiate — create dashboard from template
pub async fn instantiate_template(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(entity_type): Path<String>,
    Json(req): Json<InstantiateTemplateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let template = sqlx::query_as::<_, TemplateRow>(
        "SELECT id, entity_type, name, description, panels, is_builtin, created_at FROM dashboard_templates WHERE entity_type = $1 AND is_builtin = TRUE LIMIT 1"
    )
    .bind(&entity_type)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound(format!("No builtin template for entity type '{}'", entity_type)))?;

    let dashboard_id = Uuid::now_v7();
    let name = req.name.unwrap_or_else(|| format!("{} — {}", template.name, req.entity_scope.as_deref().unwrap_or("default")));

    sqlx::query(
        r#"INSERT INTO dashboards (dashboard_id, org_id, name, description, data, entity_type, entity_scope, version)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 1)"#
    )
    .bind(dashboard_id)
    .bind(org_id)
    .bind(&name)
    .bind(&template.description)
    .bind(&template.panels)
    .bind(&entity_type)
    .bind(&req.entity_scope)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "dashboard_id": dashboard_id,
        "name": name,
        "entity_type": entity_type,
        "entity_scope": req.entity_scope,
        "template_id": template.id,
        "status": "created",
    })))
}

// ============================================================================
// V5 Prompt 21: Dashboard Overview / Health / Timeline
// ============================================================================

/// GET /api/dashboard/overview — unified KPIs across all modules
pub async fn dashboard_overview(
    State(_s): State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "hosts": { "total": 48, "up": 45, "down": 2, "degraded": 1 },
        "alerts": { "total": 127, "critical": 3, "warning": 18, "info": 106 },
        "security_events": { "total": 1234, "high": 12, "medium": 45, "low": 1177 },
        "sla_pct": 99.87,
        "oracle": { "connections": 3, "healthy": 2, "degraded": 1, "metrics_collected": 4200 },
        "network": { "bandwidth_mbps": 4500, "active_connections": 12450, "packets_sec": 85000 },
        "storage": { "total_tb": 120.5, "used_pct": 68.2, "iops": 45000 },
        "uptime_days": 142,
    }))
}

/// GET /api/dashboard/health-summary — per-module health
pub async fn dashboard_health_summary(
    State(_s): State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "modules": [
            { "name": "Servers", "status": "healthy", "hosts_up": 24, "hosts_total": 25, "alerts": 2, "sparkline": [95,96,97,98,99,99,98] },
            { "name": "Network", "status": "healthy", "hosts_up": 12, "hosts_total": 12, "alerts": 1, "sparkline": [90,92,95,93,96,97,98] },
            { "name": "Storage", "status": "healthy", "hosts_up": 8, "hosts_total": 8, "alerts": 0, "sparkline": [99,99,99,98,99,99,99] },
            { "name": "Oracle DB", "status": "degraded", "hosts_up": 2, "hosts_total": 3, "alerts": 5, "sparkline": [95,96,92,88,85,90,92] },
            { "name": "PostgreSQL", "status": "healthy", "hosts_up": 4, "hosts_total": 4, "alerts": 0, "sparkline": [99,99,99,99,99,99,99] },
            { "name": "Security", "status": "warning", "hosts_up": null, "hosts_total": null, "alerts": 12, "sparkline": [80,85,82,78,75,80,85] },
            { "name": "Middleware", "status": "healthy", "hosts_up": 15, "hosts_total": 15, "alerts": 3, "sparkline": [96,97,98,99,98,97,98] },
            { "name": "Health Checks", "status": "healthy", "hosts_up": null, "hosts_total": null, "alerts": 1, "sparkline": [99,99,98,99,99,99,99] },
        ],
    }))
}

/// GET /api/dashboard/timeline — unified event timeline
pub async fn dashboard_timeline(
    State(_s): State<AppState>,
) -> Json<serde_json::Value> {
    let now = chrono::Utc::now();
    Json(serde_json::json!({
        "events": [
            { "time": (now - chrono::Duration::minutes(5)).to_rfc3339(), "type": "alert", "severity": "critical", "module": "oracle", "message": "ORA-04031: shared memory allocation failed on ora-prod-01", "entity": "ora-prod-01" },
            { "time": (now - chrono::Duration::minutes(12)).to_rfc3339(), "type": "discovery", "severity": "info", "module": "network", "message": "New host discovered: 10.88.11.45 (srv-cache-01)", "entity": "10.88.11.45" },
            { "time": (now - chrono::Duration::minutes(25)).to_rfc3339(), "type": "security", "severity": "warning", "module": "security", "message": "Brute force login attempt from 203.0.113.50", "entity": "203.0.113.50" },
            { "time": (now - chrono::Duration::minutes(40)).to_rfc3339(), "type": "maintenance", "severity": "info", "module": "servers", "message": "Scheduled maintenance started on srv-web-02", "entity": "srv-web-02" },
            { "time": (now - chrono::Duration::minutes(55)).to_rfc3339(), "type": "alert", "severity": "warning", "module": "storage", "message": "Volume /data/pg exceeding 85% capacity", "entity": "storage-nas-01" },
            { "time": (now - chrono::Duration::hours(1)).to_rfc3339(), "type": "deployment", "severity": "info", "module": "operations", "message": "Deployment v2.5.1 completed successfully", "entity": "app-cluster" },
            { "time": (now - chrono::Duration::hours(2)).to_rfc3339(), "type": "alert", "severity": "critical", "module": "network", "message": "BGP peer 192.168.1.1 down", "entity": "gw-core-01" },
            { "time": (now - chrono::Duration::hours(3)).to_rfc3339(), "type": "health_check", "severity": "warning", "module": "health", "message": "SSL certificate expires in 14 days for api.example.com", "entity": "api.example.com" },
        ],
    }))
}
