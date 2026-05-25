# Zentra — Execution Prompts: Phase 3 (Testing & CI/CD)
## Date: 2026-05-12
## Project: Zentra (Enterprise Monitoring & Operations Platform)
## Database: zentra (PostgreSQL on 10.88.11.162:5434)

---

## CRITICAL RULES FOR ALL PROMPTS

1. **DO NOT** break any existing code. All changes must be backward-compatible.
2. **DO NOT** change the login userid or password for the application.
3. **DO NOT** write to any database other than `zentra`. All database objects must be created under `zentra` only.
4. **DO NOT** do anything not explicitly mentioned in the prompt.
5. Each prompt must be executed sequentially — start the next prompt ONLY after the previous one is fully completed and verified.
6. After each prompt, verify the application still compiles (`cargo build`) and the frontend still builds (`cd frontend && npm run build`). 
7. **PREREQUISITE:** Phase 1 and Phase 2 prompts must be completed before starting Phase 3. The tests rely on the security fixes and validation from those phases.
8. When the development or execution gets stuck, stop the process, and then resume once again from where it was left off.
9. Build the frontend and backend on the remote server itself — do not build locally. If building on the remote server fails, only then fall back to building locally and deploying the artifacts. Otherwise, strictly follow remote deployment: push scripts and compile on the server.


---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 3: TESTING & CI/CD
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 3.1 — Create GitHub Actions CI Pipeline

**Objective:** Set up automated CI that runs on every push and pull request to catch regressions early.

**Files to create:**
- `.github/workflows/ci.yml`

**What to do:**
1. Create `.github/workflows/ci.yml` with the following jobs:

```yaml
name: Zentra CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

env:
  CARGO_TERM_COLOR: always
  ZENTRA_ALLOW_INSECURE_JWT: "1"
  ZENTRA_MASTER_KEY: "0000000000000000000000000000000000000000000000000000000000000000"

jobs:
  backend:
    name: Backend (Rust)
    runs-on: ubuntu-latest
    services:
      postgres:
        image: timescale/timescaledb:latest-pg15
        env:
          POSTGRES_USER: postgres
          POSTGRES_PASSWORD: testpassword
          POSTGRES_DB: zentra
        ports:
          - 5432:5432
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    env:
      DATABASE_URL: postgresql://postgres:testpassword@localhost:5432/zentra
      ZENTRA_JWT_SECRET: test-jwt-secret-at-least-32-bytes-long-for-ci
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo registry & build
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run migrations
        run: |
          cargo install sqlx-cli --no-default-features --features postgres || true
          sqlx migrate run --database-url "${DATABASE_URL}" --source ./migrations

      - name: Run tests
        run: cargo test --workspace

      - name: Security audit
        run: |
          cargo install cargo-audit || true
          cargo audit

  frontend:
    name: Frontend (TypeScript)
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: frontend/package-lock.json

      - name: Install dependencies
        run: npm ci

      - name: Type check
        run: npm run typecheck

      - name: Build
        run: npm run build

      - name: Security audit
        run: npm audit --audit-level=high || true
```

2. Do NOT add any deployment steps — this is CI only.
3. Do NOT include any real credentials.
4. The database service uses `zentra` as the database name.
5. Do NOT change any existing files.

**Verification:**
- The YAML file must be valid (use `yamllint` or similar)
- The workflow should run on push to main/develop and on PRs

---

## PROMPT 3.2 — Add Backend Test Infrastructure

**Objective:** Create test utilities and fixtures that all integration tests can use. This sets up the foundation for writing tests in subsequent prompts.

**Files to create:**
- `crates/zentra-api/tests/common/mod.rs` — Shared test utilities
- `crates/zentra-api/tests/common/fixtures.rs` — Test data fixtures

