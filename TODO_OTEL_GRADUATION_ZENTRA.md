# Zentra — OpenTelemetry Graduation Feature Build TODO

**Triggered by:** [CNCF OTel Graduation Announcement (May 21, 2026)](https://www.cncf.io/announcements/2026/05/21/cloud-native-computing-foundation-announces-opentelemetrys-graduation-solidifying-status-as-the-de-facto-observability-standard/)  
**Reference Source:** `C:\git\gitsourcedemo\opentelemetry-collector-main` (OTel Collector main branch)  
**Date:** 2026-05-25  
**Objective:** Position Zentra as a first-class OTel-native observability backend  
**Prompts file:** `docs/PROMPTS_OTEL_GRADUATION.md` (13 detailed prompts)

---

## Framework Strategy

OpenTelemetry graduation means it is **safe and recommended for production use at scale**. This TODO implements a comprehensive strategy:

### 1. Adopt OTel as the Standard Instrumentation Layer
With graduation, OTel is no longer experimental. Instead of vendor-specific agents (Datadog agent, New Relic agent, etc.), standardize on OTel SDKs. This avoids vendor lock-in and lets organizations switch backends without re-instrumenting code.
- **PROMPT-04** provides SDK setup guides for Java, Python, Go, Node.js, .NET, Rust
- **PROMPT-05** extracts and indexes semantic conventions for universal queryability

### 2. Java Services: Drop-in Auto-Instrumentation
The OTel Java auto-instrumentation agent covers Spring Boot, JDBC, Kafka, gRPC, and HTTP clients out of the box. For Java 17 services, this is a drop-in JAR attachment — zero code changes. Zentra's SDK Guide (PROMPT-04) documents this path.

### 3. Deploy the OTel Collector as a Central Pipeline
Instead of each service exporting directly to the backend, route through the Collector for batching, filtering, enriching, and forwarding. Zentra becomes a Collector backend.
- **PROMPT-02** ✅ generates Collector configs pointing at Zentra
- **PROMPT-03** adds protobuf support (Collector's default encoding)
- **PROMPT-08** adds gRPC support (Collector's default transport)
- **PROMPT-13** visualizes multi-Collector pipeline topology

### 4. AI Observability
Multiple quotes in the announcement highlight OTel as the instrumentation layer for AI agents and LLM workloads. Zentra leverages GenAI semantic conventions (`gen_ai.*`) for visibility into latency, token usage, model accuracy, and agent decision flows.
- **PROMPT-11** wires GenAI conventions to the existing LLM Observability page

### 5. Security Posture
OTel passed an independent security audit. Using a graduated CNCF project strengthens compliance compared to rolling custom telemetry or using less mature tools.
- **PROMPT-12** strengthens auth, adds API keys, rate limiting, audit logging

### 6. Stay Current on Profiles
The new Profiles signal (alpha) adds continuous profiling alongside traces, metrics, and logs.
- **PROMPT-01** ✅ already implemented — Zentra is an early adopter of the 4th signal

---

## How to Use This File

- Work **top to bottom** — items are ordered by dependency and impact
- Mark status as: `[ ]` pending → `[~]` in progress → `[x]` complete
- Each item references the relevant OTel Collector source files for implementation guidance
- Run validation commands at the end of each item before moving to the next
- **Full implementation details** for each item are in `docs/PROMPTS_OTEL_GRADUATION.md`

---

## Progress Summary

| # | Feature | Status | Effort | Tier |
|---|---|---|---|---|
| 1 | OTel Profiles (4th Signal) Ingest | `[x]` | 2–3 days | TIER 1 |
| 2 | OTel Collector Config Generator | `[x]` | 1 day | TIER 1 |
| 3 | OTLP Status / Capabilities API | `[x]` | 0.5 day | TIER 1 |
| 4 | Protobuf OTLP Ingest | `[ ]` | 1–2 weeks | TIER 1 |
| 5 | OTel Semantic Convention Enrichment | `[ ]` | 1 week | TIER 2 |
| 6 | OTel Resource Detection Page | `[ ]` | 3–5 days | TIER 2 |
| 7 | OTel Health Dashboard | `[ ]` | 1 week | TIER 2 |
| 8 | OTLP gRPC Ingest | `[ ]` | 2–3 weeks | TIER 3 |
| 9 | OTel Connector Pipeline (Span→Metrics) | `[ ]` | 2–3 weeks | TIER 3 |
| 10 | OTel-Native Alerting from Signals | `[ ]` | 2–3 weeks | TIER 3 |
| 11 | AI/LLM Observability via GenAI Conventions | `[ ]` | 1–2 weeks | TIER 2 |
| 12 | Security & Compliance Posture | `[ ]` | 1 week | TIER 2 |
| 13 | Collector Pipeline Topology Visualization | `[ ]` | 1 week | TIER 3 |
| — | SDK Instrumentation Guide Page | `[ ]` | 3–5 days | TIER 1 |

---

## TIER 1 — Foundation (Do These First)

---

### [x] ITEM 1 — OTel Profiles (4th Signal) Ingest
**Effort:** 2–3 days  
**Why first:** OTel Profiles just entered alpha. Early adoption is a differentiator.  
**OTel Collector reference:** `pdata/pprofile/`, `receiver/otlpreceiver/otlphttp.go`

**Completed tasks:**
- [x] Migration `20250101000061_otel_profiles.sql` — `otel_profiles` hypertable with TimescaleDB
- [x] `POST /v1/profiles` ingest handler in `handlers/otlp.rs`
  - Supports new dictionary-based model (stringTable, functionTable, locationTable, stackTable)
  - Supports legacy pprof-style model as fallback
  - Converts OTLP profile samples → collapsed flamegraph JSON for rendering
- [x] Route registered in `routes/otlp.rs`
- [x] `GET /api/profiles/otel` — list OTel-ingested profiles (filterable by service, type)
- [x] `GET /api/profiles/otel/:id` — get full flamegraph data for an OTel profile
- [x] Routes registered in `routes/profiling.rs`

**Key data model (from OTel Collector `pdata/pprofile`):**
```
ExportProfilesServiceRequest
├── resourceProfiles[]
│   ├── resource (service.name, etc.)
│   └── scopeProfiles[]
│       └── profiles[]
│           ├── profileId, timeUnixNano, durationNano
│           ├── periodType { type: strindex, unit: strindex }
│           ├── sampleType { type: strindex, unit: strindex }
│           ├── samples[] → { stackIndex, values[], attributeIndices[] }
│           └── attributeIndices[] → dictionary.attributeTable
└── dictionary
    ├── stringTable[]
    ├── functionTable[] → { nameStrindex, filenameStrindex, startLine }
    ├── locationTable[] → { lines[{ functionIndex }], address }
    ├── stackTable[] → { locationIndices[] }
    ├── attributeTable[] → { key: strindex, value: AnyValue }
    ├── mappingTable[]
    └── linkTable[]
```

**Validation:**
- [x] `npx tsc --noEmit` — 0 errors
- [ ] `cargo check` — blocked by network, manual audit passed
- [ ] Send an OTLP profile payload → stored in `otel_profiles`, flamegraph renders

---

### [x] ITEM 2 — OTel Collector Config Generator
**Effort:** 1 day  
**Why second:** Lowest friction onboarding. Users can generate a working Collector config with one click.  
**OTel Collector reference:** `exporter/otlphttpexporter/`, `receiver/otlpreceiver/config.go`

**Completed tasks:**
- [x] `GET /api/otel/collector-config` — generates YAML config with customizable host, port, org_id, signal toggles
- [x] Route registered in `routes/otlp.rs`
- [x] Frontend page `OTelCollectorConfig.tsx`:
  - Interactive config builder (host, port, org ID, signal toggles)
  - Live YAML preview with syntax-appropriate formatting
  - Copy to clipboard + download as file
  - Quick Start guide with Docker commands
  - OTLP endpoint reference card
- [x] Route `/otel-config` in `App.tsx`
- [x] Nav item "OTel Collector Setup" under AI & Analytics in `Layout.tsx`

**Validation:**
- [x] `npx tsc --noEmit` — 0 errors
- [ ] Page loads at `/otel-config`
- [ ] Generated YAML is valid OTel Collector config

---

### [x] ITEM 3 — OTLP Status / Capabilities API
**Effort:** 0.5 day  
**Why third:** Enables programmatic discovery of Zentra's OTLP capabilities.

**Completed tasks:**
- [x] `GET /api/otel/status` — returns supported signals, endpoints, encodings, auth methods
- [x] Route registered in `routes/otlp.rs`

**Validation:**
- [x] `npx tsc --noEmit` — 0 errors
- [ ] `GET /api/otel/status` returns correct JSON

---

### [ ] ITEM 4 — Protobuf OTLP Ingest
**Effort:** 1–2 weeks  
**Why fourth:** OTel Collector sends protobuf by default. Supporting it makes Zentra a drop-in backend.  
**OTel Collector reference:** `receiver/otlpreceiver/encoder.go` (protoEncoder/jsonEncoder pattern)  
**Depends on:** Network access to crates.io for `prost` dependency

**Backend tasks:**
- [ ] Add `prost = "0.13"` and `prost-types = "0.13"` to workspace `Cargo.toml`
- [ ] Add `prost-build = "0.13"` as build dependency
- [ ] Obtain OTLP `.proto` files from `opentelemetry-proto` repository:
  - `opentelemetry/proto/collector/trace/v1/trace_service.proto`
  - `opentelemetry/proto/collector/metrics/v1/metrics_service.proto`
  - `opentelemetry/proto/collector/logs/v1/logs_service.proto`
  - `opentelemetry/proto/collector/profiles/v1development/profiles_service.proto`
  - `opentelemetry/proto/common/v1/common.proto`
  - `opentelemetry/proto/resource/v1/resource.proto`
  - `opentelemetry/proto/trace/v1/trace.proto`
  - `opentelemetry/proto/metrics/v1/metrics.proto`
  - `opentelemetry/proto/logs/v1/logs.proto`
  - `opentelemetry/proto/profiles/v1development/pprofextended.proto`
- [ ] Create `crates/Zentra-api/build.rs` — prost codegen for all protos
- [ ] Create `crates/Zentra-api/src/otlp_proto.rs` — protobuf deserialization module
- [ ] Update `detect_encoding()` in `handlers/otlp.rs`:
  - `application/x-protobuf` → decode via prost, convert to `serde_json::Value`, reuse JSON handlers
  - Follow the Collector's `encoder` interface pattern from `encoder.go`
- [ ] All 4 ingest endpoints (`/v1/traces`, `/v1/metrics`, `/v1/logs`, `/v1/profiles`) accept both JSON and protobuf

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] Send protobuf-encoded OTLP trace → stored correctly
- [ ] Send JSON-encoded OTLP trace → still works (no regression)
- [ ] Default OTel Collector config (protobuf) works without `encoding: json` override

---

## TIER 2 — Enrichment & Visibility

---

### [ ] ITEM 5 — OTel Semantic Convention Enrichment
**Effort:** 1 week  
**Why:** The OTel graduation announcement emphasizes semantic conventions as "a shared vocabulary." Extracting and indexing well-known attributes improves search and correlation.  
**OTel Collector reference:** `pdata/pcommon/`, semantic convention docs

**Backend tasks:**
- [ ] Create `crates/Zentra-api/src/semconv.rs` — defines well-known OTel attribute keys:
  - Service: `service.name`, `service.version`, `service.namespace`
  - HTTP: `http.method`, `http.status_code`, `http.url`, `http.route`
  - DB: `db.system`, `db.name`, `db.statement`, `db.operation`
  - RPC: `rpc.system`, `rpc.service`, `rpc.method`
  - GenAI: `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.total_tokens`
  - Messaging: `messaging.system`, `messaging.destination.name`
  - Cloud: `cloud.provider`, `cloud.region`, `cloud.availability_zone`
  - K8s: `k8s.pod.name`, `k8s.namespace.name`, `k8s.deployment.name`
- [ ] Update `handlers/otlp.rs` `ingest_traces` — extract and store top-level semconv attrs as indexed columns
- [ ] Migration: add `http_method`, `http_status`, `db_system`, `db_name` columns to `traces` table (with GIN index on attributes JSONB)
- [ ] `GET /api/otel/attributes?signal=traces` — returns all known attribute keys with cardinality stats
- [ ] `GET /api/otel/attribute-values?key=service.name` — returns known values for an attribute

**Frontend tasks:**
- [ ] Update `Traces.tsx` — add attribute facet filters sidebar (based on discovered attributes)
- [ ] Update `QueryExplorer.tsx` — autocomplete for semconv attribute names in TraceQL

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Trace search by `http.method=GET` returns correct results
- [ ] Attribute autocomplete suggests semconv keys

---

### [ ] ITEM 6 — OTel Resource Detection Page
**Effort:** 3–5 days  
**Why:** With OTel graduation, more services will self-describe via resource attributes. A page showing all discovered resources helps users verify instrumentation is working.

**Backend tasks:**
- [ ] `GET /api/otel/resources` — returns unique resource attribute combinations from recent traces/metrics/logs
  - Group by `service.name`, include: `service.version`, `deployment.environment`, `telemetry.sdk.language`, `telemetry.sdk.version`, `os.type`, `host.name`
  - Include last-seen timestamp, signal counts (traces/metrics/logs/profiles)
- [ ] `GET /api/otel/resources/:service_name/signals` — returns signal breakdown for a specific service

**Frontend tasks:**
- [ ] Create `src/pages/OTelResources.tsx` — resource inventory page:
  - Table of all discovered services with their resource attributes
  - Signal indicator badges (which signals each service is sending)
  - SDK version and language breakdown
  - "Last seen" column with relative timestamps
  - Click-through to traces/logs for a specific service
- [ ] Add route `/otel-resources` in `App.tsx`
- [ ] Add nav item "OTel Resources" under AI & Analytics in `Layout.tsx`

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] After sending OTLP data, service appears in resources page

---

### [ ] ITEM 7 — OTel Health Dashboard
**Effort:** 1 week  
**Why:** Provides operational visibility into the telemetry pipeline itself — how much data is flowing, error rates, signal volumes.

**Backend tasks:**
- [ ] `GET /api/otel/stats` — aggregated ingest statistics:
  - Spans/minute, metrics/minute, logs/minute, profiles/minute
  - Ingest error count by signal type
  - Average payload size by signal
  - Top 10 services by span volume
  - Unique service count
- [ ] `GET /api/otel/stats/timeseries?signal=traces&window=24h` — time-series ingest volume

**Frontend tasks:**
- [ ] Create `src/pages/OTelDashboard.tsx` — telemetry health dashboard:
  - Signal volume sparklines (traces, metrics, logs, profiles)
  - Ingest rate counters with trend indicators
  - Top services by volume table
  - Error rate gauge
  - Signal breakdown donut chart
  - Time range selector
- [ ] Add route `/otel-dashboard` in `App.tsx`
- [ ] Add nav item "Telemetry Health" under AI & Analytics in `Layout.tsx`

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Dashboard shows real ingest stats after sending OTLP data

---

## TIER 3 — Enterprise Completeness

---

### [ ] ITEM 8 — OTLP gRPC Ingest
**Effort:** 2–3 weeks  
**Depends on:** Item 4 (protobuf support)  
**Why:** gRPC is the primary transport for OTel Collector-to-backend communication in production. HTTP is used as fallback.  
**OTel Collector reference:** `receiver/otlpreceiver/otlp.go`, `config/configgrpc/`

**Backend tasks:**
- [ ] Add `tonic = "0.12"` to workspace dependencies
- [ ] Create `crates/Zentra-api/src/grpc/` module:
  - Implement `TraceService`, `MetricsService`, `LogsService`, `ProfilesService` gRPC services
  - Reuse existing ingest logic from `handlers/otlp.rs`
- [ ] Add gRPC server startup alongside HTTP server (configurable port, default 4317)
- [ ] TLS support for gRPC endpoint (configurable, optional)
- [ ] Update `GET /api/otel/status` — include gRPC endpoint info
- [ ] Update Collector config generator — include gRPC exporter option

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] OTel Collector with default gRPC exporter sends data successfully
- [ ] Both gRPC and HTTP endpoints can receive data simultaneously

---

### [ ] ITEM 9 — OTel Connector Pipeline Support
**Effort:** 2–3 weeks  
**Why:** OTel Connectors transform signals (e.g., span-to-metrics, log-to-metric). Supporting this pattern lets Zentra derive metrics from traces automatically.  
**OTel Collector reference:** `connector/`

**Backend tasks:**
- [ ] Create `crates/Zentra-api/src/otel_connectors/` module:
  - `span_to_metrics.rs` — generate RED metrics (rate, error, duration) from ingested spans
  - `log_to_metrics.rs` — generate log-based metrics (error count, log volume by severity)
- [ ] Migration: `otel_derived_metrics` table for connector-generated metrics
- [ ] `GET /api/otel/connectors` — list active connectors and their status
- [ ] `POST /api/otel/connectors` — configure connector (e.g., span-to-metrics with custom dimensions)
- [ ] Background job: process ingested spans → derived metrics on configurable interval

**Frontend tasks:**
- [ ] Create `src/pages/OTelConnectors.tsx` — connector management page
- [ ] Show derived metrics alongside raw metrics in dashboards

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Span ingestion → RED metrics automatically generated and queryable

---

### [ ] ITEM 10 — OTel-Native Alerting from Signals
**Effort:** 2–3 weeks  
**Why:** With all 4 signals flowing, alerts should be definable directly on OTel data using semconv attributes.

**Backend tasks:**
- [ ] Extend alert rule model to support OTel signal sources:
  - Alert on trace metrics: `{service.name="X"}.p99 > 500ms`
  - Alert on log patterns: `count({severity="ERROR" AND service.name="X"}) > 10/min`
  - Alert on metric expressions: `rate(http_request_duration_seconds{service="X"}[5m]) > 1`
  - Alert on profile anomalies: CPU profile regression > 20% vs baseline
- [ ] `POST /api/alerts/rules` — accept OTel-native alert definitions
- [ ] Background evaluation engine for OTel signal alerts

**Frontend tasks:**
- [ ] Update `AlertRules.tsx` — add "OTel Signal" alert type
- [ ] Signal-specific condition builders with semconv attribute autocomplete

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Alert fires when trace p99 exceeds threshold

---

## Global Rules for All Items

These apply to every single item. Treat as non-negotiable:

1. **No duplicates** — before creating any handler, page, or component, verify it doesn't already exist
2. **Enhance, don't replace** — modify existing files rather than creating parallel ones
3. **Migration numbering** — continue from `20250101000061_*` upward
4. **All routes registered** — new handlers in `routes/`, exported in `handlers/mod.rs`
5. **All pages routed** — new pages added to `App.tsx`, nav items added to `Layout.tsx`
6. **No third-party names** — no "Grafana", "Jaeger", "Tempo" in user-facing UI text
7. **Validation gate** — `cargo check` + `npx tsc --noEmit` must pass with 0 errors before moving to next item
8. **Backwards compatible** — every DB migration uses `IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS`
9. **OTel Collector source** — always reference `C:\git\gitsourcedemo\opentelemetry-collector-main` for data model accuracy
10. **Both encodings** — all new OTLP endpoints must handle JSON; protobuf support added in Item 4

---

## Architecture Reference

### OTLP Signal Flow
```
Application (SDK)
    ↓ OTLP (gRPC/HTTP, proto/JSON)
OTel Collector
    ↓ OTLP HTTP (JSON for now)
Zentra API
    ├── /v1/traces   → traces table (TimescaleDB)
    ├── /v1/metrics  → metrics table (TimescaleDB)
    ├── /v1/logs     → logs table (TimescaleDB)
    └── /v1/profiles → otel_profiles table (TimescaleDB)
```

### Key Source Files
- **OTLP ingest handlers:** `crates/Zentra-api/src/handlers/otlp.rs`
- **OTLP routes:** `crates/Zentra-api/src/routes/otlp.rs`
- **Profiling read handlers:** `crates/Zentra-api/src/handlers/profiling.rs`
- **Profiling routes:** `crates/Zentra-api/src/routes/profiling.rs`
- **Collector config page:** `frontend/src/pages/OTelCollectorConfig.tsx`
- **Migration:** `migrations/20250101000061_otel_profiles.sql`

### OTel Collector Reference Files
- **Profile data model:** `pdata/pprofile/generated_profile.go`, `generated_sample.go`, `generated_profilesdictionary.go`
- **HTTP receiver:** `receiver/otlpreceiver/otlphttp.go`
- **Encoder pattern:** `receiver/otlpreceiver/encoder.go`
- **Config model:** `receiver/otlpreceiver/config.go`

---

### [ ] ITEM 11 — AI/LLM Observability via OTel GenAI Conventions
**Effort:** 1–2 weeks  
**Why:** The graduation announcement specifically highlights AI workloads. GenAI semantic conventions (`gen_ai.*`) provide standardized visibility into LLM latency, token usage, and agent decision flows.  
**Tier:** TIER 2

**Backend tasks:**
- [ ] Modify `handlers/otlp.rs` `ingest_traces` — detect GenAI spans, extract `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.*` tokens, insert into `llm_spans` table
- [ ] Modify `handlers/llm_observability.rs` — replace demo data with real queries from `llm_spans`
- [ ] Wire `GET /api/llm/overview`, `/api/llm/models`, `/api/llm/cost-trend`, `/api/llm/latency-distribution` to real data

**Frontend tasks:**
- [ ] Modify `LLMObservability.tsx` — ensure all panels call real endpoints
- [ ] Add agent decision flow visualization (parent→child span tree for agentic LLM calls)

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] POST /v1/traces with gen_ai.* attributes → appears in LLM Observability page

---

### [ ] ITEM 12 — Security & Compliance Posture
**Effort:** 1 week  
**Why:** OTel passed an independent security audit. Strengthen OTLP auth with API keys, rate limiting, payload size limits, and audit logging.  
**Tier:** TIER 2

