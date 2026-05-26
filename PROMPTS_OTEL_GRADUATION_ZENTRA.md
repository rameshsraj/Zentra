# Zentra — OpenTelemetry Graduation Implementation Prompts

**Triggered by:** [CNCF OTel Graduation (May 21, 2026)](https://www.cncf.io/announcements/2026/05/21/cloud-native-computing-foundation-announces-opentelemetrys-graduation-solidifying-status-as-the-de-facto-observability-standard/)  
**Reference Source:** `C:\git\gitsourcedemo\opentelemetry-collector-main`  
**Date:** 2026-05-25  
**Total Prompts:** 13  
**Track progress in:** `docs/TODO_OTEL_GRADUATION.md`

---

## GLOBAL RULES — MANDATORY FOR EVERY PROMPT

1. **Never duplicate code.** Verify with `grep`/`find` before creating anything. If it exists, enhance it.
2. **Enhance, don't replace.** Modify existing files rather than recreating them.
3. **Validation gate.** After every prompt: `cargo check` = 0 errors, `npx tsc --noEmit` = 0 errors.
4. **Complete 100%** before moving to the next prompt.
5. **No third-party names in UI.** No "Jaeger", "Grafana", "Prometheus", "Datadog" in user-facing text. Use generic names: "Trace Backend", "Metrics Backend", etc.
6. **No mock data in live mode.** All endpoints return real structured data or empty arrays.
7. **Migration numbering.** Continue from `20250101000061_*`. Use `IF NOT EXISTS` on all DDL.
8. **Route registration.** New handlers → `routes/*.rs` + `handlers/mod.rs`. New pages → `App.tsx` + `Layout.tsx`.
9. **OTel Collector reference.** Always cross-reference `C:\git\gitsourcedemo\opentelemetry-collector-main` for data model accuracy.
10. **Backwards compatible.** Every migration uses `ADD COLUMN IF NOT EXISTS`.
11. **Both encodings.** All OTLP endpoints must handle `application/json`. Protobuf added in PROMPT-04.

---

## PROMPT 01 — OTel Profiles (4th Signal) Ingest ✅

**Status:** Complete  
**Effort:** 2–3 days

### Context
OpenTelemetry Profiles is the 4th signal, now officially in alpha. The Collector source (`pdata/pprofile/`) reveals a **dictionary-based data model** where strings, functions, locations, and stacks are stored in shared lookup tables at the top level, and profiles reference them via integer indices. This is fundamentally different from the older pprof-style model.

### Pre-Flight
```
grep -r "otel_profiles" migrations/ --include="*.sql"
grep -r "ingest_profiles" crates/Zentra-api/src/handlers/otlp.rs
```

### Migration — `migrations/20250101000061_otel_profiles.sql`
```sql
CREATE TABLE IF NOT EXISTS otel_profiles (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id          UUID NOT NULL,
    profile_id      TEXT NOT NULL,
    service_name    TEXT NOT NULL DEFAULT 'unknown',
    resource_attributes JSONB NOT NULL DEFAULT '{}',
    profile_type    TEXT NOT NULL DEFAULT 'cpu',
    period_type     TEXT,
    period_value    BIGINT,
    duration_nanos  BIGINT NOT NULL DEFAULT 0,
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at        TIMESTAMPTZ,
    ingested_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sample_count    INTEGER NOT NULL DEFAULT 0,
    drop_frames     TEXT,
    keep_frames     TEXT,
    flamegraph_data JSONB NOT NULL DEFAULT '[]',
    attributes      JSONB NOT NULL DEFAULT '{}'
);
-- Indexes, hypertable, retention policy included
```

### Backend Tasks
- **Modify** `handlers/otlp.rs` — add `pub async fn ingest_profiles` handler
  - Parse `resourceProfiles[].scopeProfiles[].profiles[]`
  - Extract top-level `dictionary` (stringTable, functionTable, locationTable, stackTable)
  - Resolve `Sample.stackIndex → stackTable → locationTable → functionTable → stringTable`
  - Build collapsed flamegraph JSON: `[{"stack": ["main", "handler", "db_query"], "value": 150}, ...]`
  - Fallback to legacy pprof-style if no dictionary present
- **Modify** `routes/otlp.rs` — register `POST /v1/profiles`
- **Modify** `handlers/profiling.rs` — add `list_otel_profiles` and `get_otel_profile` handlers
- **Modify** `routes/profiling.rs` — register `GET /api/profiles/otel` and `GET /api/profiles/otel/:id`

### Key Data Model (from OTel Collector `pdata/pprofile`)
```
ExportProfilesServiceRequest
├── resourceProfiles[] → resource (service.name, etc.)
│   └── scopeProfiles[] → profiles[]
│       ├── profileId, timeUnixNano, durationNano
│       ├── samples[] → { stackIndex, values[], attributeIndices[] }
│       └── attributeIndices[] → dictionary.attributeTable
└── dictionary
    ├── stringTable[], functionTable[], locationTable[]
    ├── stackTable[], attributeTable[], mappingTable[], linkTable[]
```

### Validation
```
POST /v1/profiles with OTLP JSON payload → stored in otel_profiles
GET /api/profiles/otel → returns list of ingested profiles
GET /api/profiles/otel/:id → returns flamegraph data
```

---

## PROMPT 02 — OTel Collector Config Generator ✅

**Status:** Complete  
**Effort:** 1 day

### Context
With OTel as the de facto standard, the #1 onboarding friction is "how do I configure my Collector to send to Zentra?" A config generator eliminates this entirely.

### Pre-Flight
```
grep -r "collector-config" crates/Zentra-api/src/
grep -r "OTelCollectorConfig" frontend/src/
```

### Backend Tasks
- **Modify** `handlers/otlp.rs` — add:
  - `GET /api/otel/collector-config?host=X&port=Y&org_id=Z&enable_traces=true...` — returns YAML
  - `GET /api/otel/status` — returns OTLP capabilities JSON (signals, endpoints, encodings, auth)
- **Modify** `routes/otlp.rs` — register both routes

### Frontend Tasks
- **New File** `frontend/src/pages/OTelCollectorConfig.tsx` — interactive config builder:
  - Connection settings (host, port, org ID)
  - Signal toggles (traces, metrics, logs, profiles)
  - Live YAML preview
  - Copy to clipboard + download as `.yaml`
  - Quick Start guide with Docker commands
  - OTLP endpoint reference card
- **Modify** `App.tsx` — add route `/otel-config`
- **Modify** `Layout.tsx` — add nav item "OTel Collector Setup"

### Validation
```
GET /api/otel/collector-config → valid YAML
GET /api/otel/status → JSON with signals, endpoints, auth
Page loads at /otel-config, generates correct config
```

---

## PROMPT 03 — Protobuf OTLP Ingest

**Status:** Pending  
**Effort:** 1–2 weeks  
**Depends on:** Network access to crates.io  
**OTel Collector reference:** `receiver/otlpreceiver/encoder.go`

### Context
The OTel Collector sends protobuf by default. Currently Zentra returns 415 for `application/x-protobuf`. Supporting native protobuf makes Zentra a drop-in Collector backend without requiring the user to add `encoding: json` to their config. The Collector source shows a clean `encoder` interface (`encoder.go`) with `protoEncoder` and `jsonEncoder` implementations.

### Pre-Flight
```
grep -r "prost" Cargo.toml
grep -r "protobuf" crates/Zentra-api/src/
grep -r "detect_encoding" crates/Zentra-api/src/handlers/otlp.rs
```

### Dependencies
Add to workspace `Cargo.toml`:
```toml
prost = "0.13"
prost-types = "0.13"
```
Add to `crates/Zentra-api/Cargo.toml`:
```toml
[build-dependencies]
prost-build = "0.13"
```

### Proto Files
Obtain from `opentelemetry-proto` repository (or `C:\git\gitsourcedemo\` if available):
```
proto/opentelemetry/proto/
├── collector/
│   ├── trace/v1/trace_service.proto
│   ├── metrics/v1/metrics_service.proto
│   ├── logs/v1/logs_service.proto
│   └── profiles/v1development/profiles_service.proto
├── common/v1/common.proto
├── resource/v1/resource.proto
├── trace/v1/trace.proto
├── metrics/v1/metrics.proto
├── logs/v1/logs.proto
└── profiles/v1development/pprofextended.proto
```

### Backend Tasks
- **New File** `crates/Zentra-api/build.rs` — prost codegen:
  ```rust
  fn main() {
      prost_build::compile_protos(
          &["proto/opentelemetry/proto/collector/trace/v1/trace_service.proto", ...],
          &["proto/"],
      ).unwrap();
  }
  ```
- **New File** `crates/Zentra-api/src/otlp_proto.rs` — protobuf deserialization helpers:
  - `fn proto_to_json_traces(bytes: &[u8]) -> Result<Value>` — decode proto, convert to `serde_json::Value`
  - Same for metrics, logs, profiles
  - Follow the Collector's `encoder` pattern from `encoder.go`
- **Modify** `handlers/otlp.rs` — update `detect_encoding()`:
  - `application/x-protobuf` → return `OtlpEncoding::Protobuf`
  - Each ingest handler: if protobuf, decode via prost → convert to JSON → reuse existing logic
  - If JSON, unchanged behavior
- All 4 endpoints (`/v1/traces`, `/v1/metrics`, `/v1/logs`, `/v1/profiles`) accept both

### Validation
```
POST /v1/traces Content-Type: application/x-protobuf → stored correctly
POST /v1/traces Content-Type: application/json → still works
Default OTel Collector (no encoding override) → sends data successfully
cargo check → 0 errors
```

---

## PROMPT 04 — OTel SDK Instrumentation Guide Page

**Status:** Pending  
**Effort:** 3–5 days

### Context
The OTel graduation announcement highlights: "Organizations switch observability backends without re-instrumenting." Zentra should provide language-specific SDK setup guides so users can instrument their services. Java is the priority (auto-instrumentation agent covers Spring Boot, JDBC, Kafka, gRPC, HTTP clients as a drop-in JAR).

### Backend Tasks
- **Modify** `handlers/otlp.rs` — add `GET /api/otel/sdk-snippets?language=java&framework=spring-boot` — returns code snippets for SDK setup
- **Modify** `routes/otlp.rs` — register route

### Frontend Tasks
- **New File** `frontend/src/pages/OTelSdkGuide.tsx` — SDK instrumentation guide:
  - Language selector tabs: Java, Python, Go, Node.js, .NET, Rust
  - Framework-specific examples per language:
    - **Java:** Spring Boot auto-instrument (JAR agent), manual SDK, JDBC, Kafka
    - **Python:** Flask/Django auto-instrument, manual SDK, SQLAlchemy
    - **Go:** net/http, gRPC, database/sql
    - **Node.js:** Express, Fastify, pg/mysql2
    - **.NET:** ASP.NET Core auto-instrument
    - **Rust:** tracing + opentelemetry-otlp
  - Each example includes:
    - Install command (Maven/pip/go get/npm)
    - Minimal code snippet
    - Environment variables (`OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`)
    - "Copy" button for each snippet
  - Java auto-instrumentation highlight section:
    ```
    java -javaagent:opentelemetry-javaagent.jar \
         -Dotel.exporter.otlp.endpoint=http://<Zentra>:3001 \
         -Dotel.exporter.otlp.protocol=http/json \
         -Dotel.service.name=my-service \
         -jar my-app.jar
    ```
  - Link to OTel Collector Config page for pipeline setup
- **Modify** `App.tsx` — add route `/otel-sdk-guide`
- **Modify** `Layout.tsx` — add nav item "SDK Setup Guide" under AI & Analytics

### Validation
```
npx tsc --noEmit → 0 errors
Page loads at /otel-sdk-guide
Java Spring Boot example shows correct agent flags
Each language tab renders with correct snippets
```

---

## PROMPT 05 — OTel Semantic Convention Enrichment

**Status:** Pending  
**Effort:** 1 week  
**OTel Collector reference:** `pdata/pcommon/`, semantic convention specs

### Context
Semantic conventions are what make OTel data universally queryable. The graduation announcement calls them "a shared vocabulary." Currently, Zentra stores all span attributes as untyped JSONB. Extracting well-known semconv attributes into indexed columns dramatically improves query performance and enables faceted search.

### Pre-Flight
```
grep -r "semconv" crates/Zentra-api/src/
grep -r "http.method\|db.system\|rpc.system" crates/Zentra-api/src/handlers/otlp.rs
```

### Migration — `migrations/20250101000062_semconv_columns.sql`
```sql
-- Add indexed semantic convention columns to traces
ALTER TABLE traces ADD COLUMN IF NOT EXISTS http_method      TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS http_status_code INTEGER;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS http_route       TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS db_system        TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS db_name          TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS db_operation     TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS rpc_system       TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS rpc_service      TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS rpc_method       TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS messaging_system TEXT;
ALTER TABLE traces ADD COLUMN IF NOT EXISTS peer_service     TEXT;

CREATE INDEX IF NOT EXISTS idx_traces_http_method ON traces(http_method) WHERE http_method IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_traces_db_system ON traces(db_system) WHERE db_system IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_traces_http_status ON traces(http_status_code) WHERE http_status_code IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_traces_rpc_system ON traces(rpc_system) WHERE rpc_system IS NOT NULL;
-- GIN index on attributes JSONB for arbitrary attribute queries
CREATE INDEX IF NOT EXISTS idx_traces_attributes_gin ON traces USING GIN(attributes jsonb_path_ops);
```

### Backend Tasks
- **New File** `crates/Zentra-api/src/semconv.rs` — semantic convention constants and extractor:
  ```rust
  pub struct SemconvExtracted {
      pub http_method: Option<String>,
      pub http_status_code: Option<i32>,
      pub http_route: Option<String>,
      pub db_system: Option<String>,
      pub db_name: Option<String>,
      pub db_operation: Option<String>,
      pub rpc_system: Option<String>,
      pub rpc_service: Option<String>,
      pub rpc_method: Option<String>,
      pub messaging_system: Option<String>,
      pub peer_service: Option<String>,
  }
  pub fn extract(attributes: &serde_json::Value) -> SemconvExtracted { ... }
  ```
  - Support both stable (`http.request.method`) and legacy (`http.method`) attribute names
- **Modify** `handlers/otlp.rs` `ingest_traces` — call `semconv::extract()` on each span's attributes, bind extracted fields to INSERT
- **Modify** `handlers/otlp.rs` — add:
  - `GET /api/otel/attributes?signal=traces` — returns all known attribute keys with cardinality stats
  - `GET /api/otel/attribute-values?key=service.name&limit=50` — returns known values for an attribute
- **Modify** `routes/otlp.rs` — register both routes

### Frontend Tasks
- **Modify** `Traces.tsx` — add attribute facet sidebar:
  - Show discovered attributes grouped by category (HTTP, DB, RPC, Messaging)
  - Click attribute value → filters trace list
  - Show count badge per attribute value
- **Modify** `QueryExplorer.tsx` — TraceQL autocomplete suggests semconv attribute names

### Validation
```
POST /v1/traces with span having http.method=GET → http_method column populated
GET /api/otel/attributes?signal=traces → returns attribute keys with counts
Traces page facet sidebar shows HTTP methods with counts
cargo check → 0 errors
npx tsc --noEmit → 0 errors
```

---

## PROMPT 06 — OTel Resource Detection Page

**Status:** Pending  
**Effort:** 3–5 days

### Context
With OTel graduation, more services self-describe via resource attributes (`service.name`, `service.version`, `telemetry.sdk.language`, `deployment.environment`). A resource inventory page helps users verify instrumentation is working and shows which services are sending which signals.

### Backend Tasks
- **Modify** `handlers/otlp.rs` — add:
  - `GET /api/otel/resources` — aggregated distinct resources from recent OTLP data:
    ```sql
    SELECT DISTINCT ON (resource_attributes->>'service.name')
        resource_attributes->>'service.name' AS service_name,
        resource_attributes->>'service.version' AS version,
        resource_attributes->>'telemetry.sdk.language' AS sdk_language,
        resource_attributes->>'telemetry.sdk.version' AS sdk_version,
        resource_attributes->>'deployment.environment' AS environment,
        MAX(started_at) AS last_seen,
        COUNT(*) AS span_count
    FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '24h'
    GROUP BY 1, 2, 3, 4, 5
    ```
  - Include signal counts: query traces, metrics, logs, otel_profiles tables for each service
  - `GET /api/otel/resources/:service_name/signals` — signal breakdown for one service

### Frontend Tasks
- **New File** `frontend/src/pages/OTelResources.tsx` — resource inventory:
  - Table columns: Service Name, Version, SDK Language, SDK Version, Environment, Last Seen, Signals (badges)
  - Signal badges: colored pills for each active signal (Traces ▪ Metrics ▪ Logs ▪ Profiles)
  - Search/filter by service name
  - Click service → drill to traces filtered by that service
  - SDK language breakdown chart (donut: Java/Python/Go/Node/etc.)
  - "Instrumentation Health" summary cards: total services, services with all 4 signals, services with missing signals
- **Modify** `App.tsx` — add route `/otel-resources`
- **Modify** `Layout.tsx` — add nav item "OTel Resources"

### Validation
```
After sending OTLP traces → service appears in /otel-resources
Signal badges show correct signals per service
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## PROMPT 07 — OTel Health Dashboard

**Status:** Pending  
**Effort:** 1 week

### Context
Operational visibility into the telemetry pipeline itself — how much data is flowing, error rates, signal volumes. Critical for proving the OTel deployment is working.

### Backend Tasks
- **Modify** `handlers/otlp.rs` — add:
  - `GET /api/otel/stats` — aggregated ingest stats:
    ```json
    {
      "spans_per_minute": 12500,
      "metrics_per_minute": 85000,
      "logs_per_minute": 45000,
      "profiles_per_minute": 12,
      "unique_services": 47,
      "top_services": [{ "name": "order-service", "spans": 3200 }, ...],
      "error_rate_percent": 0.02,
      "avg_span_duration_ms": 45.2
    }
    ```
  - `GET /api/otel/stats/timeseries?signal=traces&window=24h&step=5m` — time-series volume data
- **Modify** `routes/otlp.rs` — register routes

### Frontend Tasks
- **New File** `frontend/src/pages/OTelDashboard.tsx` — telemetry health dashboard:
  - **Top row:** 4 stat cards (Traces/min, Metrics/min, Logs/min, Profiles/min) with sparklines
  - **Signal volume chart:** stacked area chart showing all 4 signals over time
  - **Top services table:** service name, span count, error rate, p95 latency
  - **Error rate gauge:** circular gauge showing ingest error percentage
  - **Signal breakdown:** donut chart (traces vs metrics vs logs vs profiles by volume)
  - **SDK distribution:** horizontal bar chart showing language breakdown
  - Time range selector: 1h, 6h, 24h, 7d
- **Modify** `App.tsx` — add route `/otel-dashboard`
- **Modify** `Layout.tsx` — add nav item "Telemetry Health"

### Validation
```
After sending OTLP data → dashboard shows real stats
Time range selector changes data
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## PROMPT 08 — OTLP gRPC Ingest

**Status:** Pending  
**Effort:** 2–3 weeks  
**Depends on:** PROMPT-03 (Protobuf support)  
**OTel Collector reference:** `receiver/otlpreceiver/otlp.go`, `config/configgrpc/`

### Context
gRPC is the primary Collector-to-backend transport in production. HTTP is the fallback. Supporting gRPC means Zentra works with the default Collector config out of the box.

### Dependencies
Add to workspace `Cargo.toml`:
```toml
tonic = { version = "0.12", features = ["default"] }
```

### Backend Tasks
- **New File** `crates/Zentra-api/src/grpc/mod.rs` — gRPC service module
- **New File** `crates/Zentra-api/src/grpc/trace_service.rs`:
  ```rust
  #[tonic::async_trait]
  impl TraceService for OtlpTraceService {
      async fn export(&self, request: Request<ExportTraceServiceRequest>)
          -> Result<Response<ExportTraceServiceResponse>, Status> { ... }
  }
  ```
  - Reuse existing ingest logic from `handlers/otlp.rs`
- **New File** `crates/Zentra-api/src/grpc/metrics_service.rs` — same pattern for metrics
- **New File** `crates/Zentra-api/src/grpc/logs_service.rs` — same for logs
- **New File** `crates/Zentra-api/src/grpc/profiles_service.rs` — same for profiles
- **Modify** `main.rs` — start gRPC server alongside HTTP:
  ```rust
  let grpc_server = tonic::transport::Server::builder()
      .add_service(TraceServiceServer::new(trace_svc))
      .add_service(MetricsServiceServer::new(metrics_svc))
      .add_service(LogsServiceServer::new(logs_svc))
      .add_service(ProfilesServiceServer::new(profiles_svc))
      .serve(grpc_addr);
  ```
  - Configurable port (default 4317), TLS optional
- **Modify** `GET /api/otel/status` — include gRPC endpoint info
- **Modify** Collector config generator — add gRPC exporter option

### Validation
```
OTel Collector with otlp/grpc exporter → sends data to Zentra:4317
Both gRPC and HTTP ingest work simultaneously
cargo check → 0 errors
```

---

## PROMPT 09 — OTel Connector Pipeline (Span-to-Metrics)

**Status:** Pending  
**Effort:** 2–3 weeks  
**OTel Collector reference:** `connector/`

### Context
OTel Connectors transform signals — most importantly, span-to-metrics generates RED metrics (Rate, Error, Duration) from ingested spans without requiring separate metric instrumentation. This is a huge value-add: users get metrics for free from their traces.

### Migration — `migrations/20250101000063_derived_metrics.sql`
```sql
CREATE TABLE IF NOT EXISTS otel_derived_metrics (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id          UUID NOT NULL,
    service_name    TEXT NOT NULL,
    operation_name  TEXT,
    metric_name     TEXT NOT NULL,      -- 'request_count', 'error_count', 'duration_p50', 'duration_p95', 'duration_p99'
    value           DOUBLE PRECISION NOT NULL,
    dimensions      JSONB NOT NULL DEFAULT '{}',
    bucket_start    TIMESTAMPTZ NOT NULL,
    bucket_duration INTERVAL NOT NULL DEFAULT '1 minute'
);
SELECT create_hypertable('otel_derived_metrics', 'bucket_start', migrate_data => true, if_not_exists => true);
CREATE INDEX IF NOT EXISTS idx_derived_metrics_service ON otel_derived_metrics(service_name, metric_name, bucket_start DESC);
```

### Backend Tasks
- **New File** `crates/Zentra-api/src/otel_connectors/mod.rs`
- **New File** `crates/Zentra-api/src/otel_connectors/span_to_metrics.rs`:
  - On trace ingest (or periodic background job), compute per-service per-minute:
    - `request_count` — total spans
    - `error_count` — spans with `status = 'error'`
    - `duration_p50`, `duration_p95`, `duration_p99` — percentiles
  - Dimension extraction: `http_method`, `http_route`, `db_system`, `rpc_service`
  - Store in `otel_derived_metrics`
- **Modify** `handlers/otlp.rs` — add:
  - `GET /api/otel/connectors` — list active connectors and their config
  - `POST /api/otel/connectors` — enable/configure connector
  - `GET /api/otel/derived-metrics?service=X&metric=duration_p95&window=1h` — query derived metrics
- Register all routes

### Frontend Tasks
- **New File** `frontend/src/pages/OTelConnectors.tsx` — connector management:
  - Enable/disable span-to-metrics connector
  - Configure dimensions and aggregation interval
  - Show derived metrics preview chart
  - Per-service RED metrics dashboard
- **Modify** `App.tsx` — add route `/otel-connectors`
- **Modify** `Layout.tsx` — add nav item "Signal Connectors"

### Validation
```
Ingest traces → otel_derived_metrics populated automatically
GET /api/otel/derived-metrics?service=X → returns RED metrics
Connector page shows enable/disable toggle
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## PROMPT 10 — OTel-Native Alerting from Signals

**Status:** Pending  
**Effort:** 2–3 weeks

### Context
With all 4 signals flowing through OTel, alerts should be definable directly on OTel data using semantic convention attributes. This closes the loop: instrument → collect → alert.

### Migration — `migrations/20250101000064_otel_alert_rules.sql`
```sql
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS signal_source TEXT;  -- 'traces', 'metrics', 'logs', 'profiles', 'derived'
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS otel_query JSONB;    -- structured alert condition
-- Example otel_query:
-- { "signal": "traces", "condition": "p99_duration > 500ms", "filter": { "service.name": "order-svc" } }
```

### Backend Tasks
- **Modify** `handlers/alerts.rs` — extend alert rule creation to accept OTel signal sources:
  - Trace alerts: `p99(duration) WHERE service.name = "X" > 500ms`
  - Log alerts: `count(severity = "ERROR") WHERE service.name = "X" > 10/min`
  - Metric alerts: `value > threshold` on OTLP metrics
  - Derived metric alerts: `error_rate > 5%` from span-to-metrics
- **New File** `crates/Zentra-api/src/otel_alert_evaluator.rs`:
  - Background job evaluating OTel alert rules on configurable interval
  - Query traces/metrics/logs tables using semconv columns
  - Fire alert via existing notification pipeline

### Frontend Tasks
- **Modify** `AlertRules.tsx` — add "OTel Signal" alert type:
  - Signal source selector (Traces, Metrics, Logs, Derived)
  - Condition builder with semconv attribute autocomplete
  - Threshold input with unit selector (ms, count, percent)
  - Preview: show matching data count before saving

### Validation
```
Create alert: "p99 latency > 500ms for order-service" → fires when threshold crossed
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## PROMPT 11 — AI/LLM Observability via OTel GenAI Conventions

**Status:** Pending  
**Effort:** 1–2 weeks

### Context
The graduation announcement specifically calls out AI workloads: "Organizations such as Anthropic rely on OpenTelemetry." The GenAI semantic conventions (`gen_ai.*`) provide standardized visibility into LLM latency, token usage, model accuracy, and agent decision flows. Zentra already has LLM Observability handlers — this prompt wires them to real OTel data.

### Backend Tasks
- **Modify** `handlers/otlp.rs` `ingest_traces` — detect GenAI spans and extract:
  - `gen_ai.system` (openai / anthropic / google / bedrock)
  - `gen_ai.request.model`
  - `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.total_tokens`
  - `gen_ai.response.finish_reasons`
  - `llm.request.cost` (if present)
  - Insert into `llm_spans` table (from existing migration `20250101000059_llm_spans.sql`)
- **Modify** `handlers/llm_observability.rs` — replace demo data with real queries from `llm_spans`:
  - `GET /api/llm/overview` → real totals from `llm_spans`
  - `GET /api/llm/models` → per-model breakdown
  - `GET /api/llm/cost-trend` → hourly cost from token counts × model pricing
  - `GET /api/llm/latency-distribution` → histogram from span durations

### Frontend Tasks
- **Modify** `LLMObservability.tsx` — ensure all panels call real endpoints (not stubs)
- Add "Agent Decision Flow" visualization: shows parent→child span tree for agentic LLM calls

### Validation
```
POST /v1/traces with gen_ai.* attributes → appears in LLM Observability page
Model breakdown shows real per-model stats
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## PROMPT 12 — Security & Compliance Posture

**Status:** Pending  
**Effort:** 1 week

### Context
The graduation announcement notes OTel passed an independent security audit. Zentra should leverage this for compliance: document that telemetry is collected via a CNCF-graduated project, support mTLS for Collector→Zentra, and validate auth on all OTLP endpoints.

### Backend Tasks
- **Modify** `handlers/otlp.rs` — strengthen OTLP auth:
  - Support API key auth (in addition to JWT and org-id header)
  - Rate limiting per org_id on OTLP endpoints
  - Payload size limits (configurable, default 5MB)
  - Request logging with org_id for audit trail
- **Modify** `GET /api/otel/status` — include security posture:
  ```json
  {
    "security": {
      "auth_required": true,
      "tls_enabled": false,
      "rate_limit_per_minute": 10000,
      "max_payload_bytes": 5242880,
      "audit_logging": true
    }
  }
  ```
- **Migration** — `migrations/20250101000065_otlp_api_keys.sql`:
  ```sql
  CREATE TABLE IF NOT EXISTS otlp_api_keys (
      id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
      org_id      UUID NOT NULL,
      key_hash    TEXT NOT NULL,
      name        TEXT NOT NULL,
      permissions TEXT[] NOT NULL DEFAULT '{traces,metrics,logs,profiles}',
      created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      expires_at  TIMESTAMPTZ,
      last_used   TIMESTAMPTZ
  );
  ```

### Frontend Tasks
- **Modify** `OTelCollectorConfig.tsx` — add API key option in config generator
- **Modify** admin pages — add OTLP API Key management (create, revoke, list)

### Validation
```
OTLP request without auth → 401
OTLP request with valid API key → 200
Rate limit exceeded → 429 with Retry-After header
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## PROMPT 13 — Collector Pipeline Topology Visualization

**Status:** Pending  
**Effort:** 1 week

### Context
As organizations deploy multiple Collectors (edge collectors → central collector → Zentra), visualizing the pipeline topology helps debug data flow issues.

### Backend Tasks
- **Modify** `handlers/otlp.rs` — track Collector identity from resource attributes:
  - Extract `service.name = "otelcol-contrib"`, `host.name`, `service.instance.id`
  - Store Collector instances and their first/last seen timestamps
- Add `GET /api/otel/pipeline` — returns Collector topology:
  ```json
  {
    "collectors": [
      { "id": "...", "host": "edge-1", "last_seen": "...", "signals_forwarded": 125000 }
    ],
    "flow": [
      { "from": "app-pods", "to": "edge-1", "protocol": "grpc" },
      { "from": "edge-1", "to": "Zentra", "protocol": "http/json" }
    ]
  }
  ```

### Frontend Tasks
- **New File** `frontend/src/pages/OTelPipeline.tsx` — pipeline topology:
  - Visual flow diagram: Apps → Collector(s) → Zentra
  - Per-Collector health card (last seen, throughput, error count)
  - Click Collector → show its configuration and forwarded signals
- **Modify** `App.tsx` — add route `/otel-pipeline`
- **Modify** `Layout.tsx` — add nav item "Pipeline Topology"

### Validation
```
Multiple Collectors sending data → pipeline view shows topology
Collector goes offline → shown as unhealthy
cargo check → 0 errors, npx tsc --noEmit → 0 errors
```

---

## Framework Strategy Summary

This prompt set implements the following strategic framework:

### 1. Adopt OTel as the Standard Instrumentation Layer
- **PROMPT-04** provides SDK setup guides for all major languages
- **PROMPT-05** extracts and indexes semantic conventions for queryability
- **PROMPT-11** covers AI/LLM observability via GenAI conventions

### 2. Deploy the Collector as a Central Pipeline
- **PROMPT-02** ✅ generates Collector configs pointing at Zentra
- **PROMPT-03** adds protobuf support (Collector's default encoding)
- **PROMPT-08** adds gRPC support (Collector's default transport)
- **PROMPT-13** visualizes multi-Collector pipeline topology

### 3. Support All 4 Signals
- **PROMPT-01** ✅ adds Profiles (4th signal) ingest
- **PROMPT-09** derives metrics from traces (span-to-metrics connector)
- **PROMPT-07** provides operational visibility into signal flow

### 4. AI Observability
- **PROMPT-11** wires GenAI semantic conventions to LLM Observability page

### 5. Security Posture
- **PROMPT-12** strengthens auth, adds API keys, rate limiting, audit logging

### 6. Alerting on OTel Data
- **PROMPT-10** enables native alerting on all OTel signals with semconv filters
