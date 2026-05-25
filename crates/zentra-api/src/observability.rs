//! Prometheus metrics exporter and HTTP middleware.
//!
//! Call `init_metrics()` once at startup (before building the router).
//! The `/metrics` route calls `metrics_handler()` to render the current state.
//! The `track_request` middleware records per-request counters + histograms.

use axum::{extract::Request, http::header, middleware::Next, response::{IntoResponse, Response}};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use std::time::Instant;

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the global Prometheus recorder. Idempotent — second call is a no-op.
pub fn init_metrics() {
    HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .install_recorder()
            .expect("Failed to install Prometheus metrics recorder")
    });
}

/// GET /metrics — render all registered metrics in Prometheus text format.
pub async fn metrics_handler() -> impl IntoResponse {
    let body = HANDLE
        .get()
        .map(|h| h.render())
        .unwrap_or_else(|| "# metrics recorder not initialised\n".to_string());

    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")], body)
}

/// Tower-compatible axum middleware: record HTTP request counter + latency histogram.
/// Path is normalised to `/api/<domain>` to prevent UUID-driven cardinality explosion.
pub async fn track_request(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = normalise_path(req.uri().path());
    let start = Instant::now();

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    metrics::counter!(
        "zentra_http_requests_total",
        "method" => method.clone(),
        "path"   => path.clone(),
        "status" => status,
    )
    .increment(1);

    metrics::histogram!(
        "zentra_http_request_duration_seconds",
        "method" => method,
        "path"   => path,
    )
    .record(elapsed);

    response
}

/// Truncate a path to its first three segments, e.g.:
/// `/api/targets/some-uuid/details` → `/api/targets`
/// `/health` → `/health`
fn normalise_path(path: &str) -> String {
    let parts: Vec<&str> = path.splitn(5, '/').take(4).collect();
    if parts.len() <= 2 {
        path.to_string()
    } else {
        parts.join("/")
    }
}
