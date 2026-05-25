# Zentra — Execution Prompts: Phase 1, 2, 4, 5, 6, 8
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
7. When the development or execution gets stuck, stop the process, and then resume once again from where it was left off.
8. Build the frontend and backend on the remote server itself — do not build locally. If building on the remote server fails, only then fall back to building locally and deploying the artifacts. Otherwise, strictly follow remote deployment: push scripts and compile on the server.


---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 1: CRITICAL SECURITY FIXES
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 1.1 — Fix SQL/Command Injection in Oracle SSH Queries

**Objective:** Eliminate the command injection vulnerability in `oracle_ssh_query()` where user-supplied SQL is interpolated into a shell command via `format!()`.

**File to modify:** `crates/zentra-api/src/handlers/oracle.rs`

**Current vulnerable code (around line 108-115):**
```rust
let escaped_sql = sql.replace('$', "\\$");
let cmd = format!(
    "echo \"SET PAGESIZE 0\nSET FEEDBACK OFF\nSET HEADING OFF\nSET LINESIZE 1000\nSET TRIMOUT ON\nSET TRIMSPOOL ON\n{}\nEXIT;\" | sqlplus -S {}/{}@{}:{}/{}",
    escaped_sql, db_user, db_pass, host, target.port, db_name
);
let result = ssh_utils::ssh_exec(host, &ssh_user, &cmd).await;
```

**What to do:**
1. In the `oracle_ssh_query()` function, replace the `format!()` shell command approach with a safer pattern that passes SQL via a heredoc or base64-encoded stdin to avoid shell interpretation.
2. The fix should:
   - Build the SQL input string (SET PAGESIZE 0, SET FEEDBACK OFF, etc.) as before
   - Base64-encode the entire SQL input on the local side
   - Pass it to the remote host via SSH using `echo <base64> | base64 -d | sqlplus -S ...`
   - This prevents any shell metacharacters in the SQL from being interpreted
3. Also ensure the password (`db_pass`) is not exposed in the command line. If the password contains shell metacharacters, they must not break the command.
4. The `oracle_local_query()` function (lines 27-85) is already safe — it uses stdin pipe via `tokio::process::Command`. Do NOT change `oracle_local_query()`.
5. Do NOT change any function signatures or return types.
6. Do NOT change any other handlers or endpoints.

**Verification:**
- `cargo build` must succeed
- All existing Oracle endpoints must still work
- Test with SQL containing special characters: `SELECT 'test$var' FROM dual` and `SELECT * FROM v$session WHERE username='$(whoami)'`

---

## PROMPT 1.2 — Fix SSH Password Injection in Expect Script

**Objective:** Ensure the SSH password is never interpolated into shell commands or expect scripts where special characters could cause injection.

**File to modify:** `crates/zentra-api/src/ssh_utils.rs`

**Current code (around line 198-210):**
The `setup_ssh_key_manual()` function builds an expect script where the password is read from `$env(SSHPASS)` environment variable, which is already the correct approach. However, verify and ensure:

**What to do:**
1. In `setup_ssh_key_manual()`, confirm that the password is ONLY passed via the `SSHPASS` environment variable (`.env("SSHPASS", password)`) and read inside expect as `$env(SSHPASS)`.
2. The expect script line `set pass $env(SSHPASS)` and `send "$pass\r"` is correct — verify this pattern is used and no other place interpolates the password directly.
3. Add a comment above the expect script block explaining why the password is passed via env var (to prevent command injection via special characters in passwords).
4. In the `ssh_exec()` function (around line 60), add a comment noting that `StrictHostKeyChecking=no` is a known security trade-off (tracked for Phase 7 fix).
5. Do NOT change any function signatures.
6. Do NOT change the `setup_ssh_key_auth()` function (it correctly uses `.env("SSHPASS", password)`).

**Verification:**
- `cargo build` must succeed
- SSH key setup must still work for hosts with key-based auth

---

## PROMPT 1.3 — Block SSRF via Arbitrary SSH Host

**Objective:** Prevent Server-Side Request Forgery (SSRF) by validating that SSH target hosts are not internal/private IP addresses or cloud metadata endpoints.

**File to modify:** `crates/zentra-api/src/handlers/targets.rs`

**Current vulnerable code (around line 241-267):**
```rust
pub async fn verify_ssh(
    Json(req): Json<VerifySshRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let user = req.username.unwrap_or_else(|| {
        std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into())
    });
    let result = ssh_utils::ssh_exec(&req.host, &user, "echo ZENTRA_VERIFY_OK && hostname").await;
```

**What to do:**
1. Create a new helper function `fn is_safe_ssh_host(host: &str) -> bool` at the top of the file (or in a new `validation.rs` module inside `crates/zentra-api/src/`).
2. This function must reject:
   - `127.0.0.0/8` (loopback)
   - `10.0.0.0/8` — BUT ONLY reject `169.254.169.254` (AWS metadata) and `169.254.0.0/16` (link-local). Note: `10.x.x.x` addresses ARE allowed because Zentra targets are on `10.88.x.x` network.
   - `169.254.169.254` specifically (cloud metadata endpoint)
   - `0.0.0.0`
   - `localhost`
   - Any IPv6 loopback (`::1`)
   - Empty strings
3. Apply this validation in:
   - `verify_ssh()` — before calling `ssh_exec()`
   - `setup_ssh()` — before calling `setup_ssh_key_auth()`
   - `create_target()` — validate `req.host` before saving
4. Return a `400 Bad Request` with message `"Invalid host: this address is not allowed for security reasons"` if validation fails.
5. Allow the `ZENTRA_ALLOW_PRIVATE_SSH` environment variable to bypass this check (for development). If `ZENTRA_ALLOW_PRIVATE_SSH=1`, skip validation. Default is to enforce.
6. Do NOT change any existing function signatures or return types.

**Verification:**
- `cargo build` must succeed
- `verify_ssh` with `host: "169.254.169.254"` must return 400
- `verify_ssh` with `host: "localhost"` must return 400
- `verify_ssh` with `host: "10.88.11.162"` must still work (allowed private range)

---

## PROMPT 1.4 — Remove Hardcoded Credentials from Deploy Scripts

**Objective:** Replace all hardcoded passwords, IP addresses (where used as credentials), and secrets in deployment scripts with environment variable references.

