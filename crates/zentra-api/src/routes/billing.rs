use axum::{routing::{get, post}, Router};
use crate::state::AppState;
use crate::handlers::billing;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/billing/subscription", get(billing::get_subscription))
        .route("/api/billing/checkout", post(billing::create_checkout))
        .route("/api/billing/portal", post(billing::create_portal))
        .route("/api/billing/webhook", post(billing::stripe_webhook))
}
