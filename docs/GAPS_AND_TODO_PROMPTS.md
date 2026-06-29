# Zentra — Formal Gaps & TODO Prompts

**Date:** 2026-06-24  
**Version:** 1.0.0 (zentra-saas branch)  
**Author:** Engineering Review  
**Supersedes:** `GAP_ANALYSIS.md` (2026-05-31), `TODO_PROMPTS.md` (undated)

---

## Overview

Zentra is an enterprise-grade infrastructure monitoring platform built in Rust (backend), React/TypeScript (frontend), and PostgreSQL 16 + TimescaleDB. The platform is **~85% complete** as of this review. The core architecture — ingestion pipeline, RBAC, multi-tenancy, licensing, fleet-based agent, PromQL query engine, and TimescaleDB storage — is production-ready.

This document catalogs every confirmed gap, with a self-contained TODO prompt for each one that can be handed directly to an engineer or AI coding agent. Gaps are classified by severity and grouped by domain.

---

## Severity Legend

| Severity | Meaning |
|----------|---------|
| **P0 — Critical** | Blocks users from core functionality; must be resolved before GA |
| **P1 — Important** | Degrades experience or poses compliance/security risk |
| **P2 — Enhancement** | Improves quality, coverage, or developer experience |

---

## Gap Summary

| ID | Domain | Severity | Title |
|----|--------|----------|-------|
| G-01 | Middleware Monitoring | P0 | 10 of 12 middleware handlers return hardcoded/mock data |
| G-02 | Core API | P0 | Missing `PUT /targets/:id`, `PUT /alert-rules/:id`, `DELETE /alert-rules/:id`, `PUT /notifications/channels/:id` |
| G-03 | Licensing | P0 | Production public key not set; test key in `license_verifier.rs` |
| G-04 | Licensing | P0 | License records persisted in memory only — lost on restart |
| G-05 | Oracle Monitoring | P1 | TxEventQ stats endpoints return empty arrays |
| G-06 | OTEL Integration | P1 | OTEL data not used in alert evaluation, AI insights, or log correlation |
| G-07 | OTEL Instrumentation | P1 | Non-API binaries (agent, scrape, netflow, stream) emit no OTEL spans |
| G-08 | Database | P1 | Middleware instance state not persisted — lost on restart |
| G-09 | Dashboard UI | P2 | Snapshots, playlists, and bookmarks UI not implemented (DB schema exists) |
| G-10 | Testing | P2 | No integration or E2E test suite; only manual smoke testing |
| G-11 | OTEL Integration | P2 | No SLO/SLI dashboards derived from OTEL RED metrics |

---

## TODO Prompts

---

### G-01 — Middleware Handler Live Polling

**Priority:** P0 | **Domain:** Middleware Monitoring

#### Gap

10 of 12 middleware handler files use an identical broken pattern:
- Instance list stored in `Arc<RwLock<Vec<Instance>>>` (in-memory, no DB persistence)
- `get_status` returns hardcoded zeros or static structs
- `performance_history` generates data with `(i as f64 * 0.1).sin()` — fake sine waves

Affected: `middleware_docker.rs`, `middleware_kafka.rs`, `middleware_elasticsearch.rs`, `middleware_haproxy.rs`, `middleware_tomcat.rs`, `middleware_phpfpm.rs`, `middleware_rabbitmq.rs`, `middleware_redis.rs`, `middleware_memcached.rs`, `middleware_kubernetes.rs`

Reference implementations (already correct): `middleware_nginx.rs`, `middleware_apache.rs`

#### Done When
- Each handler polls the real status endpoint of the configured instance
- `performance_history` returns data from the `middleware_metrics` TimescaleDB table
- Instance registrations persist to the database, not RAM

#### Files to Modify
- All 10 handler files listed above
- `migrations/` — add `middleware_instances` table if not present
- `crates/zentra-api/src/handlers/middleware_nginx.rs` — use as reference only

#### Prompt