**Files to modify:**
- `deploy/setup_server.sh` (line 11: `DB_URL="postgresql://postgres:zantaz@10.88.11.162:5434/postgres"`)
- `deploy/run-migrations.sh` (line 3: `export PGPASSWORD=zantaz`)
- `deploy/cleanup-postgres-db.sh` (line 6: `export PGPASSWORD=zantaz`)
- `deploy/reset-admin.sh` (line 5: `PGPASSWORD=zantaz`)
- `deploy/debug-sql.sh` (line 3: `SYS/OraclePass#21`)
- `deploy/debug-sql2.sh` (line 5: `SYS/OraclePass#21`)
- `deploy/login.json` (contains admin@smarsh-nexus.com / admin123)
- `deploy/oraconn.json` (contains SYS / OraclePass#21)
- `deploy/k8s/zentra-deployment.yaml` (line 19: plaintext database-url and jwt-secret)
- `deploy/k8s/oracle-exporter-deployment.yaml` (line 117: plaintext dsn)

**What to do:**
1. In each shell script, replace hardcoded passwords with environment variable references:
   - Replace `export PGPASSWORD=zantaz` with `export PGPASSWORD="${PGPASSWORD:?Error: PGPASSWORD not set}"`
   - Replace `DB_URL="postgresql://postgres:zantaz@..."` with `DB_URL="${DATABASE_URL:?Error: DATABASE_URL not set}"`
   - Replace `SYS/OraclePass#21` with `${ORACLE_USER:-SYS}/${ORACLE_PASSWORD:?Error: ORACLE_PASSWORD not set}`
2. In `deploy/login.json` — change to `deploy/login.json.example` and add a note: "Copy to login.json and fill in your credentials"
3. In `deploy/oraconn.json` — change to `deploy/oraconn.json.example` and add a note: "Copy to oraconn.json and fill in your credentials"
4. In K8s YAML files, replace plaintext secrets with placeholder comments:
   - `database-url: "<SET_VIA_SEALED_SECRETS_OR_VAULT>"`
   - `jwt-secret: "<SET_VIA_SEALED_SECRETS_OR_VAULT>"`
5. Do NOT change the `deploy/test-endpoints.sh` script (it reads from files, which is fine).
6. Do NOT change any Rust source code in this prompt.
7. Do NOT change the database name from `zentra`.
8. Do NOT change any login userid or password in the application itself.
9. The `run-migrations.sh` script should reference `DB_NAME=zentra` (not `postgres`).

**Verification:**
- All scripts must still be syntactically valid bash (`bash -n deploy/*.sh`)
- Scripts should fail with clear error if env vars are not set

---

## PROMPT 1.5 — Fail-Hard When ZENTRA_MASTER_KEY Not Set in Production

**Objective:** Prevent the application from starting with the insecure all-zeros dev encryption key when running in production mode.

**File to modify:** `crates/zentra-core/src/crypto.rs`

**Current code (around line 20-28):**
```rust
const DEV_KEY_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn master_key_bytes() -> Result<[u8; 32], String> {
    let hex = std::env::var("ZENTRA_MASTER_KEY").unwrap_or_else(|_| {
        tracing::warn!("ZENTRA_MASTER_KEY not set — using insecure dev key. DO NOT use in production.");
        DEV_KEY_HEX.to_string()
    });
```

**What to do:**
1. Modify `master_key_bytes()` to check `ZENTRA_ALLOW_INSECURE_JWT` environment variable (already used in main.rs for JWT).
2. If `ZENTRA_ALLOW_INSECURE_JWT` is NOT `"1"` AND `ZENTRA_MASTER_KEY` is not set:
   - Return `Err("ZENTRA_MASTER_KEY is required in production. Set a 64-char hex string (32 bytes). To bypass for local dev, set ZENTRA_ALLOW_INSECURE_JWT=1".to_string())`
3. If `ZENTRA_ALLOW_INSECURE_JWT` IS `"1"` AND `ZENTRA_MASTER_KEY` is not set:
   - Keep the current behavior (warn and use dev key)
4. Do NOT change the `encrypt()`, `decrypt()`, or `is_encrypted()` functions.
5. Do NOT change the existing tests — they set `ZENTRA_MASTER_KEY` to `DEV_KEY_HEX` before calling, so they will still pass.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-core` must pass (existing tests set the env var)
- Without `ZENTRA_MASTER_KEY` and without `ZENTRA_ALLOW_INSECURE_JWT=1`, `encrypt()` should return an error

---

## PROMPT 1.6 — Force Password Change on Default Admin Account

**Objective:** Add a `must_change_password` flag to the users table and enforce it on login for the default admin account.

**Files to modify:**
- `migrations/` — Create a NEW migration file `migrations/20250101000023_must_change_password.sql`
- `crates/zentra-api/src/handlers/auth.rs` — Modify login handler to check the flag

**What to do:**
1. Create migration `migrations/20250101000023_must_change_password.sql` with:
   ```sql
   -- Add must_change_password flag to users table (zentra database only)
   ALTER TABLE users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT false;
   -- Set flag for default admin accounts
   UPDATE users SET must_change_password = true WHERE email IN ('admin@Zentra.local', 'admin@smarsh-nexus.com');
   ```
2. In `crates/zentra-api/src/handlers/auth.rs`, in the login handler:
   - After successful password verification, check if `must_change_password` is `true` for the user
   - If true, return a JSON response with `"must_change_password": true` alongside the token (do NOT block login — just signal the frontend)
   - The frontend can then prompt the user to change their password
3. Add a `POST /api/auth/change-password` endpoint if it doesn't already exist:
   - Accept `{ "current_password": "...", "new_password": "..." }`
   - Verify current password
   - Hash new password with bcrypt
   - Update password_hash and set `must_change_password = false`
   - Do NOT change the user's email or any other field
4. Do NOT change the login userid or password.
5. Do NOT change the existing login flow — only ADD the `must_change_password` field to the response.
6. Database must be `zentra` only — the migration runs against the zentra database.

**Verification:**
- `cargo build` must succeed
- Login with existing credentials must still work
- Login response should include `must_change_password: true` for admin accounts
- After changing password, `must_change_password` should be `false`

---

## PROMPT 1.7 — Implement JWT Token Revocation

**Objective:** Check the JWT's `jti` (JWT ID) claim against the `user_sessions` table during authentication to enable token revocation.

**File to modify:** `crates/zentra-api/src/auth_middleware.rs` (or wherever `require_auth` is defined)

**What to do:**
1. Find the `require_auth` middleware function.
2. After successfully validating the JWT signature and decoding claims:
   - Extract the `jti` field from the JWT claims (this is the session_id stored in `user_sessions`)
   - Query `SELECT 1 FROM user_sessions WHERE session_id = $1 AND expires_at > NOW()` against the zentra database
   - If no row found, return `401 Unauthorized` with message `"Session revoked or expired"`
3. In the logout handler (in `auth.rs`):
   - Ensure it DELETES the session from `user_sessions` (this effectively revokes the token)
4. Do NOT change the JWT claims structure.
5. Do NOT change the login flow (session is already created on login).
6. Do NOT add new tables — use the existing `user_sessions` table.
7. To avoid excessive DB hits, add a simple TTL cache: skip the DB check if the same jti was validated within the last 60 seconds. Use a `DashMap<String, Instant>` for the cache.
8. All queries must target the zentra database only.

**Verification:**
- `cargo build` must succeed
- Login must still work and return a valid token
- After logout, using the old token must return 401
- Normal authenticated requests must still work

---

## PROMPT 1.8 — Add Password Strength Requirements

**Objective:** Enforce minimum password strength when users create or change passwords.

**File to modify:** `crates/zentra-api/src/handlers/auth.rs`

**What to do:**
1. Create a function `fn validate_password_strength(password: &str) -> Result<(), String>` in `auth.rs`.
2. The function must enforce:
   - Minimum 12 characters
   - At least one uppercase letter
   - At least one lowercase letter
   - At least one digit
   - At least one special character (`!@#$%^&*()-_=+[]{}|;:',.<>?/~`)
3. Apply this validation in:
   - The change-password endpoint (from Prompt 1.6)
   - The create-user endpoint (if it accepts a password)
4. Return `400 Bad Request` with a descriptive message listing which requirements are not met.
5. Do NOT apply this validation to the login endpoint (users must be able to log in with their existing password).
6. Do NOT change existing passwords in the database.
7. Do NOT change the login userid or password.

**Verification:**
- `cargo build` must succeed
- Attempting to set password `"abc"` should return 400 with a message about missing requirements
- Attempting to set password `"MyStr0ng!Pass99"` should succeed

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 2: INPUT VALIDATION & ERROR HANDLING
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 2.1 — Add Validator Crate and Request Validation

**Objective:** Add input validation to all API request structs using the `validator` crate with derive macros.

**Files to modify:**
- `crates/zentra-api/Cargo.toml` — Add dependency
- `crates/zentra-api/src/handlers/targets.rs` — Validate target requests
- `crates/zentra-api/src/handlers/users.rs` — Validate user requests
- `crates/zentra-api/src/handlers/auth.rs` — Validate auth requests

**What to do:**
1. Add `validator = { version = "0.18", features = ["derive"] }` to `crates/zentra-api/Cargo.toml` under `[dependencies]`.
2. Create a helper function in `crates/zentra-api/src/error.rs` to convert validation errors to `ApiError::BadRequest`:
   ```rust
   impl From<validator::ValidationErrors> for ApiError {
       fn from(e: validator::ValidationErrors) -> Self {
           ApiError::BadRequest(format!("Validation failed: {}", e))
       }
   }
   ```
3. Add `#[derive(Validate)]` and validation attributes to these request structs:

   **In targets.rs:**
   ```rust
   #[derive(Deserialize, Validate)]
   pub struct VerifySshRequest {
       #[validate(length(min = 1, max = 255))]
       pub host: String,
       #[validate(length(max = 64))]
       pub username: Option<String>,
   }
   ```

   **In auth.rs (login request):**
   ```rust
   #[validate(length(min = 1, max = 255))]
   pub email: String,
   #[validate(length(min = 1, max = 128))]
   pub password: String,
   ```

   **In users.rs (create user request):**
   ```rust
   #[validate(email)]
   pub email: String,
   #[validate(length(min = 1, max = 100))]
   pub display_name: String,
   ```

4. In each handler that accepts a validated request, call `req.validate()?` before processing.
5. Do NOT change any existing handler logic — only add validation before the existing logic.
6. Do NOT change any response formats.
7. Do NOT change any database queries.

**Verification:**
- `cargo build` must succeed
- Sending a request with an empty `host` field should return 400
- Sending a request with a 300-char host should return 400
- Normal requests with valid data must still work

---

## PROMPT 2.2 — Add ReDoS Protection for User-Supplied Regex

**Objective:** Prevent Regular Expression Denial of Service (ReDoS) attacks via user-supplied regex patterns in the log engine.

**File to modify:** `crates/zentra-log-engine/src/` — the file containing custom regex parsing

**What to do:**
1. Find where user-supplied regex patterns are compiled (look for `Regex::new()` calls that take user input).
2. Replace any `.unwrap()` on `Regex::new()` with proper error handling (`.map_err(|e| ...)` returning a descriptive error).
3. Add a maximum regex pattern length check: if the pattern is longer than 1000 characters, reject it with an error.
4. Add a compile-time limit using `regex::RegexBuilder` with `.size_limit(1_000_000)` (1MB compiled size limit) to prevent catastrophic backtracking.
5. Replace the pattern:
   ```rust
   // BEFORE:
   let re = Regex::new(&pattern).unwrap();
   // AFTER:
   let re = regex::RegexBuilder::new(&pattern)
       .size_limit(1_000_000)
       .build()
       .map_err(|e| format!("Invalid regex pattern: {}", e))?;
   ```
6. Do NOT change any function signatures.
7. Do NOT change any existing regex patterns that are hardcoded in the codebase.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-log-engine` must pass
- A valid regex like `"error|warn"` should work
- A malicious regex like `"(a+)+$"` repeated 1000 times should be rejected (size limit)

---

## PROMPT 2.3 — Add Query Time-Range Limits

**Objective:** Prevent unbounded time-range queries that could overload the database.

**File to modify:** `crates/zentra-db/src/` — the file containing `query_metrics_aggregated` or similar metric query functions

**What to do:**
1. Find the function that accepts `from: DateTime<Utc>` and `to: DateTime<Utc>` parameters for metric queries.
2. Add validation at the start of the function:
   ```rust
   let range = to - from;
   if range > chrono::Duration::days(90) {
       return Err(sqlx::Error::Protocol("Time range cannot exceed 90 days".into()));
   }
   if range < chrono::Duration::zero() {
       return Err(sqlx::Error::Protocol("'from' must be before 'to'".into()));
   }
   ```
3. If there is a `bucket_seconds` parameter, validate:
   ```rust
   if bucket_seconds < 10 {
       return Err(sqlx::Error::Protocol("Bucket size must be at least 10 seconds".into()));
   }
   ```
4. Apply similar validation to log search queries if they accept time ranges.
5. Do NOT change the function signatures.
6. Do NOT change any SQL queries.
7. All queries must continue to target the zentra database.

**Verification:**
- `cargo build` must succeed
- A query with a 30-day range should work
- A query with a 365-day range should return an error
- A query with `from` after `to` should return an error

---

## PROMPT 2.4 — Standardize Error Handling (Remove Tuple Returns)

**Objective:** Convert remaining handlers that return `(StatusCode, String)` tuples to use the unified `ApiError` enum.

**Files to modify:** `crates/zentra-api/src/handlers/targets.rs` (and any other handler files using tuple returns)

**What to do:**
1. Search all files in `crates/zentra-api/src/handlers/` for the pattern `(axum::http::StatusCode, String)` in return types.
2. For each function found:
   - Change the return type from `Result<Json<T>, (axum::http::StatusCode, String)>` to `Result<Json<T>, ApiError>`
   - Replace `.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))` with `.map_err(ApiError::Database)` (for sqlx errors) or `.map_err(|e| ApiError::Internal(e.to_string()))` (for other errors)
   - Replace `(StatusCode::NOT_FOUND, "...".into())` with `ApiError::NotFound("...".into())`
   - Replace `(StatusCode::BAD_REQUEST, "...".into())` with `ApiError::BadRequest("...".into())`
3. The `ApiError` enum in `crates/zentra-api/src/error.rs` already implements `IntoResponse` and `From<sqlx::Error>`, so this should be a straightforward replacement.
4. Do NOT change any handler logic — only change error wrapping.
5. Do NOT change any response formats for successful responses.
6. Do NOT remove the `From<(StatusCode, String)>` impl on `ApiError` — it's needed for backward compatibility during migration.

**Verification:**
- `cargo build` must succeed
- All API endpoints must return the same HTTP status codes as before
- Error responses should now be consistently `{"error": "message"}` JSON format

---

## PROMPT 2.5 — Stop Exposing Raw Database Errors to API Clients

**Objective:** Ensure no raw sqlx or internal error messages are returned to API clients.

**Files to modify:** All files in `crates/zentra-api/src/handlers/` that return errors

**What to do:**
1. In `crates/zentra-api/src/error.rs`, the `IntoResponse` implementation for `ApiError::Database` already returns `"internal database error"` (line 42). This is correct. Verify no handler bypasses this by returning `e.to_string()` directly.
2. Search all handler files for patterns like:
   - `"error": e.to_string()` — replace with a generic message
   - `"message": e.to_string()` where `e` is a `sqlx::Error` — replace with `"An internal error occurred"`
   - `format!("Failed: {}", e)` in error responses — replace with a generic message
3. Keep the `tracing::error!` or `tracing::warn!` log calls that log the full error server-side — only sanitize what goes to the client.
4. Do NOT change any successful response formats.
5. Do NOT suppress the error logging — keep logging the full error for debugging.
6. Do NOT change the login/auth error messages (they intentionally say "Invalid credentials" without details).

**Verification:**
- `cargo build` must succeed
- API error responses should never contain SQL query text, table names, or column names
- Server logs should still contain the full error details

---

## PROMPT 2.6 — Add Frontend Error Boundaries and Toast Notifications

**Objective:** Add per-feature error boundaries and replace silent `.catch(() => {})` with toast notifications.

**Files to modify:**
- `frontend/src/App.tsx` — Add error boundaries around route groups
- `frontend/src/lib/api/_client.ts` — Add error event dispatch
- Multiple page files — Fix silent error swallowing

**What to do:**
1. **Create a toast notification component** at `frontend/src/components/Toast.tsx`:
   - Simple toast that shows error/success/info messages
   - Auto-dismiss after 5 seconds
   - Uses a global event system: `window.dispatchEvent(new CustomEvent('toast', { detail: { message, type } }))`
   - Render a `<ToastContainer />` in `App.tsx`

2. **Add error event dispatch to _client.ts:**
   - In the `fetchJSON()` function, when an error occurs (non-401), dispatch a toast event:
     ```typescript
     window.dispatchEvent(new CustomEvent('toast', {
       detail: { message: `Request failed: ${res.status}`, type: 'error' }
     }))
     ```
   - Keep throwing the error so callers can still handle it

3. **Add error boundaries around route groups in App.tsx:**
   - Wrap the infrastructure routes in an `<ErrorBoundary>`
   - Wrap the database routes in an `<ErrorBoundary>`
   - Wrap the security routes in an `<ErrorBoundary>`
   - Wrap the AI routes in an `<ErrorBoundary>`
   - Use the existing `ErrorBoundary` component from `frontend/src/components/ErrorBoundary.tsx`

4. **Fix silent error swallowing:**
   - Search all `.tsx` files for `.catch(() => {})` or `.catch(()=>{})`
   - Replace with `.catch((e) => console.error('Operation failed:', e))`
   - The toast notification from step 2 will handle the user-facing notification

5. Do NOT change any API call logic.
6. Do NOT change any component props or state management.
7. Do NOT change the auth flow or login page.
8. Do NOT remove any existing functionality.

**Verification:**
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- When an API call fails, a toast notification should appear
- When a component crashes, the error boundary should show a fallback UI instead of a white screen

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 4: MEMORY SAFETY & RELIABILITY
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 4.1 — Add Eviction/TTL to Anomaly Detector Events

**Objective:** Prevent unbounded memory growth in the anomaly detector's event history.

**File to modify:** `crates/zentra-predict/src/` — the file containing `AnomalyDetector` struct

**What to do:**
1. Find the `AnomalyDetector` struct (or similar) that has a `Vec` of anomaly events.
2. Add a maximum capacity constant: `const MAX_ANOMALY_EVENTS: usize = 10_000;`
3. After adding a new event to the Vec, check if the length exceeds `MAX_ANOMALY_EVENTS`. If so, remove the oldest events (drain from the front):
   ```rust
   if self.events.len() > MAX_ANOMALY_EVENTS {
       let excess = self.events.len() - MAX_ANOMALY_EVENTS;
       self.events.drain(..excess);
   }
   ```
4. Do NOT change any function signatures.
5. Do NOT change the anomaly detection logic.
6. Do NOT change any public API.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-predict` must pass
- Events Vec should never exceed 10,000 entries