**What to do:**
1. Create `crates/zentra-api/tests/common/mod.rs`:
   ```rust
   //! Shared test utilities for zentra-api integration tests.
   //! All tests use the zentra database only.

   pub mod fixtures;

   use axum::Router;
   use sqlx::PgPool;
   use zentra_api::state::AppState;
   use zentra_api::routes::build_router;
   use zentra_stream::EventBus;
   use uuid::Uuid;

   /// Create a test app state with a real database connection.
   /// Requires DATABASE_URL environment variable pointing to the zentra database.
   pub async fn test_state() -> (AppState, PgPool) {
       let db_url = std::env::var("DATABASE_URL")
           .unwrap_or_else(|_| "postgresql://postgres:testpassword@localhost:5432/zentra".into());

       let pool = sqlx::PgPool::connect(&db_url)
           .await
           .expect("Failed to connect to zentra test database");

       // Run migrations
       sqlx::migrate!("../../migrations")
           .run(&pool)
           .await
           .expect("Failed to run migrations on zentra database");

       let event_bus = EventBus::new();
       let org_id = Uuid::new_v4();
       let jwt_secret = "test-jwt-secret-at-least-32-bytes-long-for-testing".to_string();

       let state = AppState::new(
           pool.clone(),
           pool.clone(), // Use same pool for tsdb in tests
           event_bus,
           org_id,
           jwt_secret,
       );

       (state, pool)
   }

   /// Build the full router for integration testing.
   pub async fn test_router() -> (Router, AppState, PgPool) {
       let (state, pool) = test_state().await;
       let router = build_router(state.clone());
       (router, state, pool)
   }

   /// Clean up test data created during a test.
   /// Only deletes from the zentra database.
   pub async fn cleanup_test_data(pool: &PgPool, org_id: Uuid) {
       let _ = sqlx::query("DELETE FROM targets WHERE org_id = $1")
           .bind(org_id)
           .execute(pool)
           .await;
       let _ = sqlx::query("DELETE FROM users WHERE org_id = $1")
           .bind(org_id)
           .execute(pool)
           .await;
   }
   ```

2. Create `crates/zentra-api/tests/common/fixtures.rs`:
   ```rust
   //! Test data fixtures for zentra-api integration tests.

   use serde_json::json;
   use uuid::Uuid;

   /// Create a test login request body
   pub fn login_request(email: &str, password: &str) -> serde_json::Value {
       json!({
           "email": email,
           "password": password
       })
   }

   /// Create a test target request body
   pub fn create_target_request(name: &str, host: &str) -> serde_json::Value {
       json!({
           "name": name,
           "host": host,
           "port": 22,
           "target_subtype": "linux",
           "environment": "test"
       })
   }

   /// Create a test user request body
   pub fn create_user_request(email: &str, display_name: &str) -> serde_json::Value {
       json!({
           "email": email,
           "display_name": display_name,
           "password": "TestP@ssw0rd!123",
           "role": "viewer"
       })
   }

   /// Generate a unique test email
   pub fn test_email() -> String {
       format!("test-{}@zentra-test.local", Uuid::new_v4().to_string()[..8].to_string())
   }
   ```

3. Do NOT create any actual test functions yet — those come in subsequent prompts.
4. Do NOT change any existing source files.
5. All database references must point to the `zentra` database.

**Verification:**
- `cargo build --tests -p zentra-api` must succeed (even if no tests run yet)
- The test utilities compile without errors

---

## PROMPT 3.3 — Write Authentication Integration Tests

**Objective:** Write integration tests for the complete authentication flow: login, token refresh, MFA, lockout, logout, and token revocation.

**File to create:** `crates/zentra-api/tests/auth_tests.rs`

**Prerequisite:** Prompt 3.2 must be completed first.

**What to do:**
1. Create `crates/zentra-api/tests/auth_tests.rs` with the following tests:

```rust
//! Authentication integration tests for Zentra.
//! All tests use the zentra database only.
//! Tests do NOT change the login userid or password for the application.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

/// Test: Valid login returns a JWT token and user info
#[tokio::test]
async fn test_login_success() {
    // Setup: Create a test user in the zentra database
    // Action: POST /api/auth/login with valid credentials
    // Assert: Status 200, response contains "token" field
    // Cleanup: Remove test user
}

/// Test: Invalid password returns 401
#[tokio::test]
async fn test_login_invalid_password() {
    // Action: POST /api/auth/login with wrong password
    // Assert: Status 401, response contains error message
    // Assert: Response does NOT contain raw database errors
}

/// Test: Non-existent email returns 401 (not 404 — no user enumeration)
#[tokio::test]
async fn test_login_nonexistent_user() {
    // Action: POST /api/auth/login with non-existent email
    // Assert: Status 401 (same as wrong password)
}

/// Test: Account lockout after 5 failed attempts
#[tokio::test]
async fn test_account_lockout() {
    // Setup: Create a test user
    // Action: POST /api/auth/login 5 times with wrong password
    // Assert: 6th attempt returns 429 (Too Many Requests)
    // Cleanup: Remove test user and lockout records
}

/// Test: Token refresh returns a new valid token
#[tokio::test]
async fn test_token_refresh() {
    // Setup: Login to get token + refresh_token
    // Action: POST /api/auth/refresh with refresh_token
    // Assert: Status 200, new token returned
    // Assert: New token can be used for authenticated requests
}

/// Test: Logout invalidates the session (token revocation)
#[tokio::test]
async fn test_logout_revokes_token() {
    // Setup: Login to get token
    // Action: POST /api/auth/logout with token
    // Assert: Using the old token returns 401
}

/// Test: Expired refresh token returns 401
#[tokio::test]
async fn test_expired_refresh_token() {
    // Action: POST /api/auth/refresh with an expired/invalid refresh token
    // Assert: Status 401
}

/// Test: Password change with weak password returns 400
#[tokio::test]
async fn test_change_password_weak() {
    // Setup: Login as test user
    // Action: POST /api/auth/change-password with weak password "abc"
    // Assert: Status 400, error lists requirements
}

/// Test: Password change with strong password succeeds
#[tokio::test]
async fn test_change_password_strong() {
    // Setup: Login as test user
    // Action: POST /api/auth/change-password with strong password
    // Assert: Status 200
    // Assert: Can login with new password
    // Cleanup: Reset password or delete test user
}
```

2. Implement each test fully with actual HTTP requests using `tower::ServiceExt::oneshot()`.
3. Each test must create its own test user (not use the admin account) and clean up after itself.
4. All database operations must target the `zentra` database.
5. Do NOT change any existing application code.
6. Do NOT change the login userid or password.
7. Do NOT modify the admin account in any way.

**Verification:**
- `cargo test -p zentra-api --test auth_tests` must compile
- Tests should pass against a running zentra database
- No test should leave behind test data in the database

---

## PROMPT 3.4 — Write Multi-Tenant Isolation Tests

**Objective:** Verify that data from one organization cannot be accessed by another organization.

**File to create:** `crates/zentra-api/tests/tenant_isolation_tests.rs`

**Prerequisite:** Prompt 3.3 must be completed first.

**What to do:**
1. Create `crates/zentra-api/tests/tenant_isolation_tests.rs` with:

```rust
//! Multi-tenant isolation tests for Zentra.
//! Verifies that org_id filtering is enforced on all CRUD endpoints.
//! All tests use the zentra database only.

mod common;

/// Test: Org A cannot see Org B's targets
#[tokio::test]
async fn test_target_isolation() {
    // Setup: Create two test orgs with different org_ids
    // Setup: Create a target for Org A in the zentra database
    // Action: List targets as Org B
    // Assert: Org A's target is NOT in Org B's list
    // Cleanup: Delete test data
}

/// Test: Org A cannot see Org B's alerts
#[tokio::test]
async fn test_alert_isolation() {
    // Similar pattern as above for alerts
}

/// Test: Org A cannot see Org B's dashboards
#[tokio::test]
async fn test_dashboard_isolation() {
    // Similar pattern as above for dashboards
}

/// Test: Org A cannot access Org B's target by ID
#[tokio::test]
async fn test_target_direct_access_isolation() {
    // Setup: Create a target for Org A, get its UUID
    // Action: GET /api/targets/{uuid} as Org B
    // Assert: Status 404 (not 403 — don't reveal existence)
}

/// Test: Org A cannot update Org B's target
#[tokio::test]
async fn test_target_update_isolation() {
    // Setup: Create a target for Org A
    // Action: PUT /api/targets/{uuid} as Org B
    // Assert: Status 404
    // Assert: Target is unchanged in database
}

/// Test: Org A cannot delete Org B's target
#[tokio::test]
async fn test_target_delete_isolation() {
    // Setup: Create a target for Org A
    // Action: DELETE /api/targets/{uuid} as Org B
    // Assert: Status 404
    // Assert: Target still exists in database
}
```

2. Implement each test with actual database operations against the `zentra` database.
3. Each test must create two separate org_ids and JWT tokens.
4. Do NOT change any application code.
5. Do NOT change the login userid or password.
6. Clean up all test data after each test.

