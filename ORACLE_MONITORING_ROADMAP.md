# Oracle Monitoring Roadmap — InfraScope

## Vision: Datadog / Oracle OEM-Class Oracle Monitoring

> Transform InfraScope into a full-featured Oracle Database monitoring platform
> rivaling Datadog, Oracle Enterprise Manager, and the Oracle AI Database Metrics
> Exporter project — with native metrics collection, rich dashboards, custom
> metric editing, alert log streaming, multi-database support, and a Prometheus-
> compatible scrape engine.

---

## 1. Reference Project Summary — `oracle-db-appdev-monitoring`

| Aspect | Details |
|--------|---------|
| **Language** | Go + godror (Oracle FFI) |
| **Architecture** | Prometheus exporter — HTTP `/metrics` endpoint |
| **Multi-DB** | Named database configs in YAML, single exporter instance |
| **Metrics** | 14 default contexts, unlimited custom (TOML/YAML) |
| **Metric Types** | Gauge, Counter, Histogram (with configurable buckets) |
| **Scrape Modes** | On-demand (Prometheus pull) + scheduled background |
| **Caching** | Per-metric per-database cache with custom TTL per scrape interval |
| **Connection** | godror pool, Oracle pool, go-sql pool; warm-up, reconnect, backoff |
| **Auth** | Password, External, Wallet, OCI Vault, Azure Vault, HashiCorp Vault |
| **Roles** | SYSDBA, SYSOPER, SYSASM, SYSBACKUP, SYSDG, SYSKM, SYSRAC |
| **Alert Log** | `v$diag_alert_ext` → JSON file, per-DB files, incremental, retry backoff |
| **TxEventQ** | Queue discovery, enq/deq counts, latency, per-subscriber stats |
| **Dashboards** | 2 Grafana JSONs: Oracle (16 panels), TxEventQ (14 panels) |
| **Kubernetes** | Deployment, Service, ServiceMonitor, NetworkPolicy, ConfigMap |

### Reference Default Metric Contexts (14)
1. `sessions` — by status/type (gv$session)
2. `resource` — utilization vs limits (gv$resource_limit)
3. `asm_diskgroup` — ASM disk totals/free (gv$asm_diskgroup_stat)
4. `activity` — parse/execute/commit/rollback (gv$sysstat)
5. `process` — current process count (gv$process)
6. `wait_time` — per-class wait seconds (gv$system_wait_class)
7. `tablespace` — bytes/max/free/percent (dba_tablespace_usage_metrics)
8. `db_system` — cpu_count, sga_max_size, pga_aggregate_limit (gv$parameter)
9. `db_platform` — platform name (gv$database)
10. `top_sql` — top 15 by elapsed time (gv$sqlstats)
11. `cache_hit_ratio` — buffer/library/dict (gv$sysmetric)
12. `slow_queries` — P95/P99 elapsed (custom)
13. `big_queries` — top by disk reads / buffer gets (custom)
14. `size_user_segments` / `size_dba_segments` — segment sizes (custom)

### Reference Grafana Dashboard Panels (Oracle)
- Database Status (up/down)
- SGA Max Size, PGA Aggregate Limit, CPU Count
- Active Sessions, User Commits, Execute Count
- Database Platform, Database Type
- System Wait Classes, Database Wait Times
- Cache Statistics, Cache Hit Ratios
- Top SQL, Top SQL by Elapsed Time

### Reference TxEventQ Dashboard Panels
- TxEventQ Status, Total Queues, Total Subscribers
- Overall Enqueue/Dequeue Rate
- Per-Queue Stat (enq/deq messages, depth, latency)

---

## 2. InfraScope Current Oracle Implementation — Audit

### 2.1 Backend — Plugin Layer (`plugins/database-plugin/src/`)

| File | Status | Description |
|------|--------|-------------|
| `oracle.rs` (627 lines) | ✅ Exists, **SIMULATED** | 8 subsystems: RAC, DataGuard, RMAN, Tablespace, WaitEvents, LongQueries, Listeners, PDB/CDB. SQL constants defined but **all collect() returns hardcoded values**. |
| `oracle_connection.rs` (245 lines) | ✅ Exists, **SHELL** | `OracleConnConfig`, `AuthMode` (Password/Wallet/External/Vault), `OracleRole` enum, `PoolStats`. **No actual connection code** — comments say "abstraction; real FFI requires oracle crate linked". |
| `oracle_metrics_loader.rs` (193 lines) | ✅ Exists, **FUNCTIONAL** | Parses TOML/YAML metric definition files into `OracleMetricDefinition`. TOML parser, YAML parser, SQL validation, default+custom merge. |
| `oracle_default_metrics.toml` (125 lines) | ✅ Exists | 14 metric contexts with real SQL queries. **Well-structured, production-ready SQL.** |
| `oracle_metric_executor.rs` (142 lines) | ✅ Exists, **SIMULATED** | `execute_metric()` generates sample data instead of running SQL. |
| `oracle_cache.rs` (103 lines) | ✅ Exists, **FUNCTIONAL** | In-memory cache with TTL, get/set/invalidate/stats. Uses `lazy_static` + `parking_lot::RwLock`. |
| `oracle_scheduler.rs` (112 lines) | ✅ Exists, **DEMO ONLY** | `SchedulerConfig`, `SchedulerStatus`, `ScrapeMetaMetrics` structs. `demo_status()` / `demo_meta_metrics()` return fake data. |
| `oracle_multi_db.rs` (73 lines) | ✅ Exists, **SHELL** | `MultiDbTarget`, `ScrapeResult`, `ScrapedMetric`, `assess_health()`, `detect_duplicates()`. Types defined, no real scrape logic. |
| `oracle_alertlog.rs` (93 lines) | ✅ Exists, **DEMO ONLY** | `AlertLogEntry`, `AlertLogStats`. `get_demo_entries()` returns fake ORA- error messages. |
| `oracle_txeventq.rs` (83 lines) | ✅ Exists, **DEMO ONLY** | `TxEventQueue`, `TxEventQStats`. `demo_queues()` / `demo_stats()` return fake data. |

### 2.2 Backend — API Layer (`crates/infrascope-api/src/handlers/oracle.rs`)

| Endpoint | Live Mode | Demo Mode |
|----------|-----------|-----------|
| `GET /api/oracle/overview` | ✅ SSH+sqlplus (version, sessions, uptime, SGA, PGA, db_size) | ✅ Rich demo |
| `GET /api/oracle/rac` | ✅ SSH (gv$instance) | ✅ Demo |
| `GET /api/oracle/sysmetrics` | ✅ SSH (v$sysmetric) | ✅ Demo |
| `GET /api/oracle/sysstat` | ✅ SSH (v$sysstat) | ✅ Demo |
| `GET /api/oracle/waitclasses` | ✅ SSH (v$waitclassmetric) | ✅ Demo |
| `GET /api/oracle/tablespaces` | ✅ SSH (dba_tablespace_usage_metrics) | ✅ Demo |
| `GET /api/oracle/sessions` | ✅ SSH (v$session) | ✅ Demo |
| `GET /api/oracle/dataguard` | ✅ SSH (v$dataguard_stats) | ✅ Demo |
| `GET /api/oracle/rman` | ✅ SSH (v$rman_backup_job_details) | ✅ Demo |
| `GET /api/oracle/longqueries` | ✅ SSH (v$sql) | ✅ Demo |
| `GET /api/oracle/pdb` | ✅ SSH (v$pdbs) | ✅ Demo |
| `GET /api/oracle/connections` | Demo only | ✅ Demo |
| `GET /api/oracle/alertlog` | Demo only | ✅ Demo |
| `GET /api/oracle/txeventq` | Demo only | ✅ Demo |
| Metric Editor endpoints | Demo only | ✅ Demo |

