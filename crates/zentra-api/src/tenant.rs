//! Tenancy helpers: pull the caller's `org_id` from the JWT `AuthContext`,
//! enforcing strict multi-tenant isolation.
//!
//! The `OrgId` extractor requires an `AuthContext` in request extensions
//! (inserted by `auth_middleware`). If absent, it rejects with 401.
//!
//! ## Usage in handlers
//!
//! ```ignore
//! pub async fn list_things(
//!     State(state): State<AppState>,
//!     OrgId(org_id): OrgId,
//! ) -> Result<Json<Vec<Thing>>, ApiError> {
//!     let rows = sqlx::query("... WHERE org_id = $1")
//!         .bind(org_id)
//!         .fetch_all(&state.pool).await?;
//!     Ok(Json(rows))
//! }
//! ```

use crate::auth_middleware::AuthContext;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use uuid::Uuid;

/// Axum extractor that resolves the `org_id` from the JWT `AuthContext`.
///
/// Returns 401 if no `AuthContext` is present in request extensions.
pub struct OrgId(pub Uuid);

#[axum::async_trait]
impl FromRequestParts<AppState> for OrgId {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .map(|c| OrgId(c.org_id))
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "unauthorized", "message": "auth context missing — tenant scope required"})),
                )
                    .into_response()
            })
    }
}

/// Mandatory tenant scope — returns the JWT org_id or an error.
pub fn required_org_id(ext: &axum::http::Extensions) -> Result<Uuid, &'static str> {
    ext.get::<AuthContext>()
        .map(|c| c.org_id)
        .ok_or("auth context missing")
}
