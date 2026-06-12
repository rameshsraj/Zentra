//! SAAS-05: Billing & Stripe integration — checkout, webhooks, subscription status, portal.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth_middleware::AuthContext;
use crate::error::ApiError;
use crate::state::AppState;
use crate::tenant::OrgId;

// ── Stripe config (from env — never in source) ────────────────────────────

fn stripe_secret_key() -> String {
    std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set")
}

fn stripe_webhook_secret() -> String {
    std::env::var("STRIPE_WEBHOOK_SECRET").expect("STRIPE_WEBHOOK_SECRET must be set")
}

fn stripe_price_id(plan: &str) -> Result<String, ApiError> {
    let env_key = format!("STRIPE_PRICE_{}", plan.to_uppercase());
    std::env::var(&env_key)
        .map_err(|_| ApiError::Internal(format!("Missing env: {}", env_key)))
}

// ── GET /api/billing/subscription ──────────────────────────────────────────

pub async fn get_subscription(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, (String, String, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT plan, COALESCE(subscription_status, 'trialling'), subscription_period_end \
         FROM organizations WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Organization not found".into()))?;

    let (plan, status, period_end) = row;
    let (max_targets, max_users, max_dashboards, max_alerts, retention, _rate) = plan_features(&plan);

    Ok(Json(json!({
        "plan": plan,
        "status": status,
        "current_period_end": period_end,
        "features": {
            "max_targets": max_targets,
            "max_users": max_users,
            "max_dashboards": max_dashboards,
            "max_alert_rules": max_alerts,
            "retention_days": retention,
        }
    })))
}

// ── POST /api/billing/checkout ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub plan: String,
}

