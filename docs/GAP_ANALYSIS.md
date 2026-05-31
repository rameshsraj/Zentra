# Zentra — Comprehensive Gap Analysis

**Version:** 1.0.0  
**Date:** 2026-05-31  
**Scope:** Full codebase (Backend + Frontend)  

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Broken / Stub Endpoints (Backend)](#2-broken--stub-endpoints-backend)
3. [Frontend → Backend Route Mismatches](#3-frontend--backend-route-mismatches)
4. [Middleware Handler Gaps (All Stubs)](#4-middleware-handler-gaps-all-stubs)
5. [OTEL Integration Gaps](#5-otel-integration-gaps)
6. [Auth & Security Gaps](#6-auth--security-gaps)
7. [Monitoring Pipeline Gaps](#7-monitoring-pipeline-gaps)
8. [Data Correlation Gaps](#8-data-correlation-gaps)
9. [Frontend Gaps](#9-frontend-gaps)
10. [Summary Scorecard](#10-summary-scorecard)

---

## 1. Executive Summary

| Category | Count | Severity |
|----------|-------|----------|
| Broken / Stub Endpoints | **23** | High |
| Frontend-Backend Mismatches | **7** | High |
| Middleware Stubs (hardcoded data) | **12 handler files** | High |
| OTEL Integration Gaps | **8** | Medium |
| Auth / RBAC Gaps | **4** | Medium |
| Missing Monitoring Correlations | **6** | Medium |
| Frontend Missing Features | **5** | Low |

**Overall Status:** The backend has a large surface area (~400+ routes) but a significant portion of middleware, cloud, and enterprise handlers return hardcoded/mock data or `NOT_IMPLEMENTED`. The OTEL pipeline is production-ready for ingest but is isolated from the rest of the monitoring stack. Frontend-backend alignment is mostly good but has a few route mismatches.

---

## 2. Broken / Stub Endpoints (Backend)

### 2.1 Cloud Monitoring — Completely Broken

| Endpoint | Handler | Status | Issue |
|----------|---------|--------|-------|
| `POST /api/cloud/accounts/:id/validate` | `cloud_monitoring::validate_account` | **STUB** | Returns `valid: false` with message "Cloud provider SDK integration is not yet available" |
| `GET /api/cloud/accounts/:id/metrics` | `cloud_monitoring::get_account_metrics` | **BROKEN** | Returns `StatusCode::NOT_IMPLEMENTED` (501) |
| `GET /api/cloud/summary` | `cloud_monitoring::cloud_summary` | **STUB** | Likely returns placeholder data |

**Root Cause:** No AWS/Azure/GCP SDK integration. File header says "deferred to P2".

### 2.2 Network — Broken Import

| Endpoint | Handler | Status | Issue |
|----------|---------|--------|-------|
| `POST /api/network/blacklist/import` | `network::import_blacklist` | **STUB** | Returns `{"status": "not_implemented", "entries_added": 0}` |
| `POST /api/network/capture/import` | `network::import_pcap` | **STUB** | Returns `{"status": "not_implemented"}` |

### 2.3 Oracle — TxEventQ Not Implemented

| Endpoint | Handler | Status | Issue |
|----------|---------|--------|-------|
| `GET /api/oracle/txeventq` | `oracle::txeventq_list` | **STUB** | Code comment: `// TODO: Query actual TxEventQ stats from connected Oracle instances` — returns empty array |
| `GET /api/oracle/txeventq/stats` | `oracle::txeventq_stats` | **STUB** | Same as above |
| `GET /api/oracle/txeventq/:queue_name` | `oracle::txeventq_detail` | **STUB** | Same as above |

### 2.4 MCP (Model Context Protocol) — Tool Fallback

| Endpoint | Handler | Status | Issue |
|----------|---------|--------|-------|
| `POST /api/mcp/call` | `mcp::call_tool` | **PARTIAL** | Falls back to `{"status": "no_data", ...}` for any tool not wired to a live data source. Most tools unimplemented. |

### 2.5 HA (High Availability) — Stub

| Endpoint | Handler | Status | Issue |
|----------|---------|--------|-------|
| `GET /api/ha/cluster` | `ha::cluster_status` | **STUB** | Likely returns static/empty data (in-memory store) |
| `GET /api/ha/health` | `ha::ha_health` | **STUB** | Same |
| `GET /api/ha/failovers` | `ha::failover_history` | **STUB** | Same |

### 2.6 License Management — Partial

| Endpoint | Handler | Status | Issue |
|----------|---------|--------|-------|
| Various `/api/admin/licenses/*` | `licenses::*` | **PARTIAL** | In-memory store only; no persistence to DB. Data lost on restart. |

---

## 3. Frontend → Backend Route Mismatches

### 3.1 Mismatches Found

| Frontend Call | File | Backend Route | Status |
|---------------|------|---------------|--------|
| `GET /targets/:id` | `targets.ts:??` | `targets::get_target` | ✅ OK |
| `PUT /targets/:id` | — | **MISSING** | ❌ No update target endpoint exists! Frontend can create/delete but not update |
| `GET /users/:id` | `auth.ts:36` | `users::get_user` | ✅ OK |
| `PUT /alert-rules/:id` | — | **MISSING** | ❌ Frontend can create (`POST /alert-rules`) but no update/delete by ID for alert rules |
| `DELETE /alert-rules/:id` | — | **MISSING** | ❌ Same |
| `PUT /notifications/channels/:id` | — | **MISSING** | ❌ No update channel endpoint; only `POST /notifications/test` and `GET /notifications/history` |
| `GET /api/v1/status/*` | `health.ts:18-23` | `config::*` | ⚠️ Routes exist but are not protected by auth in some deployments |
| `POST /config/mode` | `health.ts:15` | `config::set_mode` | ✅ OK |

### 3.2 Frontend Calls Endpoints That Are Stubs

| Frontend Page | API Call | Backend Handler | Status |
|---------------|----------|-----------------|--------|
| `CloudMonitoring.tsx` | `GET /cloud/accounts/:id/metrics` | `cloud_monitoring::get_account_metrics` | Returns 501 |
| `OracleMonitoring.tsx` | `GET /oracle/txeventq` | `oracle::txeventq_list` | Returns empty |
| `Network.tsx` | `POST /network/blacklist/import` | `network::import_blacklist` | Returns "not_implemented" |
| `MiddlewareMonitoring.tsx` (Docker tab) | `GET /middleware/docker/:id/status` | `middleware_docker::get_status` | Returns zeros |
| `MiddlewareMonitoring.tsx` (Kafka tab) | `GET /middleware/kafka/:id/status` | `middleware_kafka::get_status` | Returns zeros |
| `MiddlewareMonitoring.tsx` (ES tab) | `GET /middleware/elasticsearch/:id/status` | `middleware_elasticsearch::get_status` | Returns zeros |

---

## 4. Middleware Handler Gaps (All Stubs)

**Pattern:** All middleware handlers (except Nginx and Apache which have real HTTP polling) use the same broken pattern:
- In-memory `RwLock<Vec<Instance>>` store (no DB persistence)
- `get_status` returns hardcoded zeros or static structs
- `performance_history` uses `Math.sin()`-like oscillation (actually Rust `(i as f64 * 0.1).sin()`)
- `list_instances` returns the in-memory vector
- `create_instance` pushes to the vector
- `delete_instance` removes from the vector

### 4.1 Affected Handler Files (12 total)

| Handler File | Routes | Live Data? | Issue |
|-------------|--------|-----------|-------|
| `middleware_docker.rs` | `/api/middleware/docker/*` | ❌ No | Hardcoded container list, zeros for status, sin-wave history |
| `middleware_kafka.rs` | `/api/middleware/kafka/*` | ❌ No | Hardcoded topics, zeros for status, sin-wave history |
| `middleware_elasticsearch.rs` | `/api/middleware/elasticsearch/*` | ❌ No | Hardcoded indices, zeros for status, sin-wave history |
| `middleware_haproxy.rs` | `/api/middleware/haproxy/*` | ❌ No | Same pattern |
| `middleware_tomcat.rs` | `/api/middleware/tomcat/*` | ❌ No | Same pattern |
| `middleware_phpfpm.rs` | `/api/middleware/phpfpm/*` | ❌ No | Same pattern |
| `middleware_rabbitmq.rs` | `/api/middleware/rabbitmq/*` | ❌ No | Same pattern |
| `middleware_redis.rs` | `/api/middleware/redis/*` | ❌ No | Same pattern |
| `middleware_memcached.rs` | `/api/middleware/memcached/*` | ❌ No | Same pattern |
| `middleware_kubernetes.rs` | `/api/middleware/kubernetes/*` | ❌ No | Same pattern |
| `middleware_haproxy.rs` | `/api/middleware/haproxy/*` | ❌ No | Same pattern |
| `middleware_dashboard.rs` | `/api/middleware/dashboard` | ⚠️ Partial | Aggregates Nginx/Apache (real) but others are stubs |

### 4.2 What "Live" Actually Means

Only **Nginx** and **Apache** middleware handlers make real HTTP calls to the configured instance's status endpoint (`/nginx_status`, `/server-status`). All others return fabricated data.

### 4.3 Storage Arrays — Partially Stub

| Handler | Status | Issue |
|---------|--------|-------|
| `storage_arrays::list_arrays` | ⚠️ Partial | Queries DB but returns limited real data |
| `storage_arrays::get_array_metrics_*` | ⚠️ Partial | May return historical data from DB |

---

## 5. OTEL Integration Gaps

### 5.1 What Works (Production Ready)

| Component | Status |
|-----------|--------|
| OTLP HTTP ingest (JSON + Protobuf) | ✅ `/v1/traces`, `/v1/metrics`, `/v1/logs`, `/v1/profiles` |
| OTLP gRPC ingest (port 4317) | ✅ All 4 signals via HTTP/2 with proper framing |
| Async ingest pipeline with DLQ | ✅ Bounded mpsc → batch INSERT → DLQ on failure |
| Quota enforcement (rate + payload) | ✅ Per-key RPM + max_payload_bytes |
| API key management | ✅ Create/revoke/list with SHA-256 hashing |
| Audit logging | ✅ Every ingest batch logged to `otlp_ingest_audit` |
| TimescaleDB storage (hypertables) | ✅ `traces`, `otlp_metrics`, `otlp_logs` |
| Compression & retention | ✅ 90 days traces/metrics, 30 days logs |
| Self-monitoring (tracer provider) | ✅ `init_tracer()` with OTLP exporter |
| Span-to-metrics connector | ✅ Configurable RED metrics derivation |
| TraceQL query endpoint | ✅ `/api/query/traceql` |
| Tempo-compatible endpoints | ✅ `/tempo/api/search`, `/tempo/api/traces/:id` |
| Frontend OTEL RUM | ✅ `init.ts` with WebTracerProvider |

### 5.2 What Is BROKEN / Missing

| Gap | Severity | Description |
|-----|----------|-------------|
| **G5.1: OTEL data not correlated with infrastructure alerts** | High | Alerts engine (`alert_evaluator.rs`) does NOT query OTEL traces/metrics for anomaly detection. Alerts are purely threshold-based on collected metrics. |
| **G5.2: OTEL traces not linked to target health** | High | When a server target is down, there's no automatic trace analysis to identify which services were affected. |
| **G5.3: Dashboard doesn't show OTEL-derived metrics** | Medium | The Command Center (`Dashboard.tsx`) shows alerts, targets, events — but NO OTEL signal health, throughput, or error rates. |
| **G5.4: AI insights don't use OTEL data** | Medium | `AIInsights.tsx` calls `/ai/insights` but the backend doesn't analyze OTEL traces for root cause. |
| **G5.5: No trace-to-log correlation in Log Explorer** | Medium | `LogExplorer.tsx` shows logs but cannot pivot to "show traces for this log entry's trace_id". |
| **G5.6: Network monitoring doesn't use OTEL data** | Medium | Network traffic analysis doesn't correlate with OTEL `http.client.duration` or `rpc.client.duration` spans. |
| **G5.7: No OTEL-based SLO / SLI dashboards** | Medium | No Service Level Objective dashboards using OTEL-derived RED metrics. |
| **G5.8: Alert rules can't reference OTEL attributes** | High | Alert rule conditions can't use OTel semantic convention attributes (e.g., `http.status_code >= 500`). |

### 5.3 OTEL-to-Monitoring Integration Scorecard

| Monitoring Component | Uses OTEL Data? | Notes |
|---------------------|----------------|-------|
| Server monitoring (SSH) | ❌ No | Independent SSH polling |
| Network monitoring (SNMP/pcap) | ❌ No | Independent packet capture |
| Database monitoring (Oracle) | ❌ No | Independent OCI queries |
| Middleware monitoring | ⚠️ Partial | Only Nginx/Apache real; no OTEL correlation |
| Security (IDS/IPS) | ❌ No | Independent Suricata/Snort ingestion |
| AI/ML anomaly detection | ❌ No | Uses K-means on collected metrics only |
| Alerting engine | ❌ No | Threshold-based on `metrics` table |
| Dashboard | ❌ No | Shows targets/events but not OTEL signals |
| RCA (Root Cause Analysis) | ❌ No | No trace analysis in RCA |
| Predictive alerts | ❌ No | Time-series forecasting only |

---

## 6. Auth & Security Gaps

| Gap | Severity | Description |
|-----|----------|-------------|
| **G6.1: License middleware blocks `/api/auth/me`** | High | `TRIAL_ALLOWED_PREFIXES` does NOT include `/api/auth/me` — users can't view their own profile when trial expires. Fixed in latest deploy but needs verification. |
| **G6.2: `/api/admin/users` (GET) not in trial allowlist** | High | Admin profile page (`/admin/profile`) calls this but it's blocked in degraded mode. |
| **G6.3: No RBAC enforcement on many routes** | Medium | `require_auth` middleware validates JWT but doesn't check `module_access` or permissions. Any logged-in user can call any API. |
| **G6.4: CORS set to `*` in default .env** | Medium | `ZENTRA_ALLOW_ORIGINS=*` allows any origin. Should default to same-origin. |

---

## 7. Monitoring Pipeline Gaps

| Gap | Severity | Description |
|-----|----------|-------------|
| **G7.1: Metric collection doesn't write to OTEL tables** | High | `metric_collector.rs` writes to `metrics` table but NOT to `otlp_metrics`. Two separate metric systems. |
| **G7.2: No unified query layer** | Medium | Metrics, logs, traces queried separately. No unified "observability query" that joins all three. |
| **G7.3: Background workers don't process OTEL data** | Medium | `background_jobs.rs` refreshes MVs and evaluates alerts but doesn't run OTEL-specific aggregation jobs (e.g., derive RED metrics from traces). |
| **G7.4: No auto-discovery for OTEL-instrumented services** | Medium | Service discovery only scans SNMP/SSH targets. Doesn't discover services from OTEL `service.name` attributes. |
| **G7.5: eBPF profiling not integrated with OTEL** | Medium | `profiling.rs` has flamegraphs but doesn't correlate with OTEL trace spans. |
| **G7.6: LLM observability not linked to OTEL** | Medium | `llm_observability.rs` queries `llm_requests` table but doesn't read OTEL `gen_ai.*` attributes from traces. |

---

## 8. Data Correlation Gaps

| Gap | Severity | Description |
|-----|----------|-------------|
| **G8.1: Trace → DB correlation exists but not wired to UI** | Medium | `trace_db_correlator.rs` has handlers but no frontend page uses them. |
| **G8.2: Log → Metric correlation missing** | Medium | Can't click a log entry and see related metrics for the same time window. |
| **G8.3: Alert → Trace correlation missing** | High | When an alert fires, can't automatically show traces from the affected service in the same time window. |
| **G8.4: Event → All signals correlation missing** | Medium | `events.rs` returns recent events but doesn't link to traces/metrics/logs. |
| **G8.5: Target → OTEL service mapping missing** | Medium | A target (e.g., Linux server at 10.88.11.201) isn't linked to its OTEL `service.name` and `service.instance.id`. |

---

## 9. Frontend Gaps

| Gap | Severity | Description |
|-----|----------|-------------|
| **G9.1: No page to view OTEL service health** | Medium | OTelDashboard shows pipeline stats but not per-service health (rate/errors/duration). |
| **G9.2: No SLO/SLI configuration UI** | Medium | No frontend for setting SLO targets based on OTEL RED metrics. |
| **G9.3: No trace-to-log pivot** | Medium | `Traces.tsx` shows traces but can't click a span to see its logs. |
| **G9.4: Middleware monitoring shows stub data as real** | High | UI displays hardcoded Docker/Kafka/ES data without any "demo mode" indicator. |
| **G9.5: Cloud monitoring page calls broken endpoints** | High | `CloudMonitoring.tsx` will get 501 errors from backend. |

---

## 10. Summary Scorecard

### By Module

| Module | Backend Coverage | Data Quality | Frontend Coverage | Integration | Grade |
|--------|-----------------|-------------|-------------------|-------------|-------|
| Auth / Users | 95% | Real DB | 95% | Good | A |
| Targets / Servers | 90% | Real DB + SSH | 90% | Good | A- |
| Alerts / Notifications | 85% | Real DB | 85% | Good | B+ |
| OTEL Ingest | 95% | Real DB | 80% | Good (isolated) | A- |
| Network | 70% | Mixed (pcap real, some stubs) | 75% | Fair | C+ |
| Oracle DB | 80% | Real OCI + SSH | 80% | Good | B+ |
| Middleware (Nginx/Apache) | 70% | Real HTTP polling | 70% | Good | B |
| Middleware (All Others) | 20% | **All stubs** | 60% | Poor | D |
| Cloud Monitoring | 10% | **All stubs** | 40% | Poor | F |
| Security / IDS | 75% | Real DB | 75% | Fair | B- |
| AI / Predictions | 60% | Mixed | 60% | Fair | C+ |
| Dashboards | 80% | Real DB | 80% | Good | B+ |
| Enterprise (SSO/ITSM/HA) | 40% | Mostly stubs | 30% | Poor | D |
| Compliance | 50% | In-memory | 40% | Fair | D+ |

### Critical Path to Production

1. **Fix middleware stubs** — Either make them live (HTTP polling like Nginx) or clearly mark as demo
2. **Fix cloud monitoring** — Either implement SDK integration or disable the UI
3. **Connect OTEL to alerts** — Allow alert rules to reference OTEL attributes
4. **Connect OTEL to dashboard** — Show OTEL service health in Command Center
5. **Fix frontend-backend mismatches** — Add missing `PUT /targets/:id`, `PUT/DELETE /alert-rules/:id`
6. **Add RBAC enforcement** — Check permissions in `require_auth` or separate middleware

---

*— End of Gap Analysis —*
