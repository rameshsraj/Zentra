use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // User Management (V5 Prompt 2)
        .route("/api/users", get(handlers::users::list_users))
        .route("/api/users", post(handlers::users::create_user))
        .route("/api/users/bulk", post(handlers::users::bulk_action))
        .route("/api/users/:id", get(handlers::users::get_user))
        .route("/api/users/:id", put(handlers::users::update_user))
        .route("/api/users/:id", delete(handlers::users::delete_user))
        .route("/api/users/:id/lock", post(handlers::users::lock_user))
        .route("/api/users/:id/unlock", post(handlers::users::unlock_user))
        .route("/api/users/:id/password", put(handlers::users::reset_password))
        .route("/api/users/:id/modules", get(handlers::users::get_user_modules))
        .route("/api/users/:id/modules", put(handlers::users::set_user_modules))

        // User Groups (V5 Prompt 2)
        .route("/api/user-groups", get(handlers::user_groups::list_groups))
        .route("/api/user-groups", post(handlers::user_groups::create_group))
        .route("/api/user-groups/:id", get(handlers::user_groups::get_group))
        .route("/api/user-groups/:id", put(handlers::user_groups::update_group))
        .route("/api/user-groups/:id", delete(handlers::user_groups::delete_group))
        .route("/api/user-groups/:id/members", post(handlers::user_groups::add_members))
        .route("/api/user-groups/:id/members/:user_id", delete(handlers::user_groups::remove_member))

        // Roles & Permissions (V5 Prompt 3)
        .route("/api/roles", get(handlers::roles::list_roles))
        .route("/api/roles", post(handlers::roles::create_role))
        .route("/api/roles/:id", get(handlers::roles::get_role))
        .route("/api/roles/:id", put(handlers::roles::update_role))
        .route("/api/roles/:id", delete(handlers::roles::delete_role))
        .route("/api/permissions", get(handlers::roles::list_permissions))
        .route("/api/users/:id/roles", post(handlers::roles::assign_user_roles))

        // Audit Log (V5 Prompt 3)
        .route("/api/audit-log", get(handlers::audit::list_audit_log))
        .route("/api/audit-log/export", get(handlers::audit::export_audit_log))
        .route("/api/audit-log/stats", get(handlers::audit::audit_stats))

        // API Tokens (V5 Prompt 3)
        .route("/api/api-tokens", get(handlers::api_tokens::list_tokens))
        .route("/api/api-tokens", post(handlers::api_tokens::create_token))
        .route("/api/api-tokens/:id", delete(handlers::api_tokens::revoke_token))

        // System Settings (V5 Prompt 3)
        .route("/api/settings", get(handlers::system_settings::get_settings))
        .route("/api/settings", put(handlers::system_settings::update_settings))

        // License Management
        .route("/api/admin/licenses", get(handlers::licenses::list_licenses))
        .route("/api/admin/licenses", post(handlers::licenses::create_license))
        .route("/api/admin/licenses/compliance", get(handlers::licenses::get_compliance))
        .route("/api/admin/licenses/expiring", get(handlers::licenses::get_expiring))
        .route("/api/admin/licenses/audit-log", get(handlers::licenses::get_audit_log))
        .route("/api/admin/licenses/:id", get(handlers::licenses::get_license))
        .route("/api/admin/licenses/:id", put(handlers::licenses::update_license))
        .route("/api/admin/licenses/:id", delete(handlers::licenses::delete_license))
        .route("/api/admin/licenses/:id/seats", get(handlers::licenses::list_seats))
        .route("/api/admin/licenses/:id/seats/assign", post(handlers::licenses::assign_seat))
        .route("/api/admin/licenses/:id/seats/:seat_id/release", put(handlers::licenses::release_seat))

        // V5 Prompt 19: Data Audit
        .route("/api/audit/data-sources", get(handlers::data_audit::data_source_audit))
        .route("/api/audit/mock-data", get(handlers::data_audit::mock_data_check))

        // Dynamic System License Status & UI Menu Access controls
        .route("/api/system/license-status", get(handlers::license_status::get_license_status))
}