### 2.3 Frontend

| Page | Route | Status |
|------|-------|--------|
| `OracleMonitoring.tsx` | `/oracle` | ✅ Rich UI — 6+ tabs (Overview, Sessions, Tablespace, Wait Events, RAC, DataGuard, RMAN, PDB/CDB, Connections, Alert Log, TxEventQ) |
| `OracleMetricEditor.tsx` | `/oracle/metrics` | ✅ Full CRUD for metric definitions (TOML/YAML), test execute, preview results |

### 2.4 Navigation (Current)
```
Databases
  ├─ Dashboard
  ├─ Oracle          → /oracle
  ├─ Metric Editor   → /oracle/metrics
  ├─ PostgreSQL      → /postgres
  ├─ MySQL           → /mysql
  ├─ MongoDB         → /mongodb
  └─ SQL Server      → /sqlserver
```

---

## 3. Gap Analysis — InfraScope vs Reference Project

### 3.1 CRITICAL GAPS (Must Fix)

| # | Gap | Reference | InfraScope Current | Priority |
|---|-----|-----------|-------------------|----------|
| G1 | **Native Oracle Connection** | godror FFI, connection pool, warm-up, reconnect, backoff | SSH+sqlplus (fragile, slow, no pooling) | 🔴 Critical |
| G2 | **Real Metric Execution** | SQL → parse rows → Prometheus metrics | Hardcoded demo values in `execute_metric()` | 🔴 Critical |
| G3 | **Scheduled Background Scrape** | `RunScheduledScrapes()` with per-metric intervals | Shell structs only (`demo_status()`) | 🔴 Critical |
| G4 | **Multi-Database Registration** | YAML `databases:` config, named DB map | Shell types, no real multi-DB scrape | 🔴 Critical |

### 3.2 HIGH GAPS

| # | Gap | Reference | InfraScope Current | Priority |
|---|-----|-----------|-------------------|----------|
| G5 | **Alert Log Streaming** | `v$diag_alert_ext` → JSON, per-DB files, backoff retry | Demo entries only | 🟠 High |
| G6 | **Custom Metric Hot Reload** | Watches metrics files, reloads without restart | TOML/YAML parser exists but no hot reload | 🟠 High |
| G7 | **TxEventQ Real Data** | SQL against `gv$aq_sharded_subscriber_stat`, `gv$persistent_queues` | Demo data only | 🟠 High |
| G8 | **Vault Integration** | OCI Vault, Azure Key Vault, HashiCorp Vault | `AuthMode::Vault` enum exists, no implementation | 🟠 High |
| G9 | **Per-Metric Scrape Intervals** | Each metric can define `scrapeInterval`, cached between scrapes | Cache exists, no per-metric scheduling | 🟠 High |
| G10 | **Histogram Metric Support** | Custom bucket definitions in TOML/YAML | Types parsed but no histogram rendering | 🟠 High |

### 3.3 MEDIUM GAPS

| # | Gap | Reference | InfraScope Current | Priority |
|---|-----|-----------|-------------------|----------|
| G11 | **Per-Database Labels** | Arbitrary `labels:` in YAML config | Not implemented | 🟡 Medium |
| G12 | **Database-Scoped Metrics** | `databases:` field per metric to target specific DBs | Parsed but not enforced | 🟡 Medium |
| G13 | **Connection Pool Tuning** | Configurable pool min/max/increment, warm-up | Config struct exists, no real pool | 🟡 Medium |
| G14 | **Query Timeout Per-Metric** | Override timeout at metric level | Field exists in model, not used | 🟡 Medium |
| G15 | **Duplicate Database Detection** | Warns on same URL+user | `detect_duplicates()` stub | 🟡 Medium |
| G16 | **Field-to-Append** | Metric field appended to metric FQDN | Parsed in loader, not used in executor | 🟡 Medium |

### 3.4 LOW / NICE-TO-HAVE GAPS

| # | Gap | Reference | InfraScope Current | Priority |
|---|-----|-----------|-------------------|----------|
| G17 | **Kubernetes Manifests** | Deployment, Service, ServiceMonitor, ConfigMap | Not present | 🔵 Low |
| G18 | **Grafana-Compatible Dashboard Export** | JSON dashboards with PromQL expressions | InfraScope has own dashboard system | 🔵 Low |
| G19 | **Prometheus /metrics Endpoint** | Standard Prometheus exposition format | InfraScope has own scrape engine | 🔵 Low |
| G20 | **Memory Management** | Periodic `debug.FreeOSMemory()`, optional restart | Not needed in Rust (no GC) | N/A |

### 3.5 EXISTING STRENGTHS (InfraScope Ahead)

| Feature | InfraScope | Reference |
|---------|-----------|-----------|
| **RAC Deep Monitoring** | Full: instance status, sysstat per-node, interconnect, cache fusion | Not included (only gv$ queries) |
| **Data Guard** | Apply lag, transport lag, role, protection mode, switchover | Not included |
| **RMAN Backup** | Job details, failure count, last full backup, throughput | Not included |
| **PDB/CDB** | Per-PDB sessions, resource usage, resource limits | Not included |
| **Long-Running Queries** | Active SQL, SQL plan stats, blocking sessions | Only top_sql by elapsed |
| **Wait Events** | Top waits, wait class summary, active session waits | Only wait_class totals |
| **Listener Health** | TCP connect + latency measurement | Not included |
| **Metric Editor UI** | Full CRUD, test execute, preview | N/A (TOML files only) |
| **SSH Fallback** | Works without Oracle Instant Client | Requires godror/ODPI-C |

---

## 4. Implementation Roadmap

### Phase 1: Native Connection & Real Metrics (Foundation)
**Effort: ~3,000 LOC | Priority: Critical | Duration: ~1 week**

| Step | Task | Files |
|------|------|-------|
| 1.1 | **Add `oracle` Rust crate** to Cargo dependencies | `Cargo.toml`, `plugins/database-plugin/Cargo.toml` |
| 1.2 | **Implement `OracleConnectionPool`** — real FFI connection using `oracle` crate, with pool min/max/increment, warm-up, reconnect, backoff, role auth (SYSDBA etc.), wallet support | `oracle_connection.rs` |
| 1.3 | **Wire `oracle_metric_executor.rs`** — replace `generate_sample_*` with real SQL execution against connection pool, parse rows into `MetricResult`, handle labels/field_to_append/ignore_zero_result | `oracle_metric_executor.rs` |
| 1.4 | **Replace SSH+sqlplus** in `oracle.rs` API handler — use native pool for all `/api/oracle/*` endpoints | `handlers/oracle.rs` |
| 1.5 | **Multi-DB Registration** — API endpoint to register multiple Oracle databases, store configs in DB table `oracle_connections` | New: `deploy/sql/017_oracle_connections.sql`, update `handlers/oracle.rs` |
| 1.6 | **Connection Health Check** — ping + warm-up + invalidate cycle, expose per-connection up/down status | `oracle_connection.rs`, `oracle_multi_db.rs` |

