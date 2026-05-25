# Zentra — Execution Prompts: Phase 7 (Deployment Hardening) & Phase 9 (Architecture)
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
7. **PREREQUISITE:** Phases 1-6 must be completed before starting Phase 7. Phase 9 depends on all prior phases.
8. When the development or execution gets stuck, stop the process, and then resume once again from where it was left off.
9. Build the frontend and backend on the remote server itself — do not build locally. If building on the remote server fails, only then fall back to building locally and deploying the artifacts. Otherwise, strictly follow remote deployment: push scripts and compile on the server.

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 7: DEPLOYMENT HARDENING
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 7.1 — Enable SSH StrictHostKeyChecking in Deploy Scripts

**Objective:** Remove the `-o StrictHostKeyChecking=no` flag from all deployment scripts to prevent MITM attacks.

**Files to modify:**
- `deploy/check-docker.sh` (line 3)
- `deploy/check-k8s.sh` (line 2)
- `deploy/start-oracle.sh` (line 3)
- `crates/zentra-api/src/ssh_utils.rs` (line ~63: `"-o", "StrictHostKeyChecking=no"`)

**What to do:**

1. **Deploy scripts (`check-docker.sh`, `check-k8s.sh`, `start-oracle.sh`):**
   - Replace `-o StrictHostKeyChecking=no` with `-o StrictHostKeyChecking=accept-new`
   - `accept-new` accepts the key on first connection and stores it, but rejects if the key changes (TOFU model — Trust On First Use)
   - Remove `-o UserKnownHostsFile=/dev/null` if present — let SSH use the default known_hosts file
   - Example change in `check-docker.sh` line 3:
     ```bash
     # BEFORE:
     ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 root@10.88.218.59 bash -s <<'REMOTE'
     # AFTER:
     ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 root@10.88.218.59 bash -s <<'REMOTE'
     ```

