# Zentra — Mock Data Elimination TODO Prompts

> Companion to `GAP_ANALYSIS_MOCK_DATA.md`. Each prompt is self-contained and copy-pasteable. Execute in priority order (M01 → M09). Every task ends with a **Done when** acceptance check.
>
> **Global rule:** No UI value may originate from a hardcoded constant, `Math.sin`/`Math.random` synthetic generator, or an empty placeholder array that masquerades as data. All values must come from a live API call, the DB, or real infrastructure collection. When a data source is genuinely empty, render an explicit empty state ("No data yet"), never fabricated rows.

---

## P0 — HIGH (Frontend mock rendered as live)

### M01 — Wire `AIInsights.tsx` to the live `/ai/insights` endpoint
**File:** `frontend/src/pages/AIInsights.tsx`
**Backend:** `getAIInsights()` → `GET /ai/insights` (`frontend/src/lib/api/ai.ts:14`).

1. Delete the hardcoded constants: `mlModels`, `perfData`, `anomalyData`, `capacityData`, `costData`, `insights`, and the `totalSavings` derived from them (lines 14–58).
2. Replace with React Query:
   ```ts
   const { data: insights = [], isLoading } = useQuery({
     queryKey: ['ai-insights'],
     queryFn: getAIInsights,
   })
   ```
3. For ML models / performance / anomaly / capacity / cost panels: if the backend does not yet return them, either (a) extend `/ai/insights` (or add `/ai/models`, `/ai/capacity`, `/ai/cost`) to return real ML-engine output, or (b) hide those panels behind a feature flag and show an explicit "ML engine not yet enabled" empty state. **Do not** keep the synthetic generators.
4. Wire `handleRefresh` to `qc.invalidateQueries({ queryKey: ['ai-insights'] })`.
5. Update `buildKpiDrilldown` to read from the fetched data.

**Done when:** Page renders only data returned by the API; with an empty DB it shows empty states, not fabricated insights; no `Math.sin` business data remains (the `spark()` visual is handled separately in M09).

---

### M02 — Wire `LogSources.tsx` to live `/log-sources` CRUD
**File:** `frontend/src/pages/LogSources.tsx`
**Backend:** `getLogSources()` / `createLogSource()` / `deleteLogSource()` (`frontend/src/lib/api/logs.ts:11–15`).

1. Delete `mockSources` (lines 22–29).
2. Replace `useState(mockSources)` with:
   ```ts
   const { data: sources = [] } = useQuery({ queryKey: ['log-sources'], queryFn: getLogSources })
   const createMut = useMutation({ mutationFn: createLogSource, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-sources'] }) })
   const deleteMut = useMutation({ mutationFn: deleteLogSource, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-sources'] }) })
   ```
3. Wire the Add modal submit to `createMut.mutate(newSource)` and the delete button to `deleteMut.mutate(id)`.
4. Confirm the backend `LogSource` type matches `LogSourceItem`; reconcile field names (`source_type`, `last_ingested`, `log_count`) in `frontend/src/lib/api/types.ts` if needed.

**Done when:** Adding/deleting a log source persists across a page refresh; the list is empty on a fresh DB.

---

### M03 — Remove seeded `mockHistory` from `RCA.tsx`
**File:** `frontend/src/pages/RCA.tsx`

1. Delete `mockHistory` (lines 27–77).
2. Change initial state to empty: `const [results, setResults] = useState<RCAResult[]>([])`.
3. If RCA history should persist, add `GET /api/rca/history` on the backend and load it via React Query; otherwise keep the list session-only but starting empty.
4. Keep the real `analyzeRCA()` call (line 109) unchanged.

**Done when:** The RCA page opens with no fabricated history; results appear only after a real analysis.

---

## P1 — MEDIUM (Empty placeholder arrays + backend stubs)

### M04 — Wire `AlertRules.tsx` to live `/alert-rules`
**File:** `frontend/src/pages/AlertRules.tsx`
**Backend:** `getAlertRules()` (`frontend/src/lib/api/alerts.ts:14`).

1. Delete `const mockRules: AlertRule[] = []` (line 45).
2. Replace `const liveRules = mockRules` (line 62) with:
   ```ts
   const { data: liveRules = [] } = useQuery({ queryKey: ['alert-rules'], queryFn: getAlertRules })
   ```
