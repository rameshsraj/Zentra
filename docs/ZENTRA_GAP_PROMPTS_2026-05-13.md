# ZENTRA — Gap Remediation Prompts

> **Generated:** 13 May 2026  
> **Source:** `ZENTRA_GAP_ANALYSIS_2026-05-13.md`  
> **Purpose:** Copy-paste prompts to close every identified gap  
> **Execution Order:** Priority 1 (HIGH) → Priority 2 (MEDIUM) → Priority 3 (LOW)

---

## Table of Contents

1. [Priority 1 — Database Schema Gaps (HIGH)](#priority-1--database-schema-gaps-high)
2. [Priority 2 — Frontend Gaps (MEDIUM)](#priority-2--frontend-gaps-medium)
3. [Priority 3 — Backend Repository Layer (MEDIUM)](#priority-3--backend-repository-layer-medium)
4. [Priority 4 — RLS & Security Provisioning (MEDIUM)](#priority-4--rls--security-provisioning-medium)
5. [Priority 5 — Plugin Trait Alignment (LOW)](#priority-5--plugin-trait-alignment-low)
6. [Priority 6 — Deploy Scripts Sync (LOW)](#priority-6--deploy-scripts-sync-low)
7. [Priority 7 — Documentation Corrections (LOW)](#priority-7--documentation-corrections-low)

---

## Priority 1 — Database Schema Gaps (HIGH)

### Prompt 1.1 — Migration 025: Dashboard Feature Tables

```
Create a new SQL migration file at migrations/20250101000025_dashboard_features.sql that adds the four missing dashboard feature tables to the Zentra database.

Reference the existing dashboard schema in migrations/20250101000007_dashboards.sql and migrations/20250101000009_dashboard_enhance.sql for conventions (UUID PKs, org_id tenant isolation, created_at/updated_at timestamps).

Create these four tables:

1. **dashboard_snapshots**
   - snapshot_id UUID PRIMARY KEY DEFAULT gen_random_uuid()
   - org_id UUID NOT NULL
   - dashboard_id UUID NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE
   - name VARCHAR(255) NOT NULL
   - snapshot_data JSONB NOT NULL (stores full dashboard state: panels, variables, time range, filters)
   - expires_at TIMESTAMPTZ (NULL = never expires)
   - created_by UUID NOT NULL REFERENCES users(id)
   - created_at TIMESTAMPTZ NOT NULL DEFAULT now()
   - Add index on (org_id, dashboard_id)
   - Add index on (expires_at) WHERE expires_at IS NOT NULL for cleanup jobs

2. **dashboard_playlists**
   - playlist_id UUID PRIMARY KEY DEFAULT gen_random_uuid()
   - org_id UUID NOT NULL
   - name VARCHAR(255) NOT NULL
   - description TEXT
   - dashboard_ids UUID[] NOT NULL DEFAULT '{}'
   - interval_seconds INT NOT NULL DEFAULT 30
   - enabled BOOLEAN NOT NULL DEFAULT TRUE
   - created_by UUID NOT NULL REFERENCES users(id)
   - created_at TIMESTAMPTZ NOT NULL DEFAULT now()
   - updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
   - Add UNIQUE(org_id, name)

3. **dashboard_bookmarks**
   - bookmark_id UUID PRIMARY KEY DEFAULT gen_random_uuid()
   - org_id UUID NOT NULL
   - user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE
   - dashboard_id UUID NOT NULL REFERENCES dashboards(id) ON DELETE CASCADE
   - sort_order INT NOT NULL DEFAULT 0
   - created_at TIMESTAMPTZ NOT NULL DEFAULT now()
   - Add UNIQUE(user_id, dashboard_id) to prevent duplicates
   - Add index on (user_id, sort_order)

4. **query_history**
   - history_id UUID PRIMARY KEY DEFAULT gen_random_uuid()
   - org_id UUID NOT NULL
   - user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE
   - query_text TEXT NOT NULL
   - datasource VARCHAR(100) NOT NULL DEFAULT 'prometheus'
   - duration_ms INT
   - starred BOOLEAN NOT NULL DEFAULT FALSE
   - comment TEXT
   - created_at TIMESTAMPTZ NOT NULL DEFAULT now()
   - Add index on (user_id, created_at DESC)
   - Add index on (user_id, starred) WHERE starred = TRUE

Follow the project convention of using IF NOT EXISTS on all CREATE TABLE and CREATE INDEX statements. Add a comment block at the top of the file explaining what this migration adds.
```

---

### Prompt 1.2 — Migration 026: NetFlow Continuous Aggregates

```
Create a new SQL migration file at migrations/20250101000026_netflow_aggregates.sql that adds the two missing continuous aggregates for the netflow_records hypertable.

Reference the existing continuous aggregates in migrations/20250101000002_indexes.sql (metrics_1m, metrics_1h, metrics_1d) and migrations/20250101000008_timescaledb_enhance.sql (metrics_5m) for conventions.

The source hypertable is `netflow_records` (created in migration 018). Check migration 20250101000018_netflow_cloud_tables.sql for the exact column names (timestamp column, org_id, src_ip, dst_ip, src_port, dst_port, protocol, bytes, packets, etc.).

Create:

1. **netflow_1m** — 1-minute continuous aggregate
   - Use CREATE MATERIALIZED VIEW netflow_1m WITH (timescaledb.continuous) AS
   - Bucket by time_bucket('1 minute', <timestamp_column>)
   - Aggregate: SUM(bytes) as total_bytes, SUM(packets) as total_packets, COUNT(*) as flow_count
   - Group by: time bucket, org_id, protocol, src_ip, dst_ip
   - Add continuous aggregate policy: start_offset => INTERVAL '10 minutes', end_offset => INTERVAL '1 minute', schedule_interval => INTERVAL '1 minute'

2. **netflow_1h** — 1-hour continuous aggregate
   - Use CREATE MATERIALIZED VIEW netflow_1h WITH (timescaledb.continuous) AS
   - Bucket by time_bucket('1 hour', <timestamp_column>)
   - Aggregate: SUM(bytes) as total_bytes, SUM(packets) as total_packets, COUNT(*) as flow_count
   - Group by: time bucket, org_id, protocol, src_ip, dst_ip
   - Add continuous aggregate policy: start_offset => INTERVAL '4 hours', end_offset => INTERVAL '1 hour', schedule_interval => INTERVAL '1 hour'

Use SELECT add_continuous_aggregate_policy() for the refresh policies. Wrap the view creation in DO $$ blocks with exception handling so the migration is idempotent. Add a comment block at the top explaining what this migration adds.
```

---

### Prompt 1.3 — Backend: Dashboard Snapshots Repository & Handlers

```
In the Zentra project, implement backend support for the new dashboard_snapshots table:

1. **Repository** — Add a new file crates/zentra-db/src/repositories/snapshots.rs with:
   - create_snapshot(org_id, dashboard_id, name, snapshot_data, expires_at, created_by) -> Result<Uuid>
   - get_snapshot(org_id, snapshot_id) -> Result<Option<DashboardSnapshot>>
   - list_snapshots(org_id, dashboard_id) -> Result<Vec<DashboardSnapshot>>
   - delete_snapshot(org_id, snapshot_id) -> Result<bool>
   - delete_expired_snapshots() -> Result<u64> (cleanup job)
   Register the module in crates/zentra-db/src/repositories/mod.rs

2. **Model** — Add a DashboardSnapshot struct in zentra-core models or inline in the repository:
   - snapshot_id: Uuid
   - org_id: Uuid
   - dashboard_id: Uuid
   - name: String
   - snapshot_data: serde_json::Value
   - expires_at: Option<DateTime<Utc>>
   - created_by: Uuid
   - created_at: DateTime<Utc>

3. **Handler** — The handler file crates/zentra-api/src/handlers/dashboards.rs already exists. Add these handler functions:
   - create_snapshot — POST /api/dashboards/:id/snapshots
   - list_snapshots — GET /api/dashboards/:id/snapshots
   - get_snapshot — GET /api/snapshots/:snapshot_id
   - delete_snapshot — DELETE /api/snapshots/:snapshot_id

4. **Routes** — Register the new endpoints in crates/zentra-api/src/routes/dashboards.rs

Follow the existing patterns: use sqlx::query_as! macros, extract OrgId from JWT via the existing middleware, return JSON responses with proper error handling. Validate input (name length, snapshot_data not null).
```

---

### Prompt 1.4 — Backend: Dashboard Playlists Repository & Handlers

```
In the Zentra project, implement backend support for the new dashboard_playlists table:

1. **Repository** — Add crates/zentra-db/src/repositories/playlists.rs:
   - create_playlist(org_id, name, description, dashboard_ids, interval_seconds, created_by) -> Result<Uuid>
   - get_playlist(org_id, playlist_id) -> Result<Option<DashboardPlaylist>>
   - list_playlists(org_id) -> Result<Vec<DashboardPlaylist>>
   - update_playlist(org_id, playlist_id, name, description, dashboard_ids, interval_seconds, enabled) -> Result<bool>
   - delete_playlist(org_id, playlist_id) -> Result<bool>
   Register in repositories/mod.rs

2. **Handler** — Add to the existing dashboards.rs handler or create a new handler file crates/zentra-api/src/handlers/playlists.rs:
   - create_playlist — POST /api/playlists
   - list_playlists — GET /api/playlists
   - get_playlist — GET /api/playlists/:id
   - update_playlist — PUT /api/playlists/:id
   - delete_playlist — DELETE /api/playlists/:id

3. **Routes** — Register endpoints in crates/zentra-api/src/routes/dashboards.rs under a .nest("/playlists", ...) sub-router.

Follow existing project patterns. Validate that all dashboard_ids in the array exist and belong to the same org. Validate interval_seconds is between 5 and 3600.
```

---

### Prompt 1.5 — Backend: Bookmarks & Query History Repository & Handlers

```
In the Zentra project, implement backend support for dashboard_bookmarks and query_history tables:

**Part A — Bookmarks:**

1. Add crates/zentra-db/src/repositories/bookmarks.rs:
   - add_bookmark(org_id, user_id, dashboard_id) -> Result<Uuid>
   - remove_bookmark(org_id, user_id, dashboard_id) -> Result<bool>
   - list_bookmarks(org_id, user_id) -> Result<Vec<DashboardBookmark>>
   - reorder_bookmarks(org_id, user_id, bookmark_ids: Vec<Uuid>) -> Result<()>

2. Add handlers in crates/zentra-api/src/handlers/dashboards.rs:
   - POST /api/bookmarks — add bookmark
   - DELETE /api/bookmarks/:dashboard_id — remove bookmark
   - GET /api/bookmarks — list user's bookmarks
   - PUT /api/bookmarks/reorder — reorder bookmarks

**Part B — Query History:**

1. The handler file crates/zentra-api/src/handlers/query_history.rs already exists. Update it to use the new query_history table instead of any in-memory or stub implementation:
   - record_query(org_id, user_id, query_text, datasource, duration_ms) -> Result<Uuid>
   - list_history(org_id, user_id, limit, offset) -> Result<Vec<QueryHistoryEntry>>
   - star_query(org_id, user_id, history_id, starred: bool) -> Result<bool>
   - delete_history_entry(org_id, user_id, history_id) -> Result<bool>
   - get_starred(org_id, user_id) -> Result<Vec<QueryHistoryEntry>>

2. Ensure the existing routes for query_history in crates/zentra-api/src/routes/dashboards.rs are wired to the updated handlers.

Register both repository modules in repositories/mod.rs. Follow existing project conventions for error handling and OrgId extraction.
```

---

### Prompt 1.6 — Frontend: Snapshots, Playlists, Bookmarks, Query History API

```
In the Zentra frontend, add API client functions to support the four new dashboard feature endpoints.

Update frontend/src/lib/api/dashboards.ts to add:

**Snapshots:**
- createSnapshot(dashboardId: string, name: string, expiresAt?: string): Promise<{ snapshot_id: string }>
- listSnapshots(dashboardId: string): Promise<DashboardSnapshot[]>
- getSnapshot(snapshotId: string): Promise<DashboardSnapshot>
- deleteSnapshot(snapshotId: string): Promise<void>

**Playlists:**
- createPlaylist(data: CreatePlaylistRequest): Promise<{ playlist_id: string }>
- listPlaylists(): Promise<DashboardPlaylist[]>
- getPlaylist(id: string): Promise<DashboardPlaylist>
- updatePlaylist(id: string, data: UpdatePlaylistRequest): Promise<void>
- deletePlaylist(id: string): Promise<void>

**Bookmarks:**
- addBookmark(dashboardId: string): Promise<{ bookmark_id: string }>
- removeBookmark(dashboardId: string): Promise<void>
- listBookmarks(): Promise<DashboardBookmark[]>
- reorderBookmarks(bookmarkIds: string[]): Promise<void>

**Query History:**
- listQueryHistory(limit?: number, offset?: number): Promise<QueryHistoryEntry[]>
- starQuery(historyId: string, starred: boolean): Promise<void>
- deleteQueryHistory(historyId: string): Promise<void>
- getStarredQueries(): Promise<QueryHistoryEntry[]>

Add the corresponding TypeScript interfaces (DashboardSnapshot, DashboardPlaylist, DashboardBookmark, QueryHistoryEntry, CreatePlaylistRequest, UpdatePlaylistRequest) to frontend/src/lib/api/types.ts.

Follow the existing pattern in dashboards.ts: use the fetchJSON helper from _client.ts, use proper HTTP methods (GET/POST/PUT/DELETE), include error handling.
```

---

## Priority 2 — Frontend Gaps (MEDIUM)

### Prompt 2.1 — MiddlewareServicePage Dynamic Route

```
In the Zentra frontend, the MiddlewareServicePage.tsx component exists at frontend/src/pages/MiddlewareServicePage.tsx but has no route registered in App.tsx.

Update frontend/src/App.tsx to add a dynamic route for middleware services:

1. Add this route inside the protected Layout routes, after the existing /middleware/apache route:
   <Route path="/middleware/:service" element={<MiddlewareServicePage />} />

2. Ensure the import for MiddlewareServicePage is added at the top of App.tsx.

3. The route order matters — place the dynamic :service route AFTER the specific /middleware/nginx and /middleware/apache routes so those take priority.

This enables the MiddlewareServicePage component to render for all 10 remaining middleware types:
HAProxy, Tomcat, PHP-FPM, RabbitMQ, Kafka, Elasticsearch, Redis, Memcached, Docker, Kubernetes.

The :service parameter should be read via useParams() inside MiddlewareServicePage.tsx — verify the component already does this. If not, update it to extract the service name from the URL parameter and fetch data from the /api/middleware/:service backend endpoint.

Also update the MiddlewareMonitoring.tsx overview page so that clicking on any middleware service card navigates to /middleware/<service-name> using react-router's useNavigate or Link component, enabling drill-down from the overview.
```

---

### Prompt 2.2 — Frontend: Playlists UI Component

```
In the Zentra frontend, create a playlist management UI that integrates with the dashboards page.

1. Create a new component frontend/src/components/PlaylistManager.tsx:
   - Display a list of saved playlists with name, description, dashboard count, interval
   - "Create Playlist" button that opens a form modal with:
     - Name (required, max 255 chars)
     - Description (optional)
     - Dashboard selector (multi-select from available dashboards)
     - Interval (number input, 5-3600 seconds, default 30)
   - Edit and delete actions per playlist
   - "Play" button that starts auto-rotation: cycles through the dashboards in the playlist at the configured interval, rendering each dashboard in full-screen mode

2. Add a "Playlists" tab or button to the Dashboard page (frontend/src/pages/Dashboard.tsx or a dedicated page).

3. Use the API functions from dashboards.ts (listPlaylists, createPlaylist, updatePlaylist, deletePlaylist) with TanStack React Query for data fetching.

4. Use existing UI patterns: Tailwind CSS classes, Lucide icons, the same modal/form patterns used in AddTargetModal.tsx.
```

---

### Prompt 2.3 — Frontend: Bookmarks Sidebar Widget

```
In the Zentra frontend, add a bookmarks feature to the sidebar navigation.

1. Update frontend/src/components/Layout.tsx to add a "Bookmarks" section in the sidebar:
   - Show a collapsible "Bookmarks" section with a star icon
   - List the user's bookmarked dashboards as sidebar links
   - Each bookmark links to /dashboard/:id (or however dashboards are routed)
   - Allow drag-to-reorder (or up/down arrows)
   - Show a remove (unstar) button on hover

2. Add a "Bookmark" toggle button on each dashboard detail view:
   - Star icon (filled = bookmarked, outline = not bookmarked)
   - Clicking toggles the bookmark state via addBookmark/removeBookmark API calls

3. Use the API functions from dashboards.ts with React Query. Invalidate the bookmark list when adding/removing.

Follow existing Layout.tsx patterns for sidebar sections.
```

---

## Priority 3 — Backend Repository Layer (MEDIUM)

### Prompt 3.1 — Repository: Dashboards

```
In the Zentra project, create a comprehensive dashboards repository to replace direct SQL in the dashboards handler.

Create crates/zentra-db/src/repositories/dashboards.rs with these functions:

**Dashboard CRUD:**
- create_dashboard(pool, org_id, title, description, folder_id, created_by) -> Result<Dashboard>
- get_dashboard(pool, org_id, dashboard_id) -> Result<Option<Dashboard>>
- list_dashboards(pool, org_id, folder_id: Option<Uuid>, limit, offset) -> Result<Vec<Dashboard>>
- update_dashboard(pool, org_id, dashboard_id, title, description, folder_id) -> Result<bool>
- delete_dashboard(pool, org_id, dashboard_id) -> Result<bool>

**Panels:**
- create_panel(pool, dashboard_id, panel_type, title, config: serde_json::Value, grid_pos: serde_json::Value) -> Result<Uuid>
- list_panels(pool, dashboard_id) -> Result<Vec<DashboardPanel>>
- update_panel(pool, panel_id, title, config, grid_pos) -> Result<bool>
- delete_panel(pool, panel_id) -> Result<bool>

**Versions:**
- create_version(pool, dashboard_id, version_number, data: serde_json::Value, message, created_by) -> Result<Uuid>
- list_versions(pool, dashboard_id) -> Result<Vec<DashboardVersion>>
- get_version(pool, dashboard_id, version_number) -> Result<Option<DashboardVersion>>

**Folders:**
- create_folder(pool, org_id, name, parent_id: Option<Uuid>) -> Result<Uuid>
- list_folders(pool, org_id) -> Result<Vec<DashboardFolder>>
- delete_folder(pool, org_id, folder_id) -> Result<bool>

Register the module in repositories/mod.rs. Use sqlx::query_as! for type-safe queries. Define the necessary row structs (Dashboard, DashboardPanel, DashboardVersion, DashboardFolder) either in this file or in zentra-core models. Follow the pattern used in repositories/metrics.rs and repositories/targets.rs.

Then refactor crates/zentra-api/src/handlers/dashboards.rs to use the new repository functions instead of inline SQL.
```

---

### Prompt 3.2 — Repository: Users & Auth

```
In the Zentra project, create a users/auth repository to centralize user data access.

Create crates/zentra-db/src/repositories/users.rs with:

**User CRUD:**
- create_user(pool, org_id, email, password_hash, display_name, role) -> Result<User>
- get_user(pool, user_id) -> Result<Option<User>>
- get_user_by_email(pool, email) -> Result<Option<User>>
- list_users(pool, org_id, limit, offset) -> Result<Vec<User>>
- update_user(pool, user_id, display_name, email) -> Result<bool>
- delete_user(pool, org_id, user_id) -> Result<bool>
- lock_user(pool, user_id, locked_until: DateTime<Utc>) -> Result<bool>
- unlock_user(pool, user_id) -> Result<bool>

**Sessions:**
- create_session(pool, user_id, token_hash, refresh_hash, ip, user_agent, expires_at) -> Result<Uuid>
- get_session_by_token(pool, token_hash) -> Result<Option<UserSession>>
- delete_session(pool, session_id) -> Result<bool>
- list_sessions(pool, user_id) -> Result<Vec<UserSession>>
- cleanup_expired_sessions(pool) -> Result<u64>

**Login Attempts:**
- record_login_attempt(pool, email, ip, success) -> Result<()>
- count_recent_failures(pool, email, window_minutes: i32) -> Result<i64>

Register in repositories/mod.rs. Reference the users table schema from migration 20250101000012_auth_system.sql. Then refactor crates/zentra-api/src/handlers/auth.rs and crates/zentra-api/src/handlers/users.rs to use these repository functions.
```

---

### Prompt 3.3 — Repository: Oracle, Network, Security

```
In the Zentra project, create three additional repository modules:

**1. crates/zentra-db/src/repositories/oracle.rs:**
- create_connection(pool, org_id, name, host, port, service_name, auth_mode, credentials_encrypted) -> Result<Uuid>
- get_connection(pool, org_id, connection_id) -> Result<Option<OracleConnection>>
- list_connections(pool, org_id) -> Result<Vec<OracleConnection>>
- update_connection(pool, org_id, connection_id, ...) -> Result<bool>
- delete_connection(pool, org_id, connection_id) -> Result<bool>
- list_metric_definitions(pool, org_id) -> Result<Vec<OracleMetricDefinition>>
- upsert_metric_cache(pool, connection_id, metric_name, value, labels) -> Result<()>

**2. crates/zentra-db/src/repositories/network.rs:**
- insert_flow_batch(pool, flows: Vec<NetflowRecord>) -> Result<u64>
- query_flows(pool, org_id, time_range, filters) -> Result<Vec<NetflowRecord>>
- upsert_exporter(pool, exporter_ip, packets_rx, flows_rx) -> Result<()>
- list_exporters(pool, org_id) -> Result<Vec<FlowExporter>>
- add_to_blacklist(pool, org_id, ip, source, reason) -> Result<Uuid>
- remove_from_blacklist(pool, org_id, ip) -> Result<bool>
- list_blacklist(pool, org_id) -> Result<Vec<BlacklistEntry>>
- add_favorite(pool, org_id, ip_or_host, label) -> Result<Uuid>
- list_favorites(pool, org_id) -> Result<Vec<NetworkFavorite>>

**3. crates/zentra-db/src/repositories/security.rs:**
- insert_ids_alert(pool, org_id, alert_data) -> Result<Uuid>
- list_ids_alerts(pool, org_id, severity, status, limit, offset) -> Result<Vec<IdsAlert>>
- update_ids_alert_status(pool, org_id, alert_id, status) -> Result<bool>
- insert_connection_log(pool, log_data) -> Result<()>
- query_connection_logs(pool, org_id, time_range, filters) -> Result<Vec<ConnectionLog>>
- query_dns_logs(pool, org_id, time_range) -> Result<Vec<DnsLog>>
- query_ssl_logs(pool, org_id, time_range) -> Result<Vec<SslLog>>

Register all three in repositories/mod.rs. Reference migrations 014 (oracle), 018 (netflow), and 013 (network_security) for table schemas.
```

---

## Priority 4 — RLS & Security Provisioning (MEDIUM)

### Prompt 4.1 — RLS Role Provisioning & API Integration

```
In the Zentra project, the Row-Level Security migration (20250101000024_row_level_security.sql) creates RLS policies that depend on a `zentra_app` database role and `current_setting('app.current_org_id')`. This role may not exist in the database and the API server may not be setting the session variable.

Fix this in two parts:

**Part A — Database Role Setup:**

Create a new migration file migrations/20250101000027_rls_role_setup.sql that:
1. Creates the `zentra_app` role if it doesn't exist:
   DO $$ BEGIN
     IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'zentra_app') THEN
       CREATE ROLE zentra_app LOGIN;
     END IF;
   END $$;
2. Grants necessary permissions to zentra_app on all Zentra tables:
   GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO zentra_app;
   GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO zentra_app;
   ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO zentra_app;
3. Sets the password from an environment variable or uses a placeholder that must be changed.

**Part B — API Server Integration:**

Update crates/zentra-api/src/middleware.rs (or create a new middleware) to set the org_id session variable on every database connection:
1. After extracting the org_id from the JWT claims (already done by the OrgId extractor), execute:
   SET LOCAL app.current_org_id = '<org_id_uuid>';
   on the database connection before passing it to the handler.
2. Use SET LOCAL (not SET) so the variable is scoped to the current transaction only.
3. This must happen inside a transaction so the RLS policies can read it via current_setting('app.current_org_id').

Reference the existing auth_middleware.rs and tenant.rs for how org_id is currently extracted. The pool is accessed via the AppState. Wrap each handler's database operation in a transaction that first sets the session variable.
```

---

## Priority 5 — Plugin Trait Alignment (LOW)

### Prompt 5.1 — AI Plugin: Implement Plugin Trait

```
In the Zentra project, the ai-plugin crate (plugins/ai-plugin/) contains LLM security utilities but does NOT implement the Plugin trait from zentra-core. This prevents it from being orchestrated by the agent's plugin_manager.

Add a Plugin trait implementation to plugins/ai-plugin/src/lib.rs:

1. Add zentra-core as a dependency in plugins/ai-plugin/Cargo.toml
2. Create a struct AiPlugin with necessary configuration fields
3. Implement the Plugin trait:
   - metadata() → return PluginMetadata with name="ai", version, description
   - initialize() → setup any regex patterns or configuration
   - test_connection() → always return Ok(true) since AI plugin is local-only
   - collect_metrics() → run the LLM security checks (prompt injection detection, PII scan, toxicity score, hallucination detection) against configured LLM endpoints and return metrics:
     - ai_prompt_injection_detected (counter)
     - ai_pii_detected (counter by type)
     - ai_toxicity_score (gauge)
     - ai_hallucination_score (gauge)
   - health_check() → return Ok(true)
   - polling_interval() → configurable, default 5 minutes

4. Re-export AiPlugin from lib.rs so the agent can discover and load it.

Follow the pattern used in plugins/cloud-plugin/src/lib.rs for the CloudPlugin implementation.
```

---

### Prompt 5.2 — Security Plugin: Implement Plugin Trait

```
In the Zentra project, the security-plugin crate (plugins/security-plugin/) provides analytical modules but does NOT implement the Plugin trait. The security analysis modules are event-driven, but a Plugin wrapper would allow periodic security posture polling.

Add a Plugin trait implementation to plugins/security-plugin/src/lib.rs:

1. Add zentra-core as a dependency in plugins/security-plugin/Cargo.toml
2. Create a struct SecurityPlugin
3. Implement the Plugin trait:
   - metadata() → return PluginMetadata with name="security", version, description
   - initialize() → initialize the IDS engine, load rules
   - test_connection() → return Ok(true) for local analysis
   - collect_metrics() → return security posture metrics:
     - security_ids_alerts_total (counter by severity)
     - security_connections_blocked (counter)
     - security_dns_anomalies_detected (counter)
     - security_ssl_cert_expiry_days (gauge per certificate)
     - security_firewall_rules_hit (counter by rule)
   - health_check() → verify IDS engine is loaded
   - polling_interval() → default 1 minute

Follow the pattern from plugins/network-plugin/src/lib.rs.
```

---

## Priority 6 — Deploy Scripts Sync (LOW)

### Prompt 6.1 — Sync deploy/sql with Latest Migrations

```
In the Zentra project, the deploy/sql/ directory contains SQL scripts 001-021 that mirror the migrations/ directory, but migrations 022-024 have no deploy/sql counterpart.

Create the following files by copying from the migrations/ directory:

1. Copy migrations/20250101000022_storage_metrics_history.sql → deploy/sql/022_storage_metrics_history.sql
2. Copy migrations/20250101000023_must_change_password.sql → deploy/sql/023_must_change_password.sql
3. Copy migrations/20250101000024_row_level_security.sql → deploy/sql/024_row_level_security.sql

If migrations 025, 026, 027 are created by the earlier prompts, also copy:
4. Copy migrations/20250101000025_dashboard_features.sql → deploy/sql/025_dashboard_features.sql
5. Copy migrations/20250101000026_netflow_aggregates.sql → deploy/sql/026_netflow_aggregates.sql
6. Copy migrations/20250101000027_rls_role_setup.sql → deploy/sql/027_rls_role_setup.sql

Keep the same SQL content, just use the simplified naming convention (NNN_name.sql) used by the existing deploy/sql scripts.
```

---

## Priority 7 — Documentation Corrections (LOW)

### Prompt 7.1 — Fix All Documentation Inaccuracies

```
Update the Zentra project analysis document at docs/ZENTRA_PROJECT_ANALYSIS_2026-05-13.md with the following corrections:

**Section 2 (Architecture Overview):**
- Change "Zustand" to "React Context + TanStack Query v5" in the frontend tech stack

**Section 3 (Backend — Rust Crate Inventory):**
- Change "18 route modules" to "17 route modules" in the zentra-api row
- Update zentra-api handler count note: "89 handlers (including 14 undocumented: SSO, ITSM, SOC, traces, HA, maintenance, proxies, data_audit, SNMP traps, MIB management, system config, AI database, file analysis, Windows monitoring)"

**Section 5 (Frontend — React Dashboard):**
- In the Tech Stack table, change "Zustand 4.5" to "React Context (3 providers: Auth, AppMode, Theme)"
- Change "State | React Query v5 (30s stale/refetch), Zustand 4.5" to "State | TanStack React Query v5 (30s stale/refetch), React Context API"
- Change "22 Main Views" to "32 Pages" in the pages section heading
- Add the undocumented pages: Storage.tsx, Runbooks.tsx, auth/ForgotPassword.tsx, auth/ResetPassword.tsx

**Section 5 — API Integration:**
- Change "users.ts | CRUD, bulk actions, lock/unlock, roles, groups" to "auth.ts | Login, logout, MFA, sessions, password reset, user CRUD, bulk actions, lock/unlock, roles, groups"
- Remove the separate users.ts entry since user endpoints are in auth.ts

**Section 7 (Network Monitoring):**
- In the Database Schema subsection, change "network_flows" to "netflow_records" everywhere
- Note that netflow_1m and netflow_1h continuous aggregates are pending creation

**Section 19 (Database Schema — Full Inventory):**
- Under Continuous Aggregates, mark netflow_1m and netflow_1h as "Planned" or remove them
- Change the count from "7" to "5 (+ 2 planned)"
- Add undocumented aggregates: log_severity_1h, storage_array_metrics_hourly, storage_array_metrics_daily
- Change "network_flows" to "netflow_records" in the hypertable table
- Under Complete Table List, note that snapshots, playlists, bookmarks, query_history are pending creation

**Section 22 (Implementation Statistics):**
- Change "API route modules | 18" to "API route modules | 17"
- Change "Frontend pages | 22" to "Frontend pages | 32"
- Add note about AI and Security plugins being utility modules not implementing the Plugin trait
```

---

### Prompt 7.2 — Update GAP Analysis Status After Fixes

```
After implementing the fixes from all previous prompts, update docs/ZENTRA_GAP_ANALYSIS_2026-05-13.md:

1. Update the Executive Summary overall score from 85% to the new percentage
2. Mark each resolved gap with ✅ RESOLVED and the date
3. Update the Priority Remediation Plan to show completed items
4. Add a new section "Remediation Log" at the end with:
   - Date of each fix
   - Migration files created
   - Repository modules added
   - Frontend components created
   - Routes added
   - Documentation sections updated
```

---

## Execution Checklist

| # | Prompt | Priority | Type | Dependencies |
|---|--------|----------|------|-------------|
| 1.1 | Migration 025 — Dashboard features | HIGH | SQL | None |
| 1.2 | Migration 026 — NetFlow aggregates | HIGH | SQL | None |
| 1.3 | Snapshots backend | HIGH | Rust | 1.1 |
| 1.4 | Playlists backend | HIGH | Rust | 1.1 |
| 1.5 | Bookmarks & Query History backend | HIGH | Rust | 1.1 |
| 1.6 | Frontend API for all 4 features | HIGH | TypeScript | 1.3, 1.4, 1.5 |
| 2.1 | Middleware dynamic route | MEDIUM | TypeScript | None |
| 2.2 | Playlists UI | MEDIUM | TypeScript | 1.6 |
| 2.3 | Bookmarks sidebar | MEDIUM | TypeScript | 1.6 |
| 3.1 | Dashboards repository | MEDIUM | Rust | None |
| 3.2 | Users/Auth repository | MEDIUM | Rust | None |
| 3.3 | Oracle/Network/Security repos | MEDIUM | Rust | None |
| 4.1 | RLS provisioning | MEDIUM | SQL + Rust | None |
| 5.1 | AI Plugin trait | LOW | Rust | None |
| 5.2 | Security Plugin trait | LOW | Rust | None |
| 6.1 | Deploy scripts sync | LOW | Shell/SQL | 1.1, 1.2, 4.1 |
| 7.1 | Documentation fixes | LOW | Markdown | None |
| 7.2 | GAP analysis status update | LOW | Markdown | All above |

---

*Generated from ZENTRA_GAP_ANALYSIS_2026-05-13.md on 13 May 2026.*
