# TODO — OTEL Integration Gaps (Zentra)

Generated: 2026-05-30
Scope: Rust workspace at `C:\git\Zentra` (workspace pulls in `opentelemetry 0.27`, `opentelemetry_sdk`, `opentelemetry-otlp`, `opentelemetry-semantic-conventions`, `tracing-opentelemetry 0.28`) plus the React `frontend/`.

Baseline today: only `zentra-api` initializes a tracer provider (`crates/zentra-api/src/otel_self.rs`) and exports OTLP over HTTP to a configurable endpoint (default self-loop `http://127.0.0.1:5006`, 10% `TraceIdRatioBased` sampler). Every other crate uses `tracing` macros only and never emits OTEL spans/metrics. The frontend has zero instrumentation. The K8s manifest sets no OTEL env vars.

Each item below is structured as: **Gap → Why it matters → Engineer prompt** (drop into Claude Code / Cursor as-is).

---

## CRITICAL

### 1. Async context lost on `tokio::spawn` in zentra-stream

**Gap:** `crates/zentra-stream/src/{stream_processor,notification_router,alert_engine,escalation_engine}.rs` spawn background tasks without inheriting the current span context. Trace IDs diverge between the request that produced an event and the worker that handles it.

**Why it matters:** Alert routing, escalation, and notification delivery cannot be correlated back to the originating ingest/API call — the most useful end-to-end traces in the product are broken.

**Prompt:**
> In `crates/zentra-stream/`, audit every `tokio::spawn(...)` call site. For each, wrap the future with `.in_current_span()` from `tracing::Instrument`, OR explicitly capture `let span = tracing::Span::current();` outside the spawn and `let _g = span.enter();` inside, OR pass an explicit `opentelemetry::Context` and re-attach with `Context::attach`. Add a `#[instrument(skip_all, fields(event.id, alert.id))]` to the top-level handler function inside each spawned task. Verify with a unit test that emits a parent span, spawns a child task, and asserts the child span's `trace_id` equals the parent's (use `tracing_subscriber::registry` + an in-memory layer). Files to start with: `stream_processor.rs`, `notification_router.rs`, `alert_engine.rs`, `escalation_engine.rs`.

---

### 2. Frontend completely uninstrumented (no RUM, no traceparent)

**Gap:** `frontend/package.json` has no `@opentelemetry/*` deps. `frontend/src/lib/api/_client.ts` does not inject `traceparent` headers. No web-vitals, no fetch instrumentation, no user-interaction tracing.

**Why it matters:** 100% of user-side latency and errors are invisible. Backend traces start at the API edge with no upstream parent, so there is no true end-to-end view.

**Prompt:**
> Add browser RUM to `frontend/`. Install `@opentelemetry/api`, `@opentelemetry/sdk-trace-web`, `@opentelemetry/context-zone`, `@opentelemetry/instrumentation-fetch`, `@opentelemetry/instrumentation-xml-http-request`, `@opentelemetry/instrumentation-document-load`, `@opentelemetry/instrumentation-user-interaction`, `@opentelemetry/exporter-trace-otlp-http`, `@opentelemetry/resources`, `@opentelemetry/semantic-conventions`. Create `frontend/src/lib/otel/init.ts` that builds a `WebTracerProvider` with `service.name=zentra-frontend`, resource attrs from `import.meta.env`, batch span processor, OTLP HTTP exporter pointing at `VITE_OTEL_EXPORTER_OTLP_ENDPOINT`, and registers a W3C `TraceContextPropagator`. Configure `FetchInstrumentation` with `propagateTraceHeaderCorsUrls` matching the API origin so `traceparent` is injected on every API call. Wire `init.ts` as the first import in `frontend/src/main.tsx`. Confirm `frontend/src/lib/api/_client.ts` does not strip OTEL headers. Update `frontend/.env.example` with the new env var.

---

### 3. zentra-db has no per-query spans

**Gap:** `crates/zentra-db/src/repositories/*.rs` execute sqlx queries with no span wrapping. DB latency, query text, and errors never appear in trace waterfalls.

**Why it matters:** Customers running Zentra cannot see *Zentra's own* DB hot spots; internal debugging of slow queries relies on log scraping.

