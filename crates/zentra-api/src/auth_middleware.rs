//! Authn/authz middleware. Verifies a Bearer JWT, decodes claims, and inserts
//! an [`AuthContext`] into the request extensions for downstream handlers.
//!
//! Whitelist is controlled by [`is_public_path`] — login/refresh/health/docs
//! do not require a token. Everything else does.

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use sqlx::{self, Row};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;
use dashmap::DashMap;
use crate::state::AppState;

/// TTL cache for session validation — avoids hitting the DB for every request.
/// Maps jti → Instant when the session was last validated.
static SESSION_CACHE: std::sync::LazyLock<Arc<DashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtClaims {
    pub sub: Uuid,           // user_id
    pub org: Uuid,           // org_id
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub jti: String,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub role: String,
    pub jti: String,
}

impl From<JwtClaims> for AuthContext {
    fn from(c: JwtClaims) -> Self {
        Self {
            user_id: c.sub,
            org_id: c.org,
            email: c.email,
            role: c.role,
            jti: c.jti,
        }
    }
}

/// Public, unauthenticated endpoints. All others must present a Bearer token.
pub fn is_public_path(method: &Method, path: &str) -> bool {
    if method == Method::OPTIONS {
        return true;
    }
    if (*method == Method::POST && path == "/api/agents/heartbeat")
        || (*method == Method::GET && path == "/api/agents/targets")
        || (*method == Method::GET && path == "/api/agents/log-dirs")
        || (*method == Method::POST && path.starts_with("/api/agents/") && path.ends_with("/discovered"))
        || (*method == Method::POST && path.starts_with("/api/agents/by-dc/") && path.ends_with("/components"))
    {
        return true;
    }
    // Agent binary downloads — public, no auth required (HEAD for curl -I checks)
    if (*method == Method::GET || *method == Method::HEAD) && path.starts_with("/dl/") {
        return true;
    }
    // OTLP ingest endpoints use isok_ API key auth (validated inside the
    // handler via validate_otlp_key). They must bypass the JWT middleware,
    // otherwise the Bearer isok_... is rejected as an invalid JWT.
    if *method == Method::POST
        && matches!(path, "/v1/traces" | "/v1/metrics" | "/v1/logs" | "/v1/profiles")
    {
        return true;
    }
    if matches!(
        path,
        "/api/health"
            | "/api/auth/login"
            | "/api/auth/refresh"
            | "/api/signup"
            | "/api/signup/verify-email"
            | "/api/billing/webhook"
            | "/api/docs"
            | "/api/docs/redoc"
            | "/api/openapi.json"
    ) {
        return true;
    }
    // /metrics is sensitive. It can be made public for trusted Prometheus
    // network paths, but defaults to authenticated access.
    if path == "/metrics" && std::env::var("ZENTRA_PUBLIC_METRICS").ok().as_deref() == Some("1") {
        return true;
    }
    // WebSocket endpoints authenticate via query-string token validated
    // inside ws_handler before the upgrade is accepted.
    false
}

fn hash_api_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn agent_auth_context(state: &AppState, raw_key: &str) -> Option<AuthContext> {
    let key_hash = hash_api_key(raw_key);
    let row = sqlx::query(
        "SELECT org_id, agent_id FROM agent_api_keys
         WHERE key_hash = $1
           AND enabled = true
           AND (expires_at > NOW() OR (grace_until IS NOT NULL AND grace_until > NOW()))
           AND revoked_at IS NULL
         ORDER BY created_at DESC
         LIMIT 1"
    )
    .bind(&key_hash)
    .fetch_optional(&state.tsdb_pool)
    .await
    .ok()
    .flatten()?;

    let org_id: Uuid = row.get("org_id");
    let agent_id: Uuid = row.get("agent_id");
    Some(AuthContext {
        user_id: agent_id,
        org_id,
        email: format!("agent:{}", agent_id),
        role: "Agent".to_string(),
        jti: String::new(),
    })
}
fn unauthorized(reason: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "unauthorized", "message": reason})),
    )
        .into_response()
}

fn forbidden(reason: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": "forbidden", "message": reason})),
    )
        .into_response()
}

