use axum::http::{HeaderName, HeaderValue, Method};
use axum::middleware::{self, Next};
use axum::response::Response;
use zentra_api::auth_middleware::require_auth;
use zentra_api::routes::build_router;
use zentra_api::state::AppState;
use zentra_db::pool::{create_pool, create_pool_with_kind, PoolKind};
use zentra_stream::EventBus;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "x-request-id";
const DEFAULT_INSECURE_JWT: &str = "Zentra-default-jwt-secret-change-me";

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Build layered tracing subscriber:
    //   Layer 1: fmt (stdout/stderr logs)
    //   Layer 2: OpenTelemetry (sends spans to OTLP endpoint for self-monitoring)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true);

    let otel_layer = zentra_core::telemetry::init_telemetry("zentra-api");

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    info!("Zentra Server starting...");

    // ============================================================
    // Fail-loud security pre-flight
    // ============================================================
    let jwt_secret = std::env::var("ZENTRA_JWT_SECRET").unwrap_or_default();
    let allow_insecure = std::env::var("ZENTRA_ALLOW_INSECURE_JWT").ok().as_deref() == Some("1");
    if jwt_secret.is_empty() || jwt_secret == DEFAULT_INSECURE_JWT {
        if !allow_insecure {
            eprintln!(
                "FATAL: ZENTRA_JWT_SECRET is unset or using the default value.\n\
                 Set a strong secret (>= 32 bytes) before starting the server.\n\
                 To bypass for local development only, export ZENTRA_ALLOW_INSECURE_JWT=1."
            );
            std::process::exit(2);
        } else {
            warn!("ZENTRA_JWT_SECRET is insecure — ZENTRA_ALLOW_INSECURE_JWT=1 is set. DO NOT use in production.");
        }
    }
    if jwt_secret.len() < 32 && !allow_insecure {
        eprintln!("FATAL: ZENTRA_JWT_SECRET must be at least 32 bytes (got {}).", jwt_secret.len());
        std::process::exit(2);
    }

    // ZENTRA_MASTER_KEY — AES-256-GCM key for credential encryption at rest.
    let master_key = std::env::var("ZENTRA_MASTER_KEY").unwrap_or_default();
    if master_key.is_empty() && !allow_insecure {
        eprintln!(
            "FATAL: ZENTRA_MASTER_KEY is unset.\n\
             Set a 64-character hex string (32 bytes) for credential encryption.\n\
             To bypass for local development only, export ZENTRA_ALLOW_INSECURE_JWT=1."
        );
        std::process::exit(2);
    } else if master_key.is_empty() {
        warn!("ZENTRA_MASTER_KEY is unset — using insecure dev key. DO NOT use in production.");
    }

    let config = zentra_core::AppConfig::from_env();

    let pool = create_pool(&config.database_url)
        .await
        .expect("Failed to create meta database pool");

    let tsdb_pool = create_pool_with_kind(&config.database_url, PoolKind::Tsdb)
        .await
        .expect("Failed to create TSDB database pool");

    info!("Two PgPools created: meta (5s timeout, 20 conns) + tsdb (60s timeout, 10 conns)");

    // ============================================================
    // Run database migrations (idempotent — all use IF NOT EXISTS)
    // ============================================================
    match sqlx::migrate!("../../migrations").run(&pool).await {
        Ok(_) => info!("Database migrations applied successfully"),
        Err(e) => warn!("Migration runner returned error (tables may already exist): {e}"),
    }

    // ============================================================
    // Prometheus self-monitoring — install recorder before any metrics are emitted
    // ============================================================
    zentra_api::observability::init_metrics();
    info!("Prometheus metrics recorder installed — scrape at /metrics");

    let event_bus = EventBus::new();
    let state = AppState::new(pool, tsdb_pool, event_bus, config.org_id, jwt_secret.clone());

    // ============================================================
    // Start Background Jobs (Automation)
    // ============================================================
    zentra_api::background_jobs::start_background_jobs(state.clone());
    info!("Background jobs started (RED metrics refresh, alert evaluation)");

    // ============================================================
    // Reload middleware instances from DB into in-memory stores
    // ============================================================
    zentra_api::handlers::targets::load_middleware_from_db(&state.pool, config.org_id).await;

    // ============================================================
    // Background pollers for persistent metric collection
    // ============================================================
    zentra_api::handlers::storage_arrays::start_storage_poller(state.clone());
    zentra_api::handlers::metric_collector::start_metric_collector(state.clone());
    info!("Background metric collectors started (servers, network, PostgreSQL, Oracle, middleware, healthchecks, storage — every 5 min)");

    // Register Oracle connections as pools for the scrape scheduler
    {
        let conns = zentra_api::handlers::oracle::oracle_connections().read();
        for conn in conns.iter().filter(|c| c.is_active) {
            let cfg = database_plugin::oracle_connection::OracleConnConfig {
                id: conn.id.clone(),
                name: conn.name.clone(),
                host: conn.host.clone(),
                port: conn.port as u16,
                service_name: conn.service_name.clone(),
                username: conn.username.clone(),
                auth_mode: database_plugin::oracle_connection::AuthMode::Password,
                password: Some(conn.password.clone()),
                role: database_plugin::oracle_connection::OracleRole::Default,
                tns_admin_path: None,
                pool_min: conn.pool_min,
                pool_max: conn.pool_max,
                pool_increment: conn.pool_increment,
                conn_max_lifetime: std::time::Duration::from_secs(conn.conn_max_lifetime_secs as u64),
                query_timeout: std::time::Duration::from_secs(conn.query_timeout_secs as u64),
                labels: std::collections::HashMap::new(),
                ssh_user: None,
            };
            match database_plugin::oracle_connection::register_pool(cfg) {
                Ok(_) => info!(id = %conn.id, "Oracle pool registered for scrape scheduler"),
                Err(e) => warn!(id = %conn.id, err = %e.message, "Failed to register Oracle pool"),
            }
        }
    }
    database_plugin::oracle_scheduler::start_scheduler(None);
    info!("Oracle scrape scheduler started");

    // ============================================================
    // CORS — env-driven allow-list (no Any/Any/Any)
    // ============================================================
    let allow_origins_env = std::env::var("ZENTRA_ALLOW_ORIGINS").unwrap_or_default();
    let cors = if allow_origins_env.trim() == "*" {
        // Explicit wildcard request — allowed only without credentials. Logged loudly.
        warn!("CORS allow-origin is wildcard. Cookies / Authorization will not be reflected.");
        CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH, Method::OPTIONS])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                axum::http::header::ACCEPT,
                HeaderName::from_static(REQUEST_ID_HEADER),
            ])
    } else {
        let origins: Vec<HeaderValue> = allow_origins_env
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .filter_map(|s| HeaderValue::from_str(s).ok())
            .collect();
        if origins.is_empty() {
            warn!("ZENTRA_ALLOW_ORIGINS is empty — same-origin only. Set e.g. ZENTRA_ALLOW_ORIGINS=http://10.88.11.162:6010");
        } else {
            info!(origins = ?origins, "CORS allow-list");
        }
        CorsLayer::new()
            .allow_origin(origins)
            .allow_credentials(true)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH, Method::OPTIONS])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
                axum::http::header::ACCEPT,
                HeaderName::from_static(REQUEST_ID_HEADER),
            ])
            .expose_headers([HeaderName::from_static(REQUEST_ID_HEADER)])
    };

    // ============================================================
    // Rate limit — ~62 req/s per IP, burst 120.
    // tower_governor's per_millisecond(n) means "1 token per n ms".
    // 16 ms → ~62 req/s steady-state. burst_size is the initial bucket depth.
    // ============================================================
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(16)
            .burst_size(120)
            .finish()
            .expect("invalid governor config"),
    );
    let governor_layer = GovernorLayer { config: governor_conf };

    // ============================================================
    // gRPC server for OTLP (port 4317) — feature-gated
    // ============================================================
    #[cfg(feature = "grpc")]
    {
        let grpc_state = state.clone();
        tokio::spawn(async move {
            zentra_api::grpc::start_grpc_server(grpc_state).await;
        });
        info!("OTLP gRPC server spawned (default port 4317)");
    }

    // ============================================================
    // Build the layered app
    // ============================================================
    let app = build_router(state.clone())
        // outer → inner
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(middleware::from_fn(zentra_api::observability::track_request))
        .layer(middleware::from_fn(zentra_api::otel_middleware::otel_trace_middleware))
        .layer(middleware::from_fn_with_state(state.clone(), zentra_api::otlp_quota::otlp_quota_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(middleware::from_fn_with_state(state, require_auth))
        .layer(governor_layer)
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(60),
        ))
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024)) // 2 MB
        .layer(CompressionLayer::new());

    let bind_addr: std::net::Ipv4Addr = std::env::var("ZENTRA_BIND_ADDR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(std::net::Ipv4Addr::UNSPECIFIED);
    let addr = SocketAddr::from((bind_addr, config.api_port));
    info!(bind = %bind_addr, port = config.api_port, "Zentra API listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("Server error");
}

/// Security response headers — defence-in-depth on all API responses.
/// These supplement Nginx's headers for any path that reaches the backend directly.
async fn security_headers_middleware(
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.entry("cache-control").or_insert(HeaderValue::from_static("no-store"));
    h.entry("x-content-type-options").or_insert(HeaderValue::from_static("nosniff"));
    h.entry("x-frame-options").or_insert(HeaderValue::from_static("DENY"));
    h.entry("referrer-policy").or_insert(HeaderValue::from_static("strict-origin-when-cross-origin"));
    h.entry("x-xss-protection").or_insert(HeaderValue::from_static("0"));
    h.entry("permissions-policy").or_insert(HeaderValue::from_static("camera=(), microphone=(), geolocation=()"));
    h.entry("cross-origin-opener-policy").or_insert(HeaderValue::from_static("same-origin"));
    // Strip server identity headers that upstream libraries might add
    h.remove("server");
    h.remove("x-powered-by");
    resp
}

/// Inject `x-request-id` (uuid v7-ish via uuid::new_v4) for any request that doesn't have one.
async fn request_id_middleware(
    mut req: axum::extract::Request,
    next: Next,
) -> Response {
    let req_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let header_val = HeaderValue::from_str(&req_id)
        .unwrap_or_else(|_| HeaderValue::from_static("invalid"));

    req.headers_mut().insert(HeaderName::from_static(REQUEST_ID_HEADER), header_val.clone());

    let mut resp = next.run(req).await;
    resp.headers_mut().insert(HeaderName::from_static(REQUEST_ID_HEADER), header_val);
    resp
}

async fn shutdown_signal() {
    zentra_core::telemetry::shutdown_signal().await;
    info!("Shutdown signal received — draining in-flight requests");
}