**Prompt:**
> Add OTEL semantic-convention DB spans to every repository method in `crates/zentra-db/src/repositories/`. Create a helper `crates/zentra-db/src/otel.rs` exposing `async fn traced_query<F, T>(op: &str, statement: &str, table: &str, fut: F) -> Result<T>` that opens a span with `db.system=postgresql` (or `oracle` when applicable), `db.operation`, `db.statement` (truncated to 1KB), `db.sql.table`, and `otel.kind=client`, records duration as the span, and sets `otel.status_code=error` on Err. Wrap every `sqlx::query*` and `oracle::*` call site in repositories. Add `opentelemetry`, `opentelemetry-semantic-conventions`, `tracing-opentelemetry` to `crates/zentra-db/Cargo.toml`. Do not log `db.statement` for queries that touch `users`, `secrets`, or `audit` tables — set `db.statement.redacted=true` instead.

---

### 4. Plugins emit no spans, no OTEL deps

**Gap:** `plugins/{network,database,storage,server,license,security,ai,cloud}/Cargo.toml` have no `opentelemetry*` dependencies. They emit `tracing::info!/warn!/debug!` only.

**Why it matters:** Domain-specific extraction (packet capture, schema discovery, S3 transfers, LLM calls, RBAC checks) is the actual business value of the product and is entirely unobservable.

**Prompt:**
> For each crate under `plugins/`, add `opentelemetry`, `tracing-opentelemetry`, and `opentelemetry-semantic-conventions` to `Cargo.toml`. Identify the 3–5 public entry-point functions per plugin (e.g. `network-plugin::capture`, `database-plugin::extract_schema`, `storage-plugin::upload`, `ai-plugin::invoke_llm`, `security-plugin::authorize`). Add `#[instrument(skip_all, fields(...))]` to each with plugin-appropriate semantic conventions: for `ai-plugin` use `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`; for `storage-plugin` use `cloud.provider`, `aws.s3.bucket`, `http.response.status_code`; for `database-plugin` use the same `db.*` attrs as item 3; for `network-plugin` use `net.peer.name`, `net.peer.port`, `net.transport`. Record errors with `span.record_error(&err)` (use `tracing::error!(error = ?err)` equivalent). Do not initialize a tracer provider inside plugins — they inherit the host binary's provider via the global registry.

---

## HIGH

### 5. No tracer provider in non-API binaries (agent, scrape, netflow, stream)

**Gap:** Only `crates/zentra-api/src/main.rs` calls `init_tracer()`. Background binaries (`zentra-agent`, `zentra-scrape`, `zentra-netflow`, any standalone `zentra-stream` worker) never register a `TracerProvider`, so spans they create are discarded.

**Why it matters:** Even if items 1–4 land, those binaries' spans go nowhere.

**Prompt:**
> Promote `crates/zentra-api/src/otel_self.rs` to a shared module. Move it to `crates/zentra-core/src/otel.rs` (or create a new `crates/zentra-telemetry/` crate if cleaner) exposing `pub fn init_tracer(service_name: &str) -> Result<TracerProvider>` and `pub fn shutdown_tracer()`. Read `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME` (override), `OTEL_RESOURCE_ATTRIBUTES`, `ZENTRA_OTEL_SAMPLE_RATIO`, `ZENTRA_OTEL_SELF_ENABLED` from env. Call it from `main()` in `zentra-api`, `zentra-agent`, `zentra-scrape`, `zentra-netflow`, and any binary-producing crate. Each binary passes its own `service_name`. Add a `ctrl_c` handler that invokes `shutdown_tracer()` to flush the batch processor. Verify spans from `zentra-agent` show up at the configured OTLP endpoint with `service.name=zentra-agent`.

---

### 6. No metrics outside zentra-api

**Gap:** Only `crates/zentra-api/src/ingest_queue.rs` and a few handler files emit `metrics::counter!` / histograms. Stream throughput, ingestion error rates, ML inference latency, query duration percentiles are uncounted.

**Why it matters:** SRE cannot alert on Zentra's own internal health without metrics; relying on log parsing does not scale.

