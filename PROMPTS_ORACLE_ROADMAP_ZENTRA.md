# Zentra — Oracle Monitoring Roadmap Prompts
## Based on `docs/ORACLE_MONITORING_ROADMAP.md` — 12 Phases

**Date:** 2026-04-26
**Source:** `ORACLE_MONITORING_ROADMAP.md` (Sections 1–14)
**Reference Projects:** `oracle-db-appdev-monitoring`, `oracledb_exporter-master`, `Chat2DB-main`

### Rules
- Each prompt must complete **100%** before moving to the next
- After completing each prompt, run `npx tsc --noEmit` (0 errors) + `cargo check` (0 errors)
- Deploy via `deploy.ps1` and verify endpoints with `curl`
- NO mock/fake data in new code — return real structured data (empty arrays/defaults when no data)
- All API endpoints wired in `routes.rs`, registered in `handlers/mod.rs`, exported from `frontend/src/lib/api.ts`
- All routes registered in `App.tsx`, nav items in `Layout.tsx`
- Preserve existing code style — Tailwind + `cn()` utility on Oracle page, inline `style={}` on other DB pages (until P12 converts them)

### Execution Order
```
P6 → P12 → P1 → P2 → P3+P4+P5 (parallel) → P9 → P10 → P7+P11 (parallel) → P8
```

### Existing Files Reference
```
Backend:
  plugins/database-plugin/src/oracle.rs                    (627 lines, simulated subsystems)
  plugins/database-plugin/src/oracle_connection.rs         (245 lines, shell — no real connection)
  plugins/database-plugin/src/oracle_metrics_loader.rs     (193 lines, FUNCTIONAL — parses TOML/YAML)
  plugins/database-plugin/src/oracle_default_metrics.toml  (125 lines, 14 contexts, production SQL)
  plugins/database-plugin/src/oracle_metric_executor.rs    (142 lines, simulated — hardcoded values)
  plugins/database-plugin/src/oracle_cache.rs              (103 lines, FUNCTIONAL — TTL cache)
  plugins/database-plugin/src/oracle_scheduler.rs          (112 lines, demo only)
  plugins/database-plugin/src/oracle_multi_db.rs           (73 lines, shell types only)
  plugins/database-plugin/src/oracle_alertlog.rs           (93 lines, demo only)
  plugins/database-plugin/src/oracle_txeventq.rs           (83 lines, demo only)
  crates/Zentra-api/src/handlers/oracle.rs             (1269 lines, 14 endpoints — SSH+sqlplus live, demo fallback)

Frontend:
  frontend/src/pages/OracleMonitoring.tsx     (1024 lines, vertical sidebar tabs, 15 tabs, Recharts)
  frontend/src/pages/OracleMetricEditor.tsx   (existing, full CRUD for metric definitions)
  frontend/src/pages/PostgresMonitoring.tsx   (630 lines, horizontal tabs, inline styles)
  frontend/src/pages/MysqlMonitoring.tsx      (249 lines, horizontal tabs, inline styles)
  frontend/src/pages/MongodbMonitoring.tsx    (227 lines, horizontal tabs, inline styles)
  frontend/src/pages/SqlServerMonitoring.tsx  (252 lines, horizontal tabs, inline styles)
  frontend/src/components/Layout.tsx          (nav — nested submenu with 3rd-level children)
  frontend/src/App.tsx                        (routes: /oracle, /postgres, /mysql, /mongodb, /sqlserver, /oracle/metrics)
  frontend/src/lib/api.ts                     (fetchJSON, all API functions)
```

---

## PROMPT 6 (P6): Navigation Restructuring — ALL Databases
**Priority:** 🟠 High | **Effort:** ~400 LOC | **Duration:** ~1 day
**Status:** ✅ ALREADY DONE

Navigation has already been restructured in `Layout.tsx`:
- `NavItem` interface supports recursive `children` (3rd-level)
- All 5 DB types have nested submenu: `[DB Type]` → `Monitoring` + `Metric Editor`
- Sidebar renders 3rd-level items with nested expand/collapse, indentation, smaller font
- `tsc --noEmit` passes clean

**No action needed — skip to P12.**

---

## PROMPT 12 (P12): Unified Vertical Sidebar Layout — All DB Pages
**Priority:** 🟠 High | **Effort:** ~990 LOC | **Duration:** ~2 days
**Dependencies:** None (pure frontend)

### Objective
Convert PostgreSQL, MySQL, MongoDB, and SQL Server monitoring pages from horizontal button tabs
to the Oracle-style vertical left sidebar with icons. Add a separator and SQL Workspace placeholder
tab as the last item on every page.

### Step 12.1 — PostgreSQL (`frontend/src/pages/PostgresMonitoring.tsx`)

**Current:** Horizontal `<button>` tabs in a `flex` row at the top (inline styles).
**Target:** Match Oracle's layout pattern — vertical `<nav>` sidebar on the left, content area on the right.

Replace the horizontal tab bar with a vertical sidebar. Keep all existing tab content rendering
and data loading unchanged.

**Tab definitions (add icons, use Lucide imports):**
```tsx
import {
  Database, Plug, Search, Table2, List, RefreshCw, Lock, TrendingUp, Layers, Terminal,
} from 'lucide-react'

const TABS = [
  { id: 'overview', label: 'Overview', icon: Database },
  { id: 'connections', label: 'Connections', icon: Plug },
  { id: 'queries', label: 'Slow Queries', icon: Search },
  { id: 'tables', label: 'Tables', icon: Table2 },
  { id: 'indexes', label: 'Indexes', icon: List },
  { id: 'replication', label: 'Replication', icon: RefreshCw },
  { id: 'locks', label: 'Locks', icon: Lock },
  { id: 'performance', label: 'Performance', icon: TrendingUp },
  ...(pgpool?.configured ? [{ id: 'pgpool', label: 'PgPool-II', icon: Layers }] : []),
  { id: 'sql-workspace', label: 'SQL Workspace', icon: Terminal },
] as const
```

**Layout structure:**
```tsx
<div className="flex gap-4">
  {/* Left Sidebar Tabs */}
  <nav className="flex-shrink-0 w-44 space-y-1">
    {TABS.map(t => {
      const Icon = t.icon
      const isSeparator = t.id === 'sql-workspace'
      return (
        <React.Fragment key={t.id}>
          {isSeparator && <div className="border-t border-slate-700 my-2" />}
          <button
            onClick={() => setTab(t.id)}
            className={cn(
              'flex items-center gap-2 w-full px-3 py-2 rounded-lg text-xs font-medium transition-colors text-left',
              tab === t.id
                ? 'bg-blue-600/20 text-blue-400 border border-blue-500/30'
                : 'text-slate-400 hover:bg-white/5 hover:text-white border border-transparent'
            )}
          >
            <Icon className="w-3.5 h-3.5 flex-shrink-0" /> {t.label}
          </button>
        </React.Fragment>
      )
    })}
  </nav>

  {/* Right Content Area */}
  <div className="flex-1 min-w-0">
    {/* existing tab content rendering goes here */}
  </div>
</div>
```

**SQL Workspace tab placeholder content:**
```tsx
{tab === 'sql-workspace' && (
  <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-8 text-center">
    <Terminal className="w-12 h-12 text-blue-400 mx-auto mb-4" />
    <h3 className="text-lg font-semibold text-white mb-2">SQL Workspace</h3>
    <p className="text-slate-400 text-sm">
      Execute queries, browse schemas, and explore your PostgreSQL databases.
    </p>
    <p className="text-slate-500 text-xs mt-2">Coming soon — Phase 10</p>
  </div>
)}
```

**Active color:** `bg-blue-600/20 text-blue-400 border-blue-500/30` (PostgreSQL blue)

Convert all existing inline `style={}` to Tailwind classes for the page structure (PageHeader, controls, layout wrapper). Content inside each tab can remain inline-styled for now.

Add `import { cn } from '@/lib/utils'` if not present. Import `React` for `React.Fragment`.

### Step 12.2 — MySQL (`frontend/src/pages/MysqlMonitoring.tsx`)

Same conversion as PostgreSQL. Keep all existing data loading and tab content.

**Tab definitions:**
```tsx
import {
  Database, Plug, Search, HardDrive, RefreshCw, Table2, TrendingUp, Terminal,
} from 'lucide-react'

const TABS = [
  { id: 'overview', label: 'Overview', icon: Database },
  { id: 'connections', label: 'Connections', icon: Plug },
  { id: 'queries', label: 'Slow Queries', icon: Search },
  { id: 'innodb', label: 'InnoDB', icon: HardDrive },
  { id: 'replication', label: 'Replication', icon: RefreshCw },
  { id: 'tables', label: 'Tables', icon: Table2 },
  { id: 'performance', label: 'Performance', icon: TrendingUp },
  { id: 'sql-workspace', label: 'SQL Workspace', icon: Terminal },
] as const
```

**Active color:** `bg-orange-600/20 text-orange-400 border-orange-500/30` (MySQL orange)

Same sidebar layout pattern, separator before SQL Workspace, same placeholder content.

### Step 12.3 — MongoDB (`frontend/src/pages/MongodbMonitoring.tsx`)

Same conversion. MongoDB uses "Query Workspace" instead of "SQL Workspace".

**Tab definitions:**
```tsx
import {
  Database, Folder, RefreshCw, Activity, List, TrendingUp, Terminal,
} from 'lucide-react'

const TABS = [
  { id: 'overview', label: 'Overview', icon: Database },
  { id: 'collections', label: 'Collections', icon: Folder },
  { id: 'replicaset', label: 'Replica Set', icon: RefreshCw },
  { id: 'operations', label: 'Operations', icon: Activity },
  { id: 'indexes', label: 'Indexes', icon: List },
  { id: 'performance', label: 'Performance', icon: TrendingUp },
  { id: 'query-workspace', label: 'Query Workspace', icon: Terminal },
] as const
```

**Active color:** `bg-green-600/20 text-green-400 border-green-500/30` (MongoDB green)

Placeholder text: "Execute queries, browse collections, and explore your MongoDB databases."

### Step 12.4 — SQL Server (`frontend/src/pages/SqlServerMonitoring.tsx`)

Same conversion.

**Tab definitions:**
```tsx
import {
  Database, Plug, Search, HardDrive, Clock, Timer, TrendingUp, Terminal,
} from 'lucide-react'

const TABS = [
  { id: 'overview', label: 'Overview', icon: Database },
  { id: 'connections', label: 'Sessions', icon: Plug },
  { id: 'queries', label: 'Top Queries', icon: Search },
  { id: 'databases', label: 'Databases', icon: HardDrive },
  { id: 'jobs', label: 'Agent Jobs', icon: Clock },
  { id: 'waits', label: 'Wait Stats', icon: Timer },
  { id: 'performance', label: 'Performance', icon: TrendingUp },
  { id: 'sql-workspace', label: 'SQL Workspace', icon: Terminal },
] as const
```

**Active color:** `bg-red-600/20 text-red-400 border-red-500/30` (SQL Server red)

### Step 12.5 — Oracle (`frontend/src/pages/OracleMonitoring.tsx`)

Oracle already has the vertical sidebar. Only change: add SQL Workspace as the last tab.

**Add to the existing `TABS` array (after `txeventq`):**
```tsx
  { id: 'sql-workspace', label: 'SQL Workspace', icon: Terminal },
```

**Add `Terminal` to the Lucide import.**

**Add separator rendering** — in the sidebar nav loop, before the `sql-workspace` button, render:
```tsx
{isSeparator && <div className="border-t border-slate-700 my-2" />}
```
where `isSeparator = t.id === 'sql-workspace'`.