```
You are working on the Zentra monitoring platform (Rust/Axum backend).

Context: crates/zentra-api/src/handlers/middleware_nginx.rs is the reference
implementation — it fetches the instance from state, makes a reqwest::get() to
/nginx_status, parses the response, and returns real data.

Task: Apply the same pattern to these 10 handler files:
  middleware_docker.rs
  middleware_kafka.rs
  middleware_elasticsearch.rs
  middleware_haproxy.rs
  middleware_tomcat.rs
  middleware_phpfpm.rs
  middleware_rabbitmq.rs
  middleware_redis.rs
  middleware_memcached.rs
  middleware_kubernetes.rs

For each handler:

1. Replace the in-memory Vec store with a DB-backed repository. If a
   middleware_instances table does not exist in migrations/, create a new
   migration file (migrations/026-middleware_instances.sql) with columns:
   id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
   instance_type TEXT NOT NULL,
   name TEXT NOT NULL,
   host TEXT NOT NULL,
   port INT NOT NULL,
   username TEXT,
   password_enc TEXT,  -- AES-256 via zentra-core crypto
   created_at TIMESTAMPTZ DEFAULT now(),
   org_id UUID REFERENCES organizations(id) ON DELETE CASCADE

2. In get_status, build the correct polling URL per type:
   - Docker:        GET http://{host}:{port}/v1.24/info
   - Kafka:         GET http://{host}:{port}/metrics  (JMX Exporter assumed)
   - Elasticsearch: GET http://{host}:{port}/_cluster/health
                    GET http://{host}:{port}/_cluster/stats
   - HAProxy:       GET http://{host}:{port}/stats?csv
   - Tomcat:        GET http://{host}:{port}/manager/status?XML=true
   - PHP-FPM:       GET http://{host}:{port}/status?json
   - RabbitMQ:      GET http://{host}:{port}/api/overview
   - Redis:         GET http://{host}:{port}/metrics  (redis_exporter assumed)
   - Memcached:     TCP stats command via tokio::net::TcpStream + "stats\r\n"
   - Kubernetes:    GET http://{host}:{port}/metrics  (metrics-server)

3. Cache the last successful poll result in AppState for 15 seconds to avoid
   hammering the instance on every frontend poll.

4. Replace the sin-wave performance_history implementation with a real query
   against the middleware_metrics hypertable:
     SELECT time_bucket('1 minute', collected_at) AS bucket,
            avg(value) AS value
     FROM middleware_metrics
     WHERE instance_id = $1 AND metric_name = 'cpu_usage'
     AND collected_at > now() - INTERVAL '1 hour'
     GROUP BY bucket ORDER BY bucket

5. Update CreateInstanceRequest to include optional username, password, tls_cert
   fields. Encrypt password with zentra-core AES-256 before storing.

Do not change routes or response shape — only the internals of get_status
and performance_history.
```

---

### G-02 — Missing CRUD Endpoints (Targets, Alert Rules, Notification Channels)

**Priority:** P0 | **Domain:** Core API

#### Gap

The frontend calls update and delete endpoints that do not exist in the backend:

| Frontend Call | Status |
|---------------|--------|
| `PUT /api/targets/:id` | ❌ Missing |
| `PUT /api/alert-rules/:id` | ❌ Missing |
| `DELETE /api/alert-rules/:id` | ❌ Missing |
| `PUT /api/notifications/channels/:id` | ❌ Missing |

#### Done When
All four endpoints exist, are auth-protected, and return the updated/deleted resource.

#### Files to Modify
- `crates/zentra-api/src/routes/targets.rs`
- `crates/zentra-api/src/handlers/targets.rs`
- `crates/zentra-api/src/routes/alerts.rs`
- `crates/zentra-api/src/handlers/alerts.rs`
- `crates/zentra-api/src/routes/notifications.rs`
- `crates/zentra-api/src/handlers/notifications.rs`

#### Prompt

