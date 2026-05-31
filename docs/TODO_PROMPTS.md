# Zentra — High-End TODO Prompts for Identified Gaps

**Version:** 1.0.0  
**Companion to:** `GAP_ANALYSIS.md`  

---

## How to Use This Document

Each TODO prompt is self-contained and contains:
- **Context**: What the gap is and where it lives in the codebase
- **Goal**: What "done" looks like
- **Files to Modify**: Exact paths
- **Implementation Steps**: Ordered, detailed instructions
- **Acceptance Criteria**: How to verify correctness
- **Priority**: P0 (critical), P1 (important), P2 (nice-to-have)

---

## TODO-1: Fix Cloud Monitoring — Replace Stubs with Real SDK Calls
**Priority:** P0  **Module:** Cloud Monitoring  **Gap:** G2.1

### Context
The cloud monitoring module (`crates/zentra-api/src/handlers/cloud_monitoring.rs`) has three handlers that return stub responses:
- `validate_account`: Returns `{valid: false, message: "Cloud provider SDK integration is not yet available"}`
- `get_account_metrics`: Returns `StatusCode::NOT_IMPLEMENTED` (501)
- `cloud_summary`: Likely placeholder

### Goal
Implement real AWS, Azure, and GCP credential validation and metric retrieval using their respective Rust SDKs.

### Files to Modify
1. `crates/zentra-api/src/handlers/cloud_monitoring.rs`
2. `crates/zentra-api/Cargo.toml` (add SDK deps)
3. `crates/zentra-core/src/models/cloud_account.rs` (add credential fields)
4. Frontend: `frontend/src/pages/CloudMonitoring.tsx`

### Implementation Steps

1. **Add Rust SDK dependencies to `Cargo.toml`**:
   ```toml
   aws-sdk-ec2 = "1"
   aws-sdk-cloudwatch = "1"
   azure_mgmt_compute = "0.21"
   google-cloud-monitoring = "0.5"
   ```

2. **Expand `CloudAccount` model** to store encrypted credentials:
   - Add `aws_access_key_id`, `aws_secret_access_key`, `azure_tenant_id`, `azure_client_id`, `azure_client_secret`, `gcp_service_account_key` (all `Option<String>`)
   - Store credentials encrypted using the existing `license_verifier.rs` AES-256 key

3. **Implement `validate_account`**:
   - Match on `provider` field (`"aws"`, `"azure"`, `"gcp"`)
   - For AWS: Create a `aws_sdk_cloudwatch::Client` with the provided credentials and call `list_metrics()` as a validation probe
   - For Azure: Create a `azure_identity::ClientSecretCredential` and call `resource_groups.list()`
   - For GCP: Parse the service account JSON and create a `google_cloud_monitoring::Client`, then list metric descriptors
   - Return `{valid: true, message: "Account validated successfully"}` on success
   - Catch auth errors and return `{valid: false, message: <error detail>}`

4. **Implement `get_account_metrics`**:
   - Accept query params: `metric_name`, `hours`, `granularity`
   - For AWS: Use `aws_sdk_cloudwatch::Client::get_metric_statistics()` for EC2 CPUUtilization, NetworkIn, NetworkOut
   - For Azure: Use `azure_mgmt_monitor::MetricsClient::list()` for Percentage CPU
   - For GCP: Use `google_cloud_monitoring::MetricServiceClient::list_time_series()`
   - Normalize all responses into a common `CloudMetric` struct with fields: `timestamp`, `value`, `unit`, `metric_name`, `dimensions`
   - Return `Json<Vec<CloudMetric>>`

5. **Update frontend** `CloudMonitoring.tsx`:
   - In the `useQuery` for metrics, handle 501 by showing a "Cloud provider integration not enabled" banner
   - Add a loading state and error display
   - When data returns, render a Recharts area chart per metric type

