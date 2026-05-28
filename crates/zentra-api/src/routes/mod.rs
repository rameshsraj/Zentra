mod health;
mod auth;
mod targets;
mod metrics;
mod logs;
mod alerts;
mod dashboards;
mod network;
mod oracle;
mod databases;
mod security;
mod middleware_svc;
mod admin;
mod infra;
mod enterprise;
mod monitoring;
mod ai;
mod otlp;

use axum::Router;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use crate::state::AppState;

/// Middleware that rewrites `/api/v1/...` requests to `/api/...` so that
/// both versioned and unversioned paths resolve to the same handlers.
///
/// - `/api/health`    → works (existing routes)
/// - `/api/v1/health` → rewritten to `/api/health`, also works
///
/// Future breaking changes can be introduced under `/api/v2/` with a
/// separate router.
async fn rewrite_v1(mut req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if let Some(rest) = path.strip_prefix("/api/v1") {
        let new_path = format!("/api{}", rest);
        let mut parts = req.uri().clone().into_parts();
        parts.path_and_query = Some(
            if let Some(q) = req.uri().query() {
                format!("{}?{}", new_path, q).parse().unwrap()
            } else {
                new_path.parse().unwrap()
            },
        );
        *req.uri_mut() = axum::http::Uri::from_parts(parts).unwrap();
    }
    next.run(req).await
}

/// Assembles the full API router by merging all per-domain sub-routers.
///
/// Each sub-module exposes a `router() -> Router<AppState>` function that
/// defines only its own routes. This keeps every domain under ~60 lines
/// instead of one 919-line monolith.
///
/// API versioning: `/api/v1/*` is rewritten to `/api/*` via middleware,
/// so both paths return identical responses. The frontend still uses `/api/`.
pub fn build_router(state: AppState) -> Router {
    let router = Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(targets::router())
        .merge(metrics::router())
        .merge(logs::router())
        .merge(alerts::router())
        .merge(dashboards::router())
        .merge(network::router())
        .merge(oracle::router())
        .merge(databases::router())
        .merge(security::router())
        .merge(middleware_svc::router())
        .merge(admin::router())
        .merge(infra::router())
        .merge(enterprise::router())
        .merge(monitoring::router())
        .merge(ai::router())
        .merge(otlp::router());

    #[cfg(feature = "prod-license")]
    let router = router.layer(middleware::from_fn(crate::license_middleware::enforce_license_middleware));

    router
        .layer(middleware::from_fn(rewrite_v1))
        .with_state(state)
}
