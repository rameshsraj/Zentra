use axum::{
    response::IntoResponse,
    Json,
};
use serde_json::json;
use zentra_core::crypto::license_verifier::{is_trial_expired, load_and_verify_license};

/// API handler returning the current cryptographic license status and menu item permissions
pub async fn get_license_status() -> impl IntoResponse {
    let trial_expired = is_trial_expired();

    // Try loading and parsing production license
    match load_and_verify_license() {
        Ok(payload) => {
            let now = chrono::Utc::now();
            let trial_duration = chrono::Duration::days(payload.trial_duration_days);
            let expiration_time = payload.onboarded_at + trial_duration;
            let days_remaining = (expiration_time - now).num_days().max(0);

            Json(json!({
                "is_licensed": true,
                "trial_expired": trial_expired,
                "customer_name": payload.customer_name,
                "onboarded_at": payload.onboarded_at,
                "trial_duration_days": payload.trial_duration_days,
                "days_remaining": days_remaining,
                "max_targets": payload.max_targets,
                "features": {
                    "enable_oracle": payload.features.enable_oracle,
                    "enable_ebpf_network": payload.features.enable_ebpf_network,
                    "enable_ai_observability": payload.features.enable_ai_observability,
                    "enable_otel": payload.features.enable_otel,
                    "allowed_menu_items": payload.features.allowed_menu_items
                }
            }))
        }
        Err(e) => {
            Json(json!({
                "is_licensed": false,
                "trial_expired": trial_expired,
                "error": e,
                "customer_name": "Unlicensed Installation",
                "onboarded_at": null,
                "trial_duration_days": 0,
                "days_remaining": 0,
                "max_targets": 0,
                "features": {
                    "enable_oracle": false,
                    "enable_ebpf_network": false,
                    "enable_ai_observability": false,
                    "enable_otel": false,
                    "allowed_menu_items": []
                }
            }))
        }
    }
}