3. Keep `alertTemplates` (legitimate built-in templates).
4. Ensure firing/pending/enabled counts derive from `liveRules`.

**Done when:** The rules tab shows real DB-backed rules; counts reflect live state.

---

### M05 — Wire `NotificationChannels.tsx` to live endpoints
**File:** `frontend/src/pages/NotificationChannels.tsx`
**Backend:** `getNotificationChannels()` → `/notifications/channels`, `getNotificationHistory()` → `/notifications/history` (`frontend/src/lib/api/alerts.ts:39,44`).

1. Delete `channels = []` (line 34), `policies = []` (line 47), `mockHistory = []` (line 51).
2. Replace with React Query for channels and history. For policies, wire to the routing-policies endpoint if it exists, otherwise render an explicit empty state.
3. Wire `totalSent` / `totalFailed` / `enabledCount` to fetched channel data.

**Done when:** Channels and history reflect live API data; empty DB shows empty states.

---

### M06 — Real file-scan verdict in `file_analysis.rs`
**File:** `crates/zentra-api/src/handlers/file_analysis.rs:81`

1. Remove `let verdict = "clean"; // placeholder`.
2. Integrate real scanning (YARA rules and/or VirusTotal lookup behind a configured API key), or, if scanning is out of scope, set `verdict = "unscanned"` and surface that honestly in the UI.
3. Never insert `"clean"` without an actual scan.

**Done when:** `file_scans.verdict` reflects a real scan result or an honest `"unscanned"` status.

---

### M07 — Real model count in `llm_observability.rs`
**File:** `crates/zentra-api/src/handlers/llm_observability.rs:99`

1. Replace `"models": 0, // Placeholder` with a real query:
   ```rust
   let models: i64 = sqlx::query_scalar(
       "SELECT COUNT(DISTINCT model) FROM llm_requests WHERE org_id = $1"
   ).bind(org_id).fetch_one(&s.pool).await.unwrap_or(0);
   ```
   (Adjust table/column to the actual schema.)

**Done when:** The overview returns the actual distinct model count.

---

### M08 — Make `data_audit.rs` truthful
**File:** `crates/zentra-api/src/handlers/data_audit.rs`

1. After M01–M07 are complete, update the audit so that any still-incomplete module is reported as `"mock"` or `"partial"` instead of blanket `"live"`.
2. Preferably compute status dynamically (e.g., probe each endpoint or maintain a single source-of-truth registry) rather than a hand-maintained list.
3. `mock_data_check` must not return `"status": "clean"` until the gaps in `GAP_ANALYSIS_MOCK_DATA.md` are closed.

**Done when:** `/api/audit/data-sources` and `/api/audit/mock-data` accurately reflect the real state of the system.

---

## P2 — MEDIUM (DB seed)

### M09 — Gate or remove `seed_databases.sql`
**File:** `migrations/20250101000016_seed_databases.sql`

1. Remove the hardcoded Oracle target seed (host `10.88.11.162`), OR convert it into a documented optional fixture loaded only when `ZENTRA_LOAD_SAMPLE_TARGETS=true`.
2. Since migrations cannot be deleted once shipped, prefer a follow-up migration that deletes the seed row on upgrade, mirroring the Docker `cleanup-seeds.sh` behavior, so Docker and bare-metal deploys converge.
3. Verify the default admin/roles/permissions seeds (`...012`, `...015`) remain intact — those are required.

**Done when:** A fresh non-Docker deploy contains no fabricated target rows; only bootstrap auth data is seeded.

---

## P3 — LOW (Cosmetic sparklines)

### M10 — Replace synthetic `spark()` visuals with real series
**Files:** `Dashboard.tsx`, `Servers.tsx`, `Network.tsx`, `Storage.tsx`, `AlertRules.tsx`, `NotificationChannels.tsx`, `AIInsights.tsx`, `security/SecurityDashboard.tsx`

1. Where a real metric history exists, feed the `Sparkline` component a real series (e.g., from `metric_query` / TimescaleDB).
2. Where no series exists, remove the sparkline rather than render a synthetic sine wave.
3. Keep `PieChart.tsx` geometry math (`Math.cos`/`Math.sin`) — that is legitimate rendering math, not data.

**Done when:** No sparkline is fed a `Math.sin`-generated synthetic series; trend visuals are either real or absent.

---

## Verification Checklist (run after all tasks)