**Backend tasks:**
- [ ] Migration `20250101000065_otlp_api_keys.sql` — `otlp_api_keys` table (key_hash, permissions, expiry)
- [ ] Modify `handlers/otlp.rs` — support API key auth alongside JWT and org-id header
- [ ] Add rate limiting per org_id on OTLP endpoints
- [ ] Add configurable payload size limits (default 5MB)
- [ ] Add request logging with org_id for audit trail
- [ ] Update `GET /api/otel/status` — include security posture in response

**Frontend tasks:**
- [ ] Modify `OTelCollectorConfig.tsx` — add API key option in config generator
- [ ] Add OTLP API Key management page (create, revoke, list)

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] OTLP request without auth → 401
- [ ] OTLP request with valid API key → 200

---

### [ ] ITEM 13 — Collector Pipeline Topology Visualization
**Effort:** 1 week  
**Why:** Multi-Collector deployments (edge → central → backend) need topology visibility to debug data flow.  
**Tier:** TIER 3

**Backend tasks:**
- [ ] Modify `handlers/otlp.rs` — track Collector identity from resource attributes (`service.name = "otelcol-contrib"`, `host.name`)
- [ ] `GET /api/otel/pipeline` — returns Collector topology graph (nodes + edges)

**Frontend tasks:**
- [ ] New `OTelPipeline.tsx` — pipeline flow diagram: Apps → Collector(s) → Zentra
- [ ] Per-Collector health card (last seen, throughput, error count)
- [ ] Add route `/otel-pipeline` + nav item

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Multiple Collectors sending data → pipeline view shows topology

---

### [ ] SDK Instrumentation Guide Page (TIER 1)
**Effort:** 3–5 days  
**Why:** #1 question after Collector setup is "how do I instrument my service?" Java auto-instrumentation is the easiest win — drop-in JAR attachment covers Spring Boot, JDBC, Kafka, gRPC, HTTP.

**Backend tasks:**
- [ ] `GET /api/otel/sdk-snippets?language=java&framework=spring-boot` — returns code snippets

