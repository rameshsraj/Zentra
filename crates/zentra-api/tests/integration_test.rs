//! Integration tests for Zentra API — covers all P0 endpoints.
//!
//! Prerequisites:
//!   - A running PostgreSQL 16 + TimescaleDB instance.
//!   - `DATABASE_URL_TEST` environment variable set (or `.env.test` file).
//!
//! Run with:
//!   DATABASE_URL_TEST=postgres://... cargo test --test integration
//!
//! The test suite:
//!   1. Connects to the test DB and runs all migrations.
//!   2. Seeds a test admin user.
//!   3. Starts an in-process Axum server via axum-test.
//!   4. Exercises every P0 endpoint and asserts expected HTTP status codes.

use axum_test::TestServer;
use serde_json::{json, Value};
use uuid::Uuid;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Load DATABASE_URL_TEST from env or `.env.test`, panic with a clear message if absent.
fn db_url() -> String {
    dotenvy::from_filename(".env.test").ok();
    std::env::var("DATABASE_URL_TEST").expect(
        "DATABASE_URL_TEST must be set to run integration tests.\n\
         Example: DATABASE_URL_TEST=postgres://zentra:zentra@localhost:5432/zentra_test"
    )
}

const TEST_JWT_SECRET: &str = "integration-test-secret-32-bytes!";
const TEST_ORG_ID: &str = "00000000-0000-0000-0000-000000000001";
const TEST_USER_EMAIL: &str = "integration@zentra.test";
const TEST_USER_PASS: &str = "TestPass1!";

/// Mint a valid JWT for the test admin user without hitting the login endpoint.
/// This lets authenticated tests bypass DB-side brute-force counters.
fn mint_test_jwt(user_id: Uuid, org_id: Uuid) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Claims {
        sub: Uuid,
        org: Uuid,
        email: String,
        role: String,
        exp: i64,
        iat: i64,
        jti: String,
    }

    let now = chrono::Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        org: org_id,
        email: TEST_USER_EMAIL.to_string(),
        role: "admin".to_string(),
        exp: now + 3600,
        iat: now,
        jti: Uuid::new_v4().to_string(),
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    )
    .expect("JWT mint failed")
}

/// Build a test AppState + Axum router backed by the test DB.
async fn build_test_server() -> (TestServer, Uuid, String) {
    let url = db_url();

    let pool = zentra_db::pool::create_pool(&url)
        .await
        .expect("Failed to connect to test DB");

    let tsdb_pool = zentra_db::pool::create_pool_with_kind(&url, zentra_db::pool::PoolKind::Tsdb)
        .await
        .expect("Failed to connect to test TSDB pool");

    // Run all migrations (idempotent).
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Migrations failed");

    let org_id: Uuid = TEST_ORG_ID.parse().unwrap();

    // Seed test org if missing (org_id is the PK column name).
    sqlx::query(
        "INSERT INTO organizations (org_id, name, slug, plan, status, owner_email)
         VALUES ($1, 'Integration Test Org', 'integration-test', 'pro', 'active', 'integration@zentra.test')
         ON CONFLICT DO NOTHING"
    )
    .bind(org_id)
    .execute(&pool)
    .await
    .expect("Org seed failed");

    // Seed test user if missing.
    let user_id = Uuid::new_v4();
    let password_hash = bcrypt::hash(TEST_USER_PASS, 10).expect("bcrypt failed");
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM users WHERE email = $1"
    )
    .bind(TEST_USER_EMAIL)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let user_id = if let Some(existing_id) = existing {
        existing_id
    } else {
        // Insert user (no role column — role comes from user_roles table).
        sqlx::query_scalar(
            "INSERT INTO users (id, org_id, email, username, password_hash, is_active)
             VALUES ($1, $2, $3, 'integration_tester', $4, true)
             RETURNING id"
        )
        .bind(user_id)
        .bind(org_id)
        .bind(TEST_USER_EMAIL)
        .bind(password_hash)
        .fetch_one(&pool)
        .await
        .expect("User seed failed")
    };

    // Ensure an 'Admin' system role exists and assign it to the test user.
    let role_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT role_id FROM roles WHERE name = 'Admin' AND org_id = $1 LIMIT 1"
    )
    .bind(org_id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    let role_id = if let Some(rid) = role_id {
        rid
    } else {
        // INSERT … ON CONFLICT DO UPDATE to guarantee RETURNING fires.
        sqlx::query_scalar(
            "INSERT INTO roles (org_id, name, is_system)
             VALUES ($1, 'Admin', true)
             ON CONFLICT (org_id, name) DO UPDATE SET is_system = EXCLUDED.is_system
             RETURNING role_id"
        )
        .bind(org_id)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|_| Uuid::new_v4())
    };

    let _ = sqlx::query(
        "INSERT INTO user_roles (user_id, role_id, org_id)
         VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING"
    )
    .bind(user_id)
    .bind(role_id)
    .bind(org_id)
    .execute(&pool)
    .await;

    let event_bus = zentra_stream::EventBus::new();
    let state = zentra_api::state::AppState::new(
        pool,
        tsdb_pool,
        event_bus,
        org_id,
        TEST_JWT_SECRET.to_string(),
    );

    let app = zentra_api::routes::build_router(state);
    let server = TestServer::new(app).expect("TestServer::new failed");
    let token = mint_test_jwt(user_id, org_id);

    (server, user_id, token)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