```
You are working on the Zentra monitoring platform (Rust/Axum backend).

Add four missing endpoints to stop frontend 404 errors.

1. PUT /api/targets/:id (routes/targets.rs + handlers/targets.rs)
   - Accept Json<UpdateTargetRequest> where every field is Option<T>
   - Build a dynamic UPDATE query using COALESCE($n, existing_column) pattern
   - Return the updated target as Json<Target> or 404 if not found
   - Require JWT auth (use the existing auth extractor)

2. PUT /api/alert-rules/:id (routes/alerts.rs + handlers/alerts.rs)
   - Accept Json<UpdateAlertRuleRequest> (all fields optional)
   - Update the alert_rules table row
   - After update, call the existing alert_evaluator to re-evaluate the rule
     immediately so alert_state reflects any threshold change
   - Return the updated alert rule or 404

3. DELETE /api/alert-rules/:id (routes/alerts.rs + handlers/alerts.rs)
   - Delete from alert_rules WHERE id = $1 AND org_id = $2
   - Cascade delete alert_history entries for this rule (or rely on FK ON DELETE CASCADE)
   - Return 204 No Content on success, 404 if not found

4. PUT /api/notifications/channels/:id (routes/notifications.rs + handlers/notifications.rs)
   - Accept Json<UpdateChannelRequest> (name, type, config all optional)
   - Update the notification_channels table row
   - Return the updated channel or 404

All four endpoints must:
- Extract org_id from the JWT claims (same pattern as existing handlers)
- Use the existing sqlx pool from AppState
- Return consistent JSON error bodies on failure: {error: "...", code: "..."}
```

---

### G-03 — Production License Key Not Set

**Priority:** P0 | **Domain:** Licensing / Security

#### Gap

`crates/zentra-core/src/crypto/license_verifier.rs:142` contains a `TODO` comment:

```
// TODO(T05): Replace this test public key with the production key before release
const PUBLIC_KEY_PEM: &str = "...test key...";
```

Shipping with the test key means any license signed against the test private key will be accepted in production, which defeats the purpose of signed license enforcement.

#### Done When
- A real Ed25519 production key pair is generated
- The public key PEM is committed to `license_verifier.rs`
- The private key is stored securely in a secrets manager (not in the repo)
- The `tools/license-generator/` tool is updated to use the production private key path

#### Prompt

```
You are working on the Zentra monitoring platform.

Security task: Replace the test license key with a production key pair.

1. Generate an Ed25519 key pair using the OpenSSL CLI:
     openssl genpkey -algorithm ed25519 -out zentra_license_private.pem
     openssl pkey -in zentra_license_private.pem -pubout -out zentra_license_public.pem

2. Open crates/zentra-core/src/crypto/license_verifier.rs.
   Find the const PUBLIC_KEY_PEM line (near line 142, marked TODO T05).
   Replace the test key value with the content of zentra_license_public.pem.

3. Update tools/license-generator/ (likely a Rust or Python script):
   - Change it to read the private key from an env var ZENTRA_LICENSE_PRIVATE_KEY_PATH
     rather than a hardcoded path or embedded test key
   - Document this in tools/license-generator/README.md

4. Add zentra_license_private.pem to .gitignore (critical — never commit the private key)

5. Document the key rotation procedure in docs/OPERATIONS_GUIDE.md under a new
   "License Key Management" section.

Do NOT commit the private key to the repository under any circumstances.
```

---

### G-04 — License Records Lost on Restart

**Priority:** P0 | **Domain:** Licensing

#### Gap

The license management handlers (`crates/zentra-api/src/handlers/licenses.rs`) store license records in an `Arc<RwLock<Vec<License>>>` in-memory structure. All license records are lost when the API process restarts.

#### Done When
- License records are stored in the `licenses` PostgreSQL table (migration already exists)
- All CRUD operations (`create`, `list`, `get`, `revoke`) use SQLx queries against the DB
- The in-memory `RwLock<Vec>` is removed