**Add placeholder content** (same as other DBs, with cyan active color matching Oracle's scheme).

**Active color stays:** `bg-cyan-600/20 text-cyan-400 border-cyan-500/30` (Oracle cyan, already in use)

### Testing
```bash
npx tsc --noEmit  # 0 errors
# Visual check: all 5 DB pages show vertical sidebar tabs with icons
# SQL Workspace tab appears last with separator line on all 5 pages
# Tab selection works, content renders correctly
```

---

## PROMPT 1 (P1): Native Oracle Connection & Real Metrics
**Priority:** 🔴 Critical | **Effort:** ~3,000 LOC | **Duration:** ~1 week
**Dependencies:** None

### Objective
Replace SSH+sqlplus with native Oracle connection pooling using the `oracle` Rust crate.
Replace all simulated/demo metric execution with real SQL queries. Enable multi-database
registration and health checks.

### Step 1.1 — Add `oracle` Rust Crate

**File:** `plugins/database-plugin/Cargo.toml`

Add dependency:
```toml
[dependencies]
oracle = "0.6"
```

### Step 1.2 — Implement `OracleConnectionPool`

**File:** `plugins/database-plugin/src/oracle_connection.rs` (replace existing shell)

Replace the current shell code with a real connection pool implementation:

```rust
use oracle::{Connection, Connector, pool::Pool};
use std::collections::HashMap;
use parking_lot::RwLock;
use lazy_static::lazy_static;

pub struct OraclePool {
    pub config: OracleConnConfig,  // existing struct
    pool: Pool,
    pub status: ConnectionStatus,
}

pub enum ConnectionStatus {
    Connected { since: String, pool_active: u32, pool_idle: u32 },
    Disconnected { since: String, last_error: String },
    Reconnecting,
}
```

**Required methods:**
1. `new(config: OracleConnConfig) -> Result<Self>` — create pool with `min_connections`, `max_connections`, `increment`, retry with exponential backoff (1s, 2s, 4s, 8s, max 30s)
2. `get_connection() -> Result<Connection>` — borrow from pool
3. `ping() -> Result<()>` — validate pool health
4. `close()` — drain pool
5. `warm_up()` — pre-create min connections
6. `stats() -> PoolStats` — active, idle, total, wait count

**Pool registry (global):**
```rust
lazy_static! {
    static ref POOLS: RwLock<HashMap<String, OraclePool>> = RwLock::new(HashMap::new());
}

pub fn register_pool(id: &str, config: OracleConnConfig) -> Result<()>;
pub fn get_pool(id: &str) -> Option<...>;
pub fn remove_pool(id: &str);
pub fn list_pools() -> Vec<(String, ConnectionStatus)>;
```

**Auth support:** Password, External, Wallet (from existing `AuthMode` enum). Map `OracleRole` to
`oracle::Privilege` (SYSDBA, SYSOPER, SYSASM, etc.).

### Step 1.3 — Wire Real Metric Execution

**File:** `plugins/database-plugin/src/oracle_metric_executor.rs` (replace existing simulated code)

Replace `generate_sample_*` with real execution:
```rust
pub fn execute_metric(pool_id: &str, metric: &OracleMetricDefinition) -> Result<MetricResult> {
    let pool = get_pool(pool_id).ok_or("Pool not found")?;
    let conn = pool.get_connection()?;
    let mut stmt = conn.statement(&metric.request).build()?;
    let rows = stmt.query(&[])?;

    let mut values = Vec::new();
    for row_result in rows {
        let row = row_result?;
        // Parse labels from metric.labels columns
        // Parse metric values from metric.metricsdesc columns
        // Handle field_to_append
        // Handle ignore_zero_result
        values.push(MetricValue { labels, value, field_name });
    }
    Ok(MetricResult { context: metric.context.clone(), values, scraped_at: now(), duration_ms })
}
```

**Handle:**
- Labels: iterate `metric.labels`, extract column values as label key-value pairs
- MetricsDesc: iterate `metric.metricsdesc`, extract column values as gauge/counter floats
- FieldToAppend: if set, append column value to metric FQDN
- IgnoreZeroResult: skip rows where all metric values are 0
- Query timeout: use `metric.query_timeout` or default 30s
- Histogram buckets: if `metric.metrics_buckets` is set, create histogram distribution

### Step 1.4 — Replace SSH+sqlplus in API Handler

**File:** `crates/Zentra-api/src/handlers/oracle.rs`

Replace `oracle_ssh_query()` calls with native pool queries. For each endpoint:
1. Get pool for the selected connection ID (or default)
2. Execute the appropriate SQL via `pool.get_connection()` + `stmt.query()`
3. Parse results into the existing response structs
4. Keep demo fallback if no pool is configured (graceful degradation)

**Endpoints to convert (all 14):**
- `/api/oracle/overview` — `SELECT instance_name, host_name, version... FROM v$instance`
- `/api/oracle/rac` — `SELECT * FROM gv$instance`
- `/api/oracle/sysmetrics` — `SELECT metric_name, value FROM v$sysmetric WHERE group_id=2`
- `/api/oracle/sysstat` — `SELECT name, value FROM v$sysstat WHERE name IN (...)`
- `/api/oracle/wait-classes` — `SELECT wait_class, total_waits, time_waited FROM v$system_wait_class`
- `/api/oracle/tablespaces` — `SELECT tablespace_name, used_percent... FROM dba_tablespace_usage_metrics`
- `/api/oracle/top-queries` — `SELECT * FROM (SELECT sql_id, sql_text... FROM v$sqlstats ORDER BY elapsed_time DESC) WHERE ROWNUM <= 20`
- `/api/oracle/running-queries` — `SELECT s.sid, s.serial#, s.username... FROM v$session s WHERE s.status='ACTIVE'`
- `/api/oracle/cache-hit-ratio` — `SELECT name, value FROM v$sysmetric WHERE metric_name LIKE '%Hit%'`
- `/api/oracle/dataguard` — `SELECT * FROM v$dataguard_stats` + `v$database`
- `/api/oracle/rman` — `SELECT * FROM v$rman_backup_job_details ORDER BY start_time DESC`
- `/api/oracle/pdb` — `SELECT con_id, name, open_mode... FROM v$pdbs`
- `/api/oracle/listeners` — test TCP connect to listener ports
- `/api/oracle/performance-history` — last 60 rows from cache or `v$sysmetric_history`

### Step 1.5 — Multi-Database Registration

**New file:** `deploy/sql/017_oracle_connections.sql`
```sql
CREATE TABLE IF NOT EXISTS oracle_connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER DEFAULT 1521,
    service_name TEXT,
    sid TEXT,
    username TEXT NOT NULL,
    auth_mode TEXT DEFAULT 'password',   -- password, wallet, external, vault
    oracle_role TEXT DEFAULT 'default',  -- default, sysdba, sysoper, sysasm
    wallet_location TEXT,
    vault_path TEXT,
    labels TEXT,                          -- JSON: arbitrary key-value labels
    pool_min INTEGER DEFAULT 2,
    pool_max INTEGER DEFAULT 10,
    pool_increment INTEGER DEFAULT 1,
    enabled BOOLEAN DEFAULT TRUE,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);
```

**API endpoints (add to `handlers/oracle.rs`):**
1. `GET /api/oracle/connections` — list all registered Oracle databases
2. `POST /api/oracle/connections` — register new Oracle database
3. `PUT /api/oracle/connections/:id` — update connection config
4. `DELETE /api/oracle/connections/:id` — remove connection (drain pool)
5. `POST /api/oracle/connections/:id/test` — test connectivity, return version + status

### Step 1.6 — Connection Health Check

Add to `oracle_connection.rs`:
```rust
pub fn health_check(pool_id: &str) -> HealthStatus {
    // 1. Ping pool
    // 2. Execute: SELECT 1 FROM dual
    // 3. Return: up/down + response time + version
}
```

Expose via existing `/api/oracle/overview` endpoint — include per-connection health in response.

### Registration
- `routes.rs`: add 5 new connection management routes
- `api.ts`: add `OracleConnection` interface + 5 API functions
- `Cargo.toml`: add `oracle = "0.6"` dependency

### Testing
```bash
cargo check                    # 0 errors
npx tsc --noEmit              # 0 errors
# Verify: POST /api/oracle/connections with real Oracle credentials
# Verify: GET /api/oracle/overview returns real data from native connection
# Verify: GET /api/oracle/tablespaces returns real tablespace data
```

---

## PROMPT 2 (P2): Scheduled Scrape & Caching
**Priority:** 🔴 Critical | **Effort:** ~2,000 LOC | **Duration:** ~4 days
**Dependencies:** P1 (native connection must exist)

### Objective
Replace demo scheduler with a real background scrape task that iterates all registered Oracle
databases, executes all active metrics at configurable intervals, stores results in cache, and
tracks scrape meta-metrics.

### Step 2.1 — Background Scrape Task

**File:** `plugins/database-plugin/src/oracle_scheduler.rs` (replace demo code)

```rust
use tokio::time::{interval, Duration};

pub struct MetricScheduler {
    pub status: SchedulerStatus,     // Running/Stopped/Error
    pub configs: Vec<SchedulerConfig>,
    pub meta: ScrapeMetaMetrics,
}

impl MetricScheduler {
    pub async fn start(&mut self) {
        // 1. Load all registered Oracle connections
        // 2. Load all metric definitions (default + custom)
        // 3. For each connection + metric combo:
        //    - Check if scrape is due (last_scrape + interval <= now)
        //    - Execute metric via oracle_metric_executor
        //    - Store result in oracle_cache
        //    - Update meta-metrics (duration, success/failure)
        // 4. Sleep until next interval tick
    }

    pub fn stop(&mut self);
    pub fn force_scrape(&mut self, connection_id: Option<&str>);
    pub fn status(&self) -> &SchedulerStatus;
}
```

**Launch on app startup** — register as a tokio background task in the API server initialization.

### Step 2.2 — Per-Metric Scrape Interval

Each `OracleMetricDefinition` has an optional `scrape_interval` field (from TOML/YAML).
- If set, honor it (e.g., 60s for light metrics, 300s for heavy queries)
- If not set, use global default (30s)
- Track `last_scraped_at` per connection+metric key in cache

### Step 2.3 — Per-Metric Query Timeout

Each metric can define `query_timeout` in its TOML/YAML definition.
- Pass to `stmt.set_query_timeout(Duration::from_secs(timeout))` on the oracle statement
- Default: 30 seconds
- On timeout: record error in meta-metrics, serve stale cache value

### Step 2.4 — Scrape Meta-Metrics

Track per connection per metric:
```rust
pub struct ScrapeMetaMetrics {
    pub total_scrapes: u64,
    pub successful_scrapes: u64,
    pub failed_scrapes: u64,
    pub total_duration_ms: u64,
    pub avg_duration_ms: f64,
    pub last_scrape_at: Option<String>,
    pub last_error: Option<String>,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub per_connection: HashMap<String, ConnectionScrapeStats>,
}
```

### Step 2.5 — Scheduler Control API

**Add to `handlers/oracle.rs`:**
1. `GET /api/oracle/scheduler/status` — current status, meta-metrics, per-connection stats
2. `POST /api/oracle/scheduler/start` — start background scraping
3. `POST /api/oracle/scheduler/stop` — stop background scraping
4. `POST /api/oracle/scheduler/force-scrape` — immediate full scrape (optional: `?connection_id=X`)

### Registration
- `routes.rs`: add 4 scheduler routes
- `api.ts`: add scheduler API functions

### Testing
```bash
cargo check
# Verify: POST /api/oracle/scheduler/start → scheduler begins
# Verify: GET /api/oracle/scheduler/status → shows scrape counts, durations
# Verify: GET /api/oracle/overview → data comes from cache (fast response)
```

---

## PROMPT 3 (P3): Alert Log & TxEventQ — Real Data
**Priority:** 🟠 High | **Effort:** ~1,500 LOC | **Duration:** ~3 days
**Dependencies:** P1 (native connection)

### Objective
Replace demo alert log and TxEventQ data with real Oracle queries.

### Step 3.1 — Real Alert Log Collection

**File:** `plugins/database-plugin/src/oracle_alertlog.rs` (replace demo code)

```rust
pub async fn collect_alert_log(pool_id: &str, since: Option<&str>) -> Result<Vec<AlertLogEntry>> {
    // SQL: SELECT originating_timestamp, message_text, message_type, message_level,
    //            component_id, host_id, module_id
    //      FROM v$diag_alert_ext
    //      WHERE originating_timestamp > :since
    //      ORDER BY originating_timestamp DESC
    //      FETCH FIRST 500 ROWS ONLY
}
```

**Features:**
- Incremental collection: track `last_timestamp` per connection, only fetch new entries
- JSON export: write entries to `data/alertlog/{connection_id}.json` (append mode)
- Retry with exponential backoff on ORA errors
- Parse severity from `message_level` (CRITICAL=1, WARNING=2, INFO=8, etc.)

### Step 3.2 — Alert Log → Log Engine Integration

Feed alert log entries into Zentra's existing log ingestion pipeline:
- Convert `AlertLogEntry` → log engine format with labels: `{source="oracle", connection_id="X", severity="critical"}`
- Enables LogQL querying of Oracle alert logs

### Step 3.3 — Real TxEventQ Metrics

**File:** `plugins/database-plugin/src/oracle_txeventq.rs` (replace demo code)

```sql
-- Queue list
SELECT queue_name, queue_type, enqueue_enabled, dequeue_enabled
FROM dba_queues WHERE queue_type LIKE '%SHARDED%'

-- Per-queue stats
SELECT queue_name, num_msgs, enqueue_msgs, dequeue_msgs, spill_msgs,
       avg_msg_age, max_msg_age
FROM gv$persistent_queues

-- Per-subscriber stats  
SELECT queue_name, subscriber_name, enqueued_msgs, dequeued_msgs,
       subscriber_lag_secs
FROM gv$aq_sharded_subscriber_stat
```

### Step 3.4 — TxEventQ Alerting

Add threshold-based alert definitions to `oracle_default_metrics.toml`:
- Queue depth > 10,000 → warning
- Consumer lag > 60s → warning, > 300s → critical
- Message age > 600s → critical

### Testing
```bash
cargo check
# Verify: GET /api/oracle/alertlog → real alert log entries from v$diag_alert_ext
# Verify: GET /api/oracle/txeventq → real queue stats
```

---

## PROMPT 4 (P4): Custom Metrics & Hot Reload
**Priority:** 🟠 High | **Effort:** ~1,000 LOC | **Duration:** ~2 days
**Dependencies:** P1 (native connection for testing)

### Objective
Enable hot-reloading of TOML/YAML metric definition files, implement histogram bucket support,
field-to-append, database-scoped metrics, and per-database labels.

### Step 4.1 — Hot Reload

**File:** `plugins/database-plugin/src/oracle_metrics_loader.rs` (extend)

Add SHA256 file hash tracking:
```rust
pub struct MetricFileWatcher {
    pub file_path: String,
    pub last_hash: String,
    pub last_loaded: String,
}

impl MetricFileWatcher {
    pub fn has_changed(&self) -> bool;       // compute SHA256, compare
    pub fn reload(&mut self) -> Result<Vec<OracleMetricDefinition>>;
}
```

Check on every scheduler tick. If hash changed → reload definitions, reinitialize caches for affected metrics.

### Step 4.2 — Histogram Support

In `oracle_metric_executor.rs`, when `metric.metrics_buckets` is defined:
- Execute SQL, get numeric values
- Distribute into configured bucket boundaries
- Store as histogram type in cache (bucket counts + sum + count)

### Step 4.3 — Field-to-Append

When `metric.field_to_append` is set:
- Read the specified column value from each row
- Append to the metric FQDN: `oracledb_{context}_{field_value}_{metric_name}`

### Step 4.4 — Database-Scoped Metrics

When `metric.databases` is set (list of connection IDs):
- Only execute this metric against the listed connections
- Skip all other connections during scheduled scrape

### Step 4.5 — Per-Database Labels

From `oracle_connections` table `labels` column (JSON):
- Inject configured labels into all metrics from that connection
- E.g., `{env: "production", region: "us-east-1"}` → added to every metric from that connection

### Testing
```bash
cargo check
# Modify oracle_default_metrics.toml while scheduler is running → metrics auto-reload
# Verify via GET /api/oracle/scheduler/status → shows reload count
```

---

## PROMPT 5 (P5): Vault Integration
**Priority:** 🟠 High | **Effort:** ~800 LOC | **Duration:** ~2 days
**Dependencies:** P1 (connection pool must support vault auth)

### Objective
Implement credential fetching from OCI Vault, Azure Key Vault, and HashiCorp Vault.

### Step 5.1 — OCI Vault

**New file:** `plugins/database-plugin/src/oracle_vault.rs`

```rust
pub async fn get_oci_vault_secret(vault_id: &str, secret_name: &str) -> Result<String> {
    // HTTP call to OCI Vault REST API
    // Auth: instance principal or config-based
}
```

### Step 5.2 — Azure Key Vault

```rust
pub async fn get_azure_vault_secret(vault_url: &str, secret_name: &str) -> Result<String> {
    // HTTP call to Azure Key Vault REST API
    // Auth: managed identity or client credentials
}
```

### Step 5.3 — HashiCorp Vault

```rust
pub async fn get_hashicorp_vault_secret(config: &HashiCorpVaultConfig) -> Result<String> {
    // Support: KVv1, KVv2, Database mount type
    // Auth: token, approle, or proxy mode
}
```

### Step 5.4 — Password File

```rust
pub fn read_password_file(path: &str) -> Result<String> {
    // Read first line of file, trim whitespace
    // Container-friendly: mount secret as file
}
```

### Step 5.5 — Credential Rotation

In `oracle_connection.rs`, on auth failure (ORA-01017):
1. Re-fetch credentials from configured vault
2. Update pool config
3. Retry connection

### Testing
```bash
cargo check
# Verify: register connection with vault_path set → credentials fetched from vault
```

---

## PROMPT 9 (P9): Universal Metric Editor — All Databases
**Priority:** 🟠 High | **Effort:** ~2,000 LOC | **Duration:** ~3 days
**Dependencies:** P4 (reuses oracle metric loader pattern)

### Objective
Create default metric definition files (TOML) for PostgreSQL, MySQL, MongoDB, and SQL Server.
Build a shared Metric Editor frontend component that works for all DB types.

### Step 9.1 — PostgreSQL Default Metrics

**New file:** `plugins/database-plugin/src/postgres_default_metrics.toml`

10 metric contexts with real SQL:
```toml
[[metric]]
context = "pg_connections"
labels = ["state"]
metricsdesc = { count = "Number of connections by state" }
request = "SELECT state, count(*) as count FROM pg_stat_activity GROUP BY state"

[[metric]]
context = "pg_database_size"
labels = ["datname"]
metricsdesc = { size_bytes = "Database size in bytes" }
request = "SELECT datname, pg_database_size(datname) as size_bytes FROM pg_database WHERE datistemplate = false"

# ... 8 more contexts: replication_lag, table_bloat, index_usage, lock_waits,
#     cache_hit_ratio, wal_stats, vacuum_stats, slow_queries
```

### Step 9.2 — MySQL Default Metrics

**New file:** `plugins/database-plugin/src/mysql_default_metrics.toml`

9 metric contexts with real SQL from `SHOW STATUS`, `information_schema`, `performance_schema`.

### Step 9.3 — MongoDB Default Metrics

**New file:** `plugins/database-plugin/src/mongodb_default_metrics.toml`

8 metric contexts using MongoDB commands: `serverStatus`, `rs.status()`, `dbStats`, etc.

### Step 9.4 — SQL Server Default Metrics

**New file:** `plugins/database-plugin/src/sqlserver_default_metrics.toml`

10 metric contexts with real SQL from `sys.dm_*` DMVs.

### Step 9.5 — Shared Metric Editor Component

**New file:** `frontend/src/pages/MetricEditor.tsx`

A single page component that accepts `dbType` as a URL parameter:
```tsx
export default function MetricEditorPage() {
  const { dbType } = useParams()  // 'oracle' | 'postgres' | 'mysql' | 'mongodb' | 'sqlserver'
  // Load metrics for this DB type
  // CRUD: add/edit/delete/test metric definitions
  // Same UI as existing OracleMetricEditor but generic
}
```

### Step 9.6 — Routes & Navigation

**`App.tsx`:** Add routes:
```tsx
<Route path="postgres/metrics" element={<MetricEditorPage />} />
<Route path="mysql/metrics" element={<MetricEditorPage />} />
<Route path="mongodb/metrics" element={<MetricEditorPage />} />
<Route path="sqlserver/metrics" element={<MetricEditorPage />} />
```

**`Layout.tsx`:** Already has Metric Editor in each DB submenu (done in P6).

**Backend:** Add generic metric editor endpoints:
1. `GET /api/:db_type/metrics/definitions` — list metric definitions for DB type
2. `POST /api/:db_type/metrics/definitions` — add custom metric
3. `PUT /api/:db_type/metrics/definitions/:id` — update metric
4. `DELETE /api/:db_type/metrics/definitions/:id` — delete metric
5. `POST /api/:db_type/metrics/test` — test-execute a metric definition

### Testing
```bash
cargo check
npx tsc --noEmit
# Verify: GET /api/postgres/metrics/definitions → returns 10 default PostgreSQL metrics
# Verify: /postgres/metrics route renders the shared metric editor
```

---

## PROMPT 10 (P10): SQL Workspace — In-Page Tab
**Priority:** 🟠 High | **Effort:** ~2,500 LOC | **Duration:** ~4 days
**Dependencies:** P12 (vertical sidebar must exist), P1 partially (backend execute)

### Objective
Build a shared `SqlWorkspaceTab` component and wire it into the SQL Workspace tab on all 5 DB
monitoring pages. Replace the placeholder from P12 with a functional Monaco editor, schema browser,
results table, query history, and export.

### Step 10.1 — Shared SqlWorkspaceTab Component

**New file:** `frontend/src/components/SqlWorkspaceTab.tsx`

Props:
```tsx
interface SqlWorkspaceTabProps {
  dbType: 'oracle' | 'postgres' | 'mysql' | 'mongodb' | 'sqlserver'
  connectionId?: string   // optional: preselect connection
}
```

**Layout (3-panel):**
```
┌─────────────┬─────────────────────────────────────┐
│ Schema      │ Monaco Editor                  [▶ Run]│
│ Browser     │                                [Ctrl+Enter]
│ (tree view) │ ┌─────────────────────────────────┐ │
│             │ │ SELECT ...                       │ │
│ ► schema    │ │                                  │ │
│   ► table1  │ └─────────────────────────────────┘ │
│   ► table2  │──────────────────────────────────────│
│             │ Results (N rows, Xs)                  │
│ [AI Ask]    │ ┌─────┬─────┬─────┐                │
│             │ │ col1│ col2│ col3│                │
│ [History ▾] │ ├─────┼─────┼─────┤                │
│             │ │ ... │ ... │ ... │                │
│             │ └─────┴─────┴─────┘                │
│             │ [Export CSV] [Export JSON] [Pg 1/N]  │
└─────────────┴─────────────────────────────────────┘
```

**Features:**
1. **Monaco Editor** — add `@monaco-editor/react` dependency. Set language based on `dbType`:
   - oracle → `sql` (PL/SQL keywords)
   - postgres → `pgsql`
   - mysql → `mysql`
   - mongodb → `javascript` (MongoDB shell)
   - sqlserver → `sql` (T-SQL keywords)
2. **Schema Browser** — tree view: databases → schemas → tables/views/functions. Click table → insert `SELECT * FROM table LIMIT 10` into editor
3. **Execute** — Ctrl+Enter or ▶ button. POST to backend, display results in paginated table
4. **Results Table** — sortable columns, pagination (50 rows per page), row count + execution time
5. **AI Ask** — text input: type natural language, POST to AI endpoint, get SQL back, insert into editor
6. **Query History** — dropdown of last 50 queries with timestamps, click to re-insert
7. **Export** — CSV and JSON download buttons

### Step 10.2 — Backend: Execute SQL

**New file:** `crates/Zentra-api/src/handlers/database_query.rs`

```rust
pub async fn execute_query(body: Json<ExecuteQueryRequest>) -> impl IntoResponse {
    // body: { db_type, connection_id, sql, limit, offset }
    // 1. Get connection pool for db_type + connection_id
    // 2. Execute SQL with timeout (30s default)
    // 3. Parse result columns and rows
    // 4. Return: { columns: [...], rows: [[...]], row_count, execution_time_ms }
    // Safety: read-only by default (reject DDL unless explicitly allowed)
}
```

**Endpoints:**
1. `POST /api/db/execute` — execute SQL query
2. `POST /api/db/explain` — execute EXPLAIN/EXPLAIN PLAN

### Step 10.3 — Backend: Schema Metadata

**New file:** `crates/Zentra-api/src/handlers/database_metadata.rs`

```rust
pub async fn get_schemas(query: Query<DbTypeQuery>) -> impl IntoResponse;
pub async fn get_tables(query: Query<SchemaQuery>) -> impl IntoResponse;
pub async fn get_columns(query: Query<TableQuery>) -> impl IntoResponse;
pub async fn get_indexes(query: Query<TableQuery>) -> impl IntoResponse;
```

**Endpoints:**
1. `GET /api/db/:db_type/schemas?connection_id=X` — list schemas
2. `GET /api/db/:db_type/tables?connection_id=X&schema=Y` — list tables
3. `GET /api/db/:db_type/columns?connection_id=X&schema=Y&table=Z` — list columns
4. `GET /api/db/:db_type/indexes?connection_id=X&schema=Y&table=Z` — list indexes

Each endpoint executes DB-specific SQL:
- Oracle: `ALL_TABLES`, `ALL_TAB_COLUMNS`, `ALL_INDEXES`
- PostgreSQL: `information_schema.tables`, `information_schema.columns`
- MySQL: `information_schema.tables`, `information_schema.columns`
- SQL Server: `sys.tables`, `sys.columns`, `sys.indexes`
- MongoDB: `db.getCollectionNames()`, `Object.keys(doc)` for fields

### Step 10.4 — Query History

**New file (SQL migration):** `deploy/sql/018_query_history.sql`
```sql
CREATE TABLE IF NOT EXISTS query_history (
    id TEXT PRIMARY KEY,
    db_type TEXT NOT NULL,
    connection_id TEXT,
    sql_text TEXT NOT NULL,
    row_count INTEGER,
    execution_time_ms INTEGER,
    status TEXT,          -- 'success', 'error'
    error_message TEXT,
    executed_by TEXT,
    executed_at TEXT DEFAULT (datetime('now'))
);
```

**Endpoints:**
1. `GET /api/db/history?db_type=X&limit=50` — list recent queries
2. `DELETE /api/db/history/:id` — delete a history entry

### Step 10.5 — Wire SQL Workspace Tab into All 5 DB Pages

Replace the P12 placeholder `{tab === 'sql-workspace' && (...)}` content with:
```tsx
{tab === 'sql-workspace' && (
  <SqlWorkspaceTab dbType="postgres" />
)}
```

Do this in all 5 files: `OracleMonitoring.tsx`, `PostgresMonitoring.tsx`, `MysqlMonitoring.tsx`,
`MongodbMonitoring.tsx`, `SqlServerMonitoring.tsx`.

### Dependencies
```
npm install @monaco-editor/react
```

### Registration
- `handlers/mod.rs`: add `pub mod database_query;` and `pub mod database_metadata;`
- `routes.rs`: add all query/metadata/history routes
- `api.ts`: add types + API functions for execute, schema, history

### Testing
```bash
npx tsc --noEmit
cargo check
# Verify: navigate to /oracle → click SQL Workspace tab → Monaco editor loads
# Verify: type SELECT 1 FROM dual → click Run → results appear
# Verify: schema browser shows tables for connected Oracle DB
# Verify: same works on /postgres, /mysql, /mongodb, /sqlserver tabs
```

---

## PROMPT 7 (P7): Oracle Dashboard Polish
**Priority:** 🟡 Medium | **Effort:** ~2,000 LOC | **Duration:** ~3 days
**Dependencies:** P1, P2 (real data must flow)

### Objective
Enhance the Oracle monitoring dashboard with database selector, additional Grafana-reference panels,
scrape status dashboard, and connection manager.

### Step 7.1 — Database Selector

Add a dropdown at the top of `OracleMonitoring.tsx`:
```tsx
<select value={selectedConnectionId} onChange={...}>
  {connections.map(c => <option key={c.id} value={c.id}>{c.name} ({c.host})</option>)}
</select>
```

All API calls pass `?connection_id=X` to fetch data for the selected database.

### Step 7.2 — Additional Panels

Add to the Overview tab (matching Grafana reference dashboard):
- **ASM Diskgroup** — bar chart of disk group usage (if ASM enabled)
- **Resource Limits** — table of `v$resource_limit` (processes, sessions, enqueue_resources)
- **DB Platform** — stat card showing `v$database.platform_name`
- **DB Block Size** — from `v$parameter` where `name='db_block_size'`

### Step 7.3 — Scrape Status Section

Add a "Scrape Status" tab to the Oracle sidebar:
```tsx
{ id: 'scrape-status', label: 'Scrape Status', icon: BarChart3 },
```

Content: table showing per-metric scrape health:
- Metric context, last scrape time, duration, status (success/error), cache age
- Connection-level health indicators (green/yellow/red)

### Step 7.4 — Connection Manager Tab

The existing `connections` tab already exists. Enhance it:
- Show registered connections with status indicators
- Add/Edit/Delete connection forms (modal)
- Test connection button with result feedback

### Testing
```bash
npx tsc --noEmit
# Verify: database selector switches data source
# Verify: scrape status tab shows real scrape stats
```

---

## PROMPT 11 (P11): AI Chatbot Database Tools
**Priority:** 🟡 Medium | **Effort:** ~2,000 LOC | **Duration:** ~3 days
**Dependencies:** P10 (needs execute_sql backend)

### Objective
Enhance the Zentra chatbot with database-specific AI tools inspired by Chat2DB.

### Step 11.1 — NL-to-SQL Tool

**File:** `frontend/src/pages/Chatbot.tsx` (add tool)

Add to chatbot tool list:
```tsx
{
  id: 'natural_language_to_sql',
  name: 'Natural Language to SQL',
  description: 'Convert plain English to SQL for any database type',
  parameters: { query: 'string', db_type: 'string', connection_id: 'string' },
}
```

**Backend:** `POST /api/ai/nl-to-sql` — sends user query + schema context to LLM, returns SQL.

### Step 11.2 — SQL Explain Tool

Tool: `explain_sql` — user pastes SQL, AI returns plain English explanation.
Backend: `POST /api/ai/explain-sql`

### Step 11.3 — SQL Optimize Tool

Tool: `optimize_sql` — user pastes slow SQL, AI suggests indexes/rewrites/partitions.
Backend: `POST /api/ai/optimize-sql`

### Step 11.4 — SQL Convert Tool

Tool: `convert_sql` — convert SQL between dialects.
Backend: `POST /api/ai/convert-sql` with `{ sql, source_dialect, target_dialect }`

### Step 11.5 — Execute SQL Tool

Tool: `execute_sql` — chatbot can run queries against any registered DB.
Uses the `POST /api/db/execute` endpoint from P10.

### Step 11.6 — Performance Analyze Tool

Tool: `analyze_performance` — AI reads current metrics for a DB, provides health summary and recommendations.
Backend: `POST /api/ai/analyze-performance` with `{ db_type, connection_id }`

### Registration
- `routes.rs`: add 5 AI tool routes
- `handlers/mod.rs`: add `pub mod ai_tools;` (or extend existing `ai.rs`)
- `api.ts`: add AI tool API functions
- `Chatbot.tsx`: add 6 new tools to tool list

### Testing
```bash
npx tsc --noEmit
# Verify: in chatbot, select "Natural Language to SQL" tool
# Verify: type "show me the top 5 tables by size in Oracle" → SQL generated
```

---

## PROMPT 8 (P8): Kubernetes & Deployment
**Priority:** 🔵 Low | **Effort:** ~500 LOC | **Duration:** ~1 day
**Dependencies:** P1, P2

### Objective
Create Kubernetes deployment manifests for Zentra with Oracle metric collection.

### Step 8.1 — K8s Deployment YAML

**New file:** `deploy/k8s/oracle-collector.yaml`
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: Zentra-oracle-collector
spec:
  replicas: 1
  template:
    spec:
      containers:
        - name: Zentra
          image: Zentra:latest
          env:
            - name: ORACLE_DSN
              valueFrom:
                secretKeyRef:
                  name: oracle-credentials
                  key: dsn
          ports:
            - containerPort: 7001
          volumeMounts:
            - name: metrics-config
              mountPath: /etc/Zentra/metrics
      volumes:
        - name: metrics-config
          configMap:
            name: Zentra-metrics
```

### Step 8.2 — ServiceMonitor

**New file:** `deploy/k8s/servicemonitor.yaml`
```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: Zentra
spec:
  endpoints:
    - port: api
      path: /api/v1/metrics
      interval: 30s
  selector:
    matchLabels:
      app: Zentra
```

### Step 8.3 — ConfigMap

**New file:** `deploy/k8s/configmap.yaml`

Mount `oracle_default_metrics.toml` + custom metrics as ConfigMap.

### Testing
```bash
# Verify: kubectl apply -f deploy/k8s/ → resources created
# Verify: pod starts and connects to Oracle DB
```

---

## Summary

| Prompt | Phase | LOC | Duration | Priority | Status |
|--------|-------|-----|----------|----------|--------|
| P6 | Navigation | ~400 | 1 day | 🟠 High | ✅ Done |
| P12 | Vertical Sidebar ALL DBs | ~990 | 2 days | 🟠 High | Pending |
| P1 | Native Oracle Connection | ~3,000 | 1 week | 🔴 Critical | Pending |
| P2 | Scheduled Scrape | ~2,000 | 4 days | 🔴 Critical | Pending |
| P3 | Alert Log + TxEventQ | ~1,500 | 3 days | 🟠 High | Pending |
| P4 | Custom Metrics & Hot Reload | ~1,000 | 2 days | 🟠 High | Pending |
| P5 | Vault Integration | ~800 | 2 days | 🟠 High | Pending |
| P9 | Universal Metric Editor | ~2,000 | 3 days | 🟠 High | Pending |
| P10 | SQL Workspace (in-page tab) | ~2,500 | 4 days | 🟠 High | Pending |
| P7 | Oracle Dashboard Polish | ~2,000 | 3 days | 🟡 Medium | Pending |
| P11 | AI Chatbot DB Tools | ~2,000 | 3 days | 🟡 Medium | Pending |
| P8 | Kubernetes Deployment | ~500 | 1 day | 🔵 Low | Pending |
| **TOTAL** | | **~18,190** | **~5.5 weeks** | | |
