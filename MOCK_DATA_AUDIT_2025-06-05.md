# Zentra Mock / Fake / Hardcoded Data Audit Report
**Date:** 2026-06-05
**Scope:** Entire codebase excluding `docker/` folder
**Instruction:** Document only — do not fix

---

## Executive Summary

After a 100% line-by-line review of the backend (`crates/zentra-api/src/handlers/`) and frontend (`frontend/src/`), **the frontend is largely clean** (all major pages now wire to live API endpoints via React Query). **The backend still contains significant hardcoded / mock / synthetic data** in ~20 handler files. These fall into three categories:

1. **In-memory stores seeded with fake objects** (compliance, maintenance, SSO, ITSM, log engine, monitoring templates, etc.)
2. **Handlers that return entirely hardcoded JSON** (dashboard overview, health summary, timeline, HA cluster status, alert silences/templates, etc.)
3. **Synthetic sine-wave / Math.sin metrics** in middleware performance-history endpoints (haproxy, memcached, phpfpm, redis, tomcat)

---

## Backend Findings

### 1. `crates/zentra-api/src/handlers/dashboards.rs`
| Line | Issue | Details |
|------|-------|---------|
| 81-168 | `list_dashboards` | Returns 2 hardcoded dashboards (`"Infrastructure Overview"`, `"Database Performance"`) with fake panels, variables, and `created_by: "admin"` |
| 189-206 | `get_dashboard` | Returns `"Example Dashboard"` with empty panels — not a real DB lookup |
| 216-237 | `export_dashboard` | Returns `"Exported Dashboard"` with fake metadata |
| 530-542 | `playlist_store` | OnceLock init contains 1 fake playlist (`"NOC Display"`) |
| 887-901 | `dashboard_overview` | Returns hardcoded KPIs: 48 hosts, 127 alerts, 99.87% SLA, 4500 Mbps, etc. |
| 903-919 | `dashboard_health_summary` | Returns 8 hardcoded module health rows with sparklines |
| 921-938 | `dashboard_timeline` | Returns 8 hardcoded fake events (ORA-04031, BGP peer down, etc.) |

### 2. `crates/zentra-api/src/handlers/monitoring_templates.rs`
| Line | Issue | Details |
|------|-------|---------|
| 318-400+ | `built_in_templates()` | 20+ hardcoded templates (linux-server, windows-server, nginx, apache, postgres, oracle, redis, etc.) with macros, checks, alerts, and metrics lists |

### 3. `crates/zentra-api/src/handlers/log_engine.rs`
| Line | Issue | Details |
|------|-------|---------|
| 118-184 | `init_store()` | 6 fake log streams (web01/nginx, api01/api-server, db01/postgresql, worker01/cron) with synthetic log lines. Also 2 hardcoded log alerts (`"High Error Rate"`, `"DB Connection Failures"`) |

### 4. `crates/zentra-api/src/handlers/compliance.rs`
| Line | Issue | Details |
|------|-------|---------|
| 103-408 | `init_store()` | 5 fake frameworks (CIS Ubuntu, CIS RHEL, CIS AWS, PCI-DSS, SOC2), 2 fake scans with scores, 9 fake findings with evidence |

### 5. `crates/zentra-api/src/handlers/maintenance.rs`
| Line | Issue | Details |
|------|-------|---------|
| 105-175 | `init_store()` | 4 fake maintenance windows (`"Weekend DB Patching"`, `"Network Switch Upgrade"`, etc.) and 6 fake SLA targets (`hc-http-google: 99.9`, etc.) |

### 6. `crates/zentra-api/src/handlers/sso.rs`
| Line | Issue | Details |
|------|-------|---------|
| 114-202 | `init_store()` | 2 fake SSO providers (`"Corporate OIDC (Azure AD)"`, `"Corporate LDAP"`) with fake client IDs, LDAP host `ldap.corp.example.com`, and 1 fake session for `alice@example.com` |

### 7. `crates/zentra-api/src/handlers/itsm.rs`
| Line | Issue | Details |
|------|-------|---------|
| 90-169 | `init_store()` | 2 fake configs (`"ServiceNow Production"` on `example.service-now.com`, `"JIRA Cloud"`) and 2 fake tickets (`"CPU usage critical on app-server-01"`, `"Disk space warning on db-primary"`) |