**Verification:**
- `cargo test -p zentra-api --test tenant_isolation_tests` must compile
- All tests must pass

---

## PROMPT 3.5 — Write Security Validation Tests

**Objective:** Write tests to verify the security fixes from Phase 1 (SSRF blocking, input validation, error sanitization).

**File to create:** `crates/zentra-api/tests/security_tests.rs`

**Prerequisite:** Phase 1 prompts and Prompt 3.2 must be completed first.

**What to do:**
1. Create `crates/zentra-api/tests/security_tests.rs` with:

```rust
//! Security validation tests for Zentra.
//! Verifies Phase 1 security fixes are working correctly.
//! All tests use the zentra database only.

mod common;

// === SSRF Tests ===

/// Test: SSH verify rejects localhost
#[tokio::test]
async fn test_ssrf_block_localhost() {
    // Action: POST /api/targets/verify-ssh with host "localhost"
    // Assert: Status 400 with security error message
}

/// Test: SSH verify rejects 127.0.0.1
#[tokio::test]
async fn test_ssrf_block_loopback() {
    // Action: POST /api/targets/verify-ssh with host "127.0.0.1"
    // Assert: Status 400
}

/// Test: SSH verify rejects cloud metadata endpoint
#[tokio::test]
async fn test_ssrf_block_metadata() {
    // Action: POST /api/targets/verify-ssh with host "169.254.169.254"
    // Assert: Status 400
}

/// Test: SSH verify allows valid hosts (10.88.x.x range)
#[tokio::test]
async fn test_ssrf_allow_valid_host() {
    // Action: POST /api/targets/verify-ssh with host "10.88.11.162"
    // Assert: Status is NOT 400 (may be timeout or connection refused, but not SSRF block)
}

// === Input Validation Tests ===

/// Test: Empty host is rejected
#[tokio::test]
async fn test_validation_empty_host() {
    // Action: POST /api/targets/verify-ssh with host ""
    // Assert: Status 400
}

/// Test: Oversized host is rejected
#[tokio::test]
async fn test_validation_long_host() {
    // Action: POST /api/targets/verify-ssh with host "a".repeat(300)
    // Assert: Status 400
}

/// Test: Invalid email in user creation is rejected
#[tokio::test]
async fn test_validation_invalid_email() {
    // Action: POST /api/users with email "not-an-email"
    // Assert: Status 400
}

// === Error Sanitization Tests ===

/// Test: Database errors are not exposed in API responses
#[tokio::test]
async fn test_error_sanitization() {
    // Action: Trigger a database error (e.g., query with invalid UUID format)
    // Assert: Response body does NOT contain "sqlx", "pg_", "column", "relation"
    // Assert: Response body contains generic error message
}

// === Password Strength Tests ===

/// Test: Weak password is rejected during password change
#[tokio::test]
async fn test_password_strength_weak() {
    // Setup: Login as test user
    // Action: POST /api/auth/change-password with password "abc"
    // Assert: Status 400 with descriptive requirements message
}

/// Test: Strong password is accepted
#[tokio::test]
async fn test_password_strength_strong() {
    // Setup: Login as test user
    // Action: POST /api/auth/change-password with password "MyStr0ng!Pass99"
    // Assert: Status 200
}
```

2. Implement each test fully.
3. Do NOT change any application code.
4. Do NOT change the login userid or password.
5. All database operations must target the `zentra` database only.

**Verification:**
- `cargo test -p zentra-api --test security_tests` must compile and pass
- No test should modify production data

---

## PROMPT 3.6 — Write Core Crate Unit Tests

**Objective:** Add unit tests to the core crates that currently have minimal test coverage.

**Files to modify:**
- `crates/zentra-query/src/` — Add PromQL parser tests
- `crates/zentra-netflow/src/` — Add NetFlow parser tests
- `crates/zentra-ingestion/src/` — Add SNMP BER decoder tests

**What to do:**