fn is_agent_scope_path(method: &Method, path: &str) -> bool {
    if (*method == Method::POST && path == "/api/agents/heartbeat")
        || (*method == Method::POST && path == "/api/agent/ingest/heartbeat")
        || (*method == Method::GET && path == "/api/agents/targets")
        || (*method == Method::GET && path == "/api/agents/log-dirs")
        || (*method == Method::POST && path.starts_with("/api/agents/") && path.ends_with("/discovered"))
        || (*method == Method::POST && path.starts_with("/api/agents/") && path.ends_with("/rotate-key"))
        || (*method == Method::POST && path.starts_with("/api/agents/by-dc/") && path.ends_with("/components"))
        || (*method == Method::POST && matches!(path, "/v1/metrics" | "/v1/logs" | "/api/agent/ingest/metrics" | "/api/agent/ingest/logs"))
    {
        return true;
    }
    false
}

pub async fn require_auth(State(state): State<AppState>, mut req: Request<Body>, next: Next) -> Response {
    let path = req.uri().path().to_string();

    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);

    if let Some(token) = bearer.as_deref() {
        if token.starts_with("isok_") {
            if (*req.method() == Method::POST
                && matches!(path.as_str(), "/v1/traces" | "/v1/metrics" | "/v1/logs" | "/v1/profiles"))
                || (*req.method() == Method::POST && path == "/api/agents/register")
            {
                return next.run(req).await;
            }
            return forbidden("OTLP key is only allowed on OTLP ingest endpoints");
        }
        if token.starts_with("isak_") {
            if !is_agent_scope_path(req.method(), &path) {
                return forbidden("Agent key is not allowed on this endpoint");
            }
            if let Some(ctx) = agent_auth_context(&state, token).await {
                req.extensions_mut().insert(ctx);
                return next.run(req).await;
            }
            return unauthorized("Invalid, revoked, or expired agent key");
        }
    }

    if is_public_path(req.method(), req.uri().path()) {
        return next.run(req).await;
    }

    let token = match req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => return unauthorized("missing bearer token"),
    };

    let secret = state.jwt_secret.as_ref();
    if secret.is_empty() {
        return unauthorized("server jwt secret not configured");
    }

    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 30;
    validation.validate_exp = true;
    let claims = match decode::<JwtClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    ) {
        Ok(d) => d.claims,
        Err(e) => return unauthorized(&format!("invalid token: {}", e)),
    };

    // Check session validity (token revocation) with TTL cache
    if !claims.jti.is_empty() {
        let cache_key = claims.jti.clone();
        let now = Instant::now();
        let cache_valid = SESSION_CACHE
            .get(&cache_key)
            .map(|t| now.duration_since(*t).as_secs() < 60)
            .unwrap_or(false);
        if !cache_valid {
            let session_exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM user_sessions WHERE session_id = $1 AND expires_at > NOW())"
            )
            .bind(Uuid::parse_str(&claims.jti).unwrap_or_default())
            .fetch_one(&state.pool)
            .await
            .unwrap_or(false);
            if !session_exists {
                return unauthorized("Session revoked or expired");
            }
            SESSION_CACHE.insert(cache_key, now);
        }
    }

    let ctx: AuthContext = claims.into();
    req.extensions_mut().insert(ctx);
    next.run(req).await
}

fn is_admin_role(role: &str) -> bool {
    matches!(role, "Admin" | "Org Admin" | "Super Admin")
}

pub async fn require_admin_role(req: Request<Body>, next: Next) -> Response {
    match req.extensions().get::<AuthContext>() {
        Some(ctx) if is_admin_role(&ctx.role) => next.run(req).await,
        Some(_) => (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "forbidden", "message": "Admin role required"})),
        )
            .into_response(),
        None => unauthorized("auth context missing"),
    }
}

/// Helper for handlers: pull the AuthContext out of request extensions.
/// Returns 401 if absent (defence in depth — should not happen if middleware ran).
pub fn ctx_from(parts: &axum::http::request::Parts) -> Result<&AuthContext, Response> {
    parts
        .extensions
        .get::<AuthContext>()
        .ok_or_else(|| unauthorized("auth context missing"))
}