**Prompt:**
> Define a shared meter in the module produced by item 5: `pub fn meter(name: &'static str) -> opentelemetry::metrics::Meter`. Add metrics to the following crates with these names (use OTEL semantic conventions where they exist):
> - `zentra-ingestion`: `zentra.ingestion.payloads` counter (attrs: `signal`, `status`), `zentra.ingestion.payload_bytes` histogram, `zentra.ingestion.duration` histogram
> - `zentra-stream`: `zentra.stream.events` counter (attrs: `event.type`, `status`), `zentra.stream.lag_seconds` gauge, `zentra.stream.queue_depth` updown counter
> - `zentra-query`: `zentra.query.duration` histogram (attrs: `query.type=traceql|logql|sql`), `zentra.query.results` counter
> - `zentra-predict` / `zentra-ml`: `zentra.ml.inference_duration` histogram (attrs: `model.name`, `model.version`), `zentra.ml.predictions` counter
> - `zentra-db`: `db.client.operation.duration` histogram (attrs: `db.system`, `db.operation`)
>
> Wire a `PeriodicReader` with OTLP metrics exporter in `init_tracer` (rename to `init_telemetry`). Document the metric catalog in `docs/observability/metrics.md`.

---

### 7. K8s manifest missing OTEL env vars

**Gap:** `deploy/k8s/zentra-deployment.yaml` ConfigMap and Deployment have no `OTEL_*` keys. Self-instrumentation effectively off in prod.

**Why it matters:** Production has no Zentra-of-Zentra observability.

**Prompt:**
> Edit `deploy/k8s/zentra-deployment.yaml`. Add to the ConfigMap: `OTEL_EXPORTER_OTLP_ENDPOINT` (default to the in-cluster collector service from item 17), `OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf`, `ZENTRA_OTEL_SELF_ENABLED=true`, `ZENTRA_OTEL_SAMPLE_RATIO=0.1`. On each Deployment spec, set `OTEL_SERVICE_NAME` (per-deployment), and `OTEL_RESOURCE_ATTRIBUTES=service.namespace=zentra,deployment.environment=$(ENVIRONMENT),k8s.pod.name=$(POD_NAME),k8s.namespace.name=$(POD_NAMESPACE),k8s.node.name=$(NODE_NAME)` using the downward API for the `$(POD_*)` values. Repeat for any Helm chart under `deploy/helm/` if present.

---

### 8. No W3C extract on non-HTTP sources

**Gap:** `zentra-agent`, `zentra-netflow`, and `zentra-scrape` ingest data over non-HTTP transports (custom protocols, UDP, scrape pull). No `TextMapPropagator::extract` call exists in those crates.

**Why it matters:** Cross-system correlation impossible — agent-collected telemetry can never be parent-linked to customer app spans.

**Prompt:**
> For each of `zentra-agent`, `zentra-netflow`, `zentra-scrape`: (a) identify the wire format used (proto schema / packet headers / scrape protocol). (b) Add a `traceparent` (and optional `tracestate`) field to the wire schema where the producer can supply it. (c) On the receive path, build an `opentelemetry::propagation::Extractor` over the incoming carrier, call `global::get_text_map_propagator(|p| p.extract(&carrier))`, and use the returned `Context` as the parent for the top-level processing span via `Span::set_parent(cx)` from `tracing-opentelemetry::OpenTelemetrySpanExt`. (d) Document the new field in `docs/protocols/`. (e) Add a roundtrip integration test in `tests/` that injects a known `traceparent` and asserts the receive-side span has the matching `trace_id`.

---

### 9. Resource attrs only set in zentra-api

**Gap:** Even when other services are wired (items 5–6), they need uniform resource attrs. Today only `zentra-api` sets `service.name/version/namespace`.

**Why it matters:** Collector cannot fan out / route / tail-sample by service.

**Prompt:**
> Inside the shared `init_telemetry` (item 5), always attach the following resource attributes from env, defaulting safely:
> `service.name` (required, panic if missing), `service.version` (from `CARGO_PKG_VERSION`), `service.namespace=zentra`, `service.instance.id` (UUID v7 generated at startup), `deployment.environment` (from `ZENTRA_ENV`, default `dev`), plus anything in `OTEL_RESOURCE_ATTRIBUTES`. Use `opentelemetry_sdk::Resource::from_detectors` with the env detector so users can extend via env without code change. Verify in a unit test that calling `init_telemetry("zentra-test")` yields a Resource containing all five required keys.

---

## MEDIUM

### 10. Inconsistent semantic conventions

**Gap:** The ~10 manual spans in `zentra-api` mostly use ad-hoc field names (`operation_name`, `user`). They do not consistently apply `http.*`, `db.*`, `messaging.*`, `rpc.*` conventions.