### 3.6.1 — PromQL Parser Tests
In the zentra-query crate, find the PromQL parser module and add a `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_metric() {
        // "cpu_usage" should parse to a metric selector
    }

    #[test]
    fn test_parse_metric_with_labels() {
        // 'cpu_usage{host="prod-1"}' should parse correctly
    }

    #[test]
    fn test_parse_aggregation() {
        // 'sum(cpu_usage) by (host)' should parse to aggregation node
    }

    #[test]
    fn test_parse_rate_function() {
        // 'rate(cpu_usage[5m])' should parse to function call with range
    }

    #[test]
    fn test_parse_nested_expression() {
        // 'sum(rate(cpu_usage[5m])) by (host)' should parse correctly
    }

    #[test]
    fn test_parse_binary_operation() {
        // 'cpu_usage > 80' should parse to binary op
    }

    #[test]
    fn test_parse_empty_input() {
        // "" should return an error, not panic
    }

    #[test]
    fn test_parse_malformed_input() {
        // "{{{{" should return an error, not panic
    }

    #[test]
    fn test_parse_unmatched_parens() {
        // "sum(rate(cpu[5m])" (missing closing paren) should error
    }
}
```

### 3.6.2 — NetFlow Parser Tests
In the zentra-netflow crate, add tests for v5 parsing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_v5_valid_packet() {
        // Construct a valid NetFlow v5 packet (24-byte header + 48-byte record)
        // Verify it parses correctly
    }

    #[test]
    fn test_parse_v5_truncated_packet() {
        // Packet shorter than 24 bytes should error, not panic
    }

    #[test]
    fn test_parse_v5_record_count_mismatch() {
        // Header says 5 records but only 2 records of data
        // Should parse only the available records, not crash
    }

    #[test]
    fn test_parse_v5_zero_records() {
        // Valid header with count=0 should return empty list
    }

    #[test]
    fn test_parse_v5_max_records() {
        // Header says 30 records (max for v5) with full data
        // Should parse all 30
    }
}
```

### 3.6.3 — SNMP BER Decoder Tests
In the zentra-ingestion crate, add tests for BER decoding:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_integer() {
        // BER-encoded integer should decode correctly
    }

    #[test]
    fn test_decode_octet_string() {
        // BER-encoded string should decode correctly
    }

    #[test]
    fn test_decode_oid() {
        // BER-encoded OID should decode correctly
    }

    #[test]
    fn test_decode_truncated_data() {
        // Truncated BER data should return error, not panic
    }

    #[test]
    fn test_decode_zero_length() {
        // Zero-length BER data should handle gracefully
    }
}
```

**Implementation notes:**
1. Implement each test with actual test data (construct binary packets, BER-encoded data, PromQL strings).
2. For NetFlow, create test packets with correct byte layout (big-endian).
3. Do NOT change any production code — only add `#[cfg(test)]` blocks.
4. Do NOT change any function signatures.

**Verification:**
- `cargo test -p zentra-query` must pass
- `cargo test -p zentra-netflow` must pass
- `cargo test -p zentra-ingestion` must pass

---

## PROMPT 3.7 — Add Frontend Test Infrastructure

**Objective:** Set up Vitest and React Testing Library for the frontend and write initial tests.

**Files to modify:**
- `frontend/package.json` — Add test dependencies
- `frontend/vitest.config.ts` — Create Vitest config

**Files to create:**
- `frontend/vitest.config.ts`
- `frontend/src/setupTests.ts`
- `frontend/src/lib/api/__tests__/_client.test.ts`
- `frontend/src/contexts/__tests__/AuthContext.test.tsx`

**What to do:**
1. Add test dependencies to `frontend/package.json` devDependencies:
   ```json
   "@testing-library/react": "^14.0.0",
   "@testing-library/jest-dom": "^6.0.0",
   "@testing-library/user-event": "^14.0.0",
   "vitest": "^1.6.0",
   "jsdom": "^24.0.0",
   "@vitest/coverage-v8": "^1.6.0"
   ```

2. Add test script to `frontend/package.json` scripts:
   ```json
   "test": "vitest run",
   "test:watch": "vitest",
   "test:coverage": "vitest run --coverage"
   ```

3. Create `frontend/vitest.config.ts`:
   ```typescript
   import { defineConfig } from 'vitest/config'
   import react from '@vitejs/plugin-react'
   import path from 'path'

   export default defineConfig({
     plugins: [react()],
     test: {
       environment: 'jsdom',
       setupFiles: './src/setupTests.ts',
       globals: true,
     },
     resolve: {
       alias: {
         '@': path.resolve(__dirname, './src'),
       },
     },
   })
   ```

4. Create `frontend/src/setupTests.ts`:
   ```typescript
   import '@testing-library/jest-dom'
   ```