---

## PROMPT 4.2 — Add Eviction to Baseline Engine History

**Objective:** Prevent unbounded memory growth in the baseline engine's history HashMap.

**File to modify:** `crates/zentra-predict/src/` — the file containing `BaselineEngine` struct

**What to do:**
1. Find the `BaselineEngine` struct that has a `HashMap` for baseline history.
2. Add a periodic cleanup method that removes entries not updated in the last 7 days:
   ```rust
   pub fn cleanup_stale_baselines(&mut self) {
       let cutoff = Utc::now() - chrono::Duration::days(7);
       self.baselines.retain(|_key, baseline| baseline.last_updated > cutoff);
   }
   ```
3. If the baseline struct doesn't have a `last_updated` field, add one (as `chrono::DateTime<Utc>`).
4. Call `cleanup_stale_baselines()` at the start of `recompute_all()` (the hourly recompute).
5. Do NOT change any function signatures.
6. Do NOT change the baseline computation logic.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-predict` must pass

---

## PROMPT 4.3 — Bound NetFlow Aggregation HashMap

**Objective:** Prevent unbounded memory growth in the NetFlow flow aggregation HashMap.

**File to modify:** `crates/zentra-netflow/src/` — the file containing `FlowAggregator` struct

**What to do:**
1. Find the `FlowAggregator` struct with its `HashMap` of flow buckets.
2. Add a maximum capacity constant: `const MAX_FLOW_BUCKETS: usize = 100_000;`
3. Before inserting a new flow bucket, check if the map is at capacity:
   ```rust
   if self.buckets.len() >= MAX_FLOW_BUCKETS {
       // Remove the oldest 10% of entries by last_seen timestamp
       let mut entries: Vec<_> = self.buckets.iter()
           .map(|(k, v)| (k.clone(), v.last_seen))
           .collect();
       entries.sort_by_key(|(_, ts)| *ts);
       let to_remove = entries.len() / 10;
       for (key, _) in entries.into_iter().take(to_remove) {
           self.buckets.remove(&key);
       }
   }
   ```
4. If the flow bucket struct doesn't have a `last_seen` field, add one.
5. Do NOT change the flow aggregation logic.
6. Do NOT change any function signatures.

**Verification:**
- `cargo build` must succeed
- Flow aggregation should work the same but with bounded memory

---

## PROMPT 4.4 — Add Eviction to ML Seasonal Baselines

**Objective:** Prevent unbounded memory growth in the ML anomaly detection seasonal baselines.

**File to modify:** `crates/zentra-ml/src/` — the file containing `SeasonalDetector` or `AnomalyEngine`

**What to do:**
1. Find the seasonal baseline storage (likely a `HashMap` keyed by `target:metric`).
2. Add a maximum capacity constant: `const MAX_SEASONAL_KEYS: usize = 50_000;`
3. Add a `last_updated: Instant` field to the seasonal baseline entry if not present.
4. Add a cleanup method that removes entries not seen in 24 hours:
   ```rust
   pub fn cleanup_stale(&mut self) {
       let cutoff = Instant::now() - Duration::from_secs(86400);
       self.baselines.retain(|_key, entry| entry.last_updated > cutoff);
   }
   ```
5. Call `cleanup_stale()` periodically (e.g., every time `update()` is called and `len() > MAX_SEASONAL_KEYS`).
6. Do NOT change the anomaly detection algorithms.
7. Do NOT change any function signatures.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-ml` must pass