2. **Application code (`ssh_utils.rs`):**
   - In the `ssh_exec()` function, change `StrictHostKeyChecking=no` to `StrictHostKeyChecking=accept-new`
   - Keep `UserKnownHostsFile=/dev/null` for the application (the API server shouldn't accumulate known_hosts state) BUT add a configuration option:
   - Add support for `ZENTRA_SSH_KNOWN_HOSTS` environment variable:
     ```rust
     let known_hosts = std::env::var("ZENTRA_SSH_KNOWN_HOSTS")
         .unwrap_or_else(|_| "/dev/null".to_string());
     // Use: "-o", &format!("UserKnownHostsFile={}", known_hosts),
     ```
   - Add a comment explaining this trade-off and how to set `ZENTRA_SSH_KNOWN_HOSTS=/root/.ssh/known_hosts` in production.

3. Do NOT change the `setup_ssh_key_auth()` or `setup_ssh_key_manual()` functions — they need `StrictHostKeyChecking=no` because they're establishing first-time trust.
4. Do NOT change any function signatures.
5. Do NOT change any other logic in the scripts.

**Verification:**
- `cargo build` must succeed
- `bash -n deploy/check-docker.sh` must pass
- `bash -n deploy/check-k8s.sh` must pass
- `bash -n deploy/start-oracle.sh` must pass
- SSH to known hosts should work (key stored on first connection)
- SSH to hosts with changed keys should be rejected

---

## PROMPT 7.2 — Run Services as Non-Root User

**Objective:** Create a dedicated `zentra` service account and configure systemd services to run under it instead of root.

**File to modify:** `deploy/setup_server.sh`

**What to do:**
1. Add user creation at the top of the script (after `set -euo pipefail`):
   ```bash
   # Create zentra service user (non-root, no login shell)
   if ! id -u zentra >/dev/null 2>&1; then
       useradd --system --shell /usr/sbin/nologin --home-dir /store/Zentra --no-create-home zentra
       echo "Created service user: zentra"
   fi

   # Set ownership of deploy directory
   chown -R zentra:zentra "${DEPLOY_PATH}"
   ```

2. Change the systemd service files to use the zentra user:
   ```ini
   # BEFORE:
   User=root
   # AFTER:
   User=zentra
   Group=zentra
   ```

3. Apply to BOTH service definitions:
   - `zentra-api.service` (line 30)
   - `zentra-frontend.service` (line 50)

4. Add capabilities if needed (the API binds to port 5006 which is > 1024, so no special caps needed):
   ```ini
   # Allow binding to ports > 1024 (already fine, but document it)
   # If you need port < 1024, add: AmbientCapabilities=CAP_NET_BIND_SERVICE
   ```

5. Do NOT change the `DB_URL` line (that was already handled in Phase 1 Prompt 1.4).
6. Do NOT change any other script logic.
7. Do NOT change the database — this is infrastructure only.

**Verification:**
- `bash -n deploy/setup_server.sh` must pass
- The zentra user should be created when the script runs
- Services should start under the zentra user account

---

## PROMPT 7.3 — Pin K8s Image Tags and Fix Resource Limits

**Objective:** Replace `:latest` tags with versioned tags and increase resource limits for production.

**File to modify:** `deploy/k8s/zentra-deployment.yaml`

**What to do:**
1. Replace all `:latest` image tags with version `1.0.0`:
   ```yaml
   # BEFORE:
   image: Zentra/api:latest
   # AFTER:
   image: zentra/api:1.0.0
   imagePullPolicy: IfNotPresent
   ```
   Apply to:
   - `zentra-api` deployment (line ~52)
   - `Zentra-frontend` deployment (line ~97)

2. Add `imagePullPolicy: IfNotPresent` to all container specs.

3. Increase API resource limits for production:
   ```yaml
   # BEFORE:
   resources:
     requests:
       memory: "256Mi"
       cpu: "250m"
     limits:
       memory: "1Gi"
       cpu: "1000m"
   # AFTER:
   resources:
     requests:
       memory: "512Mi"
       cpu: "500m"
     limits:
       memory: "2Gi"
       cpu: "2000m"
   ```

4. Increase frontend resource limits:
   ```yaml
   # BEFORE:
   resources:
     requests:
       memory: "64Mi"
       cpu: "50m"
     limits:
       memory: "256Mi"
       cpu: "200m"
   # AFTER:
   resources:
     requests:
       memory: "128Mi"
       cpu: "100m"
     limits:
       memory: "512Mi"
       cpu: "500m"
   ```

5. Do NOT change the namespace, service names, or port numbers.
6. Do NOT change any secret references.
7. Do NOT change the Dockerfile (that's a separate concern).

**Verification:**
- `kubectl apply --dry-run=client -f deploy/k8s/zentra-deployment.yaml` should pass (if kubectl available)
- No `:latest` tags should remain in the file

---

## PROMPT 7.4 — Add Prometheus Endpoint Authentication

**Objective:** Protect the `/metrics` Prometheus endpoint with basic authentication instead of relying solely on IP restrictions.

**File to modify:** `deploy/nginx.conf`

**What to do:**
1. Add a new `auth_basic` block to the `/metrics` location:
   ```nginx
   # BEFORE:
   location /metrics {
       proxy_pass http://127.0.0.1:5002;
       allow 127.0.0.1;
       deny all;
   }

   # AFTER:
   location /metrics {
       # Require basic auth AND IP restriction for defense-in-depth
       auth_basic "Zentra Metrics";
       auth_basic_user_file /etc/nginx/.htpasswd_metrics;
       allow 127.0.0.1;
       allow 10.88.0.0/16;  # Allow monitoring network
       deny all;
       proxy_pass http://127.0.0.1:5002;
   }
   ```

2. Add a comment in `deploy/setup_server.sh` about creating the htpasswd file:
   ```bash
   # Create metrics auth file (run once manually):
   # htpasswd -c /etc/nginx/.htpasswd_metrics prometheus
   ```

3. Do NOT change any other nginx configuration.
4. Do NOT change the application code.
5. Do NOT change the database.

**Verification:**
- Nginx config syntax check: `nginx -t -c deploy/nginx.conf` (if nginx available)
- `/metrics` should require basic auth

---

## PROMPT 7.5 — Fix CSP Header (Remove unsafe-inline and unsafe-eval)

**Objective:** Tighten the Content-Security-Policy header to remove `unsafe-inline` and `unsafe-eval`.

**File to modify:** `deploy/nginx.conf`

**What to do:**
1. Change the CSP header on line 24:
   ```nginx
   # BEFORE:
   add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ws: wss:; frame-ancestors 'self';" always;

   # AFTER:
   add_header Content-Security-Policy "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' ws: wss:; frame-ancestors 'self'; base-uri 'self'; form-action 'self';" always;
   ```

   **Notes:**
   - Remove `'unsafe-inline'` from `script-src` (Vite/React build produces bundled JS, no inline scripts)
   - Remove `'unsafe-eval'` from `script-src` (React doesn't need eval)
   - Keep `'unsafe-inline'` in `style-src` because TailwindCSS may inject inline styles via class manipulation
   - Add `base-uri 'self'` to prevent base tag injection
   - Add `form-action 'self'` to prevent form submission to external domains

2. If the frontend uses any inline `<script>` tags in `index.html` (check `frontend/index.html`):
   - If it does, keep `'unsafe-inline'` in `script-src` and add a TODO comment
   - If it does NOT, proceed with removing it

3. Do NOT change any other nginx directives.
4. Do NOT change the application code.

**Verification:**
- The frontend should load and function correctly without `unsafe-eval`
- Check browser console for CSP violations (should be none)

---

## PROMPT 7.6 — Add Rollback Capability to Deploy Script

**Objective:** Before deploying, backup the current binary so deployments can be rolled back.

**File to modify:** `deploy/deploy.sh`

**What to do:**
1. In the `deploy_to_server()` function, BEFORE uploading the new binary (before line 62), add a backup step:
   ```bash
   # Backup current binary before deployment
   echo "  Backing up current binary..."
   ssh -p "${SERVER_PORT}" "${SERVER_USER}@${SERVER_HOST}" "
     if [ -f ${DEPLOY_PATH}/bin/zentra-server ]; then
       cp ${DEPLOY_PATH}/bin/zentra-server ${DEPLOY_PATH}/bin/zentra-server.bak.\$(date +%Y%m%d_%H%M%S)
       echo '  Backup created'
     else
       echo '  No existing binary to backup (first deploy)'
     fi
     # Keep only last 3 backups
     ls -t ${DEPLOY_PATH}/bin/zentra-server.bak.* 2>/dev/null | tail -n +4 | xargs rm -f 2>/dev/null || true
   "
   ```

2. BEFORE running migrations (before line 86), add a database backup:
   ```bash
   # Backup database before migration
   echo "  Backing up zentra database..."
   ssh -p "${SERVER_PORT}" "${SERVER_USER}@${SERVER_HOST}" "
     mkdir -p ${DEPLOY_PATH}/backups
     PGPASSWORD=\${PGPASSWORD:-} pg_dump -h localhost -p 5434 -U postgres -d zentra -Fc \
       -f ${DEPLOY_PATH}/backups/zentra_pre_deploy_\$(date +%Y%m%d_%H%M%S).dump 2>/dev/null \
       || echo '  WARNING: Database backup failed (pg_dump not available or PGPASSWORD not set)'
     # Keep only last 5 backups
     ls -t ${DEPLOY_PATH}/backups/zentra_pre_deploy_*.dump 2>/dev/null | tail -n +6 | xargs rm -f 2>/dev/null || true
   "
   ```

3. Improve the health check (after line 106):
   ```bash
   # Health check with retry
   echo "  Running health check..."
   HEALTH_OK=false
   for i in 1 2 3 4 5; do
     sleep 2
     if curl -sf http://localhost:${BACKEND_PORT}/api/health | grep -q '"status":"ok"'; then
       HEALTH_OK=true
       echo "  Health check PASSED (attempt $i)"
       break
     fi
     echo "  Health check attempt $i failed, retrying..."
   done

   if [ "$HEALTH_OK" = false ]; then
     echo "  CRITICAL: Health check failed after 5 attempts!"
     echo "  Rolling back binary..."
     LATEST_BACKUP=$(ls -t ${DEPLOY_PATH}/bin/zentra-server.bak.* 2>/dev/null | head -1)
     if [ -n "$LATEST_BACKUP" ]; then
       cp "$LATEST_BACKUP" ${DEPLOY_PATH}/bin/zentra-server
       systemctl restart zentra-api 2>/dev/null || true
       echo "  Rolled back to $LATEST_BACKUP"
     fi
   fi
   ```

4. The database backup MUST target the `zentra` database only (`-d zentra`).
5. Do NOT change the build steps.
6. Do NOT change the deploy target or server address.
7. Do NOT change any application code.

**Verification:**
- `bash -n deploy/deploy.sh` must pass
- Deploying should create a `.bak` file
- Failed health check should trigger rollback

---

## PROMPT 7.7 — Standardize Ports (Single Source of Truth)

**Objective:** Create a single configuration file that defines all ports, and update scripts to reference it.

**File to create:** `deploy/ports.env`
**Files to modify:** `deploy/deploy.sh`, `deploy/start-remote.sh`, `deploy/restart-services.sh`, `deploy/full-test.sh`

**What to do:**
1. Create `deploy/ports.env`:
   ```bash
   # Zentra Port Configuration — Single Source of Truth
   # Source this file in all scripts: source "$(dirname "$0")/ports.env"
   ZENTRA_API_PORT="${ZENTRA_API_PORT:-5006}"
   ZENTRA_FRONTEND_PORT="${ZENTRA_FRONTEND_PORT:-5010}"
   ZENTRA_AGENT_PORT="${ZENTRA_AGENT_PORT:-5001}"
   ZENTRA_METRICS_PORT="${ZENTRA_METRICS_PORT:-5002}"
   POSTGRES_PORT="${POSTGRES_PORT:-5434}"
   ```

2. In each script, add at the top (after `set -e` or shebang):
   ```bash
   # Source port configuration
   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
   source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true
   ```

3. Replace hardcoded port numbers with the variables:
   - `deploy/deploy.sh`: Replace `BACKEND_PORT="5006"` and `FRONTEND_PORT="5010"` with sourced values
   - `deploy/start-remote.sh`: Replace `BACKEND_PORT=6006` and `FRONTEND_PORT=6010` with sourced values
   - `deploy/restart-services.sh`: Replace hardcoded `6006` and `6001` with variables
   - `deploy/full-test.sh`: Replace `BASE="http://127.0.0.1:6010"` with `BASE="http://127.0.0.1:${ZENTRA_FRONTEND_PORT}"`

4. Do NOT change any test logic or API paths in the scripts.
5. Do NOT change any application code.
6. Do NOT change the database configuration.

**Verification:**
- `bash -n deploy/deploy.sh` must pass
- `bash -n deploy/start-remote.sh` must pass
- `bash -n deploy/restart-services.sh` must pass
- `bash -n deploy/full-test.sh` must pass
- All scripts should use the same port values from `ports.env`

---

## PROMPT 7.8 — Add Post-Deploy Smoke Test

**Objective:** Create an automated smoke test script that validates core functionality after deployment.

**File to create:** `deploy/smoke-test.sh`

**What to do:**
1. Create `deploy/smoke-test.sh`:
   ```bash
   #!/usr/bin/env bash
   # Zentra Post-Deploy Smoke Test
   # Run after deployment to verify core functionality.
   # Usage: ./deploy/smoke-test.sh [base_url]
   set -euo pipefail

   # Source port config
   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
   source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true

   BASE="${1:-http://127.0.0.1:${ZENTRA_API_PORT:-5006}}"
   PASS=0
   FAIL=0

   check() {
       local name="$1"
       local url="$2"
       local expected_status="${3:-200}"
       local method="${4:-GET}"
       local body="${5:-}"

       if [ "$method" = "POST" ]; then
           code=$(curl -sf -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" -d "$body" "$url" 2>/dev/null || echo "000")
       else
           code=$(curl -sf -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000")
       fi

       if [ "$code" = "$expected_status" ]; then
           echo "  ✓ PASS: $name (HTTP $code)"
           PASS=$((PASS + 1))
       else
           echo "  ✗ FAIL: $name (expected $expected_status, got $code)"
           FAIL=$((FAIL + 1))
       fi
   }

   echo "============================================"
   echo " Zentra Smoke Test"
   echo " Target: ${BASE}"
   echo " Date: $(date)"
   echo "============================================"
   echo ""

   echo "--- Core Health ---"
   check "Health endpoint" "${BASE}/api/health" "200"

   echo ""
   echo "--- Database Connectivity ---"
   # Health endpoint checks DB internally
   HEALTH_BODY=$(curl -sf "${BASE}/api/health" 2>/dev/null || echo "{}")
   if echo "$HEALTH_BODY" | grep -q '"status":"ok"'; then
       echo "  ✓ PASS: Database connected (zentra)"
       PASS=$((PASS + 1))
   else
       echo "  ✗ FAIL: Database check failed"
       FAIL=$((FAIL + 1))
   fi

   echo ""
   echo "--- Auth Endpoints ---"
   check "Login endpoint reachable" "${BASE}/api/auth/login" "400" "POST" '{}'
   # 400 is expected because we send empty body

   echo ""
   echo "--- API Endpoints ---"
   check "System status" "${BASE}/api/system/status" "200"

   echo ""
   echo "--- Security Headers ---"
   HEADERS=$(curl -sI "${BASE}/api/health" 2>/dev/null || echo "")
   if echo "$HEADERS" | grep -qi "x-content-type-options"; then
       echo "  ✓ PASS: X-Content-Type-Options present"
       PASS=$((PASS + 1))
   else
       echo "  ✗ FAIL: X-Content-Type-Options missing"
       FAIL=$((FAIL + 1))
   fi

   echo ""
   echo "============================================"
   echo " RESULTS: $PASS passed, $FAIL failed"
   echo " Total:   $((PASS + FAIL)) checks"
   if [ $FAIL -gt 0 ]; then
       echo " STATUS:  FAILED"
       exit 1
   else
       echo " STATUS:  PASSED"
   fi
   echo "============================================"
   ```

2. Do NOT include any hardcoded credentials.
3. Do NOT change any application code.
4. Do NOT write to any database.
5. The script only reads from the API — no mutations.

**Verification:**
- `bash -n deploy/smoke-test.sh` must pass
- Running against a live Zentra instance should produce pass/fail output

---
---

# ═══════════════════════════════════════════════════════════════════════════
# PHASE 9: ARCHITECTURE & LONG-TERM
# ═══════════════════════════════════════════════════════════════════════════

---

## PROMPT 9.1 — Define Plugin Trait Interface

**Objective:** Create a standard `Plugin` trait that all monitoring plugins must implement, replacing the current hardcoded enum wrapper.

**File to create:** `crates/zentra-core/src/plugin.rs`
**File to modify:** `crates/zentra-core/src/lib.rs` (add `pub mod plugin;`)

**What to do:**
1. Create `crates/zentra-core/src/plugin.rs`:
   ```rust
   //! Plugin trait definition for Zentra monitoring plugins.
   //! All plugins must implement this trait to integrate with the agent collection loop.

   use async_trait::async_trait;
   use serde::{Deserialize, Serialize};
   use std::collections::HashMap;
   use uuid::Uuid;

   /// Metadata about a plugin
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct PluginInfo {
       pub name: String,
       pub version: String,
       pub description: String,
       pub plugin_type: PluginType,
   }

   /// Plugin category
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
   pub enum PluginType {
       Server,
       Network,
       Database,
       Storage,
       Security,
       Cloud,
       Custom(String),
   }

   /// Result of a single metric collection
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CollectedMetric {
       pub name: String,
       pub value: f64,
       pub labels: HashMap<String, String>,
       pub timestamp: chrono::DateTime<chrono::Utc>,
   }

   /// Result of a collection run
   #[derive(Debug, Clone)]
   pub struct CollectionResult {
       pub metrics: Vec<CollectedMetric>,
       pub logs: Vec<String>,
       pub errors: Vec<String>,
   }

   /// Plugin configuration passed to initialize()
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct PluginConfig {
       pub target_id: Uuid,
       pub target_host: String,
       pub target_port: u16,
       pub credentials: Option<HashMap<String, String>>,
       pub settings: HashMap<String, String>,
   }

   /// The core plugin trait. All monitoring plugins must implement this.
   #[async_trait]
   pub trait Plugin: Send + Sync {
       /// Return metadata about this plugin
       fn info(&self) -> PluginInfo;

       /// Initialize the plugin with configuration
       async fn initialize(&mut self, config: PluginConfig) -> Result<(), String>;

       /// Collect metrics from the target
       async fn collect(&self) -> Result<CollectionResult, String>;

       /// Health check — is the plugin connected and working?
       async fn health_check(&self) -> Result<bool, String>;

       /// Cleanup resources
       async fn shutdown(&self) -> Result<(), String> {
           Ok(()) // Default: no-op
       }
   }
   ```

2. Add `pub mod plugin;` to `crates/zentra-core/src/lib.rs`.
3. Add `async-trait = { workspace = true }` to `crates/zentra-core/Cargo.toml` if not already present.
4. Do NOT change any existing plugin implementations yet — this is just the trait definition.
5. Do NOT change any handler code.
6. Do NOT change the database.
7. The existing plugins will be migrated to this trait in future prompts.

**Verification:**
- `cargo build -p zentra-core` must succeed
- `cargo test -p zentra-core` must pass
- The trait should be importable from other crates: `use zentra_core::plugin::Plugin;`

---

## PROMPT 9.2 — Add API Versioning

**Objective:** Add `/api/v1/` prefix to all API routes while maintaining backward compatibility with the existing `/api/` prefix.

**File to modify:** `crates/zentra-api/src/routes.rs` (or wherever `build_router()` is defined)

**What to do:**
1. Find the `build_router()` function that defines all API routes.
2. Wrap ALL existing routes under a `/v1` nested router:
   ```rust
   let v1_routes = Router::new()
       .route("/health", get(handlers::health::health))
       .route("/targets", get(handlers::targets::list_targets))
       // ... all existing routes ...
       ;

   Router::new()
       // New versioned path
       .nest("/api/v1", v1_routes.clone())
       // Backward-compatible unversioned path (same routes)
       .nest("/api", v1_routes)
   ```
3. This way:
   - `/api/health` continues to work (backward compatible)
   - `/api/v1/health` also works (new versioned path)
   - Future breaking changes can go under `/api/v2/`
4. Do NOT change any handler implementations.
5. Do NOT change the frontend API client yet (it still uses `/api/`).
6. Do NOT change any response formats.
7. Do NOT change the database.
8. Do NOT remove any existing routes.

**Verification:**
- `cargo build` must succeed
- `curl http://localhost:5006/api/health` must still work
- `curl http://localhost:5006/api/v1/health` must also work
- Both paths return identical responses

---

## PROMPT 9.3 — Migrate lazy_static to std::sync::OnceLock

**Objective:** Replace `lazy_static!` macros with the standard library's `std::sync::OnceLock` (stable since Rust 1.80).

**Files to modify:** All files in `crates/zentra-api/src/` that use `lazy_static!`

**What to do:**
1. Search for all `lazy_static!` usages in the codebase:
   ```bash
   grep -rn "lazy_static" crates/
   ```
2. For each usage, replace with `std::sync::OnceLock`:
   ```rust
   // BEFORE:
   lazy_static::lazy_static! {
       static ref ORACLE_CONNECTIONS: Arc<RwLock<Vec<OracleConnection>>> = Arc::new(RwLock::new(Vec::new()));
   }

   // AFTER:
   use std::sync::OnceLock;
   use parking_lot::RwLock;

   fn oracle_connections() -> &'static RwLock<Vec<OracleConnection>> {
       static INSTANCE: OnceLock<RwLock<Vec<OracleConnection>>> = OnceLock::new();
       INSTANCE.get_or_init(|| RwLock::new(Vec::new()))
   }
   // Then replace ORACLE_CONNECTIONS.read() with oracle_connections().read()
   ```
3. For simpler cases (just a String or value):
   ```rust
   // BEFORE:
   lazy_static! { static ref CONFIG: String = "value".into(); }
   // AFTER:
   static CONFIG: OnceLock<String> = OnceLock::new();
   fn config() -> &'static str { CONFIG.get_or_init(|| "value".into()) }
   ```
4. After migration, remove `lazy_static` from `Cargo.toml` dependencies if no longer used.
5. Do NOT change any logic — only change the initialization pattern.
6. Do NOT change any function signatures that the rest of the code depends on.
7. If changing a `static ref` that is used with `.read()` and `.write()` throughout the codebase, you may need to update all call sites from `ORACLE_CONNECTIONS.read()` to `oracle_connections().read()`. That's acceptable.
8. Do NOT change the database.

**Verification:**
- `cargo build` must succeed
- `cargo test --workspace` must pass
- No `lazy_static!` macros should remain (unless used by a dependency)

---

## PROMPT 9.4 — Write Architecture Decision Records (ADRs)

**Objective:** Document key architectural decisions for future maintainers.

**Files to create:**
- `docs/adr/001-rust-axum-backend.md`
- `docs/adr/002-ssh-oracle-monitoring.md`
- `docs/adr/003-timescaledb-metrics.md`
- `docs/adr/004-multi-tenant-org-id.md`

**What to do:**
1. Create `docs/adr/001-rust-axum-backend.md`:
   ```markdown
   # ADR 001: Rust + Axum for Backend

   ## Status: Accepted
   ## Date: 2025-01-01

   ## Context
   Zentra needs a high-performance backend for real-time monitoring data ingestion and query.

   ## Decision
   Use Rust with Axum web framework and SQLx for database access.

   ## Consequences
   - **Positive:** Zero-cost abstractions, memory safety, excellent async performance
   - **Positive:** Compile-time SQL checking via sqlx macros
   - **Negative:** Steeper learning curve for new contributors
   - **Negative:** Longer compile times
   ```

2. Create `docs/adr/002-ssh-oracle-monitoring.md`:
   ```markdown
   # ADR 002: SSH + sqlplus for Oracle Monitoring

   ## Status: Accepted (under review for migration to rust-oracle)
   ## Date: 2025-01-01

   ## Context
   Oracle monitoring requires executing SQL against remote Oracle instances.

   ## Decision
   Use SSH to remote hosts and pipe SQL to sqlplus via local Oracle Instant Client.

   ## Consequences
   - **Positive:** Works without Oracle client libraries on the Zentra server (only on target)
   - **Positive:** Familiar to Oracle DBAs
   - **Negative:** Command injection risk (mitigated in Phase 1 with base64 encoding)
   - **Negative:** Performance overhead of SSH + process spawn per query
   - **Negative:** Requires SSH key distribution to all Oracle hosts

   ## Future
   Evaluate `rust-oracle` crate for native JDBC-like connections.
   ```

3. Create `docs/adr/003-timescaledb-metrics.md`:
   ```markdown
   # ADR 003: TimescaleDB for Time-Series Metrics

   ## Status: Accepted
   ## Date: 2025-01-01

   ## Context
   Metrics data is time-series and needs efficient storage, aggregation, and retention management.

   ## Decision
   Use TimescaleDB (PostgreSQL extension) with hypertables, continuous aggregates, and retention policies.

   ## Consequences
   - **Positive:** SQL-compatible, no separate time-series database to manage
   - **Positive:** Continuous aggregates for dashboard performance
   - **Positive:** Built-in compression and retention policies
   - **Negative:** Requires TimescaleDB license for some features
   - **Negative:** Single-node limitation without TimescaleDB Cloud
   ```

4. Create `docs/adr/004-multi-tenant-org-id.md`:
   ```markdown
   # ADR 004: Multi-Tenant Isolation via org_id

   ## Status: Accepted (enhanced with RLS in Phase 5)
   ## Date: 2025-01-01

   ## Context
   Zentra must support multiple organizations in a single database instance.

   ## Decision
   Use `org_id UUID` column on all tenant-scoped tables. Extract from JWT claims in middleware.

   ## Consequences
   - **Positive:** Simple implementation, no schema-per-tenant complexity
   - **Positive:** Supports shared infrastructure reporting
   - **Negative:** Relies on application code to always add WHERE org_id = $1
   - **Negative:** Cross-tenant query possible if a handler forgets the filter
   - **Mitigation:** PostgreSQL Row-Level Security added in Phase 5 as defense-in-depth
   ```

5. Do NOT change any application code.
6. Do NOT change the database.
7. These are documentation-only files.

**Verification:**
- Files should be readable Markdown
- No code changes required

---

## PROMPT 9.5 — Write Critical Runbooks

**Objective:** Create operational runbooks for incident response and database recovery.

**Files to create:**
- `docs/runbooks/incident-response.md`
- `docs/runbooks/database-recovery.md`
- `docs/runbooks/security-hardening-checklist.md`

**What to do:**
1. Create `docs/runbooks/incident-response.md`:
   ```markdown
   # Zentra Incident Response Runbook

   ## Service Health Check
   1. Check API health: `curl http://10.88.11.162:5006/api/health`
   2. Check process: `ps aux | grep zentra`
   3. Check logs: `tail -100 /store/Zentra/logs/api.log`
   4. Check database: `psql -h 10.88.11.162 -p 5434 -U postgres -d zentra -c "SELECT 1"`

   ## Common Issues

   ### API Not Responding
   1. Check if process is running: `pgrep -f zentra-server`
   2. Check port: `ss -tlnp | grep 5006`
   3. Restart: `systemctl restart zentra-api`
   4. Check logs for errors: `journalctl -u zentra-api -n 50`

   ### Database Connection Failed
   1. Check PostgreSQL: `systemctl status postgresql`
   2. Check connections: `psql -d zentra -c "SELECT count(*) FROM pg_stat_activity"`
   3. Check pool exhaustion: Look for "pool timed out" in api.log

   ### High Memory Usage
   1. Check process memory: `ps aux | grep zentra`
   2. Check for flow aggregation growth (unbounded HashMap in netflow)
   3. Restart if necessary: `systemctl restart zentra-api`

   ### Authentication Issues
   1. Verify JWT secret is set: `grep ZENTRA_JWT_SECRET /store/Zentra/.env`
   2. Check user lockout: `psql -d zentra -c "SELECT email, is_locked FROM users WHERE is_locked = true"`
   3. Unlock user: `psql -d zentra -c "UPDATE users SET is_locked = false, locked_until = NULL WHERE email = '<email>'"`

   ## Escalation
   1. Check this runbook first
   2. Check docs/adr/ for architectural context
   3. Search logs for error patterns
   4. Escalate to on-call engineer
   ```

2. Create `docs/runbooks/database-recovery.md`:
   ```markdown
   # Zentra Database Recovery Runbook

   ## Database: zentra (PostgreSQL 15 + TimescaleDB)
   ## Host: 10.88.11.162:5434

   ## Regular Backup
   ```
   ./deploy/backup-db.sh --full
   ```

   ## Restore from Backup
   ```
   # Stop API server
   systemctl stop zentra-api

   # Restore from dump
   pg_restore -h 10.88.11.162 -p 5434 -U postgres -d zentra \
     --clean --if-exists \
     /store/Zentra/backups/zentra_full_YYYYMMDD_HHMMSS.dump

   # Run any pending migrations
   sqlx migrate run --database-url "postgresql://postgres:$PGPASSWORD@10.88.11.162:5434/zentra" --source ./migrations

   # Restart API server
   systemctl start zentra-api

   # Verify
   curl http://localhost:5006/api/health
   ```

   ## Point-in-Time Recovery
   Requires WAL archiving to be enabled (not currently configured).
   TODO: Configure continuous_archiving in postgresql.conf.

   ## Schema-Only Recovery
   ```
   ./deploy/backup-db.sh --schema-only
   # Then restore schema and re-run migrations
   ```

   ## Important Notes
   - All operations target the `zentra` database ONLY
   - Do NOT restore to the `postgres` database
   - Always backup before migration deployments
   ```

3. Create `docs/runbooks/security-hardening-checklist.md`:
   ```markdown
   # Zentra Production Security Hardening Checklist

   ## Before First Production Deploy

   - [ ] Set `ZENTRA_JWT_SECRET` to a random 64+ character string
   - [ ] Set `ZENTRA_MASTER_KEY` to a random 64-character hex string (32 bytes)
   - [ ] Set `ZENTRA_ALLOW_INSECURE_JWT=0` (or unset — default is secure)
   - [ ] Change default admin password via `/api/auth/change-password`
   - [ ] Set `ZENTRA_ALLOW_ORIGINS` to specific frontend URLs (not `*`)
   - [ ] Configure PostgreSQL to require SSL connections
   - [ ] Set up SSH key-based auth for all monitored targets
   - [ ] Create non-root `zentra` service user
   - [ ] Enable nginx with TLS (HTTPS)
   - [ ] Configure firewall rules (only allow 443 from outside)
   - [ ] Set `ZENTRA_SSH_KNOWN_HOSTS=/root/.ssh/known_hosts`
   - [ ] Remove `deploy/login.json` and `deploy/oraconn.json` from server
   - [ ] Rotate all passwords from development (PostgreSQL, Oracle)

   ## Database (zentra)
   - [ ] PostgreSQL user for app should NOT be superuser
   - [ ] Enable RLS policies (Phase 5)
   - [ ] Configure automated backups (`deploy/backup-db.sh` via cron)
   - [ ] Enable TimescaleDB retention policies
   - [ ] Set `statement_timeout` in postgresql.conf

   ## Ongoing
   - [ ] Review audit_log table monthly
   - [ ] Rotate JWT secret quarterly
   - [ ] Run `cargo audit` and `npm audit` weekly
   - [ ] Review nginx access logs for suspicious patterns
   ```

4. Do NOT change any application code.
5. Do NOT change the database.
6. All database references must specify `zentra` as the target database.

**Verification:**
- All files should be readable Markdown
- Database commands in runbooks should reference `zentra` database only

---

## PROMPT 9.6 — Remove Dead Dependencies

**Objective:** Remove unused dependencies from the project.

**Files to modify:**
- `frontend/package.json` — Remove unused `zustand` dependency

**What to do:**
1. Remove `zustand` from `frontend/package.json` dependencies:
   ```json
   // REMOVE this line:
   "zustand": "^4.5.4",
   ```
2. Search for any imports of `zustand` in the frontend source:
   ```bash
   grep -rn "zustand" frontend/src/
   ```
   If there are NO imports, safely remove it.
   If there ARE imports, do NOT remove it — report the finding instead.

3. Run `npm install` to update the lock file.
4. Do NOT remove any other dependencies.
5. Do NOT change any application code.

**Verification:**
- `cd frontend && npm install` must succeed
- `cd frontend && npm run build` must succeed
- `cd frontend && npm run typecheck` must succeed
- No runtime errors related to missing zustand

---

## PROMPT 9.7 — Add CHANGELOG

**Objective:** Create a CHANGELOG file to track version history.

**File to create:** `CHANGELOG.md`

**What to do:**
1. Create `CHANGELOG.md`:
   ```markdown
   # Changelog

   All notable changes to the Zentra project will be documented in this file.

   The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

   ## [Unreleased]

   ### Security
   - Fixed SQL/command injection in Oracle SSH queries (Phase 1)
   - Fixed SSRF vulnerability in SSH host verification (Phase 1)
   - Removed hardcoded credentials from deploy scripts (Phase 1)
   - Added ZENTRA_MASTER_KEY fail-hard in production (Phase 1)
   - Added password strength requirements (Phase 1)
   - Added JWT token revocation (Phase 1)
   - Added input validation with validator crate (Phase 2)
   - Added ReDoS protection for custom regex (Phase 2)
   - Tightened CSP header (Phase 7)
   - Added SSH host key verification (Phase 7)

   ### Added
   - CI/CD pipeline with GitHub Actions (Phase 3)
   - Auth integration tests (Phase 3)
   - Multi-tenant isolation tests (Phase 3)
   - Frontend test infrastructure with Vitest (Phase 3)
   - Database backup script (Phase 5)
   - Post-deploy smoke test (Phase 7)
   - Plugin trait interface (Phase 9)
   - API versioning (/api/v1/) (Phase 9)
   - Architecture Decision Records (Phase 9)
   - Operational runbooks (Phase 9)

   ### Changed
   - Standardized error handling to ApiError enum (Phase 2)
   - Reduced frontend polling from 10s to 30s (Phase 6)
   - Parallelized Prometheus scraper (Phase 6)
   - Cached regex patterns in log engine (Phase 6)
   - Services run as non-root user (Phase 7)
   - Pinned K8s image tags to semver (Phase 7)
   - Migrated lazy_static to std::sync::OnceLock (Phase 9)

   ### Fixed
   - Memory leaks in anomaly detector, baseline engine, netflow, ML (Phase 4)
   - Panic-prone .unwrap() calls in log engine, netflow, provisioning (Phase 4)
   - Migration conflicts (006/007, 010/012, 012/015) (Phase 5)
   - Inconsistent loading/error states in frontend (Phase 8)

   ## [1.0.0] - 2025-01-01

   ### Added
   - Initial release of Zentra Enterprise Monitoring Platform
   - Infrastructure monitoring (servers, storage, network)
   - Database monitoring (Oracle RAC, ASM, Dataguard)
   - Log aggregation and search
   - Alerting with 8 notification channels
   - AI/ML anomaly detection
   - Multi-tenant support
   - React frontend with 47 routes
   ```

2. Do NOT change any application code.
3. Do NOT change the database.

**Verification:**
- File should be valid Markdown
- All phase references should match the actual work done

---

## END OF PHASE 7 & 9 PROMPTS