### Phase 2: Scheduled Scrape & Caching (Reliability)
**Effort: ~2,000 LOC | Priority: Critical | Duration: ~4 days**

| Step | Task | Files |
|------|------|-------|
| 2.1 | **Background Scrape Task** — tokio interval task, iterate all registered DBs, execute all active metrics, store in cache | `oracle_scheduler.rs` |
| 2.2 | **Per-Metric Scrape Interval** — honor `scrape_interval` field, skip metrics that aren't due, serve from cache | `oracle_scheduler.rs`, `oracle_cache.rs` |
| 2.3 | **Per-Metric Query Timeout** — enforce `query_timeout` per metric definition | `oracle_metric_executor.rs` |
| 2.4 | **Scrape Meta-Metrics** — track duration, success/failure count, cache hit/miss per connection per metric | `oracle_scheduler.rs` |
| 2.5 | **API: Scheduler Control** — start/stop/status/force-scrape endpoints | `handlers/oracle.rs` |

### Phase 3: Alert Log & TxEventQ (Observability)
**Effort: ~1,500 LOC | Priority: High | Duration: ~3 days**

| Step | Task | Files |
|------|------|-------|
| 3.1 | **Real Alert Log Collection** — query `v$diag_alert_ext` via native connection, incremental (last timestamp), JSON export, per-DB files, retry with exponential backoff | `oracle_alertlog.rs` |
| 3.2 | **Alert Log → Log Engine** — feed entries into InfraScope's log ingestion pipeline for LogQL querying | `oracle_alertlog.rs` + log engine integration |
| 3.3 | **Real TxEventQ Metrics** — query `gv$aq_sharded_subscriber_stat`, `gv$persistent_queues`, expose per-queue/subscriber stats | `oracle_txeventq.rs` |
| 3.4 | **TxEventQ Alerting** — threshold-based alerts on queue depth, consumer lag, message latency | `oracle_txeventq.rs` |

### Phase 4: Custom Metrics & Hot Reload (Flexibility)
**Effort: ~1,000 LOC | Priority: High | Duration: ~2 days**

| Step | Task | Files |
|------|------|-------|
| 4.1 | **Hot Reload** — file watcher on TOML/YAML metric files, reload definitions without restart, reinitialize caches | `oracle_metrics_loader.rs`, `oracle_scheduler.rs` |
| 4.2 | **Histogram Support** — generate histogram buckets from `metrics_buckets` definitions, expose as proper histograms | `oracle_metric_executor.rs` |
| 4.3 | **Field-to-Append** — append field value to metric FQDN (e.g., `oracledb_activity_execute_count`) | `oracle_metric_executor.rs` |
| 4.4 | **Database-Scoped Metrics** — enforce `databases:` list per metric, skip non-targeted DBs | `oracle_metric_executor.rs` |
| 4.5 | **Per-Database Labels** — inject configured labels into all metrics from that database | `oracle_multi_db.rs` |

### Phase 5: Vault Integration (Security)
**Effort: ~800 LOC | Priority: High | Duration: ~2 days**

| Step | Task | Files |
|------|------|-------|
| 5.1 | **OCI Vault** — `GetVaultSecret()` for username/password | New: `plugins/database-plugin/src/oracle_vault.rs` |
| 5.2 | **Azure Key Vault** — `GetVaultSecret()` for username/password | `oracle_vault.rs` |
| 5.3 | **HashiCorp Vault** — KVv1, KVv2, Database mount types, proxy mode | `oracle_vault.rs` |
| 5.4 | **Password File** — read password from file (container-friendly) | `oracle_connection.rs` |
| 5.5 | **Credential Rotation** — on auth failure (ORA-01017), re-fetch from vault | `oracle_connection.rs` |

### Phase 6: Navigation Restructuring (UX)
**Effort: ~200 LOC | Priority: High | Duration: ~1 day**

Target navigation structure:
```
Databases
  ├─ Dashboard                → /databases
  ├─ Oracle Database          → (submenu)
  │   ├─ Oracle Monitoring    → /oracle
  │   └─ Metric Editor        → /oracle/metrics
  ├─ PostgreSQL               → /postgres
  ├─ MySQL                    → /mysql
  ├─ MongoDB                  → /mongodb
  └─ SQL Server               → /sqlserver
```

| Step | Task | Files |
|------|------|-------|
| 6.1 | **Update NavItem type** — support 3rd-level children | `Layout.tsx` |
| 6.2 | **Add Oracle Database submenu** — group Oracle + Metric Editor under "Oracle Database" | `Layout.tsx` |
| 6.3 | **Render nested submenu** — indented 3rd-level nav items with expand/collapse | `Layout.tsx` |

### Phase 7: Dashboard Enhancement (Polish)
**Effort: ~2,000 LOC | Priority: Medium | Duration: ~3 days**

| Step | Task | Files |
|------|------|-------|
| 7.1 | **Database Selector** — dropdown to switch between registered Oracle databases | `OracleMonitoring.tsx` |
| 7.2 | **Real-Time Refresh** — auto-refresh at configurable intervals, show last-updated timestamp | `OracleMonitoring.tsx` |
| 7.3 | **Additional Panels** — match reference Grafana: ASM Diskgroup, Resource Limits, DB Platform stat cards | `OracleMonitoring.tsx` |
| 7.4 | **Scrape Status Dashboard** — show per-connection/per-metric scrape health, duration, error rates | New: section in `OracleMonitoring.tsx` |
| 7.5 | **Connection Manager Page** — add/edit/delete/test Oracle database connections | New: `OracleConnections.tsx` or tab in OracleMonitoring |

### Phase 8: Kubernetes & Deployment (Operations)
**Effort: ~500 LOC | Priority: Low | Duration: ~1 day**

| Step | Task | Files |
|------|------|-------|
| 8.1 | **K8s Deployment YAML** — Oracle metric collector as sidecar or standalone | `deploy/k8s/oracle-collector.yaml` |
| 8.2 | **ServiceMonitor** — Prometheus scrape target for InfraScope | `deploy/k8s/servicemonitor.yaml` |
| 8.3 | **ConfigMap** — default + custom metrics TOML mounted | `deploy/k8s/configmap.yaml` |

---

## 5. Summary Matrix