---

## PROMPT 4.5 — Fix Panic-Prone Code in Log Engine and NetFlow

**Objective:** Replace `.unwrap()` calls on fallible operations with proper error handling to prevent panics in production.

**Files to modify:**
- `crates/zentra-log-engine/src/` — PostgreSQL CSV parser and custom regex parser
- `crates/zentra-netflow/src/` — NetFlow v5 record parsing
- `crates/zentra-api/src/provisioning.rs` — JSON unwrap

**What to do:**
1. **Log engine — PostgreSQL CSV parser:**
   - Find where the parser assumes 14+ fields in a CSV line and uses `.unwrap()` or direct indexing
   - Replace with bounds checking: `if fields.len() < 14 { return Err("Truncated PostgreSQL log line"); }`

2. **Log engine — Custom regex parser:**
   - Find `Regex::new(&pattern).unwrap()`
   - Replace with: `Regex::new(&pattern).map_err(|e| format!("Invalid regex: {}", e))?`

3. **NetFlow v5 — Record offset bounds:**
   - Find where the parser reads v5 records by offset
   - Add validation: `if data.len() < offset + 48 { break; }` (v5 records are 48 bytes each)
   - Also validate the record count from the header: `if count as usize * 48 + 24 > data.len() { /* truncated packet */ }`

