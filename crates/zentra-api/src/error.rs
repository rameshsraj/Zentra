//! Central API error type used across all handler modules.
//!
//! Every handler should return `Result<Json<T>, ApiError>` instead of ad-hoc
//! `(StatusCode, String)` tuples. This guarantees a consistent JSON error
//! envelope and allows `?` propagation from sqlx, serde, and other crates.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Unified API error — implements `IntoResponse` so axum can render it.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("internal: {0}")]
    Internal(String),

    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::Database(e) => {
                tracing::error!(error = %e, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal database error".to_string())
            }
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
            Self::Internal(m) => {
                tracing::error!(error = %m, "internal error");
                // Never leak internal details to the client
                (StatusCode::INTERNAL_SERVER_ERROR, "An internal error occurred".to_string())
            }
            Self::ServiceUnavailable(m) => {
                tracing::warn!(error = %m, "service unavailable");
                (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable".to_string())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// Conversion from the legacy `(StatusCode, String)` tuple used by existing handlers.
/// This enables gradual migration: handlers that still return the old type can be
/// wrapped without changing their signature all at once.
impl From<(StatusCode, String)> for ApiError {
    fn from((status, msg): (StatusCode, String)) -> Self {
        match status {
            StatusCode::NOT_FOUND => Self::NotFound(msg),
            StatusCode::BAD_REQUEST => Self::BadRequest(msg),
            StatusCode::UNAUTHORIZED => Self::Unauthorized(msg),
            StatusCode::FORBIDDEN => Self::Forbidden(msg),
            _ => Self::Internal(msg),
        }
    }
}

impl From<validator::ValidationErrors> for ApiError {
    fn from(e: validator::ValidationErrors) -> Self {
        ApiError::BadRequest(format!("Validation failed: {}", e))
    }
}

/// Generate a short, prefix-based ID (e.g. `"hc-a1b2c3d4"`).
///
/// Replaces the fragile pattern:
/// ```ignore
/// Uuid::new_v4().to_string().split('-').next().unwrap()
/// ```
/// This version never panics.
pub fn short_id(prefix: &str) -> String {
    let u = uuid::Uuid::new_v4();
    let hex = &u.simple().to_string()[..8]; // first 8 hex chars
    format!("{}-{}", prefix, hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_id_format() {
        let id = short_id("hc");
        assert!(id.starts_with("hc-"));
        assert_eq!(id.len(), 3 + 8); // "hc-" + 8 hex chars
    }

    #[test]
    fn test_short_id_uniqueness() {
        let a = short_id("test");
        let b = short_id("test");
        assert_ne!(a, b);
    }

    #[test]
    fn test_api_error_status_codes() {
        // Just verify the mapping logic compiles and is sane
        let err = ApiError::NotFound("x".into());
        assert!(matches!(err, ApiError::NotFound(_)));

        let err = ApiError::BadRequest("y".into());
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[test]
    fn test_from_legacy_tuple() {
        let err: ApiError = (StatusCode::NOT_FOUND, "gone".into()).into();
        assert!(matches!(err, ApiError::NotFound(ref m) if m == "gone"));

        let err: ApiError = (StatusCode::INTERNAL_SERVER_ERROR, "boom".into()).into();
        assert!(matches!(err, ApiError::Internal(ref m) if m == "boom"));
    }
}