### Acceptance Criteria
- [ ] Calling `POST /api/cloud/accounts/:id/validate` with AWS credentials returns `valid: true`
- [ ] Calling `GET /api/cloud/accounts/:id/metrics` returns real CloudWatch data (not 501)
- [ ] All three providers (AWS, Azure, GCP) are supported
- [ ] Credentials are encrypted at rest
- [ ] Frontend shows real metrics charts instead of error banners

---

## TODO-2: Fix All Middleware Handlers — Live HTTP Polling
**Priority:** P0  **Module:** Middleware Monitoring  **Gap:** G2.2, G4.x

### Context
10 middleware handler files return hardcoded data:
- `middleware_docker.rs` — hardcoded container list, zeros for status
- `middleware_kafka.rs` — hardcoded topics, zeros for status
- `middleware_elasticsearch.rs` — hardcoded indices, zeros for status
- `middleware_haproxy.rs`, `middleware_tomcat.rs`, `middleware_phpfpm.rs`, `middleware_rabbitmq.rs`, `middleware_redis.rs`, `middleware_memcached.rs`, `middleware_kubernetes.rs` — same pattern

### Goal
Make all middleware handlers perform real HTTP polling against the configured instance's metrics/status endpoint, using the same pattern already established in `middleware_nginx.rs` and `middleware_apache.rs`.

### Files to Modify
1. `crates/zentra-api/src/handlers/middleware_docker.rs`
2. `crates/zentra-api/src/handlers/middleware_kafka.rs`
3. `crates/zentra-api/src/handlers/middleware_elasticsearch.rs`
4. `crates/zentra-api/src/handlers/middleware_haproxy.rs`
5. `crates/zentra-api/src/handlers/middleware_tomcat.rs`
6. `crates/zentra-api/src/handlers/middleware_phpfpm.rs`
7. `crates/zentra-api/src/handlers/middleware_rabbitmq.rs`
8. `crates/zentra-api/src/handlers/middleware_redis.rs`
9. `crates/zentra-api/src/handlers/middleware_memcached.rs`
10. `crates/zentra-api/src/handlers/middleware_kubernetes.rs`
11. `crates/zentra-api/src/handlers/middleware_nginx.rs` (reference pattern)

### Implementation Steps

1. **Study the Nginx handler** to extract the polling pattern:
   - It fetches the instance from the in-memory store
   - It makes an `reqwest::get()` call to the instance's `host:port/nginx_status` endpoint
   - It parses the response (text/html or JSON) into a struct
   - It returns the parsed data