4. **provisioning.rs:**
   - Find `.unwrap()` on JSON parsing
   - Replace with `.unwrap_or_default()` or proper error handling

5. Do NOT change any function signatures.
6. Do NOT change any parsing logic — only add error handling around it.

**Verification:**
- `cargo build` must succeed
- `cargo test` must pass for all affected crates
- Malformed input should result in errors, not panics

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 5: DATABASE & MIGRATION FIXES
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 5.1 — Fix Migration Conflicts (Dashboard Tables)

**Objective:** Resolve the conflict where migrations 006 and 007 both create `dashboard_variables` with different schemas, and migrations 001 and 007 both create `dashboards`.

**Files to modify:**
- `migrations/20250101000006_annotations.sql`
- `migrations/20250101000007_dashboards.sql`

**What to do:**
1. Read both migration files completely to understand the conflict.
2. In migration 006, if it creates `dashboard_variables` or `dashboards`, ensure it uses `CREATE TABLE IF NOT EXISTS`.
3. In migration 007, if it also creates `dashboard_variables` or `dashboards`, ensure it uses `CREATE TABLE IF NOT EXISTS` and uses `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` for any columns that might already exist from migration 006.
4. The goal is idempotency — running both migrations in order should produce the correct final schema without errors.
5. If migration 007 adds columns not in 006's version, use `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` instead of recreating the table.
6. Do NOT drop any existing tables.
7. Do NOT change any column types or constraints that would break existing data.
8. Do NOT create any new tables — only fix the conflict in existing migration files.
9. All tables must be created in the zentra database only.

**Verification:**
- Both migrations should run without errors on a fresh database
- Both migrations should run without errors on an existing database where migration 006 already ran
- `sqlx migrate run --database-url "postgresql://postgres:zantaz@10.88.11.162:5434/zentra" --source ./migrations` should succeed

---

## PROMPT 5.2 — Fix Migration Dependency (User Roles → Users)

**Objective:** Resolve the dependency where migration 010 creates `user_roles` referencing the `users` table, but `users` is created in migration 012.

**Files to modify:**
- `migrations/20250101000010_rbac_audit.sql`

**What to do:**
1. Read migration 010 and 012 to understand the dependency.
2. In migration 010, if it creates `user_roles` with a foreign key to `users`, wrap the foreign key constraint in a conditional:
   ```sql
   -- Create user_roles without FK first
   CREATE TABLE IF NOT EXISTS user_roles (
       id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       user_id UUID NOT NULL,
       role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
       ...
   );
   -- Add FK to users if users table exists (it will be created in migration 012)
   DO $$ BEGIN
       IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'users') THEN
           ALTER TABLE user_roles ADD CONSTRAINT IF NOT EXISTS fk_user_roles_user
               FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
       END IF;
   END $$;
   ```
