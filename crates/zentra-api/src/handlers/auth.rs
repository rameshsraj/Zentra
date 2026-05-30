use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde_json::json;
use sha2::{Sha256, Digest};
use sqlx;
use rand::{rngs::OsRng, RngCore};
use uuid::Uuid;

use crate::state::AppState;
use zentra_core::models::auth::*;
use validator::Validate;

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

const JWT_EXPIRY_HOURS: i64 = 24;
const REFRESH_EXPIRY_DAYS: i64 = 30;
const MAX_LOGIN_ATTEMPTS: i64 = 20;
const LOCKOUT_WINDOW_MINUTES: i64 = 5;
const LOCKOUT_DURATION_MINUTES: i64 = 15;

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn validate_password_strength(password: &str) -> Result<(), String> {
    let mut missing = Vec::new();
    if password.len() < 12 {
        missing.push("at least 12 characters");
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        missing.push("one uppercase letter");
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        missing.push("one lowercase letter");
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        missing.push("one digit");
    }
    if !password.chars().any(|c| "!@#$%^&*()-_=+[]{}|;:',.<>?/~`".contains(c)) {
        missing.push("one special character");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("Password must have: {}", missing.join(", ")))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/auth/login
// ═══════════════════════════════════════════════════════════════════════════

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    if let Err(e) = req.validate() {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": "validation_failed",
            "message": e.to_string()
        }))).into_response();
    }
    let ip_str = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| s.parse::<std::net::IpAddr>().is_ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let ua_str = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Brute-force/rate-limit gate: count recent failed attempts from this IP
    let recent_fails: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM login_attempts WHERE ip_address = $1::inet AND success = false AND attempted_at > NOW() - INTERVAL '1 minute' * $2"
    )
    .bind(&ip_str)
    .bind(LOCKOUT_WINDOW_MINUTES)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    if recent_fails >= MAX_LOGIN_ATTEMPTS {
        return (StatusCode::TOO_MANY_REQUESTS, Json(json!({
            "error": "too_many_attempts",
            "message": format!("Too many failed login attempts. Try again in {} minutes.", LOCKOUT_WINDOW_MINUTES)
        }))).into_response();
    }

    // Find user by email
    let user_row = sqlx::query_as::<_, (Uuid, Uuid, String, String, String, Option<String>, Option<String>, bool, bool, Option<chrono::DateTime<Utc>>, bool, Option<String>, bool)>(
        "SELECT id, COALESCE(org_id, '00000000-0000-0000-0000-000000000001'::uuid), email, username, password_hash, \
         COALESCE(display_name, full_name), avatar_url, COALESCE(is_active, true), COALESCE(is_locked, false), locked_until, \
         COALESCE(mfa_enabled, false), mfa_secret, COALESCE(must_change_password, false) \
         FROM users WHERE LOWER(email) = LOWER($1)"
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await;

    let user_row = match user_row {
        Ok(Some(row)) => row,
        _ => {
            // Record failed attempt
            let _ = sqlx::query(
                "INSERT INTO login_attempts (email, ip_address, success, user_agent) VALUES ($1, $2::inet, false, $3)"
            )
            .bind(&req.email)
            .bind(&ip_str)
            .bind(&ua_str)
            .execute(&state.pool)
            .await;

            return (StatusCode::UNAUTHORIZED, Json(json!({
                "error": "invalid_credentials",
                "message": "Invalid email or password"
            }))).into_response();
        }
    };

    let (user_id, org_id, email, username, password_hash, display_name, avatar_url,
         is_active, is_locked, locked_until, mfa_enabled, mfa_secret, must_change_password) = user_row;

    // Check if user is active
    if !is_active {
        return (StatusCode::FORBIDDEN, Json(json!({
            "error": "account_disabled",
            "message": "Account has been disabled"
        }))).into_response();
    }

    // Account-lock check
    if is_locked {
        if let Some(until) = locked_until {
            if until > Utc::now() {
                return (StatusCode::FORBIDDEN, Json(json!({
                    "error": "account_locked",
                    "message": format!("Account is locked until {}", until.format("%H:%M:%S UTC"))
                }))).into_response();
            }
        } else {
            return (StatusCode::FORBIDDEN, Json(json!({
                "error": "account_locked",
                "message": "Account is locked. Contact your administrator."
            }))).into_response();
        }
    }

    // Verify password
    let password_valid = bcrypt::verify(&req.password, &password_hash).unwrap_or(false);
    if !password_valid {
        // Record failed attempt
        let _ = sqlx::query(
            "INSERT INTO login_attempts (email, ip_address, success, user_agent) VALUES ($1, $2::inet, false, $3)"
        )
        .bind(&req.email)
        .bind(&ip_str)
        .bind(&ua_str)
        .execute(&state.pool)
        .await;

        // Auto-lock account after repeated failures
        if recent_fails + 1 >= MAX_LOGIN_ATTEMPTS {
            let _ = sqlx::query(
                "UPDATE users SET is_locked = true, locked_until = NOW() + INTERVAL '1 minute' * $1 WHERE email = $2"
            )
            .bind(LOCKOUT_DURATION_MINUTES)
            .bind(&req.email)
            .execute(&state.pool)
            .await;
        }

        return (StatusCode::UNAUTHORIZED, Json(json!({
            "error": "invalid_credentials",
            "message": "Invalid email or password"
        }))).into_response();
    }

    // MFA check — enforce TOTP validation when MFA is enabled
    if mfa_enabled {
        match req.mfa_code.as_deref().filter(|c| !c.is_empty()) {
            None => {
                return (StatusCode::OK, Json(json!({
                    "mfa_required": true,
                    "message": "MFA code required"
                }))).into_response();
            }
            Some(code) => {
                let secret = match mfa_secret.as_deref() {
                    Some(s) => s,
                    None => {
                        tracing::error!(user_id = %user_id, "MFA enabled but no secret stored");
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                            "error": "mfa_misconfigured",
                            "message": "MFA configuration error. Contact your administrator."
                        }))).into_response();
                    }
                };
                if !validate_totp(secret, code) {
                    let _ = sqlx::query(
                        "INSERT INTO login_attempts (email, ip_address, success, user_agent) VALUES ($1, $2::inet, false, $3)"
                    )
                    .bind(&req.email)
                    .bind(&ip_str)
                    .bind(&ua_str)
                    .execute(&state.pool)
                    .await;
                    return (StatusCode::UNAUTHORIZED, Json(json!({
                        "error": "invalid_mfa_code",
                        "message": "Invalid MFA code"
                    }))).into_response();
                }
            }
        }
    }

    // Get user's primary role
    let role_name = sqlx::query_scalar::<_, String>(
        "SELECT r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.role_id \
         WHERE ur.user_id = $1 AND ur.org_id = $2 ORDER BY r.is_system DESC LIMIT 1"
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None)
    .unwrap_or_else(|| "Viewer".into());

    // Generate JWT
    let session_id = Uuid::new_v4();
    let now = Utc::now();
    let exp = now + Duration::hours(JWT_EXPIRY_HOURS);

    let claims = JwtClaims {
        sub: user_id,
        org: org_id,
        email: email.clone(),
        role: role_name.clone(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        jti: session_id.to_string(),
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("JWT encode error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": "token_generation_failed"
            }))).into_response();
        }
    };

    // Generate refresh token
    let refresh_token = format!("rt_{}", Uuid::new_v4());
    let refresh_exp = now + Duration::days(REFRESH_EXPIRY_DAYS);

    // Store session
    let _ = sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, token_hash, refresh_token_hash, ip_address, user_agent, expires_at) \
         VALUES ($1, $2, $3, $4, $5::inet, $6, $7)"
    )
    .bind(session_id)
    .bind(user_id)
    .bind(hash_token(&token))
    .bind(hash_token(&refresh_token))
    .bind(&ip_str)
    .bind(&ua_str)
    .bind(refresh_exp)
    .execute(&state.pool)
    .await;

    // Record successful login
    let _ = sqlx::query(
        "INSERT INTO login_attempts (email, ip_address, success, user_agent) VALUES ($1, $2::inet, true, $3)"
    )
    .bind(&req.email)
    .bind(&ip_str)
    .bind(&ua_str)
    .execute(&state.pool)
    .await;

    // Update last_login
    let _ = sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;

    // Get user roles for response
    let roles: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.role_id WHERE ur.user_id = $1 AND ur.org_id = $2"
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    // Audit log
    let _ = sqlx::query(
        "INSERT INTO audit_log (org_id, user_id, action, resource_type, resource_id, details, ip_address, user_agent, status) \
         VALUES ($1, $2, 'login', 'user', $3, $4, $5::inet, $6, 'success')"
    )
    .bind(org_id)
    .bind(user_id)
    .bind(user_id.to_string())
    .bind(json!({"method": "password"}))
    .bind(&ip_str)
    .bind(&ua_str)
    .execute(&state.pool)
    .await;

    let response = LoginResponse {
        token,
        refresh_token,
        user: UserSummary {
            id: user_id,
            email,
            username,
            display_name,
            avatar_url,
            is_active,
            is_locked: false,
            last_login: Some(now),
            roles,
        },
        expires_at: exp,
        mfa_required: false,
        must_change_password,
    };

    (StatusCode::OK, Json(json!(response))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/auth/logout
// ═══════════════════════════════════════════════════════════════════════════

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Extract token from Authorization header
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(header_str) = auth_header.to_str() {
            if let Some(token) = header_str.strip_prefix("Bearer ") {
                let token_hash = hash_token(token);
                // Delete session
                let _ = sqlx::query("DELETE FROM user_sessions WHERE token_hash = $1")
                    .bind(&token_hash)
                    .execute(&state.pool)
                    .await;
            }
        }
    }

    (StatusCode::OK, Json(json!({"message": "Logged out successfully"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/auth/refresh
// ═══════════════════════════════════════════════════════════════════════════

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let refresh_hash = hash_token(&req.refresh_token);

    // Find session by refresh token
    let session = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT s.session_id, s.user_id FROM user_sessions s WHERE s.refresh_token_hash = $1 AND s.expires_at > NOW()"
    )
    .bind(&refresh_hash)
    .fetch_optional(&state.pool)
    .await;

    let (session_id, user_id) = match session {
        Ok(Some(s)) => s,
        _ => {
            return (StatusCode::UNAUTHORIZED, Json(json!({
                "error": "invalid_refresh_token",
                "message": "Refresh token is invalid or expired"
            }))).into_response();
        }
    };

    // Fetch user info
    let user_info = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT COALESCE(org_id, '00000000-0000-0000-0000-000000000001'::uuid), email, username FROM users WHERE id = $1 AND COALESCE(is_active, true) = true"
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await;

    let (org_id, email, _username) = match user_info {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::UNAUTHORIZED, Json(json!({
                "error": "user_not_found"
            }))).into_response();
        }
    };

    // Get role
    let role_name = sqlx::query_scalar::<_, String>(
        "SELECT r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.role_id \
         WHERE ur.user_id = $1 AND ur.org_id = $2 ORDER BY r.is_system DESC LIMIT 1"
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None)
    .unwrap_or_else(|| "Viewer".into());

    // Generate new JWT
    let now = Utc::now();
    let exp = now + Duration::hours(JWT_EXPIRY_HOURS);

    let claims = JwtClaims {
        sub: user_id,
        org: org_id,
        email,
        role: role_name,
        exp: exp.timestamp(),
        iat: now.timestamp(),
        jti: session_id.to_string(),
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": "token_generation_failed"
            }))).into_response();
        }
    };

    // Rotate refresh token and extend session expiry
    let new_refresh_token = format!("rt_{}", Uuid::new_v4());
    let new_refresh_exp = now + Duration::days(REFRESH_EXPIRY_DAYS);

    let _ = sqlx::query(
        "UPDATE user_sessions SET token_hash = $1, refresh_token_hash = $2, expires_at = $3 WHERE session_id = $4"
    )
    .bind(hash_token(&token))
    .bind(hash_token(&new_refresh_token))
    .bind(new_refresh_exp)
    .bind(session_id)
    .execute(&state.pool)
    .await;

    (StatusCode::OK, Json(json!({
        "token": token,
        "refresh_token": new_refresh_token,
        "expires_at": exp.to_rfc3339(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/auth/change-password
// ═══════════════════════════════════════════════════════════════════════════

pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let claims = match extract_claims(&headers, &state.jwt_secret) {
        Some(c) => c,
        None => {
            return (StatusCode::UNAUTHORIZED, Json(json!({
                "error": "not_authenticated"
            }))).into_response();
        }
    };

    // Fetch current password hash
    let current_hash = sqlx::query_scalar::<_, String>(
        "SELECT password_hash FROM users WHERE id = $1"
    )
    .bind(claims.sub)
    .fetch_optional(&state.pool)
    .await;

    let current_hash = match current_hash {
        Ok(Some(h)) => h,
        _ => {
            return (StatusCode::NOT_FOUND, Json(json!({
                "error": "user_not_found"
            }))).into_response();
        }
    };

    // Verify current password if provided
    if let Some(ref current) = req.current_password {
        if !bcrypt::verify(current, &current_hash).unwrap_or(false) {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": "invalid_current_password",
                "message": "Current password is incorrect"
            }))).into_response();
        }
    }

    // Validate new password strength
    if let Err(msg) = validate_password_strength(&req.new_password) {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": "weak_password",
            "message": msg
        }))).into_response();
    }

    // Hash new password
    let new_hash = match bcrypt::hash(&req.new_password, 12) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("bcrypt hash error: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": "password_hash_failed"
            }))).into_response();
        }
    };

    // Update password and clear must_change_password flag
    let result = sqlx::query(
        "UPDATE users SET password_hash = $1, must_change_password = false, password_changed_at = NOW() WHERE id = $2"
    )
    .bind(&new_hash)
    .bind(claims.sub)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({
            "success": true,
            "message": "Password changed successfully"
        }))).into_response(),
        Err(e) => {
            tracing::error!("password update error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": "update_failed"
            }))).into_response()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/auth/me
