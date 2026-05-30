# Zentra — Mock Data Gap Analysis

> **Scope:** Deep analysis of the entire codebase (frontend `frontend/src`, backend `crates/`, DB `migrations/`, Docker `docker/`) to locate every instance of mock, demo, hardcoded, synthetic, or placeholder data that is surfaced to the user as if it were live.
>
> **Goal:** Achieve a "no mock data" guarantee — every value rendered in the UI must originate from a live API call, the database, or real infrastructure collection.
>
> **Date:** 2026-05-30
> **Method:** Static analysis (grep + targeted reads). No runtime profiling.

---

## 1. Executive Summary

| Area | Severity | Count | Status |
|------|----------|-------|--------|
| Frontend pages rendering hardcoded business data | **HIGH** | 3 | Mock shown as live |
| Frontend pages with empty placeholder arrays (API exists, not wired) | **MEDIUM** | 2 | Always empty |
| Frontend synthetic sparkline visuals | **LOW** | 8 files | Cosmetic |
| Backend placeholder/stub responses | **MEDIUM** | 3 | Hardcoded values |
| Backend self-audit reporting "clean" (stale/misleading) | **MEDIUM** | 1 | Incorrect claim |
| DB seed/demo target rows | **MEDIUM** | 1 | Cleaned only in Docker |

**Key finding:** The backend already exposes the correct live endpoints (`/api/log-sources`, `/api/ai/insights`, `/api/alert-rules`, `/api/notifications/channels`, `/api/notifications/history`), **but several frontend pages ignore them and render hardcoded arrays instead.** This is the primary gap and is fully fixable on the frontend.

---

## 2. HIGH Severity — Frontend Mock Data Rendered as Live

### 2.1 `frontend/src/pages/AIInsights.tsx`
The entire page is driven by hardcoded constants. **No API call is made.**

- `mlModels` (lines 14–20) — 5 fake ML models with fixed accuracy/precision values.
- `perfData` (lines 23–28) — 24h synthetic CPU/memory/latency via `Math.sin`.
- `anomalyData` (lines 30–35) — 7-day synthetic anomaly counts.
- `capacityData` (lines 37–42) — 30-day synthetic capacity forecast.
- `costData` (lines 44–49) — fixed compute/storage/network/database cost figures.
- `insights` (lines 51–56) — 4 fake insight cards with fabricated hostnames (`prod-web-03`, `db-primary`).

Inline comment at line 22 admits: *"Static representative data — will be replaced by real API calls once ML engine is live"*.

- **Live endpoint available:** `getAIInsights()` → `/ai/insights` (`frontend/src/lib/api/ai.ts:14`).
- **Impact:** Users see fabricated AI insights, cost savings, and forecasts that do not reflect their infrastructure.

### 2.2 `frontend/src/pages/LogSources.tsx`
- `mockSources` (lines 22–29) — 6 fake log sources with hardcoded IPs (`10.88.11.162`, `10.88.11.163`), fake log counts, and fake timestamps.
- Page state is `useState(mockSources)` (line 51) — Add/Delete only mutate local React state and are **never persisted**.
- **Live endpoints available:** `getLogSources()` / `createLogSource()` / `deleteLogSource()` → `/log-sources` (`frontend/src/lib/api/logs.ts:11–15`).
- **Impact:** The Log Sources page is entirely non-functional against the backend; data vanishes on refresh.

### 2.3 `frontend/src/pages/RCA.tsx`
- `mockHistory` (lines 27–77) — 2 fully fabricated RCA results (nginx + Oracle errors) with fake solutions and Stack Overflow URLs.
- Initial state seeds the results list with this mock: `useState<RCAResult[]>(mockHistory)` (line 94).
- *Partial:* new analyses **do** call the real `analyzeRCA()` API (line 109), but the page always opens pre-populated with fake history.
- **Impact:** Users see fabricated historical RCA reports they never ran.

---

## 3. MEDIUM Severity — Empty Placeholder Arrays (API Exists, Not Wired)

### 3.1 `frontend/src/pages/AlertRules.tsx`
- `mockRules: AlertRule[] = []` (line 45), then `const liveRules = mockRules` (line 62) — the page **never** consumes live data; all counts (firing/pending/enabled) are derived from a permanently empty array.
- `alertTemplates` (lines ~38–43) — these are legitimate built-in templates (acceptable), but the actual rules list is dead.
- **Live endpoint available:** `getAlertRules()` → `/alert-rules` (`frontend/src/lib/api/alerts.ts:14`).

### 3.2 `frontend/src/pages/NotificationChannels.tsx`
- `channels: NotifChannel[] = []` (line 34), `policies: NotifPolicy[] = []` (line 47), `mockHistory = []` (line 51) — all three are permanently empty and never fetched.
- **Live endpoints available:** `getNotificationChannels()` → `/notifications/channels`, `getNotificationHistory()` → `/notifications/history` (`frontend/src/lib/api/alerts.ts:39,44`).

---

## 4. LOW Severity — Synthetic Sparkline Visuals (Cosmetic)

A `spark()` helper that generates a decorative sparkline shape from a base value using `Math.sin` appears in:

- `frontend/src/pages/Dashboard.tsx:28`
- `frontend/src/pages/Servers.tsx:12`
- `frontend/src/pages/Network.tsx:30`
- `frontend/src/pages/Storage.tsx:10`
- `frontend/src/pages/AlertRules.tsx:12`
- `frontend/src/pages/NotificationChannels.tsx:14`
- `frontend/src/pages/AIInsights.tsx:11`
- `frontend/src/pages/security/SecurityDashboard.tsx:7`

These do **not** fabricate KPI values (the headline numbers are real where the page is wired), but the mini-trend lines are synthetic shapes, not real time-series. **Recommendation:** replace with real sparkline series from metrics history, or remove the trend line where no series exists. Lower priority than Sections 2–3.

---

## 5. MEDIUM Severity — Backend Placeholder / Stub Responses

### 5.1 `crates/zentra-api/src/handlers/file_analysis.rs:81`
```rust
let verdict = "clean"; // placeholder
```
File scan verdict is hardcoded to `"clean"` and inserted into `file_scans`. No real YARA/VirusTotal scanning occurs. Inline comment confirms intent. **Impact:** Security feature reports all files as clean regardless of content.

### 5.2 `crates/zentra-api/src/handlers/llm_observability.rs:99`
```rust
"models": 0, // Placeholder or query count(distinct model)
```
The distinct-model count is hardcoded to `0` instead of querying the database.

### 5.3 `crates/zentra-api/src/handlers/database_query.rs:152` (`execute_demo`)
```rust
fn execute_demo(...) -> ... { (vec![], vec![], 0, None, Some("No live connection available...".into())) }
```
This is an **acceptable** no-connection fallback (returns an explicit error, not fake rows). Listed for completeness — **no action required** beyond renaming for clarity.

---

## 6. MEDIUM Severity — Misleading Self-Audit

### `crates/zentra-api/src/handlers/data_audit.rs`
- `data_source_audit` (lines 10–62) hardcodes **every** module as `"live"`, including `AI Insights → "ML engine predictions"` and `Logs → "Log ingestion engine"`, which contradicts the mock data found in Sections 2.1 and 2.2.
- `mock_data_check` (lines 74–103) returns `"status": "clean", "mock_patterns_found": 0`.
- **Impact:** This endpoint gives false assurance that the system is mock-free. It must be corrected (or made to dynamically verify) after remediation, otherwise it actively hides the gaps.

---

## 7. MEDIUM Severity — Database Seed / Demo Data

### 7.1 `migrations/20250101000016_seed_databases.sql`
Seeds an Oracle target row (`...0202`, `Oracle ORCL (Prod)`, host `10.88.11.162`).

- **Docker:** `docker/docker-entrypoint.sh` deletes targets `...0201` and `...0202` on first boot (`cleanup-seeds.sh`), so the Docker image ships clean.
- **Non-Docker / dev deploy:** the seed row persists and appears as a real target.
- **Impact:** Inconsistent state between Docker and bare-metal deploys; a hardcoded prod IP is embedded in a migration.

### 7.2 Legitimate seed data (NO action required)
- `20250101000012_auth_system.sql` / `20250101000015_auth_fix.sql` — seed the default admin user, roles, permissions, and system settings. These are **required bootstrap data**, not mock data.
- `20250101000014_oracle_monitoring.sql` — seeds default Oracle metric **definitions** (SQL templates), which are product configuration, not fabricated metrics.
- `20250101000009_dashboard_enhance.sql` — default dashboard folders (product config).

---

## 8. Confirmed Clean / Intentional

- `crates/zentra-api/src/handlers/config.rs:17–27` — Demo mode is **permanently disabled** (`is_demo_mode()` always returns `false`). This confirms the project's intent to be live-only and is the correct baseline.
- `crates/zentra-api/src/state.rs` — `demo_mode` defaults to `false` unless `ZENTRA_DEMO_MODE=true` is explicitly set.
- `storage_arrays.rs` `list_volumes` / `list_replications` return empty vectors (honest empty state, not fake rows).

---

## 9. Remediation Priority

1. **P0 (HIGH):** Wire `AIInsights.tsx`, `LogSources.tsx`, `RCA.tsx` to their existing live endpoints; delete all mock constants. (Section 2)
2. **P1 (MEDIUM):** Wire `AlertRules.tsx` and `NotificationChannels.tsx` to live endpoints; remove empty placeholder arrays. (Section 3)
3. **P1 (MEDIUM):** Fix backend placeholders — real file-scan verdict, real model count. (Section 5.1, 5.2)
4. **P1 (MEDIUM):** Make `data_audit.rs` reflect reality (or compute dynamically). (Section 6)
5. **P2 (MEDIUM):** Gate or remove `seed_databases.sql`; remove hardcoded prod IP. (Section 7.1)
6. **P3 (LOW):** Replace synthetic `spark()` visuals with real series or remove. (Section 4)

> Detailed, copy-pasteable remediation steps are in **`MOCK_DATA_TODO_PROMPTS.md`**.

---
---

# Part B — OTEL (OpenTelemetry) Functionality Gap Analysis

> **Scope:** Deep review of the OTLP ingest pipeline (`handlers/otlp.rs`, `ingest_queue.rs`, `otlp_proto.rs`, `grpc.rs`, `otlp_quota.rs`), the OTEL query/read endpoints, auth, and the trace/metric/log schema (`migrations/`). Goal: find missing logic and broken functionality, not just mock data.
>
> **Headline:** The OTEL **read/UI layer is solid** (frontend pages are wired to live `/api/otel/*` endpoints with proper empty states, RED materialized view is refreshed by a background job). However, the **write/ingest layer has multiple critical, data-losing bugs**: gRPC silently discards everything, traces are stored without `org_id`, and metric ingestion fails against the actual schema. In practice, **OTLP metrics ingestion is broken and ingested traces are invisible to all org-scoped queries.**

## B1. Severity Summary

| ID | Area | Severity | Effect |
|----|------|----------|--------|
| OTEL-1 | gRPC ingest (`grpc.rs`) | **CRITICAL** | All gRPC-delivered telemetry silently dropped |
| OTEL-2 | Traces insert missing `org_id` | **CRITICAL** | Ingested traces invisible to every org-scoped query |
| OTEL-3 | Metrics insert vs schema mismatch | **CRITICAL** | OTLP metric ingestion fails → DLQ; no metrics stored |
| OTEL-4 | Protobuf metrics lose data points | **HIGH** | Protobuf/gRPC metrics carry no values |
| OTEL-5 | Histogram/Summary metric types dropped | **HIGH** | Only gauge/sum handled; histograms lost |
| OTEL-6 | Custom alert rules never evaluated | **HIGH** | User-created OTEL alert rules are dead |
| OTEL-7 | SQL injection in `otel_attribute_values` | **HIGH (security)** | Unparameterized `key` interpolated into SQL |
| OTEL-8 | GenAI model field never populated | **MEDIUM** | GenAI breakdown by model is always null |
| OTEL-9 | Profiles flamegraph never built | **MEDIUM** | Profiles stored with empty flamegraph |
| OTEL-10 | Connectors config is a no-op | **MEDIUM** | Enable/disable/dimensions not persisted |
| OTEL-11 | Pipeline topology `flow` always empty | **MEDIUM** | No edges in pipeline graph |
| OTEL-12 | `last_used_at` never updated | **MEDIUM** | API-key usage tracking dead |
| OTEL-13 | org_id not filtered in several reads | **MEDIUM (security)** | Cross-tenant data exposure |
| OTEL-14 | No single-trace / waterfall endpoint | **MEDIUM (missing feature)** | Span events/links stored but unusable |
| OTEL-15 | `otel_status` reports stale capabilities | **LOW** | Says gRPC "planned" though enabled by default |

## B2. CRITICAL

### OTEL-1 — gRPC ingest silently drops all data
**File:** `crates/zentra-api/src/grpc.rs:132-224`
The four gRPC handlers (`grpc_export_traces/metrics/logs/profiles`) decode the protobuf, `info!`-log a count, and return a success `ExportServiceResponse` — but **never enqueue the batch** to `state.ingest_queue`. Compare with the HTTP handlers in `otlp.rs:147-157` which do enqueue.
- The `grpc` feature is **on by default** (`crates/zentra-api/Cargo.toml:64`), so the server listens on `:4317` and ACKs every export while discarding it.
- **Effect:** Any SDK/Collector using the standard OTLP/gRPC exporter loses 100% of its data, with no error returned.

### OTEL-2 — Ingested traces are stored without `org_id`
**File:** `crates/zentra-api/src/ingest_queue.rs:391-408`
`insert_traces_batch` inserts columns `(trace_id, span_id, parent_span_id, operation_name, service_name, started_at, duration_us, status, tags)` — **`org_id` is omitted**, so it defaults to `NULL` (the column is nullable per `migrations/20250101000070_traces_base_schema.sql:15`).
- Every read query filters `WHERE org_id = $1` (`otel_stats`, `otel_resources`, `otel_span_metrics`, `otel_genai`, `otel_resource_signals`).
- **Effect:** HTTP-ingested spans are written but **never returned** by any org-scoped endpoint — the Telemetry Health dashboard, RED metrics, resources, and GenAI views all show zero despite data being present.

### OTEL-3 — Metric insert does not match the `metrics` schema
**Files:** `crates/zentra-api/src/ingest_queue.rs:501-513` vs `migrations/20250101000001_schema.sql:40-52`
The insert targets `metrics (org_id, metric_name, value, time, resource_id, labels)`, but the real table is:
```
timestamp TIMESTAMPTZ NOT NULL, org_id, target_id UUID NOT NULL,
plugin_id NOT NULL, resource_type NOT NULL, resource_id, metric_name, value, unit, tags, metadata
```
- The insert uses a non-existent `time` column (real column is `timestamp`) and a non-existent `labels` column (real column is `tags`).
- It omits the `NOT NULL` columns `target_id`, `plugin_id`, `resource_type`.
- **Effect:** The INSERT errors on every batch → 3 retries → **DLQ**. No OTLP metrics are ever stored. (`otel_stats.metrics_per_minute` will always be 0.)

## B3. HIGH

### OTEL-4 — Protobuf metrics decode discards data points
**File:** `crates/zentra-api/src/otlp_proto.rs:464-489`
`proto_to_json_metrics` emits only `{name, description, unit}` per metric — it never serializes `gauge/sum/histogram` `dataPoints`. The downstream `insert_metrics_batch` then finds no data points. **Effect:** even if OTEL-3 is fixed, protobuf/gRPC metrics still carry zero values; only HTTP/JSON metrics have points.

### OTEL-5 — Only gauge & sum metric types are handled
**File:** `crates/zentra-api/src/ingest_queue.rs:465-467`
`insert_metrics_batch` reads `metric.gauge` or `metric.sum` only. `histogram`, `exponentialHistogram`, and `summary` are silently ignored. **Effect:** the most common SDK metrics (request duration histograms) are dropped.

### OTEL-6 — Custom OTEL alert rules are never evaluated
**File:** `crates/zentra-api/src/handlers/otlp.rs:951-986`
`evaluate_alert_rules_internal` computes 4 **hardcoded** built-in evaluations and ignores the `otel_alert_rules` table entirely (inline comment admits: *"In a real implementation, we would check against defined rules"*). **Effect:** rules created via `POST /api/otel/alert-rules` are stored but never fire; thresholds/severity/window are not honored.

### OTEL-7 — SQL injection in attribute-values endpoint
**File:** `crates/zentra-api/src/handlers/otlp.rs:468-472`
`otel_attribute_values` interpolates the user-supplied `params.key` directly into the SQL string (`tags->>'{key}'` and `tags ? '{key}'`) with no parameterization or escaping. **Effect:** authenticated SQL injection. Must use bound parameters / jsonb operators with `$n` binds.

## B4. MEDIUM

### OTEL-8 — GenAI model dimension always null
**File:** `otlp.rs:1010-1022` vs ingest. The GenAI query reads `attributes->>'gen_ai.request.model'`, but `insert_traces_batch` only writes span attributes into the `tags` column, never the `attributes` column (which defaults `{}`). The generated `gen_ai_system` column derives from `tags`, so grouping by system works, but `model` is always null. **Fix:** read from `tags` (or populate `attributes`).

### OTEL-9 — Profiles flamegraph never constructed
**File:** `ingest_queue.rs:585-644`. `insert_profiles_batch` stores `flamegraph_data = []` and `attributes = {}` unconditionally; only a sample count is derived. The flamegraph tree is never built from `profile.samples`. **Effect:** the Profiles UI has no flamegraph to render.

### OTEL-10 — Connector configuration is a no-op
**File:** `handlers/otel_connectors.rs:51-77`. `list_connectors` returns a hardcoded single connector; `configure_connector` echoes the request back without persisting. The frontend toggle (`OTelConnectors.tsx`) appears to work but has no effect across reloads. **Fix:** persist connector config to a table and honor `enabled`/`dimensions`/`interval`.

### OTEL-11 — Pipeline topology has no edges
**File:** `otlp.rs:776-779`. `otel_pipeline` returns `"flow": []` always; collectors are detected by name heuristic but no app→collector→backend edges are produced. **Effect:** the topology graph cannot show real flow.

### OTEL-12 — API key `last_used_at` never updated
**File:** `otlp.rs:validate_otlp_key` (77-115). On a cache hit or DB hit, `last_used_at` is not written. The column and the Security UI display it, but it is always null/stale.

### OTEL-13 — Missing org scoping on several reads
**Files:** `otel_attributes` (439-448), `otel_attribute_values` (468-476), `otel_pipeline` (757-768). These query `traces` without `WHERE org_id = $1`. **Effect:** in a multi-tenant deployment, attribute keys/values and collector lists leak across orgs.

### OTEL-14 — No single-trace / waterfall retrieval
There is no `GET /api/otel/traces/:trace_id` endpoint. `span_events` and `span_links` are ingested and stored (`ingest_queue.rs:410-438`) but **no endpoint ever reads them**, so a trace waterfall / span-detail view cannot be built. This is a missing core tracing feature.

## B5. LOW

### OTEL-15 — `otel_status` advertises stale capabilities
**File:** `otlp.rs:303-327`. Reports `grpc.status = "planned"` and `tls_enabled = false`, although the gRPC server is compiled and started by default. Misleading to operators (and, given OTEL-1, gRPC is actually worse than "planned" — it ACKs and drops).

## B6. Confirmed Working (no action)
- Frontend OTEL pages (`OTelDashboard`, `OTelSpanMetrics`, `OTelResources`, `OTelConnectors`, `OTelPipeline`, `OTelAlertRules`, `OTelSecurity`) all fetch live `/api/otel/*` data and render proper empty states — **no mock data**.
- HTTP/JSON trace ingest path parses spans, events, and links correctly (modulo OTEL-2 org_id).
- RED materialized view `span_red_metrics_1m` is refreshed by a background job (`background_jobs.rs:67`).
- OTLP API-key auth, RPM rate limiting, and payload-size enforcement (`otlp_quota.rs`) are real and functional.
- DLQ + ingest audit (`otlp_dead_letter`, `otlp_ingest_audit`) are wired and working.

## B7. OTEL Remediation Priority
1. **P0:** OTEL-1 (gRPC enqueue), OTEL-2 (traces org_id), OTEL-3 (metrics schema) — without these, gRPC and metrics ingest are non-functional.
2. **P0/security:** OTEL-7 (SQL injection).
3. **P1:** OTEL-4, OTEL-5 (metric fidelity), OTEL-6 (alert rule evaluation).
4. **P2:** OTEL-8..OTEL-14.
5. **P3:** OTEL-15.

> Copy-pasteable remediation steps are appended to **`MOCK_DATA_TODO_PROMPTS.md`** as prompts **O01–O15**.

---

## B8. OpenTelemetry Conformance Verdict (vs the CNCF definition)

Measured against: *"a consistent, vendor-neutral format for instrumenting, generating, gathering, and exporting the three pillars of observability — metrics, logs, and traces — to any compatible monitoring backend."* Zentra's role is the **compatible backend** (OTLP receiver).

**Verdict: Partially conformant — currently NOT met.** The intent and surface area are right (OTLP HTTP JSON/protobuf + gRPC, all four signals, API-key auth, collector-config generation, live UI), but the three-pillar promise is broken in practice:

| Requirement | Status | Blockers |
|-------------|--------|----------|
| Vendor-neutral OTLP (HTTP JSON/protobuf, gRPC) | ⚠️ Partial | gRPC drops data (OTEL-1); gzip unsupported (new O17); non-standard custom ingest exists (new O19) |
| Reachable by stock SDKs/Collectors | ❌ No | Hardened Docker exposes no OTLP endpoint — nginx doesn't proxy `/v1/`, ports 4317/4318 not exposed (new O16); SDK snippets point at wrong endpoint `localhost:3001` (new O21) |
| Traces pillar | ⚠️ Partial | Stored without `org_id` → invisible (OTEL-2) |
| Metrics pillar | ❌ No | Insert fails vs schema → DLQ (OTEL-3); protobuf has no points (OTEL-4); histograms dropped (OTEL-5) |
| Logs pillar | ⚠️ Partial | `trace_id`/`span_id` dropped → no correlation (new O18) |
| Correlated/unified observability | ❌ No | logs↔traces link missing (O18); no trace waterfall (OTEL-14) |
| Spec-compliant responses | ⚠️ Partial | always full-success; no partial-success / `Retry-After` (new O20) |

**Path to conformance:** Part B prompts **O01–O15** make ingest *correct*; the new conformance prompts **O16–O22** (in `MOCK_DATA_TODO_PROMPTS.md`, Part C) make it *standards-conformant and reachable*. Both are required before claiming "OpenTelemetry-compatible backend for the three pillars."