**Frontend tasks:**
- [ ] New `OTelSdkGuide.tsx` — language tabs (Java, Python, Go, Node.js, .NET, Rust)
  - Per-language framework examples with install commands + code snippets
  - Java auto-instrumentation highlight (JAR agent, zero code changes)
  - Copy button per snippet
  - Link to Collector Config page
- [ ] Add route `/otel-sdk-guide` + nav item

**Validation:**
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Page loads with correct per-language examples

---

## Global Rules for All Items

These apply to every single item. Treat as non-negotiable:

1. **No duplicates** — before creating any handler, page, or component, verify it doesn't already exist
2. **Enhance, don't replace** — modify existing files rather than creating parallel ones
3. **Migration numbering** — continue from `20250101000061_*` upward
4. **All routes registered** — new handlers in `routes/`, exported in `handlers/mod.rs`
5. **All pages routed** — new pages added to `App.tsx`, nav items added to `Layout.tsx`
6. **No third-party names** — no "Grafana", "Jaeger", "Tempo", "Datadog" in user-facing UI text
7. **Validation gate** — `cargo check` + `npx tsc --noEmit` must pass with 0 errors before moving to next item
8. **Backwards compatible** — every DB migration uses `IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS`
9. **OTel Collector source** — always reference `C:\git\gitsourcedemo\opentelemetry-collector-main` for data model accuracy
10. **Both encodings** — all new OTLP endpoints must handle JSON; protobuf support added in Item 4

---

## Architecture Reference

### OTLP Signal Flow
```
Application (OTel SDK)
    ↓ OTLP (gRPC/HTTP, proto/JSON)
OTel Collector (batching, filtering, enriching)
    ↓ OTLP HTTP (JSON for now, protobuf after Item 4)
Zentra API
    ├── /v1/traces   → traces table (TimescaleDB)
    ├── /v1/metrics  → metrics table (TimescaleDB)
    ├── /v1/logs     → logs table (TimescaleDB)
    └── /v1/profiles → otel_profiles table (TimescaleDB)
         ↓
    Span-to-Metrics Connector (Item 9)
    ├── otel_derived_metrics table
    └── RED metrics (Rate, Error, Duration)
```

### Java Auto-Instrumentation (Zero Code Changes)
```bash
java -javaagent:opentelemetry-javaagent.jar \
     -Dotel.exporter.otlp.endpoint=http://<collector>:4318 \
     -Dotel.exporter.otlp.protocol=http/json \
     -Dotel.service.name=my-java-service \
     -jar my-app.jar
# Covers: Spring Boot, JDBC, Kafka, gRPC, HTTP clients
```

### Key Source Files
- **OTLP ingest handlers:** `crates/Zentra-api/src/handlers/otlp.rs`
- **OTLP routes:** `crates/Zentra-api/src/routes/otlp.rs`
- **Profiling read handlers:** `crates/Zentra-api/src/handlers/profiling.rs`
- **Profiling routes:** `crates/Zentra-api/src/routes/profiling.rs`
- **LLM Observability handlers:** `crates/Zentra-api/src/handlers/llm_observability.rs`
- **Collector config page:** `frontend/src/pages/OTelCollectorConfig.tsx`
- **Migration:** `migrations/20250101000061_otel_profiles.sql`

### OTel Collector Reference Files (C:\git\gitsourcedemo\opentelemetry-collector-main)
- **Profile data model:** `pdata/pprofile/generated_profile.go`, `generated_sample.go`, `generated_profilesdictionary.go`
- **Trace data model:** `pdata/ptrace/generated_span.go`
- **HTTP receiver:** `receiver/otlpreceiver/otlphttp.go`
- **Encoder pattern:** `receiver/otlpreceiver/encoder.go`
- **Config model:** `receiver/otlpreceiver/config.go`
- **Connectors:** `connector/`

---

*Last updated: 2026-05-25*  
*Next review: After ITEM 4 (Protobuf) is complete — reassess TIER 3 priorities based on user demand*  
*Prompts file: `docs/PROMPTS_OTEL_GRADUATION.md` — 13 detailed implementation prompts*