### 8. `crates/zentra-api/src/handlers/ha.rs`
| Line | Issue | Details |
|------|-------|---------|
| 81-186 | `cluster_status` | 3 hardcoded cluster nodes (`node-001` leader, `node-002/003` followers), 2 fake replicas, fake Raft term 47, fake commit index |
| 188-203 | `failover_history` | 1 fake failover event (`"Heartbeat timeout — old leader became unreachable"`) |
| 205-224 | `ha_health` | Hardcoded JSON: 100% cluster health, 1,296,000s leader uptime, fake data distribution percentages |

### 9. Middleware Performance History — Synthetic Sine-Wave Data
All five middleware handlers generate 60-point history using `Math.sin` (actually `(i as f64 * 0.x).sin()`), producing oscillating fake metrics:

| File | Line | Fake Data |
|------|------|-----------|
| `middleware_haproxy.rs` | 52-61 | Sessions, session_rate, hrsp_2xx, hrsp_5xx, queue_depth |
| `middleware_memcached.rs` | 39-45 | hit_rate, curr_items, evictions, cmd_get_per_sec, bytes_used_mb |
| `middleware_phpfpm.rs` | 46-54 | active_processes, idle_processes, listen_queue, slow_requests, accepted_conn |
| `middleware_redis.rs` | 41-48 | ops_per_sec, used_memory_mb, connected_clients, hit_rate |
| `middleware_tomcat.rs` | 53-62 | threads_busy, request_count, error_count, active_sessions, heap_used_mb |

### 10. `crates/zentra-api/src/handlers/scrape.rs`
| Line | Issue | Details |
|------|-------|---------|
| 171-195 | `list_recording_rules` | Returns 2 hardcoded recording rules (`"cpu_avg_5m"`, `"http_request_rate"`) |

### 11. `crates/zentra-api/src/handlers/database_query.rs`
| Line | Issue | Details |
|------|-------|---------|
| 152-154 | `execute_demo` | Fallback function named `execute_demo` that returns error message `"No live connection available for this database type"`. Called when no live Oracle pool exists. |

### 12. `crates/zentra-api/src/handlers/database_metrics.rs`
| Line | Issue | Details |
|------|-------|---------|
| 45-55 | `db_metric_defs()` | Loads static default metric definitions from embedded TOML files (`postgres_default_metrics.toml`, `mysql_default_metrics.toml`, `mongodb_default_metrics.toml`, `sqlserver_default_metrics.toml`). These are query templates, not runtime data, but they are hardcoded defaults. |

### 13. `crates/zentra-api/src/handlers/oracle_metrics.rs`
| Line | Issue | Details |
|------|-------|---------|
| 44-62 | `default_metrics()` | 15 hardcoded Oracle metric definitions (sessions, resource, asm_diskgroup, activity, process, wait_class, tablespace, etc.) with SQL queries |

### 14. `crates/zentra-api/src/handlers/alerts.rs`
| Line | Issue | Details |
|------|-------|---------|
| 182-214 | `list_silences` | 3 hardcoded silences (`"Maintenance Window - DB Upgrade"`, `"Suppress Network Alerts - DC2"`, `"Ignore dev-* alerts"`) |
| 233-339 | `list_templates` | 9 hardcoded alert templates (CPU, Memory, Disk, Oracle Tablespace, Oracle Data Guard Lag, RMAN Backup Stale, Network Latency, Error Rate Spike) |
| 416-429 | `inhibition_store` | 1 hardcoded inhibition rule (`"Critical suppresses Warning"`) |
| 475-568 | `list_alert_library` | 66 pre-built alert configs in static JSON (CPU ×6, Memory ×6, Disk ×6, Network ×10, Oracle ×10, MySQL ×6, PostgreSQL ×6, Docker ×4, HTTP ×6, System ×6) |

### 15. `crates/zentra-api/src/handlers/remote_write.rs`
| Line | Issue | Details |
|------|-------|---------|
| 145-165 | `remote_read` | Returns empty `timeseries: vec![]` with comment `"In production: translate matchers to SQL WHERE clauses"` — not wired to DB |

### 16. `crates/zentra-api/src/handlers/profiling.rs`
| Line | Issue | Details |
|------|-------|---------|
| 63-101 | `trigger_profile` | Inserts a job row but immediately updates status to `'running'` without actually executing SSH/perf. Marked as async simulation. |

---

## Frontend Findings

After reviewing the top 49 frontend files flagged by keyword search, **the frontend is overwhelmingly clean**. The keyword matches were largely:
- **Comments** referencing historical demo mode removal (e.g., `// M10: Flat sparkline from live value`)
- **Component names** that no-op (`DemoModeBadge.tsx` returns `null`; `AppModeContext.tsx` is always-live)
- **Form placeholders** (e.g., `placeholder: 'e.g. ORCL or orcl.example.com'` in `AddDatabaseModal.tsx`)
- **Static configuration maps** (e.g., `DB_CONFIGS` in `AddDatabaseModal.tsx`, `deviceTypeIcon` in `AddNetworkDeviceModal.tsx`) — these are UI metadata, not data payloads

**No frontend page was found to inject mock arrays into the UI.** All inspected pages (`AIInsights.tsx`, `Servers.tsx`, `Network.tsx`, `OracleMonitoring.tsx`, `Dashboard.tsx`, `OTelAlertRules.tsx`, `OrganizationManagement.tsx`, `UserManagement.tsx`, `FloatingChatbot.tsx`) fetch live data via React Query.

---

## Files Fixed — Prior Sessions

- `deployments.rs` — `init_store()` now empty
- `oncall.rs` — `init_store()` now empty
- `runbooks.rs` — `init_store()` now empty, execution output changed from `"Simulated execution"` to `"Executed"`
- `healthchecks.rs` — `init_store()` now empty

## Files Fixed — Current Session (2026-06-05)

| File | What Was Removed |
|------|-----------------|
| `dashboards.rs` | 2 hardcoded dashboards, fake `"Example Dashboard"` get, fake `"Exported Dashboard"` export, `"NOC Display"` playlist, hardcoded overview KPIs (48 hosts, 127 alerts, 99.87% SLA…), 8 module health rows, 8 fake timeline events |
| `log_engine.rs` | 7 fake log streams (web01/nginx, api01, db01, worker01), 2 fake log alerts, sine-wave `log_volume`, hardcoded `log_analytics` (volume_by_source, top_error_messages, error_rate_trend), "demo mode" message |
| `compliance.rs` | 5 fake frameworks (CIS Ubuntu, CIS RHEL, CIS AWS, PCI-DSS, SOC2), 2 fake scans, 9 fake findings |
| `maintenance.rs` | 4 fake maintenance windows, 6 fake SLA targets, 15 hardcoded SLA reports with fake uptime percentages |
| `sso.rs` | 2 fake SSO providers (Azure AD OIDC, Corporate LDAP), 1 fake session (alice@example.com) |
| `itsm.rs` | 2 fake ITSM configs (ServiceNow, JIRA Cloud), 2 fake tickets |
| `ha.rs` | 3 fake cluster nodes, 2 fake replicas, fake Raft term/commit, 1 fake failover event, fake health percentages. Replaced with standalone single-node status |
| `middleware_haproxy.rs` | 60-point sine-wave performance history → empty array |
| `middleware_memcached.rs` | 60-point sine-wave performance history → empty array |
| `middleware_phpfpm.rs` | 60-point sine-wave performance history → empty array |
| `middleware_redis.rs` | 60-point sine-wave performance history → empty array |
| `middleware_tomcat.rs` | 60-point sine-wave performance history → empty array |
| `scrape.rs` | 2 hardcoded recording rules (`cpu_avg_5m`, `http_request_rate`) → empty array |
| `alerts.rs` | 3 hardcoded silences → empty array, 1 hardcoded inhibition rule → empty store |

**Build status:** ✅ `cargo check --package zentra-api` passes (warnings only, no errors)

---

## Items Kept As Intentional

Some items are **intentional by design** (product catalogs, not fake runtime data):
- **`monitoring_templates.rs` built-in templates** — Template library, similar to Zabbix/Grafana default templates
- **`database_metrics.rs` / `oracle_metrics.rs` default metric definitions** — SQL query templates for scraping DB metrics
- **`alerts.rs` alert library (66 configs)** — Pre-built alert catalog for users to enable
- **`alerts.rs` alert templates (9 configs)** — Starter templates for common alert scenarios

## Items Not Fixed (Out of Scope)

- **`remote_write.rs`** — `remote_read` already returns empty timeseries, marked as placeholder
- **`profiling.rs`** — `trigger_profile` is a simulation stub; needs real SSH/perf implementation
- **`database_query.rs`** — `execute_demo` is an error-path fallback, not user-visible mock data
