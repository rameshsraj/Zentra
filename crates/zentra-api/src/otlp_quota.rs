//! OT03: OTLP quota enforcement — rate_limit_rpm and max_payload_bytes.
//!
//! Tower middleware that wraps OTLP HTTP routes. Extracts the API key,
//! resolves per-key limits, and enforces them before the request body
//! is parsed. Uses a DashMap-based sliding-window counter for per-key RPM.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use dashmap::DashMap;
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

use crate::state::AppState;
use crate::handlers::otlp::validate_otlp_key;

/// Per-key RPM counter: key_hash → (request_count, window_start).
pub type RpmCounters = Arc<DashMap<String, (u64, std::time::Instant)>>;

/// Create a new RPM counter map.
pub fn new_rpm_counters() -> RpmCounters {
    Arc::new(DashMap::new())
}

/// OT03 middleware: enforce per-key rate_limit_rpm and max_payload_bytes.
///
/// Applies only to OTLP ingest paths (/v1/traces, /v1/metrics, /v1/logs, /v1/profiles).
pub async fn otlp_quota_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();

    // Only apply to OTLP ingest endpoints
    if !path.starts_with("/v1/") {
        return next.run(req).await;
    }

    let headers = req.headers().clone();

    // Extract API key from Authorization header
    let api_key = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    if let Some(ref key) = api_key {
        // Validate key and get limits
        if let Ok((org_id, _perms)) = validate_otlp_key(&state, key).await {
            // Insert AuthContext so downstream OrgId extractor resolves correctly.
            // OTLP requests authenticate via isok_ keys, not JWT, but handlers
            // still rely on the AuthContext extension for tenancy scoping.
            req.extensions_mut().insert(crate::auth_middleware::AuthContext {
                user_id: uuid::Uuid::nil(),
                org_id,
                email: String::new(),
                role: "otlp-agent".to_string(),
                jti: String::new(),
            });
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(key.as_bytes());
            let key_hash = format!("{:x}", hasher.finalize());
            let key_prefix = &key[..13.min(key.len())];

            // Check max_payload_bytes from content-length header
            if let Some(cl) = headers.get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<i64>().ok())
            {
                let max_bytes = get_key_limit(&state, &key_hash, "max_payload_bytes")
                    .await
                    .unwrap_or(5_242_880);
                if cl > max_bytes as i64 {
                    metrics::counter!("zentra_otlp_payload_oversize_total").increment(1);
                    warn!(key_prefix = key_prefix, content_length = cl, max = max_bytes, "Payload too large");
                    return (
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(json!({"error": "Payload exceeds max_payload_bytes limit"})),
                    ).into_response();
                }
            }

            // Rate limit check
            let rpm = get_key_limit(&state, &key_hash, "rate_limit_rpm")
                .await
                .unwrap_or(10_000);
            if !check_rate_limit(&state.otlp_rpm_counters, &key_hash, rpm) {
                metrics::counter!("zentra_otlp_rate_limited_total").increment(1);
                warn!(key_prefix = key_prefix, rpm_limit = rpm, "Rate limited");
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": "Rate limit exceeded (rate_limit_rpm)"})),
                ).into_response();
            }
        }
        // Invalid key — let the downstream handler deal with auth
    }

    next.run(req).await
}

/// Simple per-minute rate limit check using DashMap counters.
fn check_rate_limit(counters: &RpmCounters, key_hash: &str, max_rpm: i32) -> bool {
    let now = std::time::Instant::now();
    let mut entry = counters.entry(key_hash.to_string()).or_insert((0, now));
    let (count, window_start) = entry.value_mut();

    // Reset counter if window expired (60s)
    if now.duration_since(*window_start).as_secs() >= 60 {
        *count = 0;
        *window_start = now;
    }

    *count += 1;
    *count <= max_rpm as u64
}

async fn get_key_limit(state: &AppState, key_hash: &str, column: &str) -> Option<i32> {
    let sql = format!(
        "SELECT {} FROM otlp_api_keys WHERE key_hash = $1 AND enabled = true",
        column
    );
    sqlx::query_scalar::<_, i32>(&sql)
        .bind(key_hash)
        .fetch_optional(&state.tsdb_pool)
        .await
        .ok()?
}