3. Alternatively, if the cleanest fix is to move the `user_roles` table creation to migration 012 (after `users`), do that instead — but ensure migration 010's other contents remain unchanged.
4. Do NOT drop any existing tables.
5. Do NOT change the schema of any tables.
6. All tables in the zentra database only.

**Verification:**
- Migrations should run in order without foreign key errors
- Both `users` and `user_roles` tables should exist with correct relationships

---

## PROMPT 5.3 — Consolidate Auth Fix Migration

**Objective:** Ensure migration 015 (auth_fix) columns are properly handled as amendments to migration 012.

**File to modify:** `migrations/20250101000015_auth_fix.sql`

**What to do:**
1. Read migration 015 to see what columns it adds.
2. Ensure all `ALTER TABLE` statements use `ADD COLUMN IF NOT EXISTS` so they are idempotent.
3. If migration 015 re-creates any objects that 012 already created, ensure it handles the conflict gracefully.
4. Do NOT remove migration 015 — it serves as a fix-up migration.
5. Do NOT change migration 012.
6. All modifications in the zentra database only.

**Verification:**
- Running migrations fresh should work
- Running migrations on an existing database with 012 already applied should work

---

## PROMPT 5.4 — Add PostgreSQL Row-Level Security for Tenant Isolation

**Objective:** Add database-level tenant isolation as defense-in-depth using PostgreSQL Row-Level Security (RLS).

**File to create:** `migrations/20250101000024_row_level_security.sql`

**What to do:**
1. Create a new migration file `migrations/20250101000024_row_level_security.sql`.
2. The migration should:
   ```sql
   -- Row-Level Security for multi-tenant isolation (zentra database)
   -- This adds defense-in-depth: even if application code forgets WHERE org_id = $1,
   -- the database will enforce it.

   -- Create an application role for Zentra API connections
   DO $$ BEGIN
       IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'zentra_app') THEN
           CREATE ROLE zentra_app;
       END IF;
   END $$;

   -- Enable RLS on core tenant-scoped tables
   ALTER TABLE targets ENABLE ROW LEVEL SECURITY;
   ALTER TABLE metrics ENABLE ROW LEVEL SECURITY;
   ALTER TABLE logs ENABLE ROW LEVEL SECURITY;
   ALTER TABLE alerts ENABLE ROW LEVEL SECURITY;
   ALTER TABLE alert_rules ENABLE ROW LEVEL SECURITY;
   ALTER TABLE dashboards ENABLE ROW LEVEL SECURITY;

   -- Create policies (only if they don't exist)
   -- Policy: users can only see rows matching their org_id set via session variable
   DO $$ BEGIN
       IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_targets') THEN
           CREATE POLICY tenant_isolation_targets ON targets
               USING (org_id = current_setting('app.current_org_id')::uuid);
       END IF;
   END $$;
   -- ... repeat for other tables
   ```
3. Note: This migration creates the RLS policies but does NOT enforce them on the `postgres` superuser role (which is what the app currently uses). The policies will only take effect when the app connects as `zentra_app` role — this is a FUTURE migration to switch connection roles.
4. Add a comment explaining this is defense-in-depth and the app currently connects as superuser.
5. Do NOT change the database connection string.
6. Do NOT change the application's org_id extraction logic.
7. All objects created in the zentra database only.

**Verification:**
- Migration should run without errors
- Existing queries should continue to work (RLS not enforced for superuser)
- RLS policies should be visible: `SELECT * FROM pg_policies;`

---

## PROMPT 5.5 — Add Database Backup Script

**Objective:** Create a database backup script for the zentra database with WAL archival support.

**File to create:** `deploy/backup-db.sh`

**What to do:**
1. Create `deploy/backup-db.sh` with the following functionality:
   ```bash
   #!/usr/bin/env bash
   # Zentra Database Backup Script
   # Usage: ./deploy/backup-db.sh [--full | --schema-only]
   # Backs up the zentra database only.
   set -euo pipefail

   # Configuration from environment (no hardcoded credentials)
   DB_HOST="${POSTGRES_HOST:?Error: POSTGRES_HOST not set}"
   DB_PORT="${POSTGRES_PORT:-5434}"
   DB_USER="${POSTGRES_USER:?Error: POSTGRES_USER not set}"
   DB_NAME="zentra"  # Always backup zentra database
   BACKUP_DIR="${BACKUP_DIR:-/store/Zentra/backups}"
   TIMESTAMP=$(date +%Y%m%d_%H%M%S)

   # Require PGPASSWORD
   : "${PGPASSWORD:?Error: PGPASSWORD not set}"

   mkdir -p "${BACKUP_DIR}"

   MODE="${1:---full}"

   case "${MODE}" in
       --full)
           echo "Full backup of zentra database..."
           pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" \
               -d "${DB_NAME}" -Fc --verbose \
               -f "${BACKUP_DIR}/zentra_full_${TIMESTAMP}.dump"
           echo "Backup saved: ${BACKUP_DIR}/zentra_full_${TIMESTAMP}.dump"
           ;;
       --schema-only)
           echo "Schema-only backup of zentra database..."
           pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" \
               -d "${DB_NAME}" --schema-only \
               -f "${BACKUP_DIR}/zentra_schema_${TIMESTAMP}.sql"
           echo "Backup saved: ${BACKUP_DIR}/zentra_schema_${TIMESTAMP}.sql"
           ;;
   esac

   # Cleanup backups older than 30 days
   find "${BACKUP_DIR}" -name "zentra_*.dump" -mtime +30 -delete 2>/dev/null || true
   find "${BACKUP_DIR}" -name "zentra_*.sql" -mtime +30 -delete 2>/dev/null || true

   echo "Backup complete. Files in ${BACKUP_DIR}:"
   ls -lh "${BACKUP_DIR}"/zentra_*_${TIMESTAMP}.* 2>/dev/null || echo "(none found)"
   ```
2. Make it executable: (mention in the script header)
3. Do NOT backup any database other than `zentra`.
4. Do NOT include any hardcoded passwords.

**Verification:**
- `bash -n deploy/backup-db.sh` should pass (syntax check)
- Script should fail with clear error if env vars not set

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 6: PERFORMANCE OPTIMIZATION
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 6.1 — Reduce Frontend Polling Frequency

**Objective:** Reduce unnecessary API calls by increasing React Query staleTime from 5s to 30s and refetchInterval from 10s to 30s.

**File to modify:** `frontend/src/main.tsx`

**Current code (around line 5-15):**
```typescript
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchInterval: 10_000,
      staleTime: 5_000,
      retry: 2,
    },
  },
})
```

**What to do:**
1. Change `refetchInterval` from `10_000` to `30_000` (30 seconds).
2. Change `staleTime` from `5_000` to `30_000` (30 seconds).
3. Keep `retry: 2` unchanged.
4. Do NOT change any other code in main.tsx.
5. Do NOT change any individual page query configurations.