| Phase | Gap IDs | LOC Est. | Duration | Priority |
|-------|---------|----------|----------|----------|
| P1: Native Connection | G1, G2, G4, G13 | ~3,000 | 1 week | 🔴 Critical |
| P2: Scheduled Scrape | G3, G9, G14 | ~2,000 | 4 days | 🔴 Critical |
| P3: Alert Log + TxEventQ | G5, G7 | ~1,500 | 3 days | 🟠 High |
| P4: Custom Metrics | G6, G10, G11, G12, G15, G16 | ~1,000 | 2 days | 🟠 High |
| P5: Vault Integration | G8 | ~800 | 2 days | 🟠 High |
| P6: Navigation | — | ~200 | 1 day | 🟠 High |
| P7: Dashboard Polish | — | ~2,000 | 3 days | 🟡 Medium |
| P8: Kubernetes | G17 | ~500 | 1 day | 🔵 Low |
| **TOTAL** | | **~11,000** | **~3 weeks** | |

---

## 6. Execution Dependencies

```
P1 (Native Connection) ──┬──→ P2 (Scheduled Scrape) ──→ P7 (Dashboard Polish)
                         ├──→ P3 (Alert Log + TxEventQ)
                         ├──→ P4 (Custom Metrics)
                         └──→ P5 (Vault Integration)

P6 (Navigation) ──→ independent, can start immediately

P8 (Kubernetes) ──→ after P1+P2
```

**Recommended start order:** P6 → P1 → P2 → P3+P4+P5 (parallel) → P7 → P8

---

## 7. Navigation Restructuring Detail

### Current (`Layout.tsx` lines 69–81):
```tsx
{
  to: '/databases',
  label: 'Databases',
  icon: Database,
  children: [
    { to: '/databases', label: 'Dashboard', icon: LayoutDashboard },
    { to: '/oracle', label: 'Oracle', icon: Database },
    { to: '/oracle/metrics', label: 'Metric Editor', icon: Database },
    { to: '/postgres', label: 'PostgreSQL', icon: Database },
    ...
  ],
}
```

### Proposed:
```tsx
{
  to: '/databases',
  label: 'Databases',
  icon: Database,
  children: [
    { to: '/databases', label: 'Dashboard', icon: LayoutDashboard },
    {
      to: '/oracle',
      label: 'Oracle Database',
      icon: Database,
      children: [
        { to: '/oracle', label: 'Oracle Monitoring', icon: Activity },
        { to: '/oracle/metrics', label: 'Metric Editor', icon: Settings },
      ],
    },
    { to: '/postgres', label: 'PostgreSQL', icon: Database },
    { to: '/mysql', label: 'MySQL', icon: Database },
    { to: '/mongodb', label: 'MongoDB', icon: Database },
    { to: '/sqlserver', label: 'SQL Server', icon: Database },
  ],
}
```

This requires:
1. Extending `NavItem` interface to support recursive `children`
2. Updating the sidebar render logic to support 3rd-level indentation
3. No route changes needed — just navigation grouping

---

## 8. Additional Reference Projects Analysis

### 8.1 `oracledb_exporter-master` (iamseth/oracledb_exporter)

| Aspect | Details |
|--------|---------|
| **Language** | Go + go-ora (pure Go Oracle driver, no C dependencies) |
| **Architecture** | Prometheus exporter, single DSN, `/metrics` endpoint |
| **Driver** | `github.com/sijms/go-ora/v2` — lighter than godror, no Oracle Instant Client needed |
| **Connection** | `database/sql` with `MaxIdleConns`, `MaxOpenConns`, ping+reconnect |
| **Metrics** | 7 default contexts (sessions, resource, asm_diskgroup, activity, process, wait_time, tablespace) |
| **ASM Metrics** | Dedicated `default-asm-metrics.toml` — diskgroup size, ASM uptime, per-disk I/O stats (reads/writes/bytes/IOPS), space consumers by DB by file type |
| **Custom Metrics** | TOML/YAML, hot reload via SHA256 file hash checking per scrape |
| **Histogram** | Full histogram bucket support with `metricsbuckets` config |
| **Scheduled Scrape** | `RunScheduledScrapes()` with configurable interval |
| **fieldToAppend** | Appends SQL column value to metric FQDN (e.g., `oracledb_activity_execute_count`) |
| **Legacy Tablespace** | Separate calculation method using `dba_data_files` + `dba_free_space` + temp files |

**Key additions vs oracle-db-appdev-monitoring:**
- **ASM deep monitoring** — per-disk I/O counters (reads, writes, read_time, write_time, bytes_read, bytes_written, IOPS, failgroup, path)
- **ASM space consumers** — space usage by database by file type (DATAFILE, TEMPFILE, etc.)
- **SHA256 hot reload** — checks file hash on every scrape cycle, reloads if changed
- **Simpler architecture** — single binary, no vault, pure Go driver (no CGO)
- **Legacy tablespace query** — alternative calculation compatible with older Oracle versions

### 8.2 `Chat2DB-main` (CodePhiliaX/Chat2DB)

| Aspect | Details |
|--------|---------|
| **Stack** | Java Spring Boot (server) + React/UmiJS (client) |
| **Databases** | 16+ types: MySQL, PostgreSQL, Oracle, SQLServer, SQLite, MariaDB, ClickHouse, DM, Presto, DB2, OceanBase, Hive, KingBase, MongoDB, Redis, Timeplus |
| **Plugin Architecture** | SPI-based: `Plugin` → `MetaData` + `DBManage` + `DBConfig` per database type |
| **AI Features** | 4 core AI capabilities powered by ChatGPT/custom LLMs |
| **Connection Mgmt** | CRUD connections, SSH tunnel, JDBC driver management, connection test, environments |
| **SQL Workspace** | Monaco editor, execute SQL, view results, table browser, object tree |
| **Dashboard/Charts** | Chart CRUD, dashboard layout, report generation from SQL queries |
| **History** | Full SQL execution history with timestamps, duration, row counts, data source tracking |

#### Chat2DB AI Features (applicable to InfraScope chatbot)

| Feature | How It Works | InfraScope Adaptation |
|---------|--------------|----------------------|
| **Natural Language → SQL** | User describes query in plain English; AI generates valid SQL for the target DB dialect | Chatbot understands "show me tablespace usage for ORCLPDB1" → generates Oracle SQL, executes, returns results |
| **SQL Explain** | User pastes SQL; AI returns plain-English explanation of what it does | Chatbot tool: "Explain this query" → helps DBAs understand complex monitoring queries |
| **SQL Optimize** | User pastes slow SQL; AI suggests index creation, query rewrites, caching strategies | Chatbot tool: "Optimize this query" → AI recommends indexes, partition strategies for any DB type |
| **SQL Convert** | Convert SQL between database dialects (Oracle → PostgreSQL, MySQL → SQLServer, etc.) | Chatbot tool: "Convert this Oracle SQL to PostgreSQL" → helps migration planning |
| **Context-Aware Chat** | Maintains 4-message context window; knows current DB connection, schema, tables | Chatbot auto-injects current database context (which DB is selected, what schema) |

#### Chat2DB Architecture Patterns (applicable to InfraScope)