2. **For each affected handler, rewrite `get_status`**:
   - Keep the in-memory instance store for registration (it's used by list/create/delete)
   - In `get_status`, fetch the instance, then construct the appropriate status URL:
     - Docker: `http://{host}:{port}/v1.24/info` or Docker Engine API
     - Kafka: `http://{host}:{port}/metrics` (JMX Exporter) or Kafka REST Proxy
     - Elasticsearch: `http://{host}:{port}/_cluster/health` and `http://{host}:{port}/_cluster/stats`
     - HAProxy: `http://{host}:{port}/stats;csv` or Unix socket
     - Tomcat: `http://{host}:{port}/manager/status?XML=true`
     - PHP-FPM: `http://{host}:{port}/status` via fcgi or HTTP
     - RabbitMQ: `http://{host}:{port}/api/overview`
     - Redis: `http://{host}:{port}/metrics` (Redis Exporter) or direct RESP via TCP
     - Memcached: `stats` command via TCP or `memcached_exporter` HTTP endpoint
     - Kubernetes: `http://{host}:{port}/metrics` via kubelet or metrics-server API

3. **Add authentication support** in `CreateXxxRequest`:
   - Add `username`, `password`, `api_key`, `tls_cert` fields
   - Use these in `reqwest::Client` when polling

4. **Add caching**:
   - Cache the last successful poll for 15 seconds in `state.cache` or the in-memory store
   - Return cached data on subsequent requests to avoid hammering the middleware

5. **For `performance_history`**:
   - Remove the `sin()` wave generation
   - Instead, query `metrics` or `middleware_metrics` TimescaleDB hypertable for historical data
   - If no history exists yet (new instance), return an empty array

6. **Update frontend** `MiddlewareMonitoring.tsx`:
   - When an instance has `status: "unknown"`, show a "Connection not established" badge
   - When status returns real data, render the dashboard panels
   - Add a "Test Connection" button that calls a new `POST /middleware/:type/:id/test` endpoint (reuse target test pattern)

### Acceptance Criteria
- [ ] Docker tab shows real container IDs, names, and CPU/memory from Docker Engine API
- [ ] Kafka tab shows real topic list and message rates from JMX Exporter
- [ ] ES tab shows real cluster health, index list, and store size from `_cluster/health`
- [ ] All 10 middleware types return live data (not hardcoded)
- [ ] `performance_history` returns real DB data (not sine waves)
- [ ] Frontend shows "Live" or "Cached" indicator with timestamp

---

## TODO-3: Add Missing CRUD Endpoints for Targets and Alert Rules
**Priority:** P0  **Module:** Core API  **Gap:** G3.1, G3.2

### Context
The frontend can create and delete targets and alert rules, but cannot update them. This breaks the edit flow in the UI.

### Goal
Add `PUT /targets/:id` and `PUT /alert-rules/:id` + `DELETE /alert-rules/:id` endpoints.

### Files to Modify
1. `crates/zentra-api/src/routes/targets.rs`
2. `crates/zentra-api/src/routes/alerts.rs`
3. `crates/zentra-api/src/handlers/targets.rs`
4. `crates/zentra-api/src/handlers/alerts.rs`

### Implementation Steps

1. **Add `PUT /targets/:id`** to `routes/targets.rs`:
   ```rust
   .route("/api/targets/:id", put(handlers::targets::update_target))
   ```

2. **Implement `update_target`** in `handlers/targets.rs`:
   - Extract `Path(id)` and `Json(req)` where `req` is `UpdateTargetRequest` (same fields as `CreateTargetRequest` but all optional)
   - Build a dynamic SQL UPDATE query that only sets fields present in the request
   - Execute against `targets` table in `state.pool`
   - Return the updated target or 404 if not found

3. **Add `PUT /alert-rules/:id`** to `routes/alerts.rs`:
   ```rust
   .route("/api/alert-rules/:id", put(handlers::alerts::update_alert_rule))
   ```

4. **Implement `update_alert_rule`** in `handlers/alerts.rs`:
   - Same pattern as `update_target` but for `alert_rules` table
   - Fields: `name`, `description`, `severity`, `metric`, `operator`, `threshold`, `duration_sec`, `enabled`, `channels`
   - On update, also re-evaluate whether the rule would currently fire and update `alert_state`

5. **Add `DELETE /alert-rules/:id`** to `routes/alerts.rs`:
   - Delete from `alert_rules` where `id = $1`
   - Also delete associated records in `alert_history` (or cascade in DB)

### Acceptance Criteria
- [ ] `PUT /api/targets/:id` with partial JSON updates only the provided fields
- [ ] `PUT /api/alert-rules/:id` updates the rule and recalculates alert state
- [ ] `DELETE /api/alert-rules/:id` removes the rule and its history
- [ ] All three endpoints return 404 for non-existent IDs
- [ ] Frontend "Edit" buttons on Target and Alert Rule pages now work

---

## TODO-4: Integrate OTEL Data into the Alert Evaluation Engine
**Priority:** P1  **Module:** OTEL + Alerts  **Gap:** G5.1, G5.8

### Context
The alert evaluator (`alert_evaluator.rs`) only queries the `metrics` table. It cannot evaluate conditions against OTEL-derived metrics (e.g., `http.request.duration > 500ms`) or trace attributes (e.g., `http.status_code >= 500`).

### Goal
Allow alert rules to reference OTEL semantic convention attributes and metrics, and have the alert evaluator query both `metrics` and `otlp_metrics` / `traces` tables.

### Files to Modify
1. `crates/zentra-api/src/alert_evaluator.rs`
2. `crates/zentra-api/src/handlers/alert_rules.rs` (validation)
3. `frontend/src/pages/AlertRuleEditor.tsx`

### Implementation Steps

1. **Extend alert rule model** to support OTEL metric sources:
   - Add `source: Enum("native" | "otel_traces" | "otel_metrics" | "otel_logs")` to `alert_rules` table (default `"native"`)
   - Add `otel_attribute_filter: Option<String>` — e.g., `service.name="api-gateway"`

2. **Implement OTEL-aware evaluation in `alert_evaluator.rs`**:
   - In `evaluate_alert()`, match on `rule.source`:
     - `"native"`: Query `metrics` table (existing behavior)
     - `"otel_metrics"`: Query `otlp_metrics` table for `metric_name = rule.metric` within the time window, filtered by `otel_attribute_filter`
     - `"otel_traces"`: Query `traces` table using `COUNT(CASE WHEN status_code >= 500 THEN 1 END) / COUNT(*) * 100` for error rate, or `AVG(duration_ms)` for latency
     - `"otel_logs"`: Query `otlp_logs` table for `severity >= rule.threshold`

3. **Add SQL queries** for OTEL evaluation:
   ```sql
   -- Error rate from traces
   SELECT (COUNT(*) FILTER (WHERE status_code >= 500) * 100.0 / COUNT(*)) as value
   FROM traces
   WHERE service_name = $1 AND start_time >= NOW() - INTERVAL '$2 seconds'
   ```

4. **Update `AlertRuleEditor.tsx`**:
   - Add a "Data Source" dropdown: Native Metrics | OTEL Traces | OTEL Metrics | OTEL Logs
   - When OTEL is selected, show an attribute filter input (e.g., `service.name = "api"`)
   - Populate the metric dropdown from OTEL metric names when source is OTEL Metrics

5. **Add migration** for the new `source` and `otel_attribute_filter` columns.

### Acceptance Criteria
- [ ] Alert rules can be created with source = "OTEL Traces" and metric = "error_rate"
- [ ] Evaluator correctly counts 500s from the `traces` table for the matching service
- [ ] Alert rules can be created with source = "OTEL Metrics" and metric = "http.request.duration"
- [ ] Frontend shows OTEL metric names in the dropdown
- [ ] Existing native alerts continue to work without modification

---

## TODO-5: Build OTEL Service Health Dashboard in Command Center
**Priority:** P1  **Module:** OTEL + Dashboard  **Gap:** G5.3, G9.1

### Context
The Command Center (`Dashboard.tsx`) shows alerts, targets, events, and network traffic but has NO OTEL service health summary. Users can't see "Service X has 15% error rate" on the main dashboard.

### Goal
Add an OTEL Service Health panel to the Command Center that shows RED metrics (Rate, Errors, Duration) per service for the last hour.

### Files to Modify
1. `crates/zentra-api/src/routes/health.rs` (or new `otel.rs` route)
2. `crates/zentra-api/src/handlers/otel_dashboard.rs` (new file)
3. `frontend/src/pages/Dashboard.tsx`
4. `frontend/src/lib/api/health.ts` (or new `otel.ts`)

### Implementation Steps

1. **Create backend endpoint** `GET /api/otel/service-health`:
   - Query `traces` hypertable for the last hour
   - Aggregate per `service_name`:
     - `rate`: `COUNT(*) / 3600.0` (requests per second)
     - `error_rate`: `COUNT(*) FILTER (WHERE status_code >= 500) * 100.0 / COUNT(*)`
     - `p50_duration`, `p95_duration`, `p99_duration`: Use `approx_percentile_of_duration` or ordered-set `percentile_cont`
   - Return `Json<Vec<OtelServiceHealth>>`

2. **Create frontend API client** `frontend/src/lib/api/otel.ts`:
   ```typescript
   export function getOtelServiceHealth() {
     return fetchJSON<{ services: OtelServiceHealth[] }>("/api/otel/service-health");
   }
   ```

3. **Update `Dashboard.tsx`**:
   - Add a new section: "OTEL Service Health" below the AI Insights card
   - Render a table with columns: Service Name | RPS | Error % | P50 | P95 | P99 | Status
   - Status = "Healthy" if error_rate < 1% and p95 < 500ms, else "Degraded", else "Critical"
   - Use color badges: green / yellow / red
   - Clicking a row navigates to `/traces?serviceName=X`

4. **Add a small sparkline chart** for each service showing error rate over the last hour (optional but nice).

### Acceptance Criteria
- [ ] Command Center shows a "OTEL Service Health" panel
- [ ] Panel lists all services that sent traces in the last hour
- [ ] Each row shows RPS, Error %, and latency percentiles
- [ ] Rows are sorted by error rate descending
- [ ] Clicking a service navigates to the Traces page filtered by service
- [ ] If no OTEL data exists, panel shows "No OTEL data received yet" with a link to setup docs

---

## TODO-6: Enable Trace-to-Log Correlation in Log Explorer
**Priority:** P1  **Module:** OTEL + Logs  **Gap:** G5.5, G9.3

### Context
The Log Explorer (`LogExplorer.tsx`) shows logs from `otlp_logs` but users can't click "View Traces for this Log" even though every log entry may have a `trace_id`.

### Goal
Add a "View Trace" button on each log row that pivots to the Traces page filtered by that `trace_id`.

### Files to Modify
1. `frontend/src/pages/LogExplorer.tsx`
2. `frontend/src/pages/Traces.tsx`
3. `crates/zentra-api/src/handlers/logs.rs` (ensure `trace_id` is returned)

### Implementation Steps

1. **Verify backend returns `trace_id`**:
   - Check `handlers/logs.rs` `list_otlp_logs` — ensure `trace_id` is included in the SELECT and response struct
   - If missing, add it

2. **Update `LogExplorer.tsx`**:
   - In the log table row renderer, check if `log.trace_id` is present
   - If present, render a small link icon button labeled "Trace"
   - On click: `navigate(`/traces?traceId=${log.trace_id}`)`

3. **Update `Traces.tsx`**:
   - Read `traceId` from URL query params
   - If `traceId` is present, pre-filter the trace list to show only spans with that `trace_id`
   - Also auto-expand the trace detail view if exactly one trace matches
   - Show a "Clear filter" button to return to unfiltered view

4. **Update the backend** `GET /api/otel/traces` to accept `trace_id` query param:
   - Add `trace_id: Option<String>` to the query params struct
   - If provided, add `AND trace_id = $N` to the SQL WHERE clause

### Acceptance Criteria
- [ ] Log rows with a `trace_id` show a "Trace" button
- [ ] Clicking it navigates to `/traces?traceId=ABC123`
- [ ] Traces page shows only spans for that trace ID
- [ ] If the trace_id doesn't exist, show "No traces found for this log entry"
- [ ] The flow works bidirectionally: from Traces page, clicking a log reference goes back to logs

---

## TODO-7: Connect OTEL Traces to Target Health for Root Cause Analysis
**Priority:** P1  **Module:** OTEL + Targets  **Gap:** G5.2, G8.5

### Context
When a target (e.g., a Linux server) is marked "down" or "unhealthy", there's no automatic trace analysis. We don't know which services on that server were affected.

### Goal
When a target changes state to unhealthy, automatically surface OTEL traces that originate from that target's IP or hostname in the target detail page.

### Files to Modify
1. `crates/zentra-api/src/handlers/targets.rs`
2. `crates/zentra-api/src/routes/targets.rs`
3. `frontend/src/pages/TargetDetail.tsx`

### Implementation Steps

1. **Add target-to-service mapping**:
   - Add `otel_service_names: Vec<String>` to the `targets` table (as a JSONB array)
   - Users can optionally tag a target with the OTEL service names running on it
   - OR auto-discover by joining `traces` table where `service.instance.id` matches target IP

2. **Add endpoint** `GET /api/targets/:id/otel-traces`:
   - Fetch the target's `otel_service_names` or IP
   - Query `traces` table for spans from those services in the last 1 hour
   - Return top 20 traces sorted by error rate or duration

3. **Update `TargetDetail.tsx`**:
   - Add a new tab: "OTEL Traces"
   - In that tab, call `GET /api/targets/:id/otel-traces`
   - Show a table of traces: Trace ID | Service | Operation | Duration | Status
   - Clicking a trace row navigates to the full trace view

4. **Auto-suggestion**:
   - When creating a target, offer to scan OTEL traces from the last 24h for that IP and suggest `service_name` tags

### Acceptance Criteria
- [ ] Target detail page has an "OTEL Traces" tab
- [ ] Tab shows traces from the services running on that target
- [ ] If a target is down, the tab shows the last traces before failure
- [ ] Service names can be manually tagged or auto-discovered
- [ ] Works for both SSH targets and OTEL-instrumented services

---

## TODO-8: Implement Unified Observability Query Layer
**Priority:** P2  **Module:** Core API  **Gap:** G7.2

### Context
Metrics, logs, and traces are queried via separate endpoints. There's no way to run a single query like "show me everything for service=api-gateway between 10:00 and 10:05".

### Goal
Create a unified query endpoint that can return metrics, logs, and traces for a given service and time range.

### Files to Modify
1. `crates/zentra-api/src/handlers/unified_query.rs` (new)
2. `crates/zentra-api/src/routes/health.rs` (or new route file)
3. `frontend/src/lib/api/query.ts` (new)

### Implementation Steps

1. **Create backend endpoint** `POST /api/query/unified`:
   - Request body: `{ service_name: String, start: DateTime, end: DateTime, signals: ["metrics", "logs", "traces"] }`
   - For each signal:
     - Metrics: Query `metrics` + `otlp_metrics` for that service in the time range
     - Logs: Query `otlp_logs` for that service
     - Traces: Query `traces` for that service
   - Return: `{ metrics: [...], logs: [...], traces: [...] }` with a common `timestamp` field

2. **Create frontend hook** `useUnifiedQuery`:
   - Takes service name and time range
   - Returns combined data

3. **Create a "Unified Explorer" page** (optional, can be P2):
   - Three-pane layout: Metrics chart top, Logs table middle, Trace waterfall bottom
   - All filtered by the same service and time range
   - Clicking a log filters traces; clicking a trace highlights the metric spike

### Acceptance Criteria
- [ ] `POST /api/query/unified` returns combined metrics, logs, and traces
- [ ] Results are time-aligned
- [ ] Works for any `service_name` present in OTEL data
- [ ] Response time < 2 seconds for 5-minute windows

---

## TODO-9: Add RBAC Permission Checks to `require_auth` Middleware
**Priority:** P1  **Module:** Auth  **Gap:** G6.3

### Context
`require_auth` middleware validates the JWT signature and expiry but never checks if the user has permission to access the requested module or endpoint. Any authenticated user can call admin APIs.

### Goal
Add a permission layer that checks the user's `module_access` list (stored in the JWT claims or DB) against the requested route.

### Files to Modify
1. `crates/zentra-api/src/middleware/auth.rs`
2. `crates/zentra-api/src/routes/*.rs` (add `.layer(require_permission("xxx"))`)
3. `crates/zentra-core/src/models/user.rs`

### Implementation Steps

1. **Add `permissions: Vec<String>` to JWT claims** in `auth.rs`:
   - When issuing a token, include the user's module_access as a `permissions` claim

2. **Create `require_permission(permission: &str)` middleware**:
   - Extract the JWT from `Authorization` header (reuse existing logic)
   - Decode and verify
   - Check if `permissions` contains the required string
   - If not, return `403 Forbidden` with body `{ "error": "Insufficient permissions", "required": "xxx" }`

3. **Annotate routes** with required permissions:
   ```rust
   .route("/api/admin/*", get(...).layer(require_permission("admin")))
   .route("/api/otel/keys", post(...).layer(require_permission("otel:write")))
   .route("/api/alerts/rules", post(...).layer(require_permission("alerts:write")))
   ```

4. **Update user model** to support fine-grained permissions:
   - `module_access` is currently a JSONB array of module names
   - Expand to support action-based permissions: `"targets:read"`, `"targets:write"`, `"alerts:read"`, etc.

5. **Add a role-based preset** in the admin UI:
   - Admin, Editor, Viewer roles with predefined permission sets

### Acceptance Criteria
- [ ] Authenticated users without "admin" permission get 403 on `/api/admin/*`
- [ ] Authenticated users without "alerts:write" get 403 on `POST /api/alerts/rules`
- [ ] JWT includes `permissions` claim
- [ ] Middleware returns clear error messages indicating missing permission
- [ ] Existing anonymous/public routes (login, health) are unaffected

---

## TODO-10: Mark Stub Data as Demo Mode in Frontend
**Priority:** P2  **Module:** Frontend  **Gap:** G9.4

### Context
Users see hardcoded Docker/Kafka/ES data in the UI and may think it's real. This is misleading.

### Goal
Add a visual "Demo Data" indicator when the backend returns stub/hardcoded data.

### Files to Modify
1. `crates/zentra-api/src/handlers/middleware_*.rs`
2. `frontend/src/components/MiddlewarePanel.tsx`
3. `frontend/src/components/DemoDataBadge.tsx` (new)

### Implementation Steps

1. **Add `demo_mode: true` to stub responses**:
   - In every stub handler (Docker, Kafka, ES, etc.), add a top-level field: `"_meta": { "demo_mode": true }`
   - In real handlers (Nginx, Apache), omit this field or set `false`

2. **Create `DemoDataBadge` component**:
   - A small orange/amber badge with text "Demo Data"
   - Tooltip: "This data is simulated. Connect a live instance to see real metrics."

3. **Update all middleware panel components**:
   - After fetching data, check `response._meta?.demo_mode`
   - If true, render `<DemoDataBadge />` at the top of the panel
   - Also render a "Connect Instance" CTA button if in demo mode

4. **Update `CloudMonitoring.tsx`**:
   - Since cloud metrics return 501, show a "Not Available" banner instead of trying to render charts

### Acceptance Criteria
- [ ] Docker panel shows "Demo Data" badge when viewing stub data
- [ ] Kafka panel shows "Demo Data" badge
- [ ] ES panel shows "Demo Data" badge
- [ ] Nginx panel does NOT show the badge when connected to a real instance
- [ ] Cloud monitoring shows "Integration not available" instead of error state

---

## TODO-11: Integrate OTEL Data into AI Insights Engine
**Priority:** P2  **Module:** AI + OTEL  **Gap:** G5.4

### Context
The AI insights engine (`ai.rs`) queries server metrics for anomaly detection but doesn't use OTEL trace data, which is richer for root cause analysis.

### Goal
Feed OTEL trace-derived metrics (error rate, latency, throughput) into the AI anomaly detection pipeline.

### Files to Modify
1. `crates/zentra-api/src/handlers/ai.rs`
2. `crates/zentra-api/src/ai/anomaly_detector.rs` (if exists, or create)

### Implementation Steps

1. **Add OTEL signal source to AI insights**:
   - In the `generate_insights` handler, add a new query branch for OTEL data
   - Query `traces` table for services with elevated error rates or latency in the last hour
   - Query `otlp_metrics` for services with anomalous throughput

2. **Create anomaly detection for OTEL data**:
   - Use simple statistical anomaly detection: values beyond 2 std dev from the 7-day rolling mean
   - For each anomalous service, generate an insight: `"Service {name} error rate spiked to {value}% (baseline {baseline}%)"`

3. **Add correlation**:
   - If a service shows high error rate in OTEL, check if the underlying target (server) also shows CPU/memory spikes
   - Generate a cross-signal insight: `"Error rate spike on api-gateway correlates with CPU saturation on target-01"`

4. **Update frontend** `AIInsights.tsx`:
   - Add a "Source" filter: All | Infrastructure | OTEL
   - When OTEL is selected, show trace-based insights

### Acceptance Criteria
- [ ] AI insights include OTEL-based anomalies (error rate, latency)
- [ ] Insights correlate OTEL signals with infrastructure metrics
- [ ] Frontend can filter insights by source
- [ ] Anomaly detection runs in < 500ms for the last hour of data

---

## TODO-12: Auto-Discover OTEL Services from Trace Data
**Priority:** P2  **Module:** OTEL + Discovery  **Gap:** G7.4

### Context
Service discovery only scans SNMP/SSH targets. It doesn't discover services that are sending OTEL traces, even though the `service.name` attribute uniquely identifies them.

### Goal
Auto-populate a "Discovered Services" list from OTEL `service.name` values seen in incoming traces.

### Files to Modify
1. `crates/zentra-api/src/handlers/discovery.rs` (or new `otel_discovery.rs`)
2. `crates/zentra-api/src/background_jobs.rs`
3. `frontend/src/pages/Discovery.tsx`

### Implementation Steps

1. **Add a background job** that runs every 5 minutes:
   - Query `SELECT DISTINCT service_name, service_instance_id, host_name FROM traces WHERE start_time >= NOW() - INTERVAL '1 hour'`
   - Insert or update a `discovered_services` table with fields: `id`, `service_name`, `instance_id`, `host_name`, `first_seen`, `last_seen`, `trace_count_last_hour`

2. **Add endpoint** `GET /api/otel/discovered-services`:
   - Return all rows from `discovered_services` ordered by `last_seen DESC`
   - Include a `is_mapped_to_target` boolean (join with `targets` on IP/hostname)

3. **Update Discovery page**:
   - Add an "OTEL Services" tab
   - Show a table: Service Name | Host | Last Seen | Traces/hr | Mapped Target | Actions
   - Action: "Map to Target" — links a discovered service to an existing target
   - Action: "View Traces" — navigates to Traces page filtered by service

4. **Add alerting** for new services:
   - When a new `service_name` is discovered, create an event: `"New service discovered: {name}"`

### Acceptance Criteria
- [ ] New OTEL-instrumented services appear in Discovery within 5 minutes
- [ ] Services show trace count, host, and last seen time
- [ ] Services can be mapped to existing targets
- [ ] Unmapped services trigger a "new service" event

---

## Summary: Recommended Execution Order

| Order | TODO | Effort | Impact |
|-------|------|--------|--------|
| 1 | TODO-3 (Missing CRUD endpoints) | Small | High — Unblocks frontend edit flows |
| 2 | TODO-1 (Cloud monitoring SDKs) | Large | High — Required for cloud features |
| 3 | TODO-2 (Middleware live polling) | Medium | High — Core feature credibility |
| 4 | TODO-5 (OTEL Service Health dashboard) | Medium | Medium — Major UX improvement |
| 5 | TODO-4 (OTEL in alert engine) | Medium | High — Enables OTEL-based alerting |
| 6 | TODO-9 (RBAC) | Medium | Medium — Security hardening |
| 7 | TODO-6 (Trace-to-log correlation) | Small | Medium — UX polish |
| 8 | TODO-7 (OTEL-to-target correlation) | Medium | Medium — Root cause analysis |
| 9 | TODO-10 (Demo mode badges) | Small | Low — UX honesty |
| 10 | TODO-11 (AI + OTEL) | Medium | Low — Advanced feature |
| 11 | TODO-12 (OTEL auto-discovery) | Medium | Low — Operational convenience |
| 12 | TODO-8 (Unified query) | Large | Medium — Power-user feature |

---

*— End of TODO Prompts —*