#### Files to Modify
- `crates/zentra-api/src/handlers/licenses.rs`
- `crates/zentra-db/src/repositories/licenses.rs` (already exists — wire it up)

#### Prompt

```
You are working on the Zentra monitoring platform (Rust/Axum backend).

File: crates/zentra-api/src/handlers/licenses.rs

The handlers currently use Arc<RwLock<Vec<License>>> as an in-memory store.
The licenses table and repository already exist in crates/zentra-db/src/repositories/licenses.rs.

Task: Replace every in-memory read/write in the license handlers with calls to
the DB repository.

1. Remove the Arc<RwLock<Vec<License>>> fields from AppState (or the handler state).

2. In create_license:
   - Call licenses_repo::insert(&pool, &new_license).await?
   - Return the inserted license

3. In list_licenses:
   - Call licenses_repo::list_by_org(&pool, &org_id).await?
   - Return the list

4. In get_license:
   - Call licenses_repo::get_by_id(&pool, &license_id).await?
   - Return 404 if None

5. In revoke_license:
   - Call licenses_repo::revoke(&pool, &license_id, &org_id).await?
   - Return 204 on success, 404 if not found

Follow the same error-handling pattern as the existing metrics and alerts
repositories. Use the AppState pool (state.pool).
```

---

### G-05 — Oracle TxEventQ Endpoints Not Implemented

**Priority:** P1 | **Domain:** Database Monitoring — Oracle

#### Gap

Three endpoints in `crates/zentra-api/src/handlers/oracle.rs` always return empty arrays with the comment `// TODO: Query actual TxEventQ stats from connected Oracle instances`:
- `GET /api/oracle/txeventq`
- `GET /api/oracle/txeventq/stats`
- `GET /api/oracle/txeventq/:queue_name`

#### Done When
- Endpoints query a live Oracle instance via the configured OCI connection and return real TxEventQ queue data

#### Files to Modify
- `crates/zentra-api/src/handlers/oracle.rs`

#### Prompt

```
You are working on the Zentra monitoring platform (Rust/Axum backend).

File: crates/zentra-api/src/handlers/oracle.rs

Three TxEventQ handlers (txeventq_list, txeventq_stats, txeventq_detail)
always return empty data. Implement real queries.

1. Use the existing Oracle connection mechanism (check how other oracle handlers
   retrieve connection info from the targets table or an in-memory store).

2. Implement txeventq_list:
   Query: SELECT queue_name, queue_table, queue_type, enqueue_enabled,
                  dequeue_enabled, user_comment
          FROM dba_queues
          WHERE queue_type = 'NORMAL_QUEUE'
          ORDER BY queue_name
   Return as Vec<TxEventQQueue>

3. Implement txeventq_stats:
   Query: SELECT q.queue_name,
                  NVL(s.enqueue_msgs, 0) AS enqueue_msgs,
                  NVL(s.dequeue_msgs, 0) AS dequeue_msgs,
                  NVL(s.msgs_in_queue, 0) AS msgs_in_queue,
                  NVL(s.elapsed_enqueue_time, 0) AS elapsed_enqueue_time
          FROM dba_queues q
          LEFT JOIN v$persistent_queues s ON q.queue_name = s.queue_name
          WHERE q.queue_type = 'NORMAL_QUEUE'
   Return as Vec<TxEventQStat>

4. Implement txeventq_detail (takes :queue_name path param):
   Query same as above but WHERE q.queue_name = :1

5. If Oracle connectivity is not configured for the requested target, return
   404 with body {error: "No Oracle connection configured for this target"}.

Use the sibyl or oracle-db crate that is already in Cargo.toml (check which
Oracle driver the existing oracle handlers use before adding new deps).
```

---

### G-06 — OTEL Data Isolated from Alerts, AI Insights, and Log Correlation

**Priority:** P1 | **Domain:** OTEL Integration

#### Gap

The OTEL ingest pipeline (`zentra-ingestion`) is production-ready and stores traces, metrics, and logs in TimescaleDB. However, no other subsystem consumes this data:

| Consumer | Uses OTEL Data? | Gap |
|----------|----------------|-----|
| Alert evaluator | ❌ No | Can't fire on `http.status_code >= 500` or high P99 latency |
| AI insights | ❌ No | Doesn't analyze traces for root cause |
| Log Explorer | ❌ No | Can't pivot log entry → traces with matching `trace_id` |
| Command Center | ❌ No | Shows no OTEL signal health |
| Anomaly detection | ❌ No | Uses infrastructure metrics only |

#### Done When
- Alert rules can reference OTEL metric names and OTEL semantic convention attributes
- Log Explorer shows a "View Traces" link when a log entry has a `trace_id`
- The Command Center shows OTEL ingest throughput, error rate, and P99 latency panels
- AI insights endpoint queries `otlp_traces` for recent errors when generating recommendations

#### Files to Modify
- `crates/zentra-api/src/handlers/alert_rules.rs`
- `crates/zentra-api/src/handlers/ai_insights.rs`
- `frontend/src/pages/LogExplorer.tsx`
- `frontend/src/pages/Dashboard.tsx` (Command Center)

#### Prompt

```
You are working on the Zentra monitoring platform (Rust/Axum backend + React frontend).

The OTEL ingest pipeline stores data in three TimescaleDB tables:
  - otlp_metrics (time, metric_name, value, labels jsonb, org_id)
  - otlp_traces  (trace_id, span_id, service_name, name, duration_ns,
                  status_code, attributes jsonb, org_id)
  - otlp_logs    (time, trace_id, span_id, severity, body, attributes jsonb, org_id)

Task 1 — Alert rules for OTEL metrics:
In crates/zentra-api/src/handlers/alert_rules.rs, extend CreateAlertRuleRequest
with a new optional field: otel_source: Option<String> (e.g. "otlp_metrics").
In the alert evaluator (alert_evaluator.rs), when otel_source is set, query
otlp_metrics instead of the infrastructure metrics table. Support label
filters via a labels_filter: Option<HashMap<String, String>> field that
translates to: labels @> $filter::jsonb

Task 2 — Log → Trace correlation:
In crates/zentra-api/src/handlers/logs.rs, add a new endpoint:
  GET /api/logs/:log_id/traces
  - Look up the log entry, extract trace_id
  - Query: SELECT * FROM otlp_traces WHERE trace_id = $1 AND org_id = $2
  - Return Vec<OtlpSpan>

In frontend/src/pages/LogExplorer.tsx, add a "View Traces" button next to
any log row where trace_id is not null. Clicking it navigates to
/traces?trace_id={trace_id}.

Task 3 — Command Center OTEL panels:
In frontend/src/pages/Dashboard.tsx, add a new row of three stat cards:
  - "OTEL Ingest Rate": query GET /api/otlp/stats?window=5m → spans/sec
  - "Service Error Rate": query GET /api/otlp/error-rate?window=5m → %
  - "P99 Latency": query GET /api/otlp/p99?window=5m → ms

Add the two supporting API endpoints in a new file
crates/zentra-api/src/handlers/otlp_stats.rs, querying otlp_traces.

Task 4 — AI insights use trace data:
In crates/zentra-api/src/handlers/ai_insights.rs, before calling the LLM,
fetch recent high-error spans:
  SELECT service_name, name, count(*) as error_count
  FROM otlp_traces
  WHERE status_code = 'ERROR' AND org_id = $1
    AND time > now() - INTERVAL '1 hour'
  GROUP BY service_name, name
  ORDER BY error_count DESC
  LIMIT 10
Include this data in the prompt context sent to the LLM.
```

---

### G-07 — Non-API Binaries Emit No OTEL Spans

**Priority:** P1 | **Domain:** Observability / OTEL Instrumentation

#### Gap