5. Create `frontend/src/lib/api/__tests__/_client.test.ts`:
   ```typescript
   import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

   describe('fetchJSON', () => {
     beforeEach(() => {
       // Mock fetch and localStorage
       vi.stubGlobal('fetch', vi.fn())
       vi.stubGlobal('localStorage', {
         getItem: vi.fn(),
         setItem: vi.fn(),
         removeItem: vi.fn(),
       })
     })

     afterEach(() => {
       vi.restoreAllMocks()
     })

     it('should add Authorization header when token exists', async () => {
       // Test that fetchJSON adds Bearer token from localStorage
     })

     it('should attempt token refresh on 401', async () => {
       // Test the 401 → refresh → retry flow
     })

     it('should dispatch auth:logout event when refresh fails', async () => {
       // Test that failed refresh triggers logout
     })

     it('should throw on non-OK response', async () => {
       // Test that 500 errors throw with status code
     })
   })
   ```

6. Create `frontend/src/contexts/__tests__/AuthContext.test.tsx`:
   ```typescript
   import { describe, it, expect, vi } from 'vitest'
   import { renderHook } from '@testing-library/react'

   describe('AuthContext', () => {
     it('should provide user state after login', () => {
       // Test login updates context state
     })

     it('should clear state on logout', () => {
       // Test logout clears tokens and user
     })

     it('should check permissions correctly', () => {
       // Test hasPermission returns correct values
     })
   })
   ```

7. Implement the test bodies with proper mocking.
8. Do NOT change any existing source files (except package.json for dependencies).
9. Do NOT change any API endpoints.

**Verification:**
- `cd frontend && npm install` must succeed
- `cd frontend && npm test` must run and pass
- `cd frontend && npm run build` must still succeed

---

## PROMPT 3.8 — Add Pre-Commit Hooks

**Objective:** Add git pre-commit hooks to enforce code quality before commits.

**Files to create:**
- `.pre-commit-config.yaml`
- `scripts/pre-commit.sh`

**What to do:**
1. Create `scripts/pre-commit.sh`:
   ```bash
   #!/usr/bin/env bash
   # Zentra pre-commit checks
   set -e

   echo "=== Checking Rust formatting ==="
   cargo fmt --all -- --check

   echo "=== Running Clippy ==="
   cargo clippy --all-targets -- -D warnings

   echo "=== Checking TypeScript ==="
   cd frontend
   npm run typecheck 2>/dev/null || echo "TypeScript check skipped (npm not installed)"
   cd ..

   echo "=== Pre-commit checks passed ==="
   ```

2. Create `.pre-commit-config.yaml`:
   ```yaml
   # Zentra pre-commit configuration
   # Install: pip install pre-commit && pre-commit install
   repos:
     - repo: local
       hooks:
         - id: zentra-checks
           name: Zentra Pre-Commit Checks
           entry: bash scripts/pre-commit.sh
           language: system
           pass_filenames: false
           always_run: true
   ```

3. Make `scripts/pre-commit.sh` executable (note this in the file header).
4. Do NOT install pre-commit hooks automatically — users should opt in.
5. Do NOT change any existing files.

**Verification:**
- `bash scripts/pre-commit.sh` should run all checks
- `bash -n scripts/pre-commit.sh` should pass syntax check

---

## PROMPT 3.9 — Update CI Pipeline with Test Jobs

**Objective:** Update the CI pipeline to run the tests created in prompts 3.3-3.7.

**File to modify:** `.github/workflows/ci.yml` (created in Prompt 3.1)

**Prerequisite:** Prompts 3.1 through 3.7 must be completed first.

**What to do:**
1. Update the `backend` job to include:
   ```yaml
   - name: Run unit tests
     run: cargo test --workspace --lib

   - name: Run integration tests
     run: cargo test --workspace --test '*'
     env:
       DATABASE_URL: postgresql://postgres:testpassword@localhost:5432/zentra
   ```

2. Update the `frontend` job to include:
   ```yaml
   - name: Run tests
     run: npm test

   - name: Run coverage
     run: npm run test:coverage
   ```

3. Do NOT change any test files.
4. Do NOT add deployment steps.
5. The database in CI must be `zentra`.

**Verification:**
- CI YAML should be valid
- All test jobs should reference the zentra database

---

## END OF PHASE 3 PROMPTS