**Why it matters:** Out-of-the-box dashboards (Grafana/Honeycomb/Tempo) expect semconv attribute names; deviations break templating.

**Prompt:**
> Audit every `#[instrument]` and `tracing::*_span!` in `crates/zentra-api/src/handlers/` and `crates/zentra-api/src/middleware/`. Replace ad-hoc field names with OTEL semantic conventions: HTTP handlers must set `http.request.method`, `http.route`, `url.path`, `http.response.status_code`, `server.address`, `client.address`, `user_agent.original`. Outgoing HTTP must add `url.full`, `http.request.method`, `network.protocol.version`. gRPC handlers must set `rpc.system=grpc`, `rpc.service`, `rpc.method`, `rpc.grpc.status_code`. Reference the constants in the `opentelemetry-semantic-conventions` crate rather than string literals. Add a clippy-style internal lint (a `tests/semconv_attrs.rs` integration test) that grep-asserts no banned legacy attribute names remain.

---

### 11. Static 10% head sampler drops error traces

**Gap:** `Sampler::TraceIdRatioBased(0.1)` in `otel_self.rs` is uniform — 90% of errors are discarded at the head.

**Why it matters:** The traces SRE most wants to see (5xx, slow, panicking) are gone before they reach the collector.

**Prompt:**
> Replace the head sampler in `init_telemetry` with `ParentBased(TraceIdRatioBased(ratio))` so child decisions follow parents. Then deploy an OTel Collector (item 17) and configure `tail_sampling` processor with these policies: (a) always-sample if `status=ERROR`, (b) always-sample if `http.response.status_code >= 500`, (c) always-sample if root span duration > p99 (use `latency` policy with threshold set per service), (d) probabilistic 10% otherwise. Document the policy chain in `docs/observability/sampling.md`. Add an env override `ZENTRA_OTEL_ERROR_SAMPLE_FORCE=true` to bypass tail-sampling for local debugging.

---

### 12. No OTEL log records emitted (Zentra's own logs)

**Gap:** Zentra logs via `tracing` to stdout. No OTEL logs SDK is initialized, so its own application logs are not correlated to its spans inside the collector.

**Why it matters:** Customers cannot run Zentra under a single OTLP pipeline; they need stdout scraping + traces side-by-side.

**Prompt:**
> Add `opentelemetry-appender-tracing` (and matching `opentelemetry_sdk::logs::LoggerProvider`) to the shared telemetry init. Build a `LoggerProvider` with an OTLP log exporter using the same endpoint as traces, attach the appender as a `tracing_subscriber::Layer`, and register alongside the existing fmt layer (keep fmt for local dev when `ZENTRA_OTEL_STDOUT_LOGS=true`). Verify a `tracing::error!` emitted inside an active span produces an OTEL `LogRecord` with `TraceId`/`SpanId` populated. Add `OTEL_LOGS_EXPORTER=otlp|none` env switch.

---

### 13. Non-API binaries never flush on shutdown

**Gap:** Even today, `zentra-api` has graceful shutdown; the other binaries do not. Once items 5–6 land, they will silently drop spans/metrics on rolling deploys.

**Why it matters:** Every restart loses the most recent (often most diagnostically valuable) batch.

**Prompt:**
> In every binary `main.rs` (`zentra-api`, `zentra-agent`, `zentra-scrape`, `zentra-netflow`, etc.), install a `tokio::signal::ctrl_c` + SIGTERM handler that calls `shutdown_telemetry()` (which itself calls `TracerProvider::shutdown`, `LoggerProvider::shutdown`, `MeterProvider::shutdown` with a 10s timeout). Wrap the runtime in a `select!` over the server future and the shutdown signal. On Windows, also handle `ctrl_break`. Add an integration test that starts a binary, sends SIGTERM, and asserts the OTLP endpoint received the in-flight batch.

---

### 14. No cardinality control on OTLP ingest metric labels

**Gap:** `crates/zentra-api/src/ingest_queue.rs` emits counters labeled by `signal` only today, but planned expansions add `org_id`, `service_name`. Unbounded label values are a cardinality bomb.

**Why it matters:** A single misbehaving tenant explodes the metrics backend.

