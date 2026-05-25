use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // CIS Compliance Scanner
        .route("/api/compliance/frameworks", get(handlers::compliance::list_frameworks))
        .route("/api/compliance/scans", get(handlers::compliance::list_scans))
        .route("/api/compliance/scans", post(handlers::compliance::trigger_scan))
        .route("/api/compliance/scans/:id", get(handlers::compliance::get_scan))
        .route("/api/compliance/findings", get(handlers::compliance::list_findings))
        .route("/api/compliance/summary", get(handlers::compliance::compliance_summary))

        // SSO / SAML / OIDC / LDAP
        .route("/api/sso/providers", get(handlers::sso::list_providers))
        .route("/api/sso/providers", post(handlers::sso::create_provider))
        .route("/api/sso/providers/:id", get(handlers::sso::get_provider))
        .route("/api/sso/providers/:id", axum::routing::put(handlers::sso::update_provider))
        .route("/api/sso/providers/:id", delete(handlers::sso::delete_provider))
        .route("/api/sso/providers/:id/test", post(handlers::sso::test_provider))
        .route("/api/sso/providers/:id/toggle", post(handlers::sso::toggle_provider))
        .route("/api/sso/sessions", get(handlers::sso::list_sessions))
        .route("/api/sso/summary", get(handlers::sso::sso_summary))
        .route("/api/sso/saml/metadata", get(handlers::sso::saml_metadata))

        // ITSM Integration (ServiceNow, JIRA, PagerDuty)
        .route("/api/itsm/configs", get(handlers::itsm::list_configs))
        .route("/api/itsm/configs", post(handlers::itsm::create_config))
        .route("/api/itsm/configs/:id", delete(handlers::itsm::delete_config))
        .route("/api/itsm/configs/:id/test", post(handlers::itsm::test_config))
        .route("/api/itsm/tickets", get(handlers::itsm::list_tickets))
        .route("/api/itsm/tickets", post(handlers::itsm::create_ticket))
        .route("/api/itsm/summary", get(handlers::itsm::itsm_summary))

        // Multi-Tenancy / MSP Mode
        .route("/api/tenants", get(handlers::tenants::list_tenants))
        .route("/api/tenants", post(handlers::tenants::create_tenant))
        .route("/api/tenants/summary", get(handlers::tenants::tenants_summary))
        .route("/api/tenants/:id", get(handlers::tenants::get_tenant))
        .route("/api/tenants/:id", axum::routing::put(handlers::tenants::update_tenant))
        .route("/api/tenants/:id", delete(handlers::tenants::delete_tenant))
        .route("/api/tenants/:id/suspend", post(handlers::tenants::suspend_tenant))
        .route("/api/tenants/:id/activate", post(handlers::tenants::activate_tenant))
        .route("/api/tenants/:id/usage", get(handlers::tenants::tenant_usage))

        // HA Architecture
        .route("/api/ha/cluster", get(handlers::ha::cluster_status))
        .route("/api/ha/health", get(handlers::ha::ha_health))
        .route("/api/ha/failovers", get(handlers::ha::failover_history))

        // On-Call Management
        .route("/api/oncall/schedules", get(handlers::oncall::list_schedules))
        .route("/api/oncall/schedules", post(handlers::oncall::create_schedule))
        .route("/api/oncall/schedules/:id", get(handlers::oncall::get_schedule))
        .route("/api/oncall/schedules/:id", delete(handlers::oncall::delete_schedule))
        .route("/api/oncall/schedules/:id/current", get(handlers::oncall::current_oncall))
        .route("/api/oncall/schedules/:id/shifts", get(handlers::oncall::list_shifts))
        .route("/api/oncall/schedules/:schedule_id/layers/:layer_id/overrides", post(handlers::oncall::create_override))
        .route("/api/oncall/summary", get(handlers::oncall::oncall_summary))

        // Proxies
        .route("/api/proxies", get(handlers::proxies::list_proxies))
        .route("/api/proxies", post(handlers::proxies::register_proxy))
        .route("/api/proxies/:id", get(handlers::proxies::get_proxy))
        .route("/api/proxies/:id", put(handlers::proxies::update_proxy))
        .route("/api/proxies/:id", delete(handlers::proxies::delete_proxy))
        .route("/api/proxies/:id/heartbeat", post(handlers::proxies::proxy_heartbeat))
        .route("/api/proxies/:id/data", post(handlers::proxies::proxy_forward_data))

        // Escalation Chains
        .route("/api/notifications/escalation-chains", get(handlers::notifications::list_escalation_chains))
        .route("/api/notifications/escalation-chains", post(handlers::notifications::create_escalation_chain))
        .route("/api/notifications/escalation-chains/:id", put(handlers::notifications::update_escalation_chain))
        .route("/api/notifications/escalation-chains/:id", delete(handlers::notifications::delete_escalation_chain))
        .route("/api/notifications/on-call", get(handlers::notifications::list_oncall_schedules))
        .route("/api/notifications/on-call", post(handlers::notifications::create_oncall_schedule))
}
