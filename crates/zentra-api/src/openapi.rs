//! OpenAPI 3.0 specification generated via utoipa.
//!
//! The `ApiDoc` struct is the single source of truth for the spec.
//! It is served at `/api/openapi.json` alongside the Swagger UI.

use utoipa::{Modify, OpenApi};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

// ─────────────────────────────────────────────────────────────────────────────
// Inline schemas used only in this spec (without cluttering handler structs)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct HealthResponse {
    status: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct LoginResponse {
    token: String,
    user_id: String,
    org_id: String,
    role: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct ModeResponse {
    mode: String,
    demo: bool,
    org_id: String,
    description: String,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct SetModeRequest {
    /// "demo" or "live"
    mode: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Path stubs — utoipa requires path items to reference a Rust fn.
// We use free functions here so existing handlers don't need annotation.
// ─────────────────────────────────────────────────────────────────────────────

/// GET /health — liveness probe
#[utoipa::path(
    get,
    path = "/health",
    tag = "Health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse),
        (status = 503, description = "Service is not ready"),
    )
)]
#[allow(dead_code)]
async fn health() {}

/// GET /ready — readiness probe
#[utoipa::path(
    get,
    path = "/ready",
    tag = "Health",
    responses(
        (status = 200, description = "Service is ready"),
        (status = 503, description = "Service is not ready"),
    )
)]
#[allow(dead_code)]
async fn ready() {}

/// POST /api/auth/login — exchange credentials for JWT
#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "Auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 429, description = "Too many login attempts"),
    )
)]
#[allow(dead_code)]
async fn login() {}

/// GET /api/auth/me — return authenticated user profile
#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "Auth",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "User profile"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn me() {}

/// POST /api/auth/logout — invalidate session
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "Auth",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Logged out"),
    )
)]
#[allow(dead_code)]
async fn logout() {}

/// GET /api/config/mode — current demo/live mode for the authenticated org
#[utoipa::path(
    get,
    path = "/api/config/mode",
    tag = "Config",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Current mode", body = ModeResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn get_mode() {}

/// POST /api/config/mode — switch demo/live mode for the authenticated org
#[utoipa::path(
    post,
    path = "/api/config/mode",
    tag = "Config",
    request_body = SetModeRequest,
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Mode updated", body = ModeResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn set_mode() {}