| Pattern | Description | How to Apply |
|---------|-------------|--------------|
| **Per-DB Plugin SPI** | Each database type implements `MetaData` (schemas, tables, columns, DDL, views, functions, procedures, triggers, indexes, sequences) + `DBManage` (connect, create/drop/export DB/schema/table) + `SqlBuilder` | InfraScope: Create `DatabasePlugin` trait per DB type with `list_schemas()`, `list_tables()`, `get_ddl()`, `execute_query()`, `get_metadata()` |
| **Connection Manager** | CRUD for data sources, SSH tunnels, JDBC drivers, test connectivity, per-environment tagging | InfraScope: Shared connection manager per DB type, test connection, store in `targets` table |
| **SQL Console** | Monaco Editor with DB-type-aware syntax highlighting, autocomplete from table/column metadata, execute + paginated results | InfraScope: SQL Workspace per database in chatbot, with schema-aware autocomplete |
| **Object Browser** | Tree view: Connection → Database → Schema → Tables/Views/Functions/Procedures/Triggers/Sequences | InfraScope: Database Explorer tab per DB type |
| **SQL History** | Log every query execution with duration, row count, status, data source | InfraScope: Query history per database connection |
| **Dashboard Charts** | Create charts from SQL query results, embed in dashboards | InfraScope: Already has dashboard builder — add "from SQL query" chart source |

---

## 9. Universal Database Monitoring Plan — All Databases

### 9.1 Page Layout & Navigation (Uniform for All DBs)

#### Sidebar Navigation (Layout.tsx)
```
Databases
  ├─ Dashboard                    → /databases
  ├─ Oracle Database              → (submenu)
  │   ├─ Oracle Monitoring        → /oracle
  │   └─ Metric Editor            → /oracle/metrics
  ├─ PostgreSQL                   → (submenu)
  │   ├─ PostgreSQL Monitoring    → /postgres
  │   └─ Metric Editor            → /postgres/metrics     ← NEW
  ├─ MySQL                        → (submenu)
  │   ├─ MySQL Monitoring         → /mysql
  │   └─ Metric Editor            → /mysql/metrics        ← NEW
  ├─ MongoDB                      → (submenu)
  │   ├─ MongoDB Monitoring       → /mongodb
  │   └─ Metric Editor            → /mongodb/metrics      ← NEW
  └─ SQL Server                   → (submenu)
      ├─ SQL Server Monitoring    → /sqlserver
      └─ Metric Editor            → /sqlserver/metrics    ← NEW
```

#### In-Page Layout — Oracle-Style Vertical Sidebar Tabs (ALL DBs)

Every database monitoring page (`/oracle`, `/postgres`, `/mysql`, `/mongodb`, `/sqlserver`) uses
the **same layout pattern** — a vertical left sidebar with icon+label tabs, and a right content
area. This matches the existing Oracle page design.

**SQL Workspace is always the LAST tab** in the vertical sidebar, placed directly below the last
monitoring tab for each DB type. This is standard across all database flavors.

**Oracle:**
```
 🗄 Overview
 💾 Tablespaces
 ⏱ Wait Classes
 ⚡ Activity
 ⚡ Cache & SGA
 🔍 Top Queries
 ⏳ Running Queries
 📦 PDB / CDB
 🛡 Data Guard
 💾 RMAN
 📡 Listeners
 🖥 RAC
 🔌 Connections
 📄 Alert Log
 📬 TxEventQ
 💻 SQL Workspace       ← LAST TAB
```

**PostgreSQL:**
```
 🗄 Overview
 🔌 Connections
 🔍 Slow Queries
 📊 Tables
 📇 Indexes
 🔄 Replication
 🔒 Locks
 📈 Performance
 📦 PgPool-II
 💻 SQL Workspace       ← LAST TAB
```

**MySQL:**
```
 🗄 Overview
 🔌 Connections
 🔍 Slow Queries
 💾 InnoDB
 🔄 Replication
 📊 Tables
 📈 Performance
 💻 SQL Workspace       ← LAST TAB
```

**MongoDB:**
```
 🗄 Overview
 📁 Collections
 🔄 Replica Set
 ⚡ Operations
 📇 Indexes
 📈 Performance
 💻 Query Workspace     ← LAST TAB
```

**SQL Server:**
```
 🗄 Overview
 🔌 Sessions
 🔍 Top Queries
 💾 Databases
 ⏱ Agent Jobs
 ⏳ Wait Stats
 📈 Performance
 💻 SQL Workspace       ← LAST TAB
```

> **Key:** PostgreSQL, MySQL, MongoDB, and SQL Server currently use horizontal button tabs at the
> top. All will be converted to match the Oracle page's vertical left sidebar pattern.
> SQL Workspace is always the last item — consistent position across all 5 DB types.
> See **Section 14** for detailed icon mappings, wireframes, and implementation specs.

### 9.2 Shared Features Across ALL Databases

| Feature | Description | Source |
|---------|-------------|--------|
| **Monitoring Dashboard** | Per-DB-type monitoring page with KPIs, charts, tables | Already exists for all 5 DB types |
| **Custom Metric Editor** | TOML/YAML metric definitions with SQL queries, labels, types | Currently Oracle-only → extend to all |
| **SQL Workspace** | In-page tab: Monaco editor, execute queries, view results, schema browser (embedded in each DB monitoring page, not a separate route) | NEW from Chat2DB |
| **Connection Manager** | Register/edit/delete/test database connections | Partially exists → formalize |
| **Query History** | Log all executed queries with duration, rows, status | NEW from Chat2DB |
| **Schema Explorer** | Browse databases/schemas/tables/columns/indexes/views | NEW from Chat2DB |
| **Object DDL Viewer** | View CREATE TABLE, indexes, triggers, procedures | NEW from Chat2DB |
| **Scheduled Metrics** | Background metric collection at configurable intervals | Currently Oracle-only → extend to all |
| **Metric Cache** | In-memory cache with per-metric TTL | Currently Oracle-only → extend to all |
| **Alert Log / Error Log** | Collect DB error logs, parse severity, feed to log engine | Currently Oracle-only → extend to Postgres `pg_log`, MySQL `error_log`, etc. |

### 9.3 Per-Database Specific Metrics

#### Oracle (already detailed above)
- RAC, Data Guard, RMAN, Tablespaces, Wait Events, PDB/CDB, Listeners, ASM, TxEventQ, Alert Log

#### PostgreSQL
| Metric Area | SQL Source | Status |
|-------------|-----------|--------|
| Connection stats | `pg_stat_activity` | Add |
| Replication lag | `pg_stat_replication` | Add |
| Database size | `pg_database_size()` | Add |
| Table bloat | `pg_stat_user_tables` (dead tuples) | Add |
| Index usage | `pg_stat_user_indexes` | Add |
| Lock waits | `pg_stat_activity` (wait_event) | Add |
| Cache hit ratio | `pg_stat_database` (blks_hit / blks_read) | Add |
| WAL stats | `pg_stat_wal` | Add |
| Vacuum stats | `pg_stat_user_tables` (last_autovacuum) | Add |
| Slow queries | `pg_stat_statements` (mean_exec_time) | Add |