- [ ] `grep -rnE "const (mock|sample|fake|demo|dummy)" frontend/src` returns nothing (except removed).
- [ ] `grep -rn "Math.sin\|Math.random" frontend/src/pages` returns only legitimate chart geometry.
- [ ] Each remediated page renders an explicit empty state on a fresh DB.
- [ ] `grep -rn "placeholder\|// Placeholder" crates/zentra-api/src/handlers` returns nothing actionable.
- [ ] `GET /api/audit/mock-data` reports `mock_patterns_found: 0` **truthfully**.
- [ ] Fresh `docker run` and fresh bare-metal deploy show identical (empty) data state.

---
---

# Part B — OTEL (OpenTelemetry) Functionality Prompts

> Companion to **Part B** of `GAP_ANALYSIS_MOCK_DATA.md`. Execute in priority order (O01 → O15). These fix broken/missing OTEL logic, not mock data. Every prompt ends with a **Done when** acceptance check.
>
> **Global rule:** Telemetry that is accepted (HTTP 200 / gRPC OK) MUST be durably stored and retrievable by the owning org. Never ACK data that is then discarded. All queries must be parameterized and org-scoped.

## P0 — CRITICAL (ingest is non-functional without these)

### O01 — Make gRPC ingest actually store data
**File:** `crates/zentra-api/src/grpc.rs:132-224`
1. In each of `grpc_export_traces/metrics/logs/profiles`, after decoding to JSON, enqueue to the ingest queue exactly like the HTTP handlers (`handlers/otlp.rs:147-157`):
   ```rust
   if let Some(ref queue) = state.ingest_queue {
       let batch = crate::ingest_queue::IngestBatch {
           org_id,
           signal: crate::ingest_queue::Signal::Traces, // Metrics/Logs/Profiles per handler
           payload,
           raw_bytes: proto_bytes.to_vec(),
       };
       if queue.try_send(batch).is_err() {
           return grpc_error(8, "Ingest queue full"); // RESOURCE_EXHAUSTED
       }
   }
   ```
2. Only return `grpc_ok_response` after a successful enqueue. Return `grpc-status 8` on backpressure.
3. Add an integration test: gRPC export → row appears in `traces`/`metrics`/`logs`/`otel_profiles`.

**Done when:** Sending OTLP/gRPC from a stock SDK results in stored, queryable rows; a full queue returns gRPC status 8, not OK.

### O02 — Populate `org_id` on ingested traces
**File:** `crates/zentra-api/src/ingest_queue.rs:391-408` (and span_events/span_links inserts)
1. Add `org_id` to the trace UNNEST insert and bind `item.org_id` for each row (collect an `org_ids: Vec<Uuid>` alongside the others).
2. Add `org_id` to `span_events` and `span_links` inserts (add the column in a migration first — see O14).
3. Verify the column order matches the `traces` table.

**Done when:** `SELECT count(*) FROM traces WHERE org_id IS NULL` is 0 for newly ingested spans, and `otel_stats`/`otel_resources` return the ingested data.

### O03 — Fix the metrics insert to match the real schema
**Files:** `crates/zentra-api/src/ingest_queue.rs:443-516` vs `migrations/20250101000001_schema.sql:40-52`
1. Correct the INSERT to the actual columns: `timestamp` (not `time`), `tags` (not `labels`), and provide the `NOT NULL` columns.
2. For OTLP-sourced metrics there is no `target_id`/`plugin_id`; choose one:
   - **Preferred:** add a migration making `target_id` nullable and `plugin_id`/`resource_type` default `'otlp'` for OTLP rows, OR
   - introduce a dedicated `otel_metrics` hypertable mirroring the OTLP shape and point the OTLP reads (`otel_stats`, `otel_resources`) at it.
3. Whichever path, ensure `otel_stats.metrics_per_minute` and `otel_resources` (metrics signal) read the same table the insert writes.
4. Confirm column name `timestamp` is used consistently (reads already use `timestamp`).

**Done when:** Posting OTLP metrics over HTTP results in rows in the metrics table (no DLQ entries), and `metrics_per_minute` is non-zero.

