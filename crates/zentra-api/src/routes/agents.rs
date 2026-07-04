use axum::{routing::{delete, get, patch, post}, Router};

use crate::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agents/register", post(handlers::agents::register_agent))
        .route("/api/agents/heartbeat", post(handlers::agents::heartbeat))
        .route("/api/agents/by-dc/:dc_name/logs", get(handlers::agents::agent_logs_by_dc))
        .route("/api/agents/targets", get(handlers::agents::agent_targets))
        .route("/api/agents", get(handlers::agents::list_agents))
        .route("/api/agents/:agent_id", delete(handlers::agents::deregister_agent))
        .route("/api/agents/:agent_id/discovered", post(handlers::agents::report_discovered))
        .route("/api/agents/:agent_id/discovered", get(handlers::agents::list_discovered))
        .route("/api/agents/:agent_id/discovered/:host_id", patch(handlers::agents::update_discovered_host))
        .route("/api/agent-keys", get(handlers::agents::list_agent_keys))
        .route("/api/agent-keys", post(handlers::agents::create_agent_key))
        .route("/api/agent-keys/:key_id", delete(handlers::agents::revoke_agent_key))
}