// ═══════════════════════════════════════════════════════════════════════════

pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let claims = match extract_claims(&headers, &state.jwt_secret) {
        Some(c) => c,
        None => {
            return (StatusCode::UNAUTHORIZED, Json(json!({
                "error": "not_authenticated"
            }))).into_response();
        }
    };

    // Fetch full user profile
    let user = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, Option<String>, String, String, bool, bool, Option<chrono::DateTime<Utc>>, bool, Option<chrono::DateTime<Utc>>, chrono::DateTime<Utc>, chrono::DateTime<Utc>)>(
        "SELECT id, COALESCE(org_id, '00000000-0000-0000-0000-000000000001'::uuid), email, username, COALESCE(display_name, full_name), avatar_url, COALESCE(timezone, 'UTC'), COALESCE(theme, 'dark'), \
         COALESCE(is_active, true), COALESCE(is_locked, false), last_login, COALESCE(mfa_enabled, false), password_changed_at, created_at, updated_at \
         FROM users WHERE id = $1"
    )
    .bind(claims.sub)
    .fetch_optional(&state.pool)
    .await;

    let user = match user {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::NOT_FOUND, Json(json!({
                "error": "user_not_found"
            }))).into_response();
        }
    };

    // Get roles
    let roles: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.role_id WHERE ur.user_id = $1"
    )
    .bind(claims.sub)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    // Get permissions
    let permissions: Vec<(String, String)> = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT p.resource, p.action FROM permissions p \
         JOIN role_permissions rp ON rp.permission_id = p.permission_id \
         JOIN user_roles ur ON ur.role_id = rp.role_id \
         WHERE ur.user_id = $1"
    )
    .bind(claims.sub)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let perm_list: Vec<serde_json::Value> = permissions.iter()
        .map(|(r, a)| json!({"resource": r, "action": a}))
        .collect();

    // Get module access + menu items for this user, scoped by org_id
    let is_super_admin = roles.iter().any(|r| r == "Super Admin");
    let is_admin = roles.iter().any(|r| r == "Admin") || is_super_admin;
    let org_id = user.1; // org_id from the user query

    let all_modules = vec![
        "command_center".into(), "infrastructure".into(), "database".into(),
        "middleware".into(), "security".into(), "logs".into(),
        "alerts_events".into(), "ai_analytics".into(), "opentelemetry".into(), "admin".into(),
    ];

    let (module_access, menu_items): (Vec<String>, Vec<String>) = if is_super_admin {
        // Super Admin: unconditionally gets ALL modules, no sub-item filtering
        (all_modules, vec![])
    } else if is_admin {
        // Admin: gets all modules by default but module access CAN be configured
        // If they have explicit module_access rows, use those; otherwise fall back to all
        let explicit: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT module_id FROM user_module_access WHERE user_id = $1 AND (org_id = $2 OR org_id IS NULL) ORDER BY module_id"
        )
        .bind(claims.sub)
        .bind(org_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        if explicit.is_empty() {
            (all_modules, vec![])
        } else {
            let modules = explicit.iter().filter(|m| !m.contains('.')).cloned().collect();
            let items = explicit.iter().filter(|m| m.contains('.')).cloned().collect();
            (modules, items)
        }
    } else {
        let all: Vec<String> = sqlx::query_scalar::<_, String>(
            "SELECT module_id FROM user_module_access WHERE user_id = $1 AND (org_id = $2 OR org_id IS NULL) ORDER BY module_id"
        )
        .bind(claims.sub)
        .bind(org_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        // Split: entries without dots are top-level modules, with dots are sub-menu items
        let modules = all.iter().filter(|m| !m.contains('.')).cloned().collect();
        let items = all.iter().filter(|m| m.contains('.')).cloned().collect();
        (modules, items)
    };

    (StatusCode::OK, Json(json!({
        "id": user.0,
        "org_id": org_id,
        "email": user.2,
        "username": user.3,
        "display_name": user.4,
        "avatar_url": user.5,
        "timezone": user.6,
        "theme": user.7,
        "is_active": user.8,
        "is_locked": user.9,
        "last_login": user.10,
        "mfa_enabled": user.11,
        "password_changed_at": user.12,
        "created_at": user.13,
        "updated_at": user.14,
        "roles": roles,
        "permissions": perm_list,
        "module_access": module_access,
        "menu_items": menu_items,
        "is_admin": is_admin,
        "is_super_admin": is_super_admin,
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/auth/mfa/setup
// ═══════════════════════════════════════════════════════════════════════════

pub async fn mfa_setup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let claims = match extract_claims(&headers, &state.jwt_secret) {
        Some(c) => c,
        None => {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response();
        }
    };

    // Generate a TOTP secret using a CSPRNG (20 bytes = 160-bit key, compatible with RFC 6238)
    let mut secret_bytes = [0u8; 20];
    OsRng.fill_bytes(&mut secret_bytes);
    let secret = base32_encode(&secret_bytes);

    // Store (not yet enabled — user must verify first)
    let _ = sqlx::query("UPDATE users SET mfa_secret = $1 WHERE id = $2")
        .bind(&secret)
        .bind(claims.sub)
        .execute(&state.pool)
        .await;

    let issuer = "Zentra";
    let qr_uri = format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&digits=6&period=30",
        issuer, claims.email, secret, issuer
    );

    (StatusCode::OK, Json(json!(MfaSetupResponse {
        secret: secret.clone(),
        qr_uri,
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/auth/mfa/verify
// ═══════════════════════════════════════════════════════════════════════════

pub async fn mfa_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<MfaVerifyRequest>,
) -> impl IntoResponse {
    let claims = match extract_claims(&headers, &state.jwt_secret) {
        Some(c) => c,
        None => {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response();
        }
    };

    let stored_secret = sqlx::query_scalar::<_, Option<String>>(
        "SELECT mfa_secret FROM users WHERE id = $1"
    )
    .bind(claims.sub)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten()
    .flatten();

    let secret = match stored_secret {
        Some(s) => s,
        None => {
            return (StatusCode::BAD_REQUEST, Json(json!({
                "error": "mfa_not_configured",
                "message": "MFA setup not initiated. Call /api/auth/mfa/setup first."
            }))).into_response();
        }
    };

    if !validate_totp(&secret, &req.code) {
        return (StatusCode::UNAUTHORIZED, Json(json!({
            "error": "invalid_code",
            "message": "Invalid or expired MFA code"
        }))).into_response();
    }

    if let Err(e) = sqlx::query("UPDATE users SET mfa_enabled = true WHERE id = $1")
        .bind(claims.sub)
        .execute(&state.pool)
        .await
    {
        tracing::error!(user_id = %claims.sub, "Failed to enable MFA: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "database_error"}))).into_response();
    }

    (StatusCode::OK, Json(json!({
        "message": "MFA enabled successfully"
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/auth/sessions
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let claims = match extract_claims(&headers, &state.jwt_secret) {
        Some(c) => c,
        None => {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response();
        }
    };

    let sessions = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, chrono::DateTime<Utc>, chrono::DateTime<Utc>)>(
        "SELECT session_id, host(ip_address)::text, user_agent, expires_at, created_at \
         FROM user_sessions WHERE user_id = $1 AND expires_at > NOW() ORDER BY created_at DESC"
    )
    .bind(claims.sub)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let list: Vec<serde_json::Value> = sessions.iter().map(|s| json!({
        "id": s.0,
        "ip_address": s.1,
        "user_agent": s.2,
        "expires_at": s.3,
        "created_at": s.4,
    })).collect();

    (StatusCode::OK, Json(json!(list))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/auth/sessions/:id
// ═══════════════════════════════════════════════════════════════════════════

pub async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(session_id): axum::extract::Path<Uuid>,
) -> impl IntoResponse {
    let claims = match extract_claims(&headers, &state.jwt_secret) {
        Some(c) => c,
        None => {
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response();
        }
    };

    let _ = sqlx::query("DELETE FROM user_sessions WHERE session_id = $1 AND user_id = $2")
        .bind(session_id)
        .bind(claims.sub)
        .execute(&state.pool)
        .await;

    (StatusCode::OK, Json(json!({"message": "Session revoked"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn extract_claims(headers: &HeaderMap, secret: &str) -> Option<JwtClaims> {
    let auth_header = headers.get("authorization")?;
    let header_str = auth_header.to_str().ok()?;
    let token = header_str.strip_prefix("Bearer ")?;

    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    ).ok()?;

    Some(token_data.claims)
}

fn base32_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut result = String::new();
    let mut buffer: u64 = 0;
    let mut bits_left = 0;

    for &byte in data {
        buffer = (buffer << 8) | byte as u64;
        bits_left += 8;
        while bits_left >= 5 {
            bits_left -= 5;
            result.push(ALPHABET[((buffer >> bits_left) & 0x1F) as usize] as char);
        }
    }

    if bits_left > 0 {
        buffer <<= 5 - bits_left;
        result.push(ALPHABET[(buffer & 0x1F) as usize] as char);
    }

    result
}

/// Decode a base32-encoded string (RFC 4648 alphabet, case-insensitive, padding optional).
fn base32_decode(encoded: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let normalised: Vec<u8> = encoded
        .to_ascii_uppercase()
        .bytes()
        .filter(|&b| b != b'=')
        .collect();

    let mut bits: u32 = 0;
    let mut bit_count: u32 = 0;
    let mut result = Vec::with_capacity(normalised.len() * 5 / 8);

    for byte in normalised {
        let val = ALPHABET.iter().position(|&b| b == byte)? as u32;
        bits = (bits << 5) | val;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            result.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }

    Some(result)
}

/// Compute HOTP (RFC 4226): HMAC-SHA1(key, counter) dynamically truncated to 6 digits.
fn hotp(key: &[u8], counter: u64) -> String {
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    let mut mac = Hmac::<Sha1>::new_from_slice(key)
        .expect("HMAC accepts any key size");
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();

    // Dynamic truncation (RFC 4226 §5.3)
    let offset = (result[19] & 0x0f) as usize;
    let code = ((result[offset] as u32 & 0x7f) << 24)
        | ((result[offset + 1] as u32) << 16)
        | ((result[offset + 2] as u32) << 8)
        | (result[offset + 3] as u32);

    format!("{:06}", code % 1_000_000)
}

/// Validate a 6-digit TOTP code (RFC 6238): 30-second steps, ±1 step clock-skew tolerance.
fn validate_totp(secret_base32: &str, code: &str) -> bool {
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let key = match base32_decode(secret_base32) {
        Some(k) => k,
        None => return false,
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let counter = now / 30;

    [-1i64, 0, 1].iter().any(|&delta| {
        let c = (counter as i64).saturating_add(delta) as u64;
        hotp(&key, c) == code
    })
}