### O07 — Fix SQL injection in `otel_attribute_values` *(security, do early)*
**File:** `crates/zentra-api/src/handlers/otlp.rs:461-479`
1. Stop interpolating `params.key` into the SQL string. Use bound parameters and jsonb operators:
   ```rust
   sqlx::query_scalar::<_, String>(
     "SELECT DISTINCT tags->>$1 AS val FROM traces \
      WHERE org_id = $2 AND tags ? $1 AND started_at > NOW() - INTERVAL '24 hours' \
      AND tags->>$1 IS NOT NULL ORDER BY val LIMIT $3"
   ).bind(&params.key).bind(org_id).bind(limit)
   ```
2. Also bind `limit` rather than formatting it.

**Done when:** Passing `key='); DROP TABLE traces;--` returns a normal (empty) result and does not execute injected SQL; the query is org-scoped.

## P1 — HIGH

### O04 — Serialize metric data points in protobuf decode
**File:** `crates/zentra-api/src/otlp_proto.rs:464-489`
1. Extend `proto_to_json_metrics` to emit `gauge`/`sum`/`histogram` objects with their `dataPoints` (asDouble/asInt, timeUnixNano, attributes, and for histograms: count/sum/bucketCounts/explicitBounds), matching the JSON shape `insert_metrics_batch` expects.

**Done when:** A protobuf metrics export yields the same stored values as the equivalent JSON export.

### O05 — Handle histogram, exponential histogram, and summary metrics
**File:** `crates/zentra-api/src/ingest_queue.rs:443-516`
1. In `insert_metrics_batch`, in addition to `gauge`/`sum`, handle `histogram` (store count/sum and per-bucket rows or a structured value), `exponentialHistogram`, and `summary` (quantiles).
2. Decide a storage representation (e.g., expand to `_count`/`_sum`/`_bucket` metric names like Prometheus) and document it.

**Done when:** An SDK histogram (e.g., `http.server.duration`) produces queryable count/sum/bucket series.

### O06 — Evaluate user-defined OTEL alert rules
**File:** `crates/zentra-api/src/handlers/otlp.rs:951-999`
1. Rewrite `evaluate_alert_rules_internal` to load rules from `otel_alert_rules WHERE org_id = $1 AND enabled = true`.
2. For each rule, compute the metric for its `signal`/`metric`/`window_seconds` and compare with `condition.operator` + `threshold`.
3. Return per-rule status; emit events/notifications on transition to FIRING (reuse the existing alert/event bus).
4. Keep the 4 built-ins only as seeded defaults inserted into the table on first run (not hardcoded in the evaluator).

**Done when:** A rule created via `POST /api/otel/alert-rules` fires when its threshold is breached and does not fire otherwise; built-ins live in the table.

## P2 — MEDIUM

### O08 — Populate GenAI model dimension
**File:** `crates/zentra-api/src/handlers/otlp.rs:1010-1022`
1. Change the GenAI query to read the model from `tags` (where attributes are actually stored): `tags->>'gen_ai.request.model'` and token usage from `tags->>'gen_ai.usage.*'`.
2. Alternatively, populate the `attributes` column at ingest and keep the query — but pick one consistent source.

**Done when:** The GenAI view groups by real model names and shows non-zero token counts.

### O09 — Build flamegraph data for profiles
**File:** `crates/zentra-api/src/ingest_queue.rs:585-644`
1. Parse `profile.samples` (collapsed stacks + values) into a flamegraph tree (reuse `handlers/profiling.rs:build_flame_tree`) and store it in `flamegraph_data`.
2. Store profile `attributes` instead of `{}`.

**Done when:** The Profiles UI renders a real flamegraph from an ingested profile.

### O10 — Persist connector configuration
**File:** `crates/zentra-api/src/handlers/otel_connectors.rs:51-77`
1. Add an `otel_connectors` table (org_id, type, enabled, dimensions jsonb, interval_seconds, updated_at).
2. `list_connectors` reads from it (seed the span-to-metrics default on first run); `configure_connector` upserts.
3. Honor `enabled`/`dimensions`/`interval` in the span-to-metrics derivation job.

**Done when:** Toggling a connector persists across reloads and changes derivation behavior.

### O11 — Produce real pipeline topology edges
**File:** `crates/zentra-api/src/handlers/otlp.rs:751-780`
1. Build `flow` edges: apps (service.name without collector marker) → detected collectors → Zentra, using resource attributes (e.g., `telemetry.sdk.*`, collector name) to attribute spans to a collector.
2. Org-scope the query.

**Done when:** `otel_pipeline.flow` returns non-empty edges reflecting real senders.