**Prompt:**
> Introduce a cardinality guard in the shared telemetry crate: `pub fn bounded_label(key: &str, value: &str, max_unique: usize) -> Cow<'_, str>` backed by a sharded LRU. When the unique-value count for a key exceeds `max_unique` (default 1000), return `"_other"`. Apply this wrapper everywhere a tenant-supplied string (org_id, service_name, tenant_id, customer_id) is used as a metric attribute. Add a metric `zentra.metrics.cardinality.dropped` counter to track when truncation kicks in. Make the cap configurable per-key via `ZENTRA_METRICS_CARDINALITY_CAP_<KEY>` env vars.

---

## LOW

### 15. No continuous profiling producer

**Gap:** `/v1/profiles` accepts pprof payloads (consumer side), but no Rust binary in the workspace produces them. Profiles signal is one-way.

**Why it matters:** Cannot demo self-profiling; competitive gap vs. Pyroscope/Polar Signals.

**Prompt:**
> Add a feature-gated `pprof-profiling` feature to the shared telemetry crate. When enabled, spawn a background task that uses the `pprof` crate (cpu profiler, 99Hz, 10s windows) to capture profiles, encodes them as `pprof.proto`, and POSTs them to `OTEL_EXPORTER_OTLP_ENDPOINT/v1/profiles` with `service.name` / `service.instance.id` resource attrs. Make it off by default; opt in via `ZENTRA_PROFILING_ENABLED=true`. Document overhead (~1–3% CPU) in `docs/observability/profiling.md`.

---

### 16. No p50/p95/p99 surfacing for self-traces

**Gap:** `crates/zentra-api/src/handlers/otlp.rs::otel_spans_query` returns raw waterfalls. Percentile aggregation happens offline.

**Why it matters:** UI cannot show "your API p99 is X" without an extra query path.

**Prompt:**
> Add a new endpoint `GET /api/otel/traces/aggregates?service=&operation=&from=&to=` that runs a TimescaleDB continuous aggregate (or equivalent on Oracle) computing `count`, `error_rate`, `p50`, `p95`, `p99` over span duration grouped by `service.name` + `span.name`. Create the continuous aggregate in a new migration under `migrations/`. Wire a React hook `useTraceAggregates()` in `frontend/src/lib/api/` and surface the values on the existing service overview page. Cache results 30s server-side via the existing cache layer.

---

### 17. No standalone OTel Collector in compose/K8s

**Gap:** `docker/docker-compose.yml` has no `otel/opentelemetry-collector-contrib` service; `deploy/k8s/` has no Collector Deployment. Self-loop into `zentra-api` is the only path.

**Why it matters:** Cannot apply attribute processors, tail-sampling (item 11), fan-out to multiple backends, or load-balanced export.

**Prompt:**
> Add an `otel-collector` service to `docker/docker-compose.yml` using `otel/opentelemetry-collector-contrib:0.111.0`. Mount `docker/otel-collector-config.yaml` with: OTLP receiver (grpc:4317, http:4318), processors `memory_limiter`, `batch`, `resourcedetection` (system, env), `tail_sampling` (policies from item 11), `attributes` (drop high-cardinality keys), and exporters `otlphttp` -> `zentra-api`, plus `prometheus` on `:8889`. Mirror the setup in K8s: add `deploy/k8s/otel-collector-deployment.yaml` (Deployment + Service + ConfigMap), expose ports 4317/4318/8889 cluster-internal, and point each app's `OTEL_EXPORTER_OTLP_ENDPOINT` at `http://otel-collector:4318` (item 7). Document the topology in `docs/observability/collector.md` with an ASCII diagram showing apps -> collector -> zentra-api ingest.

---

## Roll-up

| Phase | Items | Goal |
|-------|-------|------|
| P0 (Critical) | 1, 2, 3, 4 | Restore end-to-end traces: async context, frontend RUM, DB visibility, plugin observability |
| P1 (High) | 5, 6, 7, 8, 9 | Make non-API services observable and properly identified in production |
| P2 (Medium) | 10, 11, 12, 13, 14 | Conventions, sampling quality, logs, durability, safety |
| P3 (Low) | 15, 16, 17 | Profiling, percentile UI, proper Collector topology |

Suggested order: 5 → 9 → 7 → 17 (telemetry plumbing first), then 1 → 3 → 4 → 2 (instrumentation), then 6 → 10 → 11 → 12 → 8 (quality), then 13 → 14 → 16 → 15 (polish).
