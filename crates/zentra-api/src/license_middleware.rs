use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use zentra_core::crypto::license_verifier::{check_license_active, load_and_verify_license};

/// Axum Middleware to validate license integrity and limits dynamically on production builds.
/// In local development or with licensing disabled, it passes through.
pub async fn enforce_license_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if check_license_active() {
        // Parse and verify standard OS-specific system license json
        let license = match load_and_verify_license() {
            Ok(lic) => lic,
            Err(e) => {
                tracing::error!(target: "license_middleware", "License verification failed: {}", e);
                return Err(StatusCode::FORBIDDEN);
            }
        };

        // Enforce route-based premium feature gates
        let path = request.uri().path();
        
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