/// GET /api/health — public, always 200.
#[tokio::test]
async fn test_health_check() {
    let (server, _, _) = build_test_server().await;
    let resp = server.get("/api/health").await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["status"], "ok", "health response must include status=ok");
}

/// POST /api/auth/login — correct credentials → 200 + JWT.
#[tokio::test]
async fn test_login_success() {
    let (server, _, _) = build_test_server().await;
    let resp = server
        .post("/api/auth/login")
        .json(&json!({ "email": TEST_USER_EMAIL, "password": TEST_USER_PASS }))
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body["token"].is_string(), "login must return a JWT token");
}

/// POST /api/auth/login — wrong password → 401.
#[tokio::test]
async fn test_login_wrong_password() {
    let (server, _, _) = build_test_server().await;
    let resp = server
        .post("/api/auth/login")
        .json(&json!({ "email": TEST_USER_EMAIL, "password": "WrongPassword!" }))
        .await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

/// GET /api/targets — requires auth → 401 without token.
#[tokio::test]
async fn test_targets_requires_auth() {
    let (server, _, _) = build_test_server().await;
    let resp = server.get("/api/targets").await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

/// GET /api/targets — with valid JWT → 200 + array.
#[tokio::test]
async fn test_list_targets_authenticated() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/targets")
        .authorization_bearer(&token)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body.is_array() || body.get("targets").is_some(), "targets must return array");
}

/// POST /api/targets → 200/201 with created target, then PUT to update, then DELETE.
#[tokio::test]
async fn test_target_crud() {
    let (server, _, token) = build_test_server().await;

    // Create
    let resp = server
        .post("/api/targets")
        .authorization_bearer(&token)
        .json(&json!({
            "name": "integration-test-host",
            "host": "10.0.0.1",
            "port": 22,
            "type": "server",
            "enabled": true
        }))
        .await;
    let status = resp.status_code();
    assert!(
        status == axum::http::StatusCode::OK || status == axum::http::StatusCode::CREATED,
        "POST /api/targets must return 200 or 201, got {status}"
    );
    let created: Value = resp.json();
    let target_id = created["target_id"]
        .as_str()
        .or_else(|| created["id"].as_str())
        .expect("response must include target id");

    // Update
    let resp = server
        .put(&format!("/api/targets/{target_id}"))
        .authorization_bearer(&token)
        .json(&json!({ "name": "integration-test-host-updated" }))
        .await;
    let s = resp.status_code();
    assert!(
        s == axum::http::StatusCode::OK || s == axum::http::StatusCode::NO_CONTENT,
        "PUT /api/targets/:id must return 200 or 204, got {s}"
    );

    // Delete
    let resp = server
        .delete(&format!("/api/targets/{target_id}"))
        .authorization_bearer(&token)
        .await;
    let s = resp.status_code();
    assert!(
        s == axum::http::StatusCode::OK || s == axum::http::StatusCode::NO_CONTENT,
        "DELETE /api/targets/:id must return 200 or 204, got {s}"
    );
}

/// POST /api/alert-rules → create a rule, then delete it.
#[tokio::test]
async fn test_alert_rule_crud() {
    let (server, _, token) = build_test_server().await;

    // Create
    let resp = server
        .post("/api/alert-rules")
        .authorization_bearer(&token)
        .json(&json!({
            "name": "Integration Test Alert",
            "metric": "cpu_usage_percent",
            "operator": ">",
            "threshold": 95.0,
            "severity": "critical",
            "for_duration": "5m",
            "aggregation": "avg",
            "group": "Integration"
        }))
        .await;
    let s = resp.status_code();
    assert!(
        s == axum::http::StatusCode::OK || s == axum::http::StatusCode::CREATED,
        "POST /api/alert-rules must return 200 or 201, got {s}"
    );
    let created: Value = resp.json();
    let rule_id = created["rule_id"]
        .as_str()
        .or_else(|| created["id"].as_str())
        .expect("response must include rule id");

    // Delete
    let resp = server
        .delete(&format!("/api/alert-rules/{rule_id}"))
        .authorization_bearer(&token)
        .await;
    let s = resp.status_code();
    assert!(
        s == axum::http::StatusCode::OK || s == axum::http::StatusCode::NO_CONTENT,
        "DELETE /api/alert-rules/:id must return 200 or 204, got {s}"
    );
}