**Verification:**
- `cd frontend && npm run build` must succeed
- The app should still work but make ~3x fewer API calls per page

---

## PROMPT 6.2 — Add Request Debouncing for Search Inputs

**Objective:** Add 300ms debounce to search and filter inputs to prevent excessive API calls while typing.

**File to create:** `frontend/src/lib/useDebounce.ts`
**Files to modify:** Frontend pages with search inputs

**What to do:**
1. Create a custom hook `useDebounce` at `frontend/src/lib/useDebounce.ts`:
   ```typescript
   import { useState, useEffect } from 'react'

   export function useDebounce<T>(value: T, delay: number = 300): T {
     const [debouncedValue, setDebouncedValue] = useState<T>(value)
     useEffect(() => {
       const timer = setTimeout(() => setDebouncedValue(value), delay)
       return () => clearTimeout(timer)
     }, [value, delay])
     return debouncedValue
   }
   ```
2. Find pages with search inputs that trigger API calls on every keystroke. Common locations:
   - Log Explorer page (search query)
   - User Management page (search/filter)
   - Any page with a text input that triggers `useQuery` with the input value as a query key
3. In those pages, apply the hook:
   ```typescript
   const [searchInput, setSearchInput] = useState('')
   const debouncedSearch = useDebounce(searchInput, 300)
   // Use debouncedSearch in the query key instead of searchInput
   ```
4. Do NOT change any API endpoints.
5. Do NOT change any component props.
6. Only apply to search/filter inputs — do NOT debounce form submissions or button clicks.

**Verification:**
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- Typing in search should wait 300ms before triggering an API call

---

## PROMPT 6.3 — Add React.memo to Chart Components

**Objective:** Prevent expensive recharts re-renders when parent components update.

**Files to modify:** All chart components in `frontend/src/components/charts/`

**What to do:**
1. List all chart component files in `frontend/src/components/charts/`.
2. For each chart component that is a function component (not already wrapped in `React.memo`):
   - Wrap the default export in `React.memo()`:
   ```typescript
   // BEFORE:
   export default function SparklineChart({ data, ...props }) { ... }
   // AFTER:
   function SparklineChart({ data, ...props }) { ... }
   export default React.memo(SparklineChart)
   ```
3. Import `React` or `memo` from `'react'` if not already imported.
4. Do NOT change any chart logic, props, or rendering.
5. Do NOT change any chart styles.

**Verification:**
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- Charts should render identically

---

## PROMPT 6.4 — Parallelize Prometheus Scraper

**Objective:** Replace sequential target scraping with parallel execution.

**File to modify:** `crates/zentra-scrape/src/` — the file containing `scrape_all()` or similar

**What to do:**
1. Find the `scrape_all()` function (or equivalent) that iterates over scrape targets sequentially.
2. Replace the sequential loop with `futures::future::join_all()` for parallel execution:
   ```rust
   // BEFORE:
   for target in &targets {
       let result = scrape_target(target).await;
       // ...
   }
   // AFTER:
   let futures: Vec<_> = targets.iter()
       .map(|target| scrape_target(target))
       .collect();
   let results = futures::future::join_all(futures).await;
   ```
3. Add a concurrency limit using `futures::stream::StreamExt::buffer_unordered()` with a max of 10 concurrent scrapes:
   ```rust
   use futures::stream::{self, StreamExt};
   let results: Vec<_> = stream::iter(targets.iter())
       .map(|target| scrape_target(target))
       .buffer_unordered(10)
       .collect()
       .await;
   ```
