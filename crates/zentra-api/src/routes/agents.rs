use axum::{routing::{delete, get, patch, post, put}, Router};

use crate::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agents/register", post(handlers::agents::register_agent))
        .route("/api/agents/heartbeat", post(handlers::agents::heartbeat))
        .route("/api/agent/ingest/heartbeat", post(handlers::agents::heartbeat))
        .route("/api/agents/by-dc/:dc_name/logs", get(handlers::agents::agent_logs_by_dc))
        .route("/api/agents/by-dc/:dc_name/components", post(handlers::agents::report_components))
        .route("/api/agents/by-dc/:dc_name/components", get(handlers::agents::list_components))
        .route("/api/agents/components/confirm", post(handlers::agents::confirm_components))
        .route("/api/agents/targets", get(handlers::agents::agent_targets))
        .route("/api/agents/log-dirs", get(handlers::agents::agent_log_dirs))
        .route("/api/agents", get(handlers::agents::list_agents))
        .route("/api/agents/:agent_id", delete(handlers::agents::deregister_agent))
        .route("/api/agents/:agent_id/discovered", post(handlers::agents::report_discovered))
        .route("/api/agents/:agent_id/discovered", get(handlers::agents::list_discovered))
        .route("/api/agents/:agent_id/discovered/:host_id", patch(handlers::agents::update_discovered_host))
        .route("/api/agents/:agent_id/rotate-key", post(handlers::agents::rotate_agent_key))
        .route("/api/agent-keys", get(handlers::agents::list_agent_keys))
        .route("/api/agent-keys", post(handlers::agents::create_agent_key))
        .route("/api/agent-keys/:key_id", delete(handlers::agents::revoke_agent_key))
        .route("/api/org-enrollment-keys", post(handlers::org_enrollment_keys::create_org_enrollment_key))
        .route("/api/org-enrollment-keys", get(handlers::org_enrollment_keys::list_org_enrollment_keys))
        .route("/api/org-enrollment-keys/:id", put(handlers::org_enrollment_keys::update_org_enrollment_key))
        .route("/api/org-enrollment-keys/:id", delete(handlers::org_enrollment_keys::delete_org_enrollment_key))
        .route("/api/org-enrollment-keys/:id/registrations", get(handlers::org_enrollment_keys::list_org_enrollment_key_registrations))
        .route("/dl/:filename", get(handlers::agents::download_agent))
}