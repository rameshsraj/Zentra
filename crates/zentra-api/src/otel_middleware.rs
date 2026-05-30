//! OTel Context Propagation Middleware
//!
//! Extracts W3C `traceparent` / `tracestate` headers from incoming HTTP requests
//! and injects them into the current tracing span so distributed traces flow
//! through Zentra end-to-end.
//!
//! Also creates a root span for each HTTP request with standard OTel HTTP
//! semantic conventions (method, route, status_code, etc.).

use axum::{extract::Request, middleware::Next, response::Response};
use tracing::Instrument;

/// Axum middleware that:
/// 1. Creates a tracing span with OTel HTTP semantic conventions
/// 2. Runs the request inside that span (async-safe via `.instrument()`)
/// 3. Records response status code on the span
///
/// W3C `traceparent` propagation is handled automatically by
/// `tracing-opentelemetry` + `tower_http::trace::TraceLayer`.
pub async fn otel_trace_middleware(req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let version = format!("{:?}", req.version());

    // Item 10: Extract semantic convention attributes
    let user_agent = req.headers()
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let host = req.headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let span = tracing::info_span!(
        "http.request",
        http.request.method = %method,
        http.route = %path,
        url.path = %path,
        http.response.status_code = tracing::field::Empty,
        network.protocol.version = %version,
        server.address = %host,
        user_agent.original = %user_agent,
        otel.kind = "server",
        otel.status_code = tracing::field::Empty,
    );

    // Run the downstream handler inside the span (async-safe)
    let response = next.run(req).instrument(span.clone()).await;

    // Record response status on the span
    let status = response.status().as_u16();
    span.record("http.response.status_code", status);
    if status >= 500 {
        span.record("otel.status_code", "ERROR");
    } else {
        span.record("otel.status_code", "OK");
    }

    response
}