### O12 — Update `last_used_at` on API-key validation
**File:** `crates/zentra-api/src/handlers/otlp.rs:77-115`
1. On a successful DB validation (and periodically on cache hits, e.g., throttled to once/min per key), `UPDATE otlp_api_keys SET last_used_at = NOW() WHERE key_hash = $1` (fire-and-forget).

**Done when:** The Security UI shows a recent `last_used_at` after ingest with a key.

### O13 — Org-scope attribute and pipeline reads
**Files:** `otel_attributes` (431-458), `otel_attribute_values` (461-479, also O07), `otel_pipeline` (751-780)
1. Add `WHERE org_id = $1` (bound) to all three; thread `OrgId` through.

**Done when:** Two orgs cannot see each other's attribute keys/values or collectors.

### O14 — Add a single-trace / waterfall endpoint
**Files:** `routes/otlp.rs`, `handlers/otlp.rs`, migration for `span_events`/`span_links` `org_id`
1. Add `GET /api/otel/traces/:trace_id` returning all spans for the trace (org-scoped), plus their `span_events` and `span_links`, ordered for a waterfall.
2. Add `org_id` columns to `span_events`/`span_links` (migration) and populate them at ingest (ties to O02).
3. Optionally add `GET /api/otel/traces` (recent trace list with filters) to back a Trace Explorer.

**Done when:** A trace can be fetched by ID with its spans/events/links and rendered as a waterfall.

## P3 — LOW

### O15 — Make `otel_status` reflect real capabilities
**File:** `crates/zentra-api/src/handlers/otlp.rs:303-327`
1. Report `grpc.status = "enabled"` (and the actual port) when the `grpc` feature is compiled; reflect real `tls_enabled`.
2. Keep it consistent with what is actually wired (after O01).

**Done when:** `GET /api/otel/status` accurately describes the running server.

## OTEL Verification Checklist
- [ ] OTLP/gRPC export from a stock SDK produces rows in `traces`/`metrics`/`logs` (O01).
- [ ] `SELECT count(*) FROM traces WHERE org_id IS NULL` = 0 after ingest (O02).
- [ ] OTLP metrics (HTTP and gRPC, JSON and protobuf) appear with correct values; `otlp_dead_letter` stays empty (O03, O04, O05).
- [ ] Injecting SQL via `key=` is inert and org-scoped (O07, O13).
- [ ] A user-created OTEL alert rule fires on breach (O06).
- [ ] GenAI view shows real models/tokens (O08); a profile renders a flamegraph (O09).
- [ ] Connector toggles persist (O10); pipeline shows edges (O11); `last_used_at` updates (O12).
- [ ] `GET /api/otel/traces/:trace_id` returns a full waterfall (O14).
- [ ] `GET /api/otel/status` matches the running config (O15).

---
---

# Part C — OpenTelemetry Conformance (vs the CNCF definition)

> **Measured against:** *"OpenTelemetry provides a consistent, vendor-neutral format for instrumenting, generating, gathering, and exporting the three pillars of observability — metrics, logs, and traces — to any compatible monitoring backend."*
>
> **Zentra's role:** a *compatible monitoring backend* (an OTLP receiver), not an SDK vendor. To satisfy the definition it must accept **standard OTLP** (vendor-neutral), over the **standard transports/encodings**, for **all three pillars**, and store them as **correlated, queryable** observability data.

## C0. Conformance Verdict — **Partially conformant / currently NOT met**

| Definition requirement | Status | Blocking items |
|------------------------|--------|----------------|
| Vendor-neutral OTLP format (HTTP JSON, HTTP protobuf, gRPC) | ⚠️ Partial | gRPC drops data (O01); gzip unsupported (O17); a non-standard custom ingest exists (O19) |
| Reachable by stock SDKs/Collectors (standard endpoints/ports) | ❌ No | In hardened Docker, no OTLP endpoint is exposed (O16); SDK snippets point at a wrong endpoint (O21) |
| **Traces** pillar | ⚠️ Partial | Stored but `org_id` NULL → invisible (O02) |
| **Metrics** pillar | ❌ No | Insert fails vs schema → DLQ (O03), protobuf has no points (O04), histograms dropped (O05) |
| **Logs** pillar | ⚠️ Partial | Stored but `trace_id`/`span_id` dropped → no correlation (O18) |
| Unified/correlated observability across pillars | ❌ No | Logs↔traces link missing (O18); no trace waterfall (O14) |
| Spec-compliant responses (partial success, retryable errors) | ⚠️ Partial | Always full-success; no `Retry-After` (O20) |