#### MySQL
| Metric Area | SQL Source | Status |
|-------------|-----------|--------|
| Connections | `SHOW GLOBAL STATUS` (Threads_connected) | Add |
| Queries per sec | `SHOW GLOBAL STATUS` (Queries, Questions) | Add |
| InnoDB buffer pool | `SHOW ENGINE INNODB STATUS`, `information_schema.innodb_buffer_pool_stats` | Add |
| Replication lag | `SHOW SLAVE STATUS` (Seconds_Behind_Master) | Add |
| Table locks | `SHOW GLOBAL STATUS` (Table_locks_waited) | Add |
| Slow queries | `performance_schema.events_statements_summary_by_digest` | Add |
| Binary log | `SHOW MASTER STATUS`, `SHOW BINARY LOGS` | Add |
| Schema size | `information_schema.tables` (SUM data_length + index_length) | Add |
| Handler stats | `SHOW GLOBAL STATUS` (Handler_read_*, Handler_write) | Add |

#### MongoDB
| Metric Area | Command Source | Status |
|-------------|---------------|--------|
| Server status | `db.serverStatus()` | Add |
| Connection pool | `serverStatus.connections` | Add |
| Operations | `serverStatus.opcounters` | Add |
| Replication | `rs.status()`, `rs.conf()` | Add |
| WiredTiger cache | `serverStatus.wiredTiger.cache` | Add |
| Collection stats | `db.collection.stats()` | Add |
| Index stats | `db.collection.aggregate($indexStats)` | Add |
| Slow operations | `db.currentOp()`, `system.profile` | Add |

#### SQL Server
| Metric Area | SQL Source | Status |
|-------------|-----------|--------|
| Connections | `sys.dm_exec_connections` | Add |
| Wait stats | `sys.dm_os_wait_stats` | Add |
| Buffer cache hit | `sys.dm_os_performance_counters` | Add |
| Database size | `sys.master_files` | Add |
| Query stats | `sys.dm_exec_query_stats` | Add |
| AG health | `sys.dm_hadr_availability_replica_states` | Add |
| Job status | `msdb.dbo.sysjobs` + `sysjobhistory` | Add |
| Index fragmentation | `sys.dm_db_index_physical_stats` | Add |
| TempDB usage | `sys.dm_db_task_space_usage` | Add |
| Deadlocks | `sys.dm_exec_requests` (blocking_session_id) | Add |

---

## 10. AI-Powered Database Chatbot Enhancement

### 10.1 New Chatbot Tools (from Chat2DB inspiration)

Enhance the existing InfraScope chatbot (`Chatbot.tsx`) with database-specific AI tools:

| Tool | Description | Implementation |
|------|-------------|---------------|
| `natural_language_to_sql` | Convert "show me top 5 tables by size" → valid SQL for target DB type | LLM call with DB schema context |
| `explain_sql` | Explain what a SQL query does in plain English | LLM call with query + schema |
| `optimize_sql` | Suggest indexes, rewrites, partition strategies for slow queries | LLM call with query + execution plan |
| `convert_sql` | Convert SQL between dialects (Oracle → PostgreSQL, etc.) | LLM call with source SQL + target dialect |
| `execute_sql` | Execute arbitrary SQL against a registered database connection | API call → backend executes → returns results |
| `get_schema_info` | List databases, schemas, tables, columns for a connection | API call → metadata query |
| `get_query_history` | Show recent query executions with timing | API call → history table |
| `analyze_performance` | AI analysis of current DB performance metrics + recommendations | LLM call with metric context |

### 10.2 SQL Workspace Page (NEW)

A shared component usable by all database types:

```
┌─────────────────────────────────────────────────────┐
│ SQL Workspace — Oracle (ORCLPDB1)           [▶ Run] │
├────────────┬────────────────────────────────────────┤
│ Schema     │ Monaco Editor                          │
│ Browser    │ ┌────────────────────────────────────┐ │
│            │ │ SELECT tablespace_name,             │ │
│ ► SYSTEM   │ │   used_percent                     │ │
│   ► TABLES │ │ FROM dba_tablespace_usage_metrics  │ │
│   ► VIEWS  │ │ ORDER BY used_percent DESC;        │ │
│   ► PROCS  │ └────────────────────────────────────┘ │
│ ► HR       ├────────────────────────────────────────┤
│ ► APP      │ Results                                │
│            │ ┌──────────────────┬──────────────┐   │
│ [AI Chat]  │ │ TABLESPACE_NAME  │ USED_PERCENT │   │
│ Ask AI...  │ ├──────────────────┼──────────────┤   │
│            │ │ APP_DATA         │ 82.5         │   │
│            │ │ USERS            │ 65.3         │   │
│            │ │ SYSTEM           │ 45.1         │   │
│            │ └──────────────────┴──────────────┘   │
│            │ 3 rows | 0.045s | History ▾           │
└────────────┴────────────────────────────────────────┘
```

**Features:**
- Monaco editor with DB-type-aware syntax highlighting
- Schema browser sidebar (tables, views, functions, procedures, triggers, indexes)
- Query execution with paginated results table
- AI sidebar: type natural language, get SQL generated
- Query history with re-run capability
- Export results to CSV/JSON
- Execution plan visualization

---

## 11. Updated Implementation Phases (with new additions)

### NEW Phase 9: Universal Metric Editor (All DBs)
**Effort: ~2,000 LOC | Priority: High | Duration: ~3 days**

| Step | Task | Files |
|------|------|-------|
| 9.1 | **PostgreSQL default-metrics.toml** — 10 metric contexts from pg_stat_* views | `plugins/database-plugin/src/postgres_default_metrics.toml` |
| 9.2 | **MySQL default-metrics.toml** — 9 metric contexts from SHOW STATUS / information_schema | `plugins/database-plugin/src/mysql_default_metrics.toml` |
| 9.3 | **MongoDB default-metrics.toml** — 8 metric contexts from serverStatus / rs.status | `plugins/database-plugin/src/mongodb_default_metrics.toml` |
| 9.4 | **SQL Server default-metrics.toml** — 10 metric contexts from sys.dm_* views | `plugins/database-plugin/src/sqlserver_default_metrics.toml` |
| 9.5 | **Shared Metric Editor page** — reusable component accepting DB type prop | `frontend/src/pages/MetricEditor.tsx` (shared) |
| 9.6 | **Per-DB Metric Editor routes** — `/postgres/metrics`, `/mysql/metrics`, `/mongodb/metrics`, `/sqlserver/metrics` | `App.tsx`, `Layout.tsx` |

### NEW Phase 10: SQL Workspace — In-Page Tab (from Chat2DB)
**Effort: ~2,500 LOC | Priority: High | Duration: ~4 days**

| Step | Task | Files |
|------|------|-------|
| 10.1 | **Shared `SqlWorkspaceTab.tsx`** — Reusable component accepting `dbType` prop: Monaco editor, schema browser, results table, AI ask, query history, CSV/JSON export | `frontend/src/components/SqlWorkspaceTab.tsx` |
| 10.2 | **Backend: Execute SQL** — API endpoint to execute arbitrary SQL against registered connections | `handlers/database_query.rs` |
| 10.3 | **Backend: Schema metadata** — API endpoint for tables/columns/indexes/views per connection | `handlers/database_metadata.rs` |
| 10.4 | **Query History** — store/retrieve executed queries with timing, row count, status | SQL migration + API handler |
| 10.5 | **Add SQL Workspace tab** to all 5 DB monitoring pages (Oracle, PostgreSQL, MySQL, MongoDB, SQL Server) | Each `*Monitoring.tsx` file |