/// GET /api/alert-rules — authenticated list.
#[tokio::test]
async fn test_list_alert_rules() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/alert-rules")
        .authorization_bearer(&token)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body.is_array() || body.get("rules").is_some(), "alert-rules must return array");
}

/// GET /api/ha/cluster — must not be 500.
#[tokio::test]
async fn test_ha_cluster_not_500() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/ha/cluster")
        .authorization_bearer(&token)
        .await;
    let s = resp.status_code().as_u16();
    assert!(s < 500, "GET /api/ha/cluster must not return 5xx, got {s}");
}

/// GET /api/notifications/channels — authenticated list.
#[tokio::test]
async fn test_list_notification_channels() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/notifications/channels")
        .authorization_bearer(&token)
        .await;
    resp.assert_status_ok();
}

/// GET /api/alerts — authenticated list.
#[tokio::test]
async fn test_list_alerts() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/alerts")
        .authorization_bearer(&token)
        .await;
    resp.assert_status_ok();
}

/// GET /api/logs — authenticated list.
#[tokio::test]
async fn test_list_logs() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/logs?limit=10")
        .authorization_bearer(&token)
        .await;
    let s = resp.status_code().as_u16();
    assert!(s < 500, "GET /api/logs must not return 5xx, got {s}");
}

/// GET /api/slo/services — must not be 500.
#[tokio::test]
async fn test_slo_services_not_500() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/slo/services?window=1h")
        .authorization_bearer(&token)
        .await;
    let s = resp.status_code().as_u16();
    assert!(s < 500, "GET /api/slo/services must not return 5xx, got {s}");
}

/// GET /api/slo/targets — authenticated list.
#[tokio::test]
async fn test_slo_targets_list() {
    let (server, _, token) = build_test_server().await;
    let resp = server
        .get("/api/slo/targets")
        .authorization_bearer(&token)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body.get("targets").is_some(), "slo/targets must include targets key");
}

/// POST /api/slo/targets → create then delete.
#[tokio::test]
async fn test_slo_target_crud() {
    let (server, _, token) = build_test_server().await;

    let resp = server
        .post("/api/slo/targets")
        .authorization_bearer(&token)
        .json(&json!({
            "service_name": "integration-svc",
            "metric": "error_rate",
            "threshold": 1.0,
            "window_days": 30
        }))
        .await;
    let s = resp.status_code();
    assert!(
        s == axum::http::StatusCode::OK || s == axum::http::StatusCode::CREATED,
        "POST /api/slo/targets must return 200 or 201, got {s}"
    );
    let body: Value = resp.json();
    let id = body["id"].as_str().expect("response must include id");

    let resp = server
        .delete(&format!("/api/slo/targets/{id}"))
        .authorization_bearer(&token)
        .await;
    let s = resp.status_code().as_u16();
    assert!(s < 300, "DELETE /api/slo/targets/:id must succeed, got {s}");
}

/// Unauthenticated request to a protected endpoint returns 401, not 500.
#[tokio::test]
async fn test_protected_endpoint_returns_401_not_500() {
    let (server, _, _) = build_test_server().await;
    for path in ["/api/targets", "/api/alert-rules", "/api/alerts", "/api/notifications/channels"] {
        let resp = server.get(path).await;
        assert_eq!(
            resp.status_code(),
            axum::http::StatusCode::UNAUTHORIZED,
            "{path} without token must return 401"
        );
    }
}

/// Expired JWT is rejected with 401.
#[tokio::test]
async fn test_expired_jwt_rejected() {
    use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Claims {
        sub: Uuid, org: Uuid, email: String, role: String,
        exp: i64, iat: i64, jti: String,
    }

    let (server, user_id, _) = build_test_server().await;
    let org_id: Uuid = TEST_ORG_ID.parse().unwrap();
    let past = chrono::Utc::now().timestamp() - 7200; // 2 hours ago
    let claims = Claims {
        sub: user_id, org: org_id,
        email: TEST_USER_EMAIL.to_string(),
        role: "admin".to_string(),
        exp: past, iat: past - 3600,
        jti: Uuid::new_v4().to_string(),
    };
    let expired_token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(TEST_JWT_SECRET.as_bytes()),
    ).unwrap();

    let resp = server
        .get("/api/targets")
        .authorization_bearer(&expired_token)
        .await;
    assert_eq!(
        resp.status_code(),
        axum::http::StatusCode::UNAUTHORIZED,
        "expired JWT must return 401"
    );
}
