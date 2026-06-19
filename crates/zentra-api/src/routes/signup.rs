use axum::{routing::{get, post}, Router};
use crate::state::AppState;
use crate::handlers::signup;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/signup", post(signup::signup))
        .route("/api/signup/verify-email", post(signup::verify_email))
        .route("/api/onboarding/checklist", get(signup::onboarding_checklist))
}