> **Note:** SQL Workspace is embedded as a vertical sidebar tab inside each DB monitoring page,
> not a separate route. See Section 14 for full layout details.

### NEW Phase 12: Unified Vertical Sidebar Layout (All DB Pages)
**Effort: ~990 LOC | Priority: High | Duration: ~2 days**

| Step | Task | Files |
|------|------|-------|
| 12.1 | **PostgreSQL** — Convert horizontal tabs → vertical sidebar with icons, add SQL Workspace tab | `PostgresMonitoring.tsx` |
| 12.2 | **MySQL** — Convert horizontal tabs → vertical sidebar with icons, add SQL Workspace tab | `MysqlMonitoring.tsx` |
| 12.3 | **MongoDB** — Convert horizontal tabs → vertical sidebar with icons, add Query Workspace tab | `MongodbMonitoring.tsx` |
| 12.4 | **SQL Server** — Convert horizontal tabs → vertical sidebar with icons, add SQL Workspace tab | `SqlServerMonitoring.tsx` |
| 12.5 | **Oracle** — Add SQL Workspace tab to existing vertical sidebar | `OracleMonitoring.tsx` |

> **Note:** See Section 14 for per-database tab definitions, icons, and layout specs.

### NEW Phase 11: AI Chatbot Database Tools (from Chat2DB)
**Effort: ~2,000 LOC | Priority: Medium | Duration: ~3 days**

| Step | Task | Files |
|------|------|-------|
| 11.1 | **NL-to-SQL tool** — LLM generates valid SQL from natural language + schema context | `Chatbot.tsx` + backend API |
| 11.2 | **SQL Explain tool** — LLM explains SQL in plain English | `Chatbot.tsx` |
| 11.3 | **SQL Optimize tool** — LLM suggests index/rewrite/partition optimizations | `Chatbot.tsx` |
| 11.4 | **SQL Convert tool** — LLM converts SQL between dialects | `Chatbot.tsx` |
| 11.5 | **Execute SQL tool** — chatbot can run queries against any registered DB | `Chatbot.tsx` + backend API |
| 11.6 | **Performance Analyze tool** — AI summary of current DB health + recommendations | `Chatbot.tsx` |

---

## 12. Revised Summary Matrix

| Phase | Gap IDs | LOC Est. | Duration | Priority |
|-------|---------|----------|----------|----------|
| P1: Native Oracle Connection | G1, G2, G4, G13 | ~3,000 | 1 week | 🔴 Critical |
| P2: Scheduled Scrape | G3, G9, G14 | ~2,000 | 4 days | 🔴 Critical |
| P3: Alert Log + TxEventQ | G5, G7 | ~1,500 | 3 days | 🟠 High |
| P4: Custom Metrics | G6, G10, G11, G12, G15, G16 | ~1,000 | 2 days | 🟠 High |
| P5: Vault Integration | G8 | ~800 | 2 days | 🟠 High |
| P6: Navigation (ALL DBs) | — | ~400 | 1 day | 🟠 High |
| P7: Oracle Dashboard Polish | — | ~2,000 | 3 days | 🟡 Medium |
| P8: Kubernetes | G17 | ~500 | 1 day | 🔵 Low |
| **P9: Universal Metric Editor** | — | **~2,000** | **3 days** | 🟠 **High** |
| **P10: SQL Workspace (in-page tab)** | — | **~2,500** | **4 days** | 🟠 **High** |
| **P11: AI Chatbot DB Tools** | — | **~2,000** | **3 days** | 🟡 **Medium** |
| **P12: Vertical Sidebar Layout (All DBs)** | — | **~990** | **2 days** | 🟠 **High** |
| **TOTAL** | | **~18,190** | **~5.5 weeks** | |

---

## 13. Revised Execution Dependencies

```
P1 (Native Oracle Conn) ──┬──→ P2 (Scheduled Scrape) ──→ P7 (Dashboard Polish)
                          ├──→ P3 (Alert Log + TxEventQ)
                          ├──→ P4 (Custom Metrics)
                          └──→ P5 (Vault Integration)

P6 (Navigation ALL DBs) ──→ independent, START FIRST

P9 (Universal Metric Editor) ──→ after P4 (reuses oracle metric loader pattern)

P10 (SQL Workspace Tab) ──→ independent, can start in parallel

P11 (AI Chatbot) ──→ after P10 (needs execute_sql backend)

P12 (Vertical Sidebar ALL DBs) ──→ independent, START EARLY (pairs with P10)

P8 (Kubernetes) ──→ last, after P1+P2
```

**Recommended order:** P6 → P12 → P1 → P2 → P3+P4+P5 (parallel) → P9 → P10 → P7+P11 (parallel) → P8

> **Note:** P12 (vertical sidebar conversion) should be done early — it's a pure frontend layout
> change with no backend dependency, and makes the DB pages consistent before adding SQL Workspace (P10).

---

## 14. Unified Vertical Sidebar Tab Layout — All Database Pages

### 14.1 Design Decision

The Oracle Monitoring page uses a **vertical left sidebar** with icon+label tabs and a right content
area. This layout is significantly cleaner than the horizontal button tabs currently used by
PostgreSQL, MySQL, MongoDB, and SQL Server pages. All database monitoring pages will be
standardized to match the Oracle pattern.

**SQL Workspace is always the LAST tab** in the vertical sidebar, placed directly below the last
monitoring tab for each DB type. A thin separator divider visually separates the monitoring tabs
from the workspace tab. This position is **standard across all 5 database flavors**.

### 14.2 Current vs Target Layout

**Current (PostgreSQL, MySQL, MongoDB, SQL Server):**
```
┌──────────────────────────────────────────────────┐
│ PageHeader                                        │
│ [Overview] [Connections] [Queries] [Tables] ...   │  ← horizontal button tabs
│                                                    │
│ Content area below                                 │
└──────────────────────────────────────────────────┘
```

**Target (matching Oracle — all DB pages):**
```
┌──────────────────────────────────────────────────┐
│ PageHeader                              [Refresh] │
│ [Auto-Refresh ON/OFF]                             │
├──────────────┬───────────────────────────────────┤
│ 🗄 Overview   │                                    │
│ 🔌 Connections│  Content area                      │
│ 🔍 Queries   │  KPIs, Charts, Tables              │
│ 📊 Tables    │                                    │
│ 📇 Indexes   │  (changes based on selected tab)   │
│ 🔄 Replication│                                   │
│ 🔒 Locks     │                                    │
│ 📈 Performance│                                   │
│ ─────────────│───────────────────────────────────│
│ 💻 SQL       │  SQL Workspace: Monaco editor,     │
│    Workspace │  schema browser, results table,    │
│              │  AI ask, query history, export      │
└──────────────┴───────────────────────────────────┘
       ↑ always last, below all monitoring tabs
```

