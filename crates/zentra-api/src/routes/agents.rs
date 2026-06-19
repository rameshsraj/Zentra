use axum::{routing::{delete, get, post}, Router};

use crate::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agents/register", post(handlers::agents::register_agent))
        .route("/api/agents/heartbeat", post(handlers::agents::heartbeat))
        .route("/api/agents/targets", get(handlers::agents::agent_targets))
        .route("/api/agents", get(handlers::agents::list_agents))
        .route("/api/agents/:agent_id", delete(handlers::agents::deregister_agent))
}