**Conclusion:** With Part B (O01–O15) the ingest becomes *correct*; with Part C (O16–O22) it becomes *standards-conformant and reachable*. Both are required to truthfully claim "OpenTelemetry-compatible backend for the three pillars."

## C1 — CRITICAL Conformance Prompts

### O16 — Make OTLP ingest reachable in the hardened deployment
**Files:** `docker/nginx.conf`, `docker/Dockerfile` (EXPOSE), `docker/docker-entrypoint.sh`
**Problem:** After D05 hardening, nginx (`:5010`) only proxies `/api/`. OTLP clients POST to `/v1/traces|metrics|logs`, which hit the SPA catch-all (`location /`) and receive `index.html` with HTTP 200 — **silently non-ingested**. gRPC `:4317` and OTLP/HTTP `:4318` are not `EXPOSE`d either. Net effect: **the hardened image accepts zero external telemetry.**
1. Add an nginx location to proxy the OTLP HTTP paths to the backend:
   ```nginx
   location /v1/ {
       proxy_pass http://127.0.0.1:5006;
       proxy_http_version 1.1;
       proxy_set_header Host $host;
       proxy_set_header X-Real-IP $remote_addr;
       proxy_request_buffering off;        # stream large batches
       client_max_body_size 16m;           # align with max_payload_bytes
   }
   ```
2. Decide the external OTLP surface and document it:
   - **HTTP:** reachable at `https://<host>:5010/v1/...` (via nginx), and/or expose the conventional `4318`.
   - **gRPC:** `EXPOSE 4317` and publish it (`-p 4317:4317`) — gRPC needs HTTP/2 end-to-end; either expose the gRPC port directly or configure nginx `grpc_pass` on a dedicated server block.
3. Update the README/deployment notes with the exact OTLP endpoint and a working `OTEL_EXPORTER_OTLP_ENDPOINT` example.

**Done when:** A stock OTel SDK/Collector pointed at the documented endpoint successfully delivers traces/metrics/logs to a fresh hardened container, and rows appear in the DB.

### O17 — Support `Content-Encoding: gzip` on OTLP ingest
**Files:** `crates/zentra-api/src/handlers/otlp.rs` (ingest handlers), `crates/zentra-api/src/grpc.rs`
**Problem:** No request decompression exists. OTel Collectors and many SDKs send `Content-Encoding: gzip` (HTTP) and set the gRPC compression flag. Compressed payloads currently fail JSON/protobuf decode.
1. In each HTTP ingest handler, detect `Content-Encoding: gzip` (and `deflate`) and decompress `body` (e.g., `flate2::read::GzDecoder`) before parsing. Treat absent/`identity` as raw.
2. In gRPC, honor the 1-byte compression flag in the frame header (currently `strip_grpc_frame` ignores it) — decompress when set.
3. Reject unknown encodings with HTTP 415 / gRPC `UNIMPLEMENTED`.

**Done when:** `OTEL_EXPORTER_OTLP_COMPRESSION=gzip` exports (HTTP and gRPC) are ingested correctly.

## C2 — HIGH Conformance Prompts

### O18 — Correlate logs with traces (logs↔traces unification)
**Files:** `crates/zentra-api/src/ingest_queue.rs:518-583` (`insert_logs_batch`), logs schema migration
**Problem:** `proto_to_json_logs`/JSON logs carry `traceId`/`spanId`/`severityNumber`/`observedTimeUnixNano`, but `insert_logs_batch` stores only `(org_id, timestamp, source, level, message, metadata)`. Correlation between logs and traces — a core OTel value — is lost.
1. Add columns `trace_id TEXT`, `span_id TEXT`, `severity_number INT`, `observed_at TIMESTAMPTZ` to the logs table (idempotent migration) with an index on `trace_id`.
2. Extract and store these in `insert_logs_batch`.
3. Surface them in log query responses so the UI can pivot log↔trace.

**Done when:** A log record exported with a trace context is retrievable by `trace_id` and links to its trace.