4. Do NOT change the scrape logic for individual targets.
5. Do NOT change any function signatures.
6. Do NOT change the result handling.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-scrape` must pass
- Scraping N targets should now take ~max(target_time) instead of sum(target_times)

---

## PROMPT 6.5 — Cache Compiled Regex Patterns in Log Engine

**Objective:** Avoid recompiling regex patterns on every log parse by caching them.

**File to modify:** `crates/zentra-log-engine/src/` — the file containing regex compilation for custom patterns

**What to do:**
1. Find where `Regex::new()` is called with user-configured patterns during log parsing.
2. Add a thread-safe regex cache using `DashMap`:
   ```rust
   use dashmap::DashMap;
   use once_cell::sync::Lazy;
   use regex::Regex;

   static REGEX_CACHE: Lazy<DashMap<String, Regex>> = Lazy::new(DashMap::new);

   fn get_or_compile_regex(pattern: &str) -> Result<Regex, String> {
       if let Some(re) = REGEX_CACHE.get(pattern) {
           return Ok(re.clone());
       }
       let re = regex::RegexBuilder::new(pattern)
           .size_limit(1_000_000)
           .build()
           .map_err(|e| format!("Invalid regex: {}", e))?;
       REGEX_CACHE.insert(pattern.to_string(), re.clone());
       Ok(re)
   }
   ```
3. Replace all `Regex::new(pattern)` calls in the parsing code with `get_or_compile_regex(pattern)`.
4. Add `dashmap` to `crates/zentra-log-engine/Cargo.toml` if not already present (it's in the workspace dependencies).
5. Add `once_cell = "1"` to `crates/zentra-log-engine/Cargo.toml` if not present.
6. Do NOT change any parsing logic beyond the regex compilation.
7. Do NOT change any function signatures.

**Verification:**
- `cargo build` must succeed
- `cargo test -p zentra-log-engine` must pass
- Second parse with the same pattern should be significantly faster (no recompilation)

---

## PROMPT 6.6 — Reuse Buffer and Deduplicate NetFlow Listeners

**Objective:** Fix the per-packet buffer allocation and reduce code duplication across NetFlow protocol listeners.

**File to modify:** `crates/zentra-netflow/src/` — the file containing `listen_v5()`, `listen_v9()`, `listen_ipfix()`, `listen_sflow()`

**What to do:**
1. **Fix buffer allocation:** In each listener, move the buffer allocation outside the loop:
   ```rust
   // BEFORE (inside loop):
   loop {
       let mut buf = vec![0u8; 65535];
       let (len, addr) = socket.recv_from(&mut buf).await?;
   }
   // AFTER (outside loop):
   let mut buf = vec![0u8; 65535];
   loop {
       let (len, addr) = socket.recv_from(&mut buf).await?;
   }
   ```

2. **Validate NetFlow v5 record count:**
   After reading the v5 header (24 bytes), validate:
   ```rust
   let count = u16::from_be_bytes([buf[2], buf[3]]) as usize;
   if count * 48 + 24 > len {
       tracing::warn!("Truncated NetFlow v5 packet: header says {} records but only {} bytes", count, len);
       continue;
   }
   ```

3. Do NOT create a generic listener function in this prompt (that's a larger refactor for Phase 9).
4. Do NOT change the parsing logic for any protocol.
5. Do NOT change any function signatures.

**Verification:**
- `cargo build` must succeed
- NetFlow listeners should work identically but with less memory allocation pressure

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 8: UX & ACCESSIBILITY
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 8.1 — Standardize Loading States Across All Pages

**Objective:** Use the existing `LoadingSkeleton` component consistently across all pages instead of ad-hoc spinners or empty renders.

**Files to modify:** All page components in `frontend/src/pages/`

**What to do:**
1. Find all page components that use `useQuery` hooks.
2. For each page, add proper loading state handling:
   ```typescript
   const { data, isLoading, isError, error } = useQuery({ ... })

   if (isLoading) {
     return <LoadingSkeleton variant="table" /> // or "card", "chart" as appropriate
   }

   if (isError) {
     return (
       <div className="p-6 text-center">
         <p className="text-red-400">Failed to load data</p>
         <p className="text-sm text-zinc-500 mt-1">{error?.message || 'Unknown error'}</p>
       </div>
     )
   }
   ```
3. Import `LoadingSkeleton` from `'../components/LoadingSkeleton'` in each file.
4. Choose the appropriate skeleton variant based on the page content:
   - Dashboard pages → `"card"` variant
   - List/table pages → `"table"` variant
   - Chart-heavy pages → `"chart"` variant
5. Do NOT change any data fetching logic.
6. Do NOT change any component props.
7. Do NOT change the auth flow.
8. Remove any existing ad-hoc `<Loader2 className="animate-spin" />` patterns and replace with `LoadingSkeleton`.

**Verification:**
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- Every page should show a skeleton while loading (no blank screens)
- Every page should show an error message if data fails to load

---

## PROMPT 8.2 — Standardize Date and Number Formatting

**Objective:** Use consistent formatting utilities for dates and numbers across all pages.

**File to create:** `frontend/src/lib/format.ts`
**Files to modify:** Pages with inconsistent formatting

**What to do:**
1. Create `frontend/src/lib/format.ts` with:
   ```typescript
   import { format, formatDistanceToNow } from 'date-fns'

   export function formatDate(date: string | Date | null | undefined): string {
     if (!date) return '—'
     const d = typeof date === 'string' ? new Date(date) : date
     if (isNaN(d.getTime())) return '—'
     return format(d, 'MMM d, yyyy HH:mm')
   }

   export function formatRelative(date: string | Date | null | undefined): string {
     if (!date) return '—'
     const d = typeof date === 'string' ? new Date(date) : date
     if (isNaN(d.getTime())) return '—'
     return formatDistanceToNow(d, { addSuffix: true })
   }

   export function formatNumber(value: number | null | undefined): string {
     if (value == null) return '—'
     return new Intl.NumberFormat('en-US').format(value)
   }

   export function formatBytes(bytes: number | null | undefined): string {
     if (bytes == null) return '—'
     if (bytes === 0) return '0 B'
     const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
     const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(1024))
     return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
   }

   export function formatPercent(value: number | null | undefined): string {
     if (value == null) return '—'
     return `${value.toFixed(1)}%`
   }
   ```
2. Search for pages using raw `new Date().toLocaleString()`, `toFixed()`, or manual number formatting.
3. Replace with the appropriate utility function from `format.ts`.
4. Do NOT change any data or component logic.
5. Do NOT change any API calls.

**Verification:**
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- Dates and numbers should be consistently formatted across all pages

---

## PROMPT 8.3 — Add ARIA Labels and Accessibility Attributes

**Objective:** Add basic accessibility attributes to navigation, data tables, and interactive elements.

**Files to modify:**
- `frontend/src/components/Layout.tsx` — sidebar navigation
- `frontend/src/components/charts/DataTable.tsx` — data table (if exists)
- `frontend/src/components/CommandPalette.tsx` — command palette
- Modal components

**What to do:**
1. **Layout.tsx — Sidebar navigation:**
   - Add `role="navigation"` and `aria-label="Main navigation"` to the sidebar `<nav>` element
   - Add `aria-current="page"` to the active nav item
   - Add a skip-to-content link at the very top:
     ```tsx
     <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:p-2 focus:bg-blue-600 focus:text-white">
       Skip to content
     </a>
     ```
   - Add `id="main-content"` to the main content area

2. **DataTable.tsx (if exists):**
   - Add `role="table"` if using a `<div>` instead of `<table>`
   - Add `aria-sort` to sortable column headers
   - Add `scope="col"` to `<th>` elements

3. **CommandPalette.tsx:**
   - Add `role="dialog"` and `aria-label="Command palette"` to the modal
   - Add `role="listbox"` to the results list
   - Add `role="option"` to each result item
   - Add `aria-activedescendant` for keyboard navigation

4. **Modal components:**
   - Add `role="dialog"` and `aria-modal="true"`
   - Add `aria-labelledby` pointing to the modal title

5. Do NOT change any visual appearance.
6. Do NOT change any component logic.
7. Do NOT change any event handlers.

**Verification:**
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- Screen readers should be able to navigate the sidebar, tables, and modals

---

## PROMPT 8.4 — Filter Command Palette by User Permissions

**Objective:** Only show navigation items in the Command Palette that the current user has permission to access.

**File to modify:** `frontend/src/components/CommandPalette.tsx`

**What to do:**
1. Import `useAuth` from the auth context.
2. Find the array of command palette items (navigation options).
3. Add a `permission` or `requiredRole` field to each item:
   ```typescript
   { name: 'Users', path: '/admin/users', requiredRole: 'admin' }
   { name: 'Roles', path: '/admin/roles', requiredRole: 'admin' }
   { name: 'Dashboard', path: '/dashboard' }  // no restriction
   ```
4. Filter the items before rendering:
   ```typescript
   const { hasRole, isAdmin } = useAuth()
   const filteredItems = items.filter(item => {
     if (!item.requiredRole) return true
     if (item.requiredRole === 'admin') return isAdmin
     return hasRole(item.requiredRole)
   })
   ```
5. Do NOT remove any items — only hide them based on permissions.
6. Do NOT change the Command Palette appearance or keyboard shortcuts.
7. Do NOT change the routing — users who type a URL directly should still hit the route's own permission check.

**Verification:**
- `cd frontend && npm run build` must succeed
- Admin users should see all items
- Non-admin users should not see admin-only items in the palette
- Non-admin users can still navigate via direct URL (route-level auth handles it)

---

## END OF PHASE 1, 2, 4, 5, 6, 8 PROMPTS