/// GET /api/targets — list monitored targets
#[utoipa::path(
    get,
    path = "/api/targets",
    tag = "Targets",
    security(("bearerAuth" = [])),
    params(
        ("page" = Option<i64>, Query, description = "Page number (0-indexed)"),
        ("page_size" = Option<i64>, Query, description = "Items per page (max 200)"),
        ("target_type" = Option<String>, Query, description = "Filter by type: server|database|network|storage"),
        ("search" = Option<String>, Query, description = "Full-text search"),
    ),
    responses(
        (status = 200, description = "Paginated target list"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn list_targets() {}

/// GET /api/alerts — list active alerts
#[utoipa::path(
    get,
    path = "/api/alerts",
    tag = "Alerts",
    security(("bearerAuth" = [])),
    params(
        ("severity" = Option<String>, Query, description = "critical|warning|info"),
        ("state" = Option<String>, Query, description = "firing|pending|resolved"),
        ("limit" = Option<i64>, Query, description = "Max results (default 100)"),
    ),
    responses(
        (status = 200, description = "Alert list"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn list_alerts() {}

/// GET /api/alerts/rules — list alert rules
#[utoipa::path(
    get,
    path = "/api/alerts/rules",
    tag = "Alerts",
    security(("bearerAuth" = [])),
    responses(
        (status = 200, description = "Alert rule list"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn list_alert_rules() {}

/// GET /api/metrics/query — instant metric query
#[utoipa::path(
    get,
    path = "/api/metrics/query",
    tag = "Metrics",
    security(("bearerAuth" = [])),
    params(
        ("target_id" = Option<String>, Query, description = "Target UUID"),
        ("metric_name" = Option<String>, Query, description = "Exact metric name"),
        ("from" = Option<String>, Query, description = "ISO-8601 start time"),
        ("to" = Option<String>, Query, description = "ISO-8601 end time"),
    ),
    responses(
        (status = 200, description = "Metric data points"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn query_metrics() {}

/// GET /api/dashboards — list dashboards
#[utoipa::path(
    get,
    path = "/api/dashboards",
    tag = "Dashboards",
    security(("bearerAuth" = [])),
    params(
        ("folder" = Option<String>, Query, description = "Filter by folder name"),
        ("search" = Option<String>, Query, description = "Search by title"),
    ),
    responses(
        (status = 200, description = "Dashboard list"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn list_dashboards() {}

/// GET /api/logs/search — search log entries
#[utoipa::path(
    get,
    path = "/api/logs/search",
    tag = "Logs",
    security(("bearerAuth" = [])),
    params(
        ("query" = Option<String>, Query, description = "Full-text search query"),
        ("target_id" = Option<String>, Query, description = "Filter by target UUID"),
        ("level" = Option<String>, Query, description = "error|warn|info|debug"),
        ("from" = Option<String>, Query, description = "ISO-8601 start time"),
        ("to" = Option<String>, Query, description = "ISO-8601 end time"),
        ("limit" = Option<i64>, Query, description = "Max results (default 100)"),
    ),
    responses(
        (status = 200, description = "Log entries"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn search_logs() {}

/// GET /api/anomaly/list — list detected anomalies
#[utoipa::path(
    get,
    path = "/api/anomaly/list",
    tag = "AI / ML",
    security(("bearerAuth" = [])),
    params(
        ("target_id" = Option<String>, Query, description = "Filter by target UUID"),
        ("severity" = Option<String>, Query, description = "critical|warning|info"),
        ("detector" = Option<String>, Query, description = "zscore|ewma|seasonal"),
        ("limit" = Option<i64>, Query, description = "Max results (default 100)"),
    ),
    responses(
        (status = 200, description = "Anomaly list"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn list_anomalies() {}

/// GET /api/netflow/top-talkers — top bandwidth consumers
#[utoipa::path(
    get,
    path = "/api/netflow/top-talkers",
    tag = "Network",
    security(("bearerAuth" = [])),
    params(
        ("limit" = Option<i64>, Query, description = "Number of results (default 20)"),
        ("protocol" = Option<String>, Query, description = "Filter by protocol name"),
    ),
    responses(
        (status = 200, description = "Top talker list"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    )
)]
#[allow(dead_code)]
async fn top_talkers() {}

// ─────────────────────────────────────────────────────────────────────────────
// Security modifier — utoipa 4.x uses Modify trait for security schemes
// ─────────────────────────────────────────────────────────────────────────────

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ApiDoc — the root OpenAPI document
// ─────────────────────────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zentra API",
        version = "1.0.0",
        description = "Unified infrastructure monitoring, observability, and network analysis platform.\n\nAll endpoints except `/health`, `/ready`, and `/api/auth/login` require `Authorization: Bearer <jwt>` header.",
        contact(name = "Zentra Team"),
        license(name = "Proprietary"),
    ),
    servers(
        (url = "/", description = "Current host")
    ),
    tags(
        (name = "Health",     description = "Liveness and readiness probes"),
        (name = "Auth",       description = "Authentication — login, logout, user profile"),
        (name = "Config",     description = "Runtime mode and configuration"),
        (name = "Targets",    description = "Monitored target management"),
        (name = "Metrics",    description = "Metric ingestion and query"),
        (name = "Alerts",     description = "Alert rules, firing alerts, silences"),
        (name = "Dashboards", description = "Dashboard CRUD and playlist management"),
        (name = "Logs",       description = "Log search and structured analytics"),
        (name = "Network",    description = "NetFlow, topology, and deep packet analysis"),
        (name = "AI / ML",    description = "Anomaly detection, RCA, predictive insights"),
    ),
    paths(
        health, ready,
        login, me, logout,
        get_mode, set_mode,
        list_targets,
        list_alerts, list_alert_rules,
        query_metrics,
        list_dashboards,
        search_logs,
        list_anomalies,
        top_talkers,
    ),
    components(
        schemas(
            HealthResponse, LoginRequest, LoginResponse,
            ErrorResponse, ModeResponse, SetModeRequest,
        ),
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// Return the generated OpenAPI spec as a JSON value.
pub fn spec_json() -> serde_json::Value {
    let doc = ApiDoc::openapi();
    serde_json::to_value(doc).unwrap_or_else(|_| serde_json::json!({"error": "spec generation failed"}))
}