Only `zentra-api` initializes a tracer provider. The following binaries/crates use `tracing::` macros but produce no OTEL spans visible in the trace backend:
- `zentra-agent` — fleet collector
- `zentra-scrape` — Prometheus scraper
- `zentra-netflow` — NetFlow UDP collector
- `zentra-stream` — event bus / alert routing

Additionally, `tokio::spawn` calls in these crates lose the trace context (no `Span::current().context()` propagation).

#### Done When
- All four binaries initialize an OTLP tracer provider on startup
- Key async tasks are wrapped in explicit spans
- `tokio::spawn` calls propagate context via `tracing::Instrument`

#### Files to Modify
- `crates/zentra-agent/src/main.rs`
- `crates/zentra-scrape/src/main.rs`
- `crates/zentra-netflow/src/main.rs`
- `crates/zentra-stream/src/main.rs`
- Each crate's `Cargo.toml` (add `opentelemetry`, `opentelemetry_otlp`, `tracing-opentelemetry`)

#### Prompt

```
You are working on the Zentra monitoring platform (Rust).

Context: crates/zentra-api/src/telemetry.rs contains init_tracer() which sets
up an OTLP HTTP exporter. Use the same pattern to instrument four other binaries.

Task: In each of the following main.rs files, add OTEL initialization:
  crates/zentra-agent/src/main.rs
  crates/zentra-scrape/src/main.rs
  crates/zentra-netflow/src/main.rs
  crates/zentra-stream/src/main.rs

Steps for each binary:

1. Add to the crate's Cargo.toml (copy exact versions from zentra-api/Cargo.toml):
   opentelemetry = { version = "...", features = ["rt-tokio"] }
   opentelemetry_otlp = { version = "...", features = ["http-proto", "reqwest-client"] }
   opentelemetry-semantic-conventions = "..."
   tracing-opentelemetry = "..."

2. In main(), before starting the main loop, call:
   zentra_core::telemetry::init_tracer("zentra-agent")  (or the binary name)
   Read the OTLP endpoint from OTEL_EXPORTER_OTLP_ENDPOINT env var (default
   "http://localhost:4318").

3. Wrap the top-level async task in each binary's main loop with a span:
   For zentra-agent: instrument the heartbeat loop and each plugin poll cycle
   For zentra-scrape: instrument each scrape_target() call with span attributes:
     target.url, target.job_name, scrape.duration_ms
   For zentra-netflow: instrument each flow batch processing call with:
     netflow.version, netflow.flow_count
   For zentra-stream: instrument each event dispatch with:
     event.type, event.channel_count

4. For all tokio::spawn calls in these binaries, propagate context:
   Replace:
     tokio::spawn(async move { ... })
   With:
     tokio::spawn(async move { ... }.instrument(tracing::Span::current()))

5. On process shutdown (SIGTERM/SIGINT handler), call:
   opentelemetry::global::shutdown_tracer_provider();

Do not change any business logic — only add instrumentation.
```

---

### G-08 — Middleware Instance State Lost on Restart

**Priority:** P1 | **Domain:** Data Persistence

#### Gap

All 12 middleware handler files store their instance lists in `Arc<RwLock<Vec<Instance>>>`. This data is not written to the database. Any middleware instances (Docker, Kafka, Elasticsearch, etc.) registered via the API are lost when the process restarts.

**Note:** This gap is partly addressed by G-02. If G-02 is completed first, G-11 is automatically resolved. Track as a dependency.

#### Done When
- Middleware instance registrations persist to the `middleware_instances` table in PostgreSQL
- On startup, handlers load their instance list from the DB

#### Prompt

```
This gap is resolved as part of G-01 (Middleware Handler Live Polling), step 1
of that prompt. Implement G-01 first. Verify by:
  1. Registering a Docker instance via POST /api/middleware/docker
  2. Restarting the API process
  3. Confirming GET /api/middleware/docker returns the instance
```

---

### G-09 — Dashboard Snapshots, Playlists, and Bookmarks UI Not Built

**Priority:** P2 | **Domain:** Frontend / Dashboard

#### Gap