### O19 — Consolidate the duplicate, non-standard trace ingest
**Files:** `crates/zentra-api/src/handlers/traces.rs:230-421`, `routes/*`
**Problem:** A second ingest `POST /api/v1/traces` (`handlers::traces::ingest_traces`) accepts a **non-OTLP custom shape** `{ "spans": [ {trace_id, span_id, ...} ] }`. This competes with the canonical OTLP handler (`handlers::otlp::ingest_traces`, which expects `resourceSpans`) and is not vendor-neutral.
1. Make the canonical OTLP `/v1/traces` (resourceSpans) the single ingest path.
2. Either remove the custom-shape endpoint, or clearly namespace it as an internal/legacy API (e.g., `/api/internal/spans`) and exclude it from anything labeled "OTLP".
3. Ensure exactly one documented OTLP trace endpoint to avoid client confusion.

**Done when:** There is one OTLP-conformant trace ingest path; no endpoint advertised as OTLP accepts a non-OTLP body.

### O20 — Spec-compliant OTLP responses (partial success + retryable errors)
**Files:** `crates/zentra-api/src/handlers/otlp.rs`, `crates/zentra-api/src/grpc.rs`
**Problem:** Handlers always return full success. The OTLP spec defines `partial_success { rejected_*, error_message }` and requires retryable signaling: HTTP `429`/`503` with `Retry-After`, gRPC `UNAVAILABLE`/`RESOURCE_EXHAUSTED`.
1. When some items are dropped (e.g., sampled out, malformed, or queue pressure), return `partial_success` with `rejected_data_points`/`rejected_spans`/`rejected_log_records` and a message.
2. On backpressure return HTTP `503` + `Retry-After` (HTTP) and gRPC status `8`/`14` (already started in O01) — never a 200/OK for dropped data.
3. Populate the gRPC `Export*ServiceResponse.partial_success` instead of always `None`.

**Done when:** A partially-rejected batch reports accurate `rejected_*` counts and overload returns a retryable status with `Retry-After`.

## C3 — MEDIUM Conformance Prompts

### O21 — Emit correct, reachable endpoints in SDK snippets & collector config
**File:** `crates/zentra-api/src/handlers/otlp.rs:330-424` (`collector_config`, `sdk_snippets`)
**Problem:** Snippets hardcode `http://localhost:3001` and `collector_config` defaults to port `3001` — neither matches the real OTLP surface (`:5010/v1` via nginx, or `:4317` gRPC). Users copy-pasting these cannot send data.
1. Derive the endpoint from the request host / a configured `ZENTRA_PUBLIC_OTLP_ENDPOINT` env var (after O16) and inject it into all snippets and the collector YAML.
2. Show both HTTP (`/v1`) and gRPC (`:4317`) variants, with the API-key `Authorization: Bearer isok_...` header.
3. Remove the `3001` default.

**Done when:** Copy-pasting any generated snippet/Collector config sends data that lands in Zentra without edits.

### O22 — Conformance test matrix (real SDK/Collector)
**Files:** new `crates/zentra-api/tests/otlp_conformance.rs` (or a scripted e2e)
1. Add an automated test that exercises, end-to-end, the full pillar/transport/encoding matrix:
   - signals: **traces, metrics (gauge/sum/histogram), logs**
   - transports: **HTTP/JSON, HTTP/protobuf, gRPC**
   - encodings: **identity, gzip**
2. Assert each lands in the correct table, scoped to the right `org_id`, and is returned by the corresponding `/api/otel/*` read endpoint.
3. Include a negative test (oversize/rate-limited → retryable status; malformed → partial_success).

**Done when:** The matrix passes in CI, proving Zentra is a conformant OTLP backend for all three pillars.

## Conformance Verification Checklist
- [ ] Fresh hardened container ingests traces/metrics/logs from a stock OTel SDK at the documented endpoint (O16).
- [ ] `OTEL_EXPORTER_OTLP_COMPRESSION=gzip` works over HTTP and gRPC (O17).
- [ ] A log with trace context is queryable by `trace_id` and links to its trace (O18).
- [ ] Exactly one OTLP-conformant trace endpoint; no OTLP endpoint accepts a non-OTLP body (O19).
- [ ] Partial/overload responses use `partial_success` and retryable statuses with `Retry-After` (O20).
- [ ] Generated SDK snippets / Collector config work unmodified (O21).
- [ ] The full pillar × transport × encoding matrix passes in CI (O22).