### 14.3 Per-Database Tab Definitions

#### Oracle Database (existing — add SQL Workspace)
| Tab | Icon | ID |
|-----|------|----|
| Overview | `Database` | `overview` |
| Tablespaces | `HardDrive` | `tablespaces` |
| Wait Classes | `Clock` | `waits` |
| Activity | `Activity` | `activity` |
| Cache & SGA | `Zap` | `cache` |
| Top Queries | `Search` | `top-queries` |
| Running Queries | `Timer` | `running` |
| PDB / CDB | `Layers` | `pdb` |
| Data Guard | `Shield` | `dataguard` |
| RMAN | `HardDrive` | `rman` |
| Listeners | `Radio` | `listeners` |
| RAC | `Server` | `rac` |
| Connections | `Plug` | `connections` |
| Alert Log | `FileText` | `alertlog` |
| TxEventQ | `Inbox` | `txeventq` |
| **SQL Workspace** | **`Terminal`** | **`sql-workspace`** |

#### PostgreSQL
| Tab | Icon | ID |
|-----|------|----|
| Overview | `Database` | `overview` |
| Connections | `Plug` | `connections` |
| Slow Queries | `Search` | `queries` |
| Tables | `Table` | `tables` |
| Indexes | `List` | `indexes` |
| Replication | `RefreshCw` | `replication` |
| Locks | `Lock` | `locks` |
| Performance | `TrendingUp` | `performance` |
| PgPool-II | `Layers` | `pgpool` |
| **SQL Workspace** | **`Terminal`** | **`sql-workspace`** |

#### MySQL
| Tab | Icon | ID |
|-----|------|----|
| Overview | `Database` | `overview` |
| Connections | `Plug` | `connections` |
| Slow Queries | `Search` | `queries` |
| InnoDB | `HardDrive` | `innodb` |
| Replication | `RefreshCw` | `replication` |
| Tables | `Table` | `tables` |
| Performance | `TrendingUp` | `performance` |
| **SQL Workspace** | **`Terminal`** | **`sql-workspace`** |

#### MongoDB
| Tab | Icon | ID |
|-----|------|----|
| Overview | `Database` | `overview` |
| Collections | `Folder` | `collections` |
| Replica Set | `RefreshCw` | `replicaset` |
| Operations | `Activity` | `operations` |
| Indexes | `List` | `indexes` |
| Performance | `TrendingUp` | `performance` |
| **Query Workspace** | **`Terminal`** | **`query-workspace`** |

#### SQL Server
| Tab | Icon | ID |
|-----|------|----|
| Overview | `Database` | `overview` |
| Sessions | `Plug` | `connections` |
| Top Queries | `Search` | `queries` |
| Databases | `HardDrive` | `databases` |
| Agent Jobs | `Clock` | `jobs` |
| Wait Stats | `Timer` | `waits` |
| Performance | `TrendingUp` | `performance` |
| **SQL Workspace** | **`Terminal`** | **`sql-workspace`** |

### 14.4 SQL Workspace Tab — In-Page Design

The SQL Workspace tab is embedded directly inside each database monitoring page. No separate route
needed — it's just another tab in the vertical sidebar.

```
┌──────────┬──────────────────────────────────────────┐
│ ...tabs  │ SQL Workspace — PostgreSQL                │
│          ├────────────┬─────────────────────────────┤
│ 💻 SQL   │ Schema     │ Monaco Editor        [▶ Run]│
│  Workspace│ Browser   │ ┌─────────────────────────┐ │
│ (active) │           │ │ SELECT schemaname,       │ │
│          │ ► public  │ │   tablename, n_live_tup │ │
│          │  ► users  │ │ FROM pg_stat_user_tables │ │
│          │  ► orders │ │ ORDER BY n_live_tup DESC │ │
│          │  ► logs   │ │ LIMIT 20;               │ │
│          │ ► pg_cat  │ └─────────────────────────┘ │
│          │           ├─────────────────────────────┤
│          │           │ Results (20 rows, 0.032s)   │
│          │ [AI Ask]  │ ┌──────────┬───────────┐   │
│          │           │ │ tablename │ n_live_tup│   │
│          │           │ ├──────────┼───────────┤   │
│          │           │ │ users    │ 1,240,000 │   │
│          │           │ │ orders   │   892,000 │   │
│          │           │ └──────────┴───────────┘   │
│          │           │ [Export CSV] [Export JSON]  │
│          │           │ History ▾                   │
└──────────┴───────────┴─────────────────────────────┘
```

**SQL Workspace Features (per tab):**
- Monaco editor with DB-dialect-specific syntax highlighting (PL/SQL, PL/pgSQL, T-SQL, etc.)
- Schema browser sidebar: databases → schemas → tables/views/functions/procedures
- Click table name to insert into editor
- Execute button (Ctrl+Enter) → paginated results table
- Row count, execution time, error display
- AI "Ask" button: natural language → generates SQL for the current DB type
- Query history dropdown: re-run previous queries
- Export results: CSV, JSON
- Execution plan visualization (EXPLAIN output)

### 14.5 Implementation Changes Required

| File | Change | LOC Est. |
|------|--------|----------|
| `PostgresMonitoring.tsx` | Convert horizontal tabs → vertical sidebar (matching Oracle pattern), add icons, add SQL Workspace tab + content | ~150 |
| `MysqlMonitoring.tsx` | Convert horizontal tabs → vertical sidebar, add icons, add SQL Workspace tab + content | ~120 |
| `MongodbMonitoring.tsx` | Convert horizontal tabs → vertical sidebar, add icons, add Query Workspace tab + content | ~120 |
| `SqlServerMonitoring.tsx` | Convert horizontal tabs → vertical sidebar, add icons, add SQL Workspace tab + content | ~120 |
| `OracleMonitoring.tsx` | Add SQL Workspace tab to existing vertical sidebar | ~80 |
| **Shared `SqlWorkspaceTab.tsx`** | Reusable SQL Workspace component accepting `dbType` prop (Monaco editor, schema browser, results table, AI ask, history, export) | ~400 |
| **TOTAL** | | **~990** |

### 14.6 Impact on Routes & Navigation

**No new routes needed.** The SQL Workspace is an in-page tab, not a separate page. The existing
navigation submenu structure in `Layout.tsx` remains as-is:
```
Databases → [DB Type] → [DB] Monitoring + Metric Editor
```

The SQL Workspace lives inside the Monitoring page itself, accessible via the vertical sidebar tab.
This is cleaner than a separate route because:
1. Users stay in context (monitoring + querying in same view)
2. Connection context is shared (same DB connection for monitoring and SQL)
3. Fewer navigation clicks
4. Consistent with the Oracle page's existing design philosophy

---

*Document updated: Section 14 added — Unified Vertical Sidebar Tab Layout + SQL Workspace as in-page tab*
*All 5 database pages will use the Oracle-style vertical sidebar with icons*
*SQL Workspace embedded as a tab, not a separate route*
*Status: AWAITING USER REVIEW*