Migration `025-dashboard_features.sql` created three tables: `dashboard_snapshots`, `dashboard_playlists`, and `dashboard_bookmarks`. The backend may have CRUD endpoints, but the frontend has no UI for these features.

#### Done When
- Users can save a dashboard snapshot with a name and timestamp
- Users can create a playlist of dashboards that auto-rotates
- Users can bookmark a specific time range + filter state

#### Prompt

```
You are working on the Zentra monitoring platform (React/TypeScript frontend).

Context: The backend already has dashboard snapshot and bookmark endpoints
(check by grepping for "snapshot" in crates/zentra-api/src/routes/).

Task: Add three UI features to frontend/src/pages/Dashboard.tsx and
frontend/src/components/DashboardToolbar.tsx:

1. Snapshot button (camera icon in toolbar):
   - On click, opens a Dialog with a "Snapshot Name" input
   - POST /api/dashboards/:id/snapshots with { name, description }
   - Show success toast "Snapshot saved"
   - Add a "Snapshots" tab in dashboard settings that lists saved snapshots
     with a "Restore" button (GET /api/dashboards/snapshots/:id → load panels)

2. Playlist mode (play icon in toolbar):
   - On click, opens a Dialog to create a playlist: name + list of dashboard IDs
     + rotation interval (30s, 60s, 5m)
   - POST /api/dashboards/playlists
   - A "Start Playlist" button launches fullscreen mode with auto-rotation
     using setInterval(() => navigate to next dashboard, interval)

3. Bookmark (bookmark icon in toolbar):
   - On click, saves current time range + active filters as a bookmark
   - POST /api/dashboards/bookmarks with { name, time_range, filters }
   - A bookmarks dropdown in the toolbar lists saved bookmarks
   - Clicking a bookmark applies that time range and filters

Use existing UI patterns (Dialog, Button, toast) from the component library
already in use in this project (likely shadcn/ui or similar — check imports
in existing Dialog usage).
```

---

### G-10 — No Integration or E2E Test Suite

**Priority:** P2 | **Domain:** Testing

#### Gap

There is no automated integration or end-to-end test suite. The only test verification to date is manual 731-endpoint smoke testing (commit `86eec90f` fixed 8 bugs found this way). Unit tests exist in some Rust crates but are minimal.

#### Done When
- A Rust integration test crate tests all P0 API endpoints against a real test DB
- A Playwright or Cypress E2E suite covers the top 10 user flows in the frontend
- Tests run in CI (cloudbuild.yaml or a separate GitHub Actions workflow)

#### Prompt

```
You are working on the Zentra monitoring platform.

Task: Set up a minimal but real integration test suite covering all P0 endpoints.

Part 1 — Rust integration tests (crates/zentra-api/tests/integration_test.rs):

1. Add a test binary to crates/zentra-api/Cargo.toml:
   [[test]]
   name = "integration"
   path = "tests/integration_test.rs"

2. In tests/integration_test.rs, use axum::test utils or reqwest::Client
   against a test server started with:
     let app = zentra_api::build_app(test_pool()).await;
     let server = TestServer::new(app).unwrap();

3. Write tests for:
   - POST /api/auth/login → 200 with JWT
   - GET /api/targets (with JWT) → 200 with array
   - POST /api/targets → 201 with created target
   - PUT /api/targets/:id → 200 with updated target
   - DELETE /api/targets/:id → 204
   - POST /api/alert-rules → 201
   - GET /api/ha/cluster → 200 (not 500)
   - GET /api/health → 200

4. Use a PostgreSQL test database (spin up via docker run in CI or use
   a DATABASE_URL_TEST env var pointing to a separate DB).

Part 2 — CI integration (cloudbuild.yaml or .github/workflows/test.yml):

Add a test step that:
  1. Starts a PostgreSQL 16 + TimescaleDB container
  2. Runs migrations (cargo sqlx migrate run)
  3. Runs cargo test --test integration
  4. Tears down the container

Do not add E2E browser tests in this PR — that is a separate effort.
```

