use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Authentication
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/auth/refresh", post(handlers::auth::refresh))
        .route("/api/auth/me", get(handlers::auth::me))
        .route("/api/auth/profile", put(handlers::auth::update_profile))
        .route("/api/auth/mfa/setup", post(handlers::auth::mfa_setup))
        .route("/api/auth/mfa/verify", post(handlers::auth::mfa_verify))
        .route("/api/auth/sessions", get(handlers::auth::list_sessions))
        .route("/api/auth/sessions/:id", delete(handlers::auth::revoke_session))
        .route("/api/auth/change-password", post(handlers::auth::change_password))
}
