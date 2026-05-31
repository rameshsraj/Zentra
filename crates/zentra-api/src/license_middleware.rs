use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use zentra_core::crypto::license_verifier::{check_license_active, is_trial_expired, load_and_verify_license};

/// Routes that remain accessible even after trial expiry.
/// These allow the user to log in, view the dashboard, and see the license status.
/// The Command Center (Dashboard) is the only UI available in degraded mode,
/// so all API endpoints it calls must be allowed here.
const TRIAL_ALLOWED_PREFIXES: &[&str] = &[
    "/api/health",
    "/api/auth/",
    "/api/system/license-status",
    "/api/system/status",
    "/api/system/db",
    "/api/config/mode",
    "/metrics",
    // Dashboard (Command Center) data endpoints — read-only
    "/api/alerts",
    "/api/targets",
    "/api/events",
    "/api/network/capture",
    "/api/network/traffic",
    "/api/network/protocols",
    "/api/ai/insights",
    "/api/predictions/summary",
    "/api/dashboard/",
];

/// Check if a path is allowed in trial-expired (degraded) mode.
fn is_allowed_in_degraded_mode(path: &str) -> bool {
    // Exact matches
    if path == "/api/health" || path == "/api/auth/me" {
        return true;
    }
    // Prefix matches
    for prefix in TRIAL_ALLOWED_PREFIXES {
        if path.starts_with(prefix) {
            return true;
        }
    }
    false
}

/// Axum Middleware to validate license integrity and limits dynamically on production builds.
/// When trial is expired, allows only health/auth/dashboard endpoints (degraded mode).
#[cfg(feature = "prod-license")]
pub async fn enforce_license_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();

    // --- Trial expiry check (graceful degradation) ---
    if is_trial_expired() {
        if is_allowed_in_degraded_mode(&path) {
            return Ok(next.run(request).await);
        }
        tracing::warn!(target: "license_middleware", path = %path, "Trial expired — blocked in degraded mode");
        let body = serde_json::json!({
            "error": "trial_expired",
            "message": "Your trial period has expired. Only the Dashboard is available. Contact the Zentra team for a production license."
        });
        return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
    }

    // --- Standard license verification (prod-license feature) ---
    if check_license_active() {
        let license = match load_and_verify_license() {
            Ok(lic) => lic,
            Err(e) => {
                tracing::error!(target: "license_middleware", "License verification failed: {}", e);
                return Err(StatusCode::FORBIDDEN);
            }
        };

        // 1. Oracle Feature Gate
        if path.starts_with("/api/v1/oracle") && !license.features.enable_oracle {
            tracing::warn!(target: "license_middleware", "Access denied: Oracle premium features are not enabled by your license.");
            return Err(StatusCode::FORBIDDEN);
        }

        // 2. eBPF / Cilium Network Feature Gate
        if path.starts_with("/api/v1/cilium") && !license.features.enable_ebpf_network {
            tracing::warn!(target: "license_middleware", "Access denied: Network eBPF features are not enabled by your license.");
            return Err(StatusCode::FORBIDDEN);
        }

        // 3. AI / LLM Observability Gate
        if path.starts_with("/api/v1/ai-observability") && !license.features.enable_ai_observability {
            tracing::warn!(target: "license_middleware", "Access denied: AI Observability features are not enabled by your license.");
            return Err(StatusCode::FORBIDDEN);
        }

        // 4. OpenTelemetry (OTel) Gate
        if (path.starts_with("/api/v1/traces") || path.starts_with("/api/v1/metrics") || path.starts_with("/api/v1/logs") || path.starts_with("/api/v1/otel")) 
            && !license.features.enable_otel 
        {
            tracing::warn!(target: "license_middleware", "Access denied: OpenTelemetry ingestion/analytics features are not enabled by your license.");
            return Err(StatusCode::FORBIDDEN);
        }
    }

    Ok(next.run(request).await)
}

/// Axum Middleware (non-prod builds). Still enforces trial expiry degradation
/// via the ZENTRA_TRIAL_EXPIRED env var set by docker-entrypoint.sh.
#[cfg(not(feature = "prod-license"))]
pub async fn enforce_license_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();

    if is_trial_expired() {
        if is_allowed_in_degraded_mode(&path) {
            return Ok(next.run(request).await);
        }
        tracing::warn!(target: "license_middleware", path = %path, "Trial expired — blocked in degraded mode");
        let body = serde_json::json!({
            "error": "trial_expired",
            "message": "Your trial period has expired. Only the Dashboard is available. Contact the Zentra team for a production license."
        });
        return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
    }

    Ok(next.run(request).await)
}