---

### G-11 — No SLO/SLI Dashboards from OTEL RED Metrics

**Priority:** P2 | **Domain:** OTEL Integration / Observability

#### Gap

OTEL traces are stored and queryable, but there are no Service Level Objective dashboards showing error budgets, availability %, or latency SLOs derived from RED metrics (Rate, Errors, Duration).

#### Done When
- A new "Service Reliability" page shows per-service: request rate, error rate, and P50/P95/P99 latency
- Users can define SLO targets (e.g., "99.9% of requests < 500ms") and see current burn rate
- A time-series chart shows error budget remaining over the rolling 30-day window

#### Prompt

```
You are working on the Zentra monitoring platform (Rust backend + React frontend).

Task: Implement a Service Reliability page backed by OTEL trace data.

Part 1 — Backend endpoints (new file: crates/zentra-api/src/handlers/slo.rs):

1. GET /api/slo/services?window=1h
   Query otlp_traces to compute per-service RED metrics:
   SELECT service_name,
          count(*) AS total_requests,
          count(*) FILTER (WHERE status_code = 'ERROR') AS error_count,
          round(count(*) FILTER (WHERE status_code = 'ERROR')::numeric /
                nullif(count(*),0) * 100, 2) AS error_rate_pct,
          percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ns)/1e6 AS p50_ms,
          percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ns)/1e6 AS p95_ms,
          percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ns)/1e6 AS p99_ms
   FROM otlp_traces
   WHERE org_id = $1 AND time > now() - $2::interval
   GROUP BY service_name
   ORDER BY total_requests DESC

2. GET /api/slo/targets → list user-defined SLO targets from a new slo_targets table
   POST /api/slo/targets → create an SLO target
   Schema: { service_name, metric ("error_rate"|"p99_latency"), threshold, window_days }

3. GET /api/slo/budget/:service_name?window=30d
   Compute remaining error budget:
   budget_remaining = target_availability - actual_availability
   Return: { budget_seconds_total, budget_seconds_used, budget_seconds_remaining, burn_rate }

Part 2 — Frontend (new file: frontend/src/pages/ServiceReliability.tsx):

1. Add route /service-reliability to App.tsx router
2. Add "Service Reliability" link to the sidebar under "Analytics"
3. Page layout:
   - Top: a table of services with columns: Name, Req/s, Error Rate, P50, P95, P99, SLO Status
   - Middle: clicking a service opens a detail panel with 24h time-series charts
     (AreaChart for request rate, LineChart for latency percentiles)
   - Bottom: SLO configuration section to add/edit SLO targets

Wire up all API calls using the existing React Query pattern in the codebase.
```

---

## Completion Checklist

| ID | Title | Priority | Owner | Status |
|----|-------|----------|-------|--------|
| G-01 | Middleware Handler Live Polling | P0 | — | ⬜ Open |
| G-02 | Missing CRUD Endpoints | P0 | — | ⬜ Open |
| G-03 | Production License Key | P0 | — | ⬜ Open |
| G-04 | License Records Persistence | P0 | — | ⬜ Open |
| G-05 | Oracle TxEventQ Endpoints | P1 | — | ⬜ Open |
| G-06 | OTEL Integration (Alerts, AI, Logs) | P1 | — | ⬜ Open |
| G-07 | Non-API Binary OTEL Instrumentation | P1 | — | ⬜ Open |
| G-08 | Middleware Instance Persistence | P1 | — | ⬜ Blocked by G-01 |
| G-09 | Dashboard Snapshots/Playlists UI | P2 | — | ⬜ Open |
| G-10 | Integration & E2E Test Suite | P2 | — | ⬜ Open |
| G-11 | SLO/SLI Dashboards | P2 | — | ⬜ Open |

---

*This document was generated by engineering review on 2026-06-24 against the `zentra-saas` branch at commit `342ce972`. Update the checklist as gaps are closed.*