pub async fn create_checkout(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Json(req): Json<CheckoutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Require Org Admin or Admin
    if !matches!(ctx.role.as_str(), "Admin" | "Org Admin" | "Super Admin") {
        return Err(ApiError::Forbidden("Only Admins can manage billing".into()));
    }

    let price_id = stripe_price_id(&req.plan)?;

    // Ensure org has a Stripe customer
    let existing_customer: Option<String> = sqlx::query_scalar(
        "SELECT stripe_customer_id FROM organizations WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(None);

    let customer_id = match existing_customer {
        Some(cid) => cid,
        None => {
            let owner_email: String = sqlx::query_scalar(
                "SELECT owner_email FROM organizations WHERE org_id = $1",
            )
            .bind(org_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;

            let resp = state.http_client
                .post("https://api.stripe.com/v1/customers")
                .header("Authorization", format!("Bearer {}", stripe_secret_key()))
                .form(&[
                    ("email", owner_email.as_str()),
                    ("metadata[org_id]", &org_id.to_string()),
                ])
                .send()
                .await
                .map_err(|e| ApiError::Internal(format!("Stripe customer create: {}", e)))?;

            let body: serde_json::Value = resp.json().await
                .map_err(|e| ApiError::Internal(format!("Stripe parse: {}", e)))?;
            let cid = body["id"].as_str()
                .ok_or_else(|| ApiError::Internal("Missing customer id".into()))?
                .to_string();

            sqlx::query("UPDATE organizations SET stripe_customer_id = $1 WHERE org_id = $2")
                .bind(&cid).bind(org_id)
                .execute(&state.pool).await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            cid
        }
    };

    // Create checkout session
    let base_url = std::env::var("ZENTRA_BASE_URL").unwrap_or_else(|_| "https://app.zentra.dev".into());
    let resp = state.http_client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .header("Authorization", format!("Bearer {}", stripe_secret_key()))
        .form(&[
            ("customer", customer_id.as_str()),
            ("mode", "subscription"),
            ("line_items[0][price]", &price_id),
            ("line_items[0][quantity]", "1"),
            ("success_url", &format!("{}/billing?success=true", base_url)),
            ("cancel_url", &format!("{}/billing?cancelled=true", base_url)),
            ("metadata[org_id]", &org_id.to_string()),
            ("subscription_data[metadata][org_id]", &org_id.to_string()),
        ])
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Stripe checkout: {}", e)))?;

    let body: serde_json::Value = resp.json().await
        .map_err(|e| ApiError::Internal(format!("Stripe parse: {}", e)))?;

    let url = body["url"].as_str()
        .ok_or_else(|| ApiError::Internal("Missing checkout URL".into()))?;

    Ok(Json(json!({ "checkout_url": url })))
}

// ── POST /api/billing/portal ───────────────────────────────────────────────

pub async fn create_portal(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !matches!(ctx.role.as_str(), "Admin" | "Org Admin" | "Super Admin") {
        return Err(ApiError::Forbidden("Only Admins can manage billing".into()));
    }

    let customer_id: String = sqlx::query_scalar(
        "SELECT stripe_customer_id FROM organizations WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::BadRequest("No billing account. Complete checkout first.".into()))?;

    let base_url = std::env::var("ZENTRA_BASE_URL").unwrap_or_else(|_| "https://app.zentra.dev".into());
    let resp = state.http_client
        .post("https://api.stripe.com/v1/billing_portal/sessions")
        .header("Authorization", format!("Bearer {}", stripe_secret_key()))
        .form(&[
            ("customer", customer_id.as_str()),
            ("return_url", &format!("{}/billing", base_url)),
        ])
        .send()
        .await
        .map_err(|e| ApiError::Internal(format!("Stripe portal: {}", e)))?;

    let body: serde_json::Value = resp.json().await
        .map_err(|e| ApiError::Internal(format!("Stripe parse: {}", e)))?;

    let url = body["url"].as_str()
        .ok_or_else(|| ApiError::Internal("Missing portal URL".into()))?;

    Ok(Json(json!({ "portal_url": url })))
}

// ── POST /api/billing/webhook ──────────────────────────────────────────────

pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<StatusCode, ApiError> {
    // Validate signature
    let sig = headers.get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::BadRequest("Missing stripe-signature header".into()))?;

    let secret = stripe_webhook_secret();
    if !verify_stripe_signature(&body, sig, &secret) {
        return Err(ApiError::BadRequest("Invalid webhook signature".into()));
    }

    let event: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?;

    let event_id = event["id"].as_str().unwrap_or_default();
    let event_type = event["type"].as_str().unwrap_or_default();

    // Idempotency check
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM stripe_webhook_events WHERE event_id = $1)",
    )
    .bind(event_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if already {
        return Ok(StatusCode::OK);
    }

    // Record event
    let _ = sqlx::query(
        "INSERT INTO stripe_webhook_events (event_id, event_type) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(event_id).bind(event_type)
    .execute(&state.pool)
    .await;

    // Process
    match event_type {
        "checkout.session.completed" => {
            let org_id_str = event["data"]["object"]["metadata"]["org_id"].as_str().unwrap_or_default();
            let sub_id = event["data"]["object"]["subscription"].as_str().unwrap_or_default();
            if let Ok(org_id) = org_id_str.parse::<Uuid>() {
                let _ = sqlx::query(
                    "UPDATE organizations SET subscription_status = 'active', \
                     stripe_subscription_id = $1, subscription_past_due_since = NULL, \
                     updated_at = NOW() WHERE org_id = $2",
                )
                .bind(sub_id).bind(org_id)
                .execute(&state.pool)
                .await;
            }
        }
        "invoice.payment_succeeded" => {
            let sub_id = event["data"]["object"]["subscription"].as_str().unwrap_or_default();
            let period_end = event["data"]["object"]["lines"]["data"][0]["period"]["end"].as_i64();
            let _ = sqlx::query(
                "UPDATE organizations SET subscription_status = 'active', \
                 subscription_past_due_since = NULL, \
                 subscription_period_end = CASE WHEN $2::bigint IS NOT NULL THEN to_timestamp($2) ELSE subscription_period_end END, \
                 updated_at = NOW() WHERE stripe_subscription_id = $1",
            )
            .bind(sub_id).bind(period_end)
            .execute(&state.pool)
            .await;
        }
        "invoice.payment_failed" => {
            let sub_id = event["data"]["object"]["subscription"].as_str().unwrap_or_default();
            let _ = sqlx::query(
                "UPDATE organizations SET subscription_status = 'past_due', \
                 subscription_past_due_since = COALESCE(subscription_past_due_since, NOW()), \
                 updated_at = NOW() WHERE stripe_subscription_id = $1",
            )
            .bind(sub_id)
            .execute(&state.pool)
            .await;
        }
        "customer.subscription.deleted" => {
            let sub_id = event["data"]["object"]["id"].as_str().unwrap_or_default();
            // Downgrade to starter quotas
            let _ = sqlx::query(
                "UPDATE organizations SET subscription_status = 'cancelled', plan = 'starter', \
                 updated_at = NOW() WHERE stripe_subscription_id = $1",
            )
            .bind(sub_id)
            .execute(&state.pool)
            .await;

            let _ = sqlx::query(
                "UPDATE org_settings SET max_targets = 3, max_users = 2, max_dashboards = 5, \
                 max_alert_rules = 20, retention_days = 14 \
                 WHERE org_id = (SELECT org_id FROM organizations WHERE stripe_subscription_id = $1)",
            )
            .bind(sub_id)
            .execute(&state.pool)
            .await;
        }
        _ => {
            tracing::debug!(event_type, "Unhandled Stripe event type");
        }
    }

    Ok(StatusCode::OK)
}

// ── Stripe signature verification ──────────────────────────────────────────

fn verify_stripe_signature(payload: &str, sig_header: &str, secret: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    // Parse t= and v1= from the signature header
    let mut timestamp = "";
    let mut signature = "";
    for part in sig_header.split(',') {
        let part = part.trim();
        if let Some(t) = part.strip_prefix("t=") {
            timestamp = t;
        } else if let Some(v) = part.strip_prefix("v1=") {
            signature = v;
        }
    }

    if timestamp.is_empty() || signature.is_empty() {
        return false;
    }

    // Verify timestamp is within 5 minutes
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = chrono::Utc::now().timestamp();
        if (now - ts).unsigned_abs() > 300 {
            return false;
        }
    } else {
        return false;
    }

    let signed_payload = format!("{}.{}", timestamp, payload);
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(signed_payload.as_bytes());

    let expected = hex::encode(mac.finalize().into_bytes());
    // Constant-time comparison to prevent timing attacks
    if expected.len() != signature.len() {
        return false;
    }
    expected.bytes().zip(signature.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

// ── Subscription enforcement middleware ────────────────────────────────────

use axum::{extract::Request, middleware::Next, response::Response};

/// Replaces the old license middleware. Applied to all routes.
/// - active / trialling: full access
/// - past_due: read-only for 7 days, then same as cancelled
/// - cancelled: login + data export only
pub async fn subscription_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();
    let method = request.method().clone();

    // Always allow public endpoints
    if crate::auth_middleware::is_public_path(&method, &path) {
        return Ok(next.run(request).await);
    }

    // Allow all reads
    if method == axum::http::Method::GET || method == axum::http::Method::HEAD {
        return Ok(next.run(request).await);
    }

    // For writes, check subscription status
    // Extract org_id from auth context (set by auth middleware)
    let org_id = request.extensions().get::<AuthContext>().map(|c| c.org_id);

    if let Some(org_id) = org_id {
        let row: Option<(String, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
            "SELECT COALESCE(subscription_status, 'active'), subscription_past_due_since \
             FROM organizations WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();

        if let Some((status, past_due_since)) = row {
            match status.as_str() {
                "active" | "trialling" => { /* full access */ }
                "past_due" => {
                    // Allow writes for 7 days grace period
                    if let Some(since) = past_due_since {
                        let grace_days = (chrono::Utc::now() - since).num_days();
                        if grace_days > 7 {
                            let body = json!({
                                "error": "subscription_past_due",
                                "message": "Your subscription payment has failed. Update your payment method to restore access."
                            });
                            return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
                        }
                    }
                }
                "cancelled" => {
                    // Only allow data export
                    if !path.starts_with("/api/export") {
                        let body = json!({
                            "error": "subscription_cancelled",
                            "message": "Your subscription has been cancelled. Only data export is available."
                        });
                        return Ok((StatusCode::PAYMENT_REQUIRED, Json(body)).into_response());
                    }
                }
                _ => { /* unknown status — allow */ }
            }
        }
    }

    Ok(next.run(request).await)
}

fn plan_features(plan: &str) -> (i32, i32, i32, i32, i32, i32) {
    match plan {
        "enterprise"   => (500, 50, 200, 1000, 365, 10000),
        "professional" => (100, 20,  50,  200,  90,  2000),
        "starter"      => ( 10,  5,  10,   50,  30,   500),
        _              => (  3,  2,   5,   20,  14,   100),
    }
}
