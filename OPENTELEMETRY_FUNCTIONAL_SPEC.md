# Zentra — OpenTelemetry Module: Functional Specification

> **Version:** 1.0  
> **Last Updated:** May 2026  
> **Module Owner:** Zentra Engineering  
> **Status:** Implemented & Deployed

---

## Table of Contents

1. [Module Overview](#1-module-overview)
2. [Architecture](#2-architecture)
3. [Menu Structure](#3-menu-structure)
4. [Feature Specifications](#4-feature-specifications)
   - 4.1 [Telemetry Health](#41-telemetry-health)
   - 4.2 [Traces](#42-traces)
   - 4.3 [Discovered Services](#43-discovered-services)
   - 4.4 [Collector Setup](#44-collector-setup)
   - 4.5 [SDK Guide](#45-sdk-guide)
   - 4.6 [Pipeline Topology](#46-pipeline-topology)
   - 4.7 [Span Metrics (RED)](#47-span-metrics-red)
   - 4.8 [Signal Alerts](#48-signal-alerts)
   - 4.9 [GenAI Observability](#49-genai-observability)
   - 4.10 [OTLP Security](#410-otlp-security)
5. [OTLP Ingest Engine](#5-otlp-ingest-engine)
6. [Database Schema](#6-database-schema)
7. [Semantic Conventions](#7-semantic-conventions)
8. [Workable Prompts](#8-workable-prompts)

---

## 1. Module Overview

The OpenTelemetry module makes Zentra a **native OTLP backend** — any application instrumented with OpenTelemetry SDKs (Java, Python, Go, Node.js, .NET, Rust) can send traces, metrics, logs, and profiles directly to Zentra without any intermediary.

### Key Capabilities

| Capability | Detail |
|-----------|--------|
| **Signals** | Traces, Metrics, Logs, Profiles (4th signal, alpha) |
| **Transports** | HTTP (:7001) + gRPC (:4317) |
| **Encodings** | JSON (`application/json`) + Protobuf (`application/x-protobuf`) |
| **Auth** | JWT Bearer, OTLP API Keys (`isok_` prefix), `X-Zentra-Org-Id` header |
| **Standards** | OTLP v1.3.0, OTel Semantic Conventions |
| **Storage** | PostgreSQL + TimescaleDB hypertables |

### Why This Exists

- Applications send data via standard OTel SDKs — no custom agents needed
- Drop-in replacement for Datadog/New Relic/Grafana when customers use OTel
- CNCF-graduated standard = enterprise trust
- Profiles (4th signal) = market differentiator

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Applications                                 │
│  Java (agent JAR)  Python  Go  Node.js  .NET  Rust              │
│  ─────────────────────────────────────────────                   │
│  OTel SDK auto-instruments: HTTP, DB, gRPC, Kafka, LLM calls   │
└──────────────┬──────────────────────────┬───────────────────────┘
               │ OTLP/HTTP (JSON/Proto)   │ OTLP/gRPC (Proto)
               ▼                          ▼
┌──────────────────────────────────────────────────────────────────┐
│                      Zentra Backend                          │
│                                                                  │
│  HTTP Server (:7001)              gRPC Server (:4317)            │
│  ├── POST /v1/traces              ├── TraceService/Export        │
│  ├── POST /v1/metrics             ├── MetricsService/Export      │
│  ├── POST /v1/logs                ├── LogsService/Export         │
│  └── POST /v1/profiles            └── ProfilesService/Export     │
│                                                                  │
│  Auth Layer: JWT | OTLP API Key (isok_) | X-Zentra-Org-Id   │
│  Encoding:   JSON ←→ Protobuf (prost decode → JSON normalize)   │
│  Enrichment: Semantic Convention extraction → indexed columns    │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Shared Ingest Pipeline                   │ │
│  │  1. Auth resolution (org_id)                                │ │
│  │  2. Encoding detection (JSON or Protobuf)                   │ │
│  │  3. Decode body → serde_json::Value                         │ │
│  │  4. Extract OTel semantic conventions → indexed columns     │ │
│  │  5. INSERT into TimescaleDB hypertable                      │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────────────────────────┐
│                PostgreSQL + TimescaleDB                           │
│  traces         (hypertable, partitioned by started_at)          │
│  metrics        (hypertable, partitioned by timestamp)           │
│  logs           (hypertable, partitioned by timestamp)           │
│  otel_profiles  (hypertable, partitioned by started_at)          │
│  otlp_api_keys  (regular table)                                  │
│  otlp_ingest_audit (hypertable)                                  │
│  otel_alert_rules  (regular table)                               │
│  span_red_metrics_1m (materialized view)                         │
└──────────────────────────────────────────────────────────────────┘
```

---

## 3. Menu Structure

**Sidebar → OpenTelemetry** (icon: Satellite)

| # | Menu Item | Route | Page Component | Primary API |
|---|-----------|-------|----------------|-------------|
| 1 | Telemetry Health | `/otel-dashboard` | `OTelDashboard.tsx` | `GET /api/otel/stats` |
| 2 | Traces | `/traces` | `Traces.tsx` | `GET /api/traces` |
| 3 | Discovered Services | `/otel-resources` | `OTelResources.tsx` | `GET /api/otel/resources` |
| 4 | Collector Setup | `/otel-config` | `OTelCollectorConfig.tsx` | `GET /api/otel/collector-config` |
| 5 | SDK Guide | `/otel-sdk-guide` | `OTelSdkGuide.tsx` | `GET /api/otel/sdk-snippets` |
| 6 | Pipeline Topology | `/otel-pipeline` | `OTelPipeline.tsx` | `GET /api/otel/pipeline` |
| 7 | Span Metrics (RED) | `/otel-span-metrics` | `OTelSpanMetrics.tsx` | `GET /api/otel/span-metrics` |
| 8 | Signal Alerts | `/otel-alert-rules` | `OTelAlertRules.tsx` | `GET/POST /api/otel/alert-rules` |
| 9 | GenAI Observability | `/otel-genai` | `OTelGenAI.tsx` | `GET /api/otel/genai` |
| 10 | OTLP Security | `/otel-security` | `OTelSecurity.tsx` | `GET/POST/DELETE /api/otel/api-keys` |

---

## 4. Feature Specifications

### 4.1 Telemetry Health

**Purpose:** Real-time dashboard showing the health and throughput of the OTLP ingestion pipeline.

**Route:** `/otel-dashboard`  
**API:** `GET /api/otel/stats?window={5m|15m|1h|6h|24h|7d}`  
**Backend Handler:** `handlers/otlp.rs → otel_stats()`

**What It Shows:**
- **Spans Total** — total trace spans ingested in the selected time window
- **Spans/min** — current ingest throughput
- **Profiles Total** — continuous profiling data points ingested
- **Unique Services** — distinct `service.name` values sending data
- **Error Count & Error Rate %** — spans with `status = error`
- **Avg Span Duration** — mean latency across all spans
- **Top Services Table** — ranked by span count with error counts

**User Flow:**
1. User navigates to OpenTelemetry → Telemetry Health
2. Selects a time window (5m to 7d)
3. Dashboard auto-refreshes every 15 seconds
4. User can see if any service has elevated error rates
5. Click on a service name to jump to Traces filtered by that service

**DB Queries:**
- Aggregates from `traces` hypertable: `COUNT(*)`, `COUNT(*) FILTER (WHERE status = 'error')`, `AVG(duration_ms)`, grouped by `service_name`
- Aggregates from `otel_profiles` for profile counts

---

### 4.2 Traces

**Purpose:** Distributed tracing explorer — search, filter, and inspect individual trace spans.

**Route:** `/traces`  
**API:** `GET /api/traces?service=X&status=error&limit=50`  
**Backend Handler:** `handlers/otlp.rs → ingest_traces()` (ingest), existing trace query handlers (read)

**What It Shows:**
- **Span List** — filterable table of spans with trace_id, span_id, service, operation, duration, status
- **Trace Waterfall** — Gantt-style visualization of a single trace (parent → child spans)
- **Span Detail** — attributes, events, links, resource attributes, semantic convention fields
- **Filters** — by service name, status (ok/error/unset), HTTP method, DB system, time range

**OTLP Ingest Fields Stored Per Span:**
| Field | Source | DB Column |
|-------|--------|-----------|
| trace_id | `span.traceId` | `trace_id TEXT` |
| span_id | `span.spanId` | `span_id TEXT` |
| parent_span_id | `span.parentSpanId` | `parent_span_id TEXT` |
| service_name | `resource.attributes["service.name"]` | `service_name TEXT` |
| operation_name | `span.name` | `operation_name TEXT` |
| started_at | `span.startTimeUnixNano` | `started_at TIMESTAMPTZ` |
| ended_at | `span.endTimeUnixNano` | `ended_at TIMESTAMPTZ` |
| duration_ms | computed | `duration_ms DOUBLE` |
| status | `span.status.code` (0=unset, 1=ok, 2=error) | `status TEXT` |
| kind | `span.kind` (1-5) | `kind TEXT` |
| attributes | `span.attributes[]` | `attributes JSONB` |
| resource_attributes | `resource.attributes[]` | `resource_attributes JSONB` |
| events | `span.events[]` | `events JSONB` |
| links | `span.links[]` | `links JSONB` |
| http_method | semconv extraction | `http_method TEXT` |
| http_status_code | semconv extraction | `http_status_code INT` |
| db_system | semconv extraction | `db_system TEXT` |
| rpc_system | semconv extraction | `rpc_system TEXT` |
| messaging_system | semconv extraction | `messaging_system TEXT` |
| gen_ai_system | semconv extraction | `gen_ai_system TEXT` |

---

### 4.3 Discovered Services

**Purpose:** Auto-discovered inventory of all services and resources sending OTel data.

**Route:** `/otel-resources`  
**API:** `GET /api/otel/resources?window={1h|24h|7d}`  
**Backend Handler:** `handlers/otlp.rs → otel_resources()`

**What It Shows:**
- **Service Cards** — each distinct `service.name` with:
  - Host (`host.name`), SDK language (`telemetry.sdk.language`), SDK version
  - Span count, error rate, last seen timestamp
  - Cloud metadata (provider, region, AZ) if present
  - Kubernetes metadata (namespace, pod, node) if present
- **Active vs Stale** — services not seen in 5+ minutes flagged as stale

**User Flow:**
1. User navigates to Discovered Services
2. Sees all applications that have sent OTel data
3. Can identify which services are instrumented, which SDK they use
4. Click service → navigates to Traces filtered by that service

---

### 4.4 Collector Setup

**Purpose:** Generate an OpenTelemetry Collector configuration file, and explain that direct-to-Zentra is the primary path.

**Route:** `/otel-config`  
**API:** `GET /api/otel/collector-config?org_id=X&traces=true&metrics=true&logs=true&profiles=true`  
**Backend Handler:** `handlers/otlp.rs → collector_config()`

**What It Shows:**
- **Direct Ingest Section** (primary, highlighted green):
  - HTTP env vars: `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_PROTOCOL`, `OTEL_SERVICE_NAME`
  - gRPC env vars: same with `grpc` protocol and port 4317
  - Message: "No Collector, no Docker, no extra infrastructure"
- **Collector Config Generator** (optional, below divider):
  - Configurable: host, port, org_id, API key
  - Signal toggles: traces, metrics, logs, profiles
  - YAML preview with copy/download buttons
  - Quick Start: binary install, Docker (optional), Kubernetes
- **OTLP Endpoints Reference**: all 4 endpoints, auth methods, encoding, rate limits

**Generated YAML Structure:**
```yaml
receivers:
  otlp:
    protocols:
      grpc: { endpoint: 0.0.0.0:4317 }
      http: { endpoint: 0.0.0.0:4318 }
processors:
  batch: { timeout: 5s, send_batch_size: 1024 }
exporters:
  otlphttp/zentra:
    endpoint: http://<host>:<port>
    encoding: json
    headers: { X-Zentra-Org-Id: "<org-id>" }
service:
  pipelines:
    traces: { receivers: [otlp], processors: [batch], exporters: [otlphttp/zentra] }
    metrics: { ... }
    logs: { ... }
    profiles: { ... }
```

---

### 4.5 SDK Guide

**Purpose:** Language-specific setup instructions for instrumenting applications with OTel SDKs.

**Route:** `/otel-sdk-guide`  
**API:** `GET /api/otel/sdk-snippets?language={all|java|python|go|nodejs|dotnet|rust}`  
**Backend Handler:** `handlers/otlp.rs → sdk_snippets()`

**Supported Languages:**

| Language | Auto-Instrument? | Key Frameworks |
|----------|-----------------|----------------|
| **Java** | Yes (JAR agent, zero code changes) | Spring Boot, JDBC, Kafka, gRPC, JAX-RS |
| **Python** | Yes (`opentelemetry-instrument`) | Flask, Django, FastAPI, SQLAlchemy, requests |
| **Go** | Manual SDK | net/http, gRPC, database/sql, Gin, Echo |
| **Node.js** | Yes (`@opentelemetry/auto-instrumentations-node`) | Express, Fastify, NestJS, pg, Redis |
| **.NET** | Yes (NuGet packages) | ASP.NET Core, HttpClient, SqlClient, EF |
| **Rust** | Manual SDK | axum, tonic, reqwest, sqlx |

**Per Language, Shows:**
1. **Install** — package manager commands
2. **Configure & Run** — code snippet or CLI command
3. **Environment Variables** — `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`, etc.
4. **Supported Frameworks** — badge list

**User Flow:**
1. User selects their language from the sidebar
2. Edits the "Zentra host" field to match their deployment
3. Copies install command, adds to their build
4. Copies run command or code snippet
5. Sets env vars → application starts sending OTel data

---

### 4.6 Pipeline Topology

**Purpose:** Visualize the telemetry data flow: Application → (optional Collector) → Zentra.

**Route:** `/otel-pipeline`  
**API:** `GET /api/otel/pipeline`  
**Backend Handler:** `handlers/otlp.rs → otel_pipeline()`

**What It Shows:**
- **Three-column flow diagram:**
  - **Applications** (blue) — services classified by `service.name`
  - **Collectors** (amber) — services matching `*otelcol*` or `*collector*`
  - **Zentra** (green) — the backend with all 4 signal endpoints
- **Per Node:** service name, host, span count, last seen, active/stale indicator
- **Summary Cards:** total applications, total collectors, backend status
- **Auto-refresh:** every 30 seconds

**Detection Logic:**
- Queries `traces` table for distinct `resource_attributes->>'service.name'`
- Services named `otelcol*` or `*collector*` → classified as Collector nodes
- All others → classified as Application nodes
- `host.name` and `service.instance.id` used for deduplication

---

### 4.7 Span Metrics (RED)

**Purpose:** Derive Rate, Error, and Duration metrics automatically from trace spans — the RED methodology.

**Route:** `/otel-span-metrics`  
**API:** `GET /api/otel/span-metrics?window={5m|15m|1h|6h|24h}&group_by={service|method|db}`  
**Backend Handler:** `handlers/otlp.rs → otel_span_metrics()`

**What It Shows Per Service:**
- **Rate** — total requests + requests/second
- **Error** — total errors + error rate % (color-coded: green <1%, amber 1-5%, red >5%)
- **Duration** — latency percentiles (p50, p95, p99, avg) with visual bars
- **Grouping** — by service, HTTP method, or database system
- **Time Windows** — 5m, 15m, 1h, 6h, 24h

**Data Sources:**
- **Primary:** `span_red_metrics_1m` materialized view (pre-aggregated)
- **Fallback:** Direct query on `traces` hypertable with real-time aggregation
- Fields: `request_count`, `error_count`, `avg_duration_ms`, `p50_ms`, `p95_ms`, `p99_ms`

**User Flow:**
1. User selects time window and grouping dimension
2. Each service shows a RED card with rate, error rate, and latency bars
3. Click service name → navigates to Traces filtered by that service
4. Auto-refreshes every 15 seconds

---

### 4.8 Signal Alerts

**Purpose:** Define and evaluate alert conditions on OTel signal data.

**Route:** `/otel-alert-rules`  
**API:**
- `GET /api/otel/alert-rules` — list rules
- `POST /api/otel/alert-rules` — create rule
- `GET /api/otel/alert-rules/evaluate` — evaluate all rules now

**Backend Handler:** `handlers/otlp.rs → otel_alert_rules_list()`, `otel_alert_rules_create()`, `otel_alert_rules_evaluate()`

**Alert Rule Schema:**
```json
{
  "name": "High Error Rate",
  "signal": "traces",
  "condition": { "operator": ">", "field": "error_rate_percent" },
  "threshold": 5.0,
  "window_seconds": 300,
  "severity": "warning",
  "description": "Error rate exceeds 5% over 5 minutes"
}
```

**Built-in Default Rules:**
1. **High Error Rate** — error_rate > 5% in 5m → warning
2. **Very High Error Rate** — error_rate > 15% in 5m → critical
3. **High P99 Latency** — p99_ms > 5000 in 5m → warning
4. **Low Throughput** — requests < 1/min in 15m → info

**What It Shows:**
- **Rules Table** — name, signal, condition, threshold, window, severity
- **Create Form** — name, signal (traces/metrics/logs), severity, threshold, window
- **Evaluate Now** — runs all rules against live data, shows FIRING/OK status with current vs threshold values
- **Evaluation Results** — color-coded list of rule outcomes

**DB Table:** `otel_alert_rules`
| Column | Type |
|--------|------|
| id | UUID PK |
| org_id | UUID |
| name | TEXT |
| signal | TEXT |
| metric | TEXT |
| condition | JSONB |
| threshold | DOUBLE |
| window_seconds | INT |
| severity | TEXT |
| enabled | BOOLEAN |
| created_at | TIMESTAMPTZ |

---

### 4.9 GenAI Observability

**Purpose:** Monitor LLM/AI API calls captured via OTel GenAI semantic conventions.

**Route:** `/otel-genai`  
**API:** `GET /api/otel/genai?window={1h|6h|24h|7d}`  
**Backend Handler:** `handlers/otlp.rs → otel_genai()`

**What It Shows:**
- **Summary Cards:** total LLM calls, total tokens consumed, unique models
- **Model Performance Table** — per model:
  - Call count, error rate, avg latency, p95 latency
  - Token usage: avg input/output/total, sum total
- **Recent Calls Table** — individual LLM spans:
  - trace_id, operation, service, model, duration, status
  - Input/output/total tokens, temperature
  - Link to trace detail
- **Semantic Conventions Used** — list of `gen_ai.*` attributes

**OTel GenAI Semantic Convention Attributes:**
| Attribute | Purpose |
|-----------|---------|
| `gen_ai.system` | LLM provider (openai, anthropic, etc.) |
| `gen_ai.request.model` | Requested model name |
| `gen_ai.request.max_tokens` | Max tokens requested |
| `gen_ai.request.temperature` | Sampling temperature |
| `gen_ai.response.model` | Model used in response |
| `gen_ai.usage.input_tokens` | Input token count |
| `gen_ai.usage.output_tokens` | Output token count |
| `gen_ai.usage.total_tokens` | Total token count |

**Detection Logic:**
- Queries `traces` where `gen_ai_system IS NOT NULL`
- Extracts token counts from `attributes` JSONB field
- Groups by model and system for the performance breakdown

---

### 4.10 OTLP Security

**Purpose:** Manage OTLP API keys, rate limits, and view ingest audit logs.

**Route:** `/otel-security`  
**API:**
- `GET /api/otel/api-keys` — list keys
- `POST /api/otel/api-keys` — create key (returns raw key once)
- `DELETE /api/otel/api-keys/:id` — revoke key
- `GET /api/otel/ingest-audit?window=1h&limit=50` — audit log

**Backend Handler:** `handlers/otlp_security.rs`

**What It Shows:**
- **Summary Cards:** active keys count, audit entries (1h), auth methods supported
- **API Key Management:**
  - Create: name, permissions (traces/metrics/logs/profiles), rate limit RPM, max payload bytes, expiry
  - List: prefix, permissions, rate limit, last used, created date
  - Revoke: soft-delete (sets `enabled = false`)
  - **Raw key shown ONCE on creation** — SHA-256 hash stored in DB
- **Ingest Audit Log:**
  - Per-request: signal, endpoint, status code, span count, payload bytes, remote IP, user agent, timestamp
  - Auto-refreshes every 15 seconds

**API Key Format:** `isok_<random-32-chars>` (prefix `isok_` = Zentra OTLP Key)

**DB Tables:**

`otlp_api_keys`:
| Column | Type | Purpose |
|--------|------|---------|
| id | UUID PK | Key identifier |
| org_id | UUID | Owner organization |
| name | TEXT | Human-readable name |
| key_hash | TEXT | SHA-256 hash of raw key |
| key_prefix | TEXT | First 8 chars for display |
| permissions | TEXT[] | Allowed signals |
| rate_limit_rpm | INT | Requests per minute |
| max_payload_bytes | INT | Max body size |
| enabled | BOOLEAN | Active/revoked |
| expires_at | TIMESTAMPTZ | Optional expiry |
| last_used_at | TIMESTAMPTZ | Last usage timestamp |

`otlp_ingest_audit`:
| Column | Type | Purpose |
|--------|------|---------|
| id | BIGSERIAL | Auto-increment |
| org_id | UUID | Organization |
| api_key_id | UUID | Which key was used |
| signal | TEXT | traces/metrics/logs/profiles |
| endpoint | TEXT | /v1/traces etc. |
| status_code | INT | HTTP response code |
| span_count | INT | Items in payload |
| payload_bytes | INT | Request body size |
| remote_addr | TEXT | Client IP |
| created_at | TIMESTAMPTZ | Partitioned by this |

---

## 5. OTLP Ingest Engine

### Endpoints

| Method | Path | Signal | Handler |
|--------|------|--------|---------|
| POST | `/v1/traces` | Traces | `ingest_traces()` |
| POST | `/v1/metrics` | Metrics | `ingest_metrics()` |
| POST | `/v1/logs` | Logs | `ingest_logs()` |
| POST | `/v1/profiles` | Profiles | `ingest_profiles()` |

### Auth Resolution Order
1. **JWT Bearer** — standard Zentra auth token → extracts `org_id` from claims
2. **OTLP API Key** — `Authorization: Bearer isok_...` → validates against `otlp_api_keys` table
3. **Header Fallback** — `X-Zentra-Org-Id: <uuid>` → direct org specification

### Encoding Detection
- `Content-Type: application/json` → JSON path (default)
- `Content-Type: application/x-protobuf` → Protobuf path (prost decode → JSON normalize)
- No Content-Type → defaults to JSON

### Processing Pipeline
1. Detect encoding from `Content-Type` header
2. Resolve `org_id` from auth (JWT → API key → header)
3. Decode body: JSON via `serde_json`, Protobuf via `prost::Message::decode()` → convert to JSON
4. Walk the OTLP resource/scope/item hierarchy
5. Extract `service.name` from resource attributes
6. Extract semantic convention fields (`http_method`, `db_system`, `gen_ai_system`, etc.)
7. INSERT into the appropriate hypertable
8. Return `{ "partialSuccess": {} }` per OTLP spec

### gRPC Server (Port 4317)
- Runs alongside HTTP server via `tokio::spawn`
- Implements: `TraceService/Export`, `MetricsService/Export`, `LogsService/Export`, `ProfilesService/Export`
- Same auth resolution via gRPC metadata
- Decodes protobuf → converts to JSON → shares the same insert logic
- Feature-gated behind `--features grpc`

---

## 6. Database Schema

### Traces Hypertable (migration 001 + 062)
```sql
CREATE TABLE traces (
    org_id UUID NOT NULL,
    trace_id TEXT NOT NULL,
    span_id TEXT NOT NULL,
    parent_span_id TEXT,
    service_name TEXT NOT NULL,
    operation_name TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    duration_ms DOUBLE PRECISION GENERATED ALWAYS AS (EXTRACT(EPOCH FROM (ended_at - started_at)) * 1000) STORED,
    status TEXT DEFAULT 'unset',
    status_message TEXT,
    kind TEXT DEFAULT 'unspecified',
    attributes JSONB DEFAULT '{}',
    resource_attributes JSONB DEFAULT '{}',
    events JSONB DEFAULT '[]',
    links JSONB DEFAULT '[]',
    -- Semantic convention indexed columns (migration 062)
    http_method TEXT,
    http_status_code INT,
    db_system TEXT,
    rpc_system TEXT,
    messaging_system TEXT,
    gen_ai_system TEXT
);
SELECT create_hypertable('traces', 'started_at', if_not_exists => true);
```

### OTel Profiles Hypertable (migration 061)
```sql
CREATE TABLE otel_profiles (
    org_id UUID NOT NULL,
    profile_id TEXT NOT NULL,
    service_name TEXT NOT NULL,
    resource_attributes JSONB DEFAULT '{}',
    profile_type TEXT DEFAULT 'cpu',
    period_type TEXT,
    period_value BIGINT,
    duration_nanos BIGINT DEFAULT 0,
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    sample_count INT DEFAULT 0,
    drop_frames TEXT,
    keep_frames TEXT,
    flamegraph_data JSONB DEFAULT '[]',
    attributes JSONB DEFAULT '{}'
);
SELECT create_hypertable('otel_profiles', 'started_at', if_not_exists => true);
```

### Span RED Metrics Materialized View (migration 063)
```sql
CREATE MATERIALIZED VIEW span_red_metrics_1m AS
SELECT
    date_trunc('minute', started_at) AS bucket,
    org_id, service_name, http_method, http_status_code, db_system,
    COUNT(*) AS request_count,
    COUNT(*) FILTER (WHERE status = 'error') AS error_count,
    AVG(duration_ms) AS avg_duration_ms,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_ms) AS p50_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms) AS p95_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms) AS p99_ms,
    MIN(duration_ms) AS min_duration_ms,
    MAX(duration_ms) AS max_duration_ms
FROM traces
GROUP BY bucket, org_id, service_name, http_method, http_status_code, db_system;
```

### OTel Alert Rules (migration 064)
```sql
CREATE TABLE otel_alert_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL,
    name TEXT NOT NULL,
    signal TEXT NOT NULL,
    metric TEXT,
    condition JSONB NOT NULL,
    threshold DOUBLE PRECISION NOT NULL,
    window_seconds INT NOT NULL DEFAULT 300,
    severity TEXT NOT NULL DEFAULT 'warning',
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### OTLP API Keys + Audit (migration 065)
```sql
CREATE TABLE otlp_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    key_prefix TEXT NOT NULL,
    permissions TEXT[] NOT NULL DEFAULT '{traces,metrics,logs,profiles}',
    rate_limit_rpm INT NOT NULL DEFAULT 10000,
    max_payload_bytes INT NOT NULL DEFAULT 5242880,
    enabled BOOLEAN NOT NULL DEFAULT true,
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE otlp_ingest_audit (
    id BIGSERIAL,
    org_id UUID NOT NULL,
    api_key_id UUID,
    signal TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    status_code INT NOT NULL,
    span_count INT DEFAULT 0,
    payload_bytes INT DEFAULT 0,
    remote_addr TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SELECT create_hypertable('otlp_ingest_audit', 'created_at', if_not_exists => true);
```

---

## 7. Semantic Conventions

Zentra implements OTel Semantic Conventions for automatic attribute extraction and indexing.

**Domains Covered:**

| Domain | Key Attributes | Indexed Column |
|--------|---------------|----------------|
| Service | `service.name`, `service.version`, `service.namespace`, `service.instance.id` | `service_name` |
| HTTP | `http.method`, `http.request.method`, `http.status_code`, `http.response.status_code`, `http.route`, `url.full` | `http_method`, `http_status_code` |
| Database | `db.system`, `db.name`, `db.statement`, `db.operation` | `db_system` |
| RPC/gRPC | `rpc.system`, `rpc.service`, `rpc.method`, `rpc.grpc.status_code` | `rpc_system` |
| Messaging | `messaging.system`, `messaging.destination.name`, `messaging.operation` | `messaging_system` |
| Cloud | `cloud.provider`, `cloud.region`, `cloud.availability_zone`, `cloud.account.id` | — |
| Kubernetes | `k8s.pod.name`, `k8s.namespace.name`, `k8s.deployment.name`, `k8s.node.name` | — |
| Host | `host.name`, `host.id`, `os.type` | — |
| GenAI/LLM | `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.*_tokens` | `gen_ai_system` |
| Telemetry SDK | `telemetry.sdk.name`, `telemetry.sdk.language`, `telemetry.sdk.version` | — |

**API Endpoints for Semconv:**
- `GET /api/otel/semconv` — full catalog of known attributes with domain, description, type
- `GET /api/otel/attributes?signal=traces&window=1h` — actually-observed attributes in data
- `GET /api/otel/attribute-values?key=http.method&window=1h` — distinct values for an attribute

---

## 8. Workable Prompts

These are implementation-ready prompts for each OpenTelemetry menu item. Each prompt can be handed to a developer or AI assistant to build or enhance the feature.

---

### PROMPT OT-01: Telemetry Health Dashboard

```
GOAL: Build the Telemetry Health page (/otel-dashboard) that provides real-time
visibility into OTLP ingestion throughput and health.

BACKEND (Rust/axum):
  Handler: GET /api/otel/stats?window={5m|15m|1h|6h|24h|7d}
  - Query the `traces` hypertable:
    SELECT COUNT(*) AS spans_total,
           COUNT(*) / EXTRACT(EPOCH FROM interval) * 60 AS spans_per_minute,
           COUNT(DISTINCT service_name) AS unique_services,
           COUNT(*) FILTER (WHERE status = 'error') AS error_count,
           AVG(duration_ms) AS avg_span_duration_ms
    FROM traces WHERE org_id = $1 AND started_at > NOW() - interval
  - Query `otel_profiles` for profiles_total, profiles_per_minute
  - Top services: GROUP BY service_name ORDER BY COUNT(*) DESC LIMIT 10
  - Return JSON with all stats

FRONTEND (React/TypeScript):
  Page: OTelDashboard.tsx
  - Time window selector (button group: 5m, 15m, 1h, 6h, 24h, 7d)
  - 6 stat cards: Spans Total, Spans/min, Profiles Total, Unique Services,
    Error Count, Avg Duration
  - Error Rate gauge (circular, color-coded)
  - Top Services table with service name, span count, error count, error rate
  - Auto-refresh: useQuery with refetchInterval: 15_000
  - Click service name → navigate to /traces?service={name}

DB: Uses existing `traces` and `otel_profiles` hypertables (no new migrations)
```

---

### PROMPT OT-02: Distributed Traces Explorer

```
GOAL: Build the Traces page (/traces) for searching, filtering, and inspecting
distributed trace spans ingested via OTLP.

BACKEND (Rust/axum):
  Ingest Handler: POST /v1/traces
  - Accept application/json and application/x-protobuf (feature-gated)
  - Auth: JWT | OTLP API key | X-Zentra-Org-Id header
  - Walk resourceSpans → scopeSpans → spans
  - Extract service_name from resource attributes
  - Extract semconv fields: http_method, http_status_code, db_system,
    rpc_system, messaging_system, gen_ai_system
  - INSERT INTO traces with all fields
  - Return { "partialSuccess": {} }

  Query Handler: GET /api/traces?service=X&status=Y&operation=Z&limit=50&offset=0
  - Filter traces by service_name, status, operation_name, time range
  - Return span list with all fields
  - GET /api/traces/:trace_id — all spans for a single trace (for waterfall view)

FRONTEND (React/TypeScript):
  Page: Traces.tsx
  - Filter bar: service dropdown, status (ok/error/unset), time range picker
  - Span table: trace_id, service, operation, duration, status, started_at
  - Click row → expand trace waterfall (Gantt chart of parent→child spans)
  - Span detail panel: attributes, events, links, resource attributes
  - Pagination: limit/offset

DB: `traces` hypertable, indexed on (org_id, started_at DESC), (trace_id),
    (service_name, started_at DESC)
```

---

### PROMPT OT-03: Discovered Services (Resource Discovery)

```
GOAL: Build the Discovered Services page (/otel-resources) showing all
applications and services that have sent OTel data.

BACKEND (Rust/axum):
  Handler: GET /api/otel/resources?window={1h|24h|7d}
  - Query traces for distinct resource attribute combinations:
    SELECT DISTINCT ON (service_name)
      resource_attributes->>'service.name' AS service_name,
      resource_attributes->>'host.name' AS host,
      resource_attributes->>'telemetry.sdk.language' AS sdk_language,
      resource_attributes->>'telemetry.sdk.version' AS sdk_version,
      resource_attributes->>'cloud.provider' AS cloud_provider,
      resource_attributes->>'cloud.region' AS cloud_region,
      resource_attributes->>'k8s.namespace.name' AS k8s_namespace,
      COUNT(*) AS span_count,
      COUNT(*) FILTER (WHERE status = 'error') AS error_count,
      MAX(started_at) AS last_seen
    FROM traces WHERE org_id = $1 AND started_at > NOW() - interval
    GROUP BY 1,2,3,4,5,6,7
    ORDER BY service_name, span_count DESC

FRONTEND (React/TypeScript):
  Page: OTelResources.tsx
  - Time window selector
  - Grid of service cards showing: name, host, SDK, span count, error rate
  - Active/stale indicator (green dot vs amber warning if >5min since last seen)
  - Cloud/K8s metadata badges where available
  - Click card → /traces?service={name}
  - Auto-refresh every 30 seconds

DB: Uses existing `traces` hypertable (no new migrations)
```

---

### PROMPT OT-04: Collector Setup & Direct Ingest Guide

```
GOAL: Build the Collector Setup page (/otel-config) that explains how to send
telemetry to Zentra — primarily direct, optionally via Collector.

BACKEND (Rust/axum):
  Handler: GET /api/otel/collector-config?org_id=X&traces=true&metrics=true&...
  - Generate a valid OTel Collector YAML configuration
  - Receivers: otlp (gRPC + HTTP)
  - Exporters: otlphttp/zentra with proper endpoint, auth headers
  - Pipelines: one per enabled signal
  - Return as text/yaml

  Handler: GET /api/otel/status
  - Return JSON with OTLP version, supported signals, auth methods,
    transports (HTTP + gRPC), encoding options, rate limits

FRONTEND (React/TypeScript):
  Page: OTelCollectorConfig.tsx
  - **Primary section** (green highlight): Direct-to-Zentra env vars
    - HTTP: OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_EXPORTER_OTLP_PROTOCOL=http/json
    - gRPC: OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_EXPORTER_OTLP_PROTOCOL=grpc
    - "No Collector, no Docker, no extra infrastructure"
  - **Divider**: "Optional: OTel Collector for advanced routing"
  - **Config generator**: host, port, org_id, API key, signal toggles
  - **YAML preview** with copy + download buttons
  - **Quick Start**: binary, Docker (optional), Kubernetes install instructions
  - **Endpoints reference**: all 4 endpoints, auth, encoding, rate limits

DB: None (stateless page, config generated dynamically)
```

---

### PROMPT OT-05: SDK Instrumentation Guide

```
GOAL: Build the SDK Guide page (/otel-sdk-guide) with per-language setup
instructions for instrumenting applications.

BACKEND (Rust/axum):
  Handler: GET /api/otel/sdk-snippets?language={all|java|python|go|nodejs|dotnet|rust}
  - Return JSON array of language objects, each containing:
    - language, display_name
    - auto_instrument: { title, description, install, run, env_vars }
    - frameworks: string[] of supported libraries
  - Host placeholder: <collector> (replaced by frontend with user's input)

FRONTEND (React/TypeScript):
  Page: OTelSdkGuide.tsx
  - Left sidebar: language selector (Java, Python, Go, Node.js, .NET, Rust)
  - Host input field (defaults to window.location.hostname)
  - Per language:
    - Header with display name and auto-instrument title
    - Supported frameworks as badges
    - Install section with copyable code block
    - Configure & Run section with copyable code block
    - Environment variables table with copy buttons
  - Quick links to Collector Config, Discovered Resources, Telemetry Health

DB: None (stateless, snippets are hardcoded in handler)
```

---

### PROMPT OT-06: Pipeline Topology Visualization

```
GOAL: Build the Pipeline Topology page (/otel-pipeline) that visualizes the
telemetry data flow from apps through collectors to Zentra.

BACKEND (Rust/axum):
  Handler: GET /api/otel/pipeline
  - Query traces (last 24h) for distinct service instances:
    SELECT DISTINCT resource_attributes->>'host.name',
           resource_attributes->>'service.name',
           resource_attributes->>'service.instance.id',
           MAX(started_at) AS last_seen,
           COUNT(*) AS forwarded_spans
    FROM traces WHERE org_id = $1 AND started_at > NOW() - '24h'
    GROUP BY 1, 2, 3 ORDER BY forwarded_spans DESC LIMIT 50
  - Classify: services matching *otelcol* or *collector* → collector nodes
  - All others → application nodes
  - Return: { sources, collectors, destination, edges, totals }

FRONTEND (React/TypeScript):
  Page: OTelPipeline.tsx
  - 3 summary cards: Applications count, Collectors count, Backend status
  - 3-column flow diagram:
    - Applications (blue cards) → arrow → Collectors (amber cards) → arrow → Zentra (green)
  - Per node: service name, host, span count, last seen, active/stale dot
  - Empty state: prompt to use Collector Config or SDK Guide
  - "How Pipeline Detection Works" explanation section
  - Auto-refresh every 30 seconds

DB: Uses existing `traces` hypertable (no new migrations)
```

---

### PROMPT OT-07: Span-to-Metrics RED Dashboard

```
GOAL: Build the Span Metrics page (/otel-span-metrics) that derives Rate,
Error, and Duration metrics from trace spans automatically.

BACKEND (Rust/axum):
  Handler: GET /api/otel/span-metrics?window={5m|15m|1h|6h|24h}&group_by={service|method|db}
  - Primary source: span_red_metrics_1m materialized view
  - Fallback: direct aggregation on traces hypertable
  - Per group:
    - Rate: COUNT(*), COUNT(*)/seconds AS requests_per_second
    - Error: COUNT(*) FILTER (WHERE status='error'), error_rate %
    - Duration: AVG, p50, p95, p99, MIN, MAX of duration_ms
  - Return: { metrics: [...], window, group_by, source, total }

  Migration 063: CREATE MATERIALIZED VIEW span_red_metrics_1m
  - Aggregates per-minute by org_id, service_name, http_method, http_status_code, db_system
  - Refresh: REFRESH MATERIALIZED VIEW span_red_metrics_1m (cron or manual)

FRONTEND (React/TypeScript):
  Page: OTelSpanMetrics.tsx
  - Controls: window selector (5m-24h), group-by selector (service/method/db)
  - Per service, a RED card with 3 columns:
    - Rate: total requests + req/s
    - Error: error rate % (color-coded) + total errors
    - Duration: horizontal latency bars for p50, p95, p99, avg
  - Click service name → /traces?service={name}
  - Source indicator (materialized view or real-time)
  - Auto-refresh every 15 seconds

DB: `span_red_metrics_1m` materialized view + `traces` hypertable
```

---

### PROMPT OT-08: OTel-Native Signal Alerts

```
GOAL: Build the Signal Alerts page (/otel-alert-rules) for defining and
evaluating alert conditions on OTel data.

BACKEND (Rust/axum):
  Handler: GET /api/otel/alert-rules → list all rules for org
  Handler: POST /api/otel/alert-rules → create new rule
  Handler: GET /api/otel/alert-rules/evaluate → evaluate all rules against live data

  Rule evaluation logic:
  - For each rule, query the appropriate signal table for the window
  - Compare computed value (error_rate, p99_latency, throughput) against threshold
  - Return { evaluations: [...], firing_count }

  Built-in defaults (returned when no custom rules exist):
  - High Error Rate: error_rate > 5% in 5m (warning)
  - Very High Error Rate: error_rate > 15% in 5m (critical)
  - High P99 Latency: p99_ms > 5000 in 5m (warning)
  - Low Throughput: requests < 1 in 15m (info)

  Migration 064: CREATE TABLE otel_alert_rules

FRONTEND (React/TypeScript):
  Page: OTelAlertRules.tsx
  - Action bar: "New Rule" button, "Evaluate Now" button
  - Create form: name, signal (traces/metrics/logs), severity, threshold, window
  - Rules table: name, signal, condition, threshold, window, severity
  - Evaluation results: color-coded FIRING/OK list with current vs threshold
  - "How OTel-Native Alerting Works" explanation section

DB: `otel_alert_rules` table + queries against `traces`/`metrics`/`logs` hypertables
```

---

### PROMPT OT-09: GenAI / LLM Observability

```
GOAL: Build the GenAI Observability page (/otel-genai) for monitoring LLM API
calls using OTel GenAI semantic conventions.

BACKEND (Rust/axum):
  Handler: GET /api/otel/genai?window={1h|6h|24h|7d}
  - Query traces where gen_ai_system IS NOT NULL:
    - Model summary: GROUP BY gen_ai_system, gen_ai.request.model
      - call_count, error_count, error_rate
      - avg/p95 latency
      - avg/sum input/output/total tokens
    - Recent calls: individual spans with all GenAI attributes
    - Summary: total_calls, total_tokens, unique_models
  - Extract token counts from attributes JSONB:
    - attributes->>'gen_ai.usage.input_tokens'
    - attributes->>'gen_ai.usage.output_tokens'
    - attributes->>'gen_ai.usage.total_tokens'
  - Return: { models, recent_calls, summary, window, semconv_attributes }

FRONTEND (React/TypeScript):
  Page: OTelGenAI.tsx
  - Time window selector
  - 3 summary cards: Total Calls, Total Tokens, Unique Models
  - Model Performance section: per-model cards with error rate, latency, token usage
  - Recent Calls table: trace_id, operation, service, model, duration, status, tokens
  - Click trace_id → /traces?trace_id={id}
  - Semconv attributes reference section
  - Auto-refresh every 30 seconds

DB: Uses `traces` hypertable with gen_ai_system indexed column
Semconv: gen_ai.system, gen_ai.request.model, gen_ai.usage.* (defined in semconv.rs)
```

---

### PROMPT OT-10: OTLP Security (API Keys + Audit)

```
GOAL: Build the OTLP Security page (/otel-security) for managing API keys,
rate limits, and viewing ingest audit logs.

BACKEND (Rust/axum):
  Handlers in handlers/otlp_security.rs:

  GET /api/otel/api-keys → list keys for org (never returns raw key)
  POST /api/otel/api-keys → create key
    - Input: { name, permissions, rate_limit_rpm, max_payload_bytes, expires_in_days }
    - Generate: raw_key = "isok_" + 32 random alphanumeric chars
    - Store: SHA-256 hash of raw_key, key_prefix = first 8 chars
    - Return: { raw_key (ONCE), key: { id, name, permissions, ... } }
  DELETE /api/otel/api-keys/:id → revoke (set enabled = false)

  GET /api/otel/ingest-audit?window=1h&limit=50 → audit log
    - Query otlp_ingest_audit hypertable
    - Return: { entries, total, window }

  Key validation (called from ingest handlers):
  validate_otlp_key(pool, raw_key) → OtlpKeyInfo { org_id, key_id, permissions, rate_limit, max_payload }
    - Hash the raw key, lookup in otlp_api_keys WHERE key_hash = hash AND enabled = true
    - Check expiry, return permissions

  Migration 065: CREATE TABLE otlp_api_keys + otlp_ingest_audit

FRONTEND (React/TypeScript):
  Page: OTelSecurity.tsx
  - 3 summary cards: Active Keys, Audit Entries (1h), Auth Methods
  - Created key banner: amber warning with raw key + copy button (shown once)
  - Create form: name, permissions checkboxes, rate limit, max payload, expiry
  - Keys table: prefix, name, permissions, rate limit, last used, created, revoke button
  - Audit log table: signal, endpoint, status code, span count, payload size, IP, timestamp
  - Auto-refresh audit every 15 seconds

DB: `otlp_api_keys` + `otlp_ingest_audit` (hypertable)
```

---

### PROMPT OT-11: Semantic Convention Catalog & Attribute Discovery

```
GOAL: Build the backend APIs for browsing OTel semantic conventions and
discovering attributes observed in actual telemetry data.

BACKEND (Rust/axum):
  Module: semconv.rs — 40+ well-known attribute definitions across 10 domains

  Handler: GET /api/otel/semconv → full catalog
  - Return all SemconvAttribute entries: { key, domain, description, value_type }
  - Domains: service, http, database, rpc, messaging, cloud, k8s, host, gen_ai, telemetry

  Handler: GET /api/otel/attributes?signal=traces&window=1h
  - Query traces for actually-observed attribute keys:
    SELECT DISTINCT jsonb_object_keys(attributes) AS key FROM traces
    WHERE org_id = $1 AND started_at > NOW() - interval
  - Cross-reference with semconv catalog for known descriptions
  - Return: { attributes: [{ key, domain, description, count }] }

  Handler: GET /api/otel/attribute-values?key=http.method&window=1h
  - Query distinct values for a specific attribute key
  - Return: { key, values: ["GET", "POST", ...], total }

FRONTEND: Currently consumed by other OTel pages for autocomplete and filter dropdowns.
Could be exposed as a dedicated "Attribute Explorer" page in the future.

DB: Uses `traces` hypertable JSONB attributes column
```

---

### PROMPT OT-12: Protobuf Encoding Support

```
GOAL: Accept application/x-protobuf OTLP payloads alongside JSON, so the
default OTel Collector/SDK configuration (protobuf) works out of the box.

BACKEND (Rust/axum):
  Module: otlp_proto.rs — hand-written prost-annotated structs matching OTLP v1.3.0 proto:
  - ExportTraceServiceRequest, ResourceSpans, ScopeSpans, Span, ...
  - ExportMetricsServiceRequest, ResourceMetrics, ScopeMetrics, Metric, ...
  - ExportLogsServiceRequest, ResourceLogs, ScopeLogs, LogRecord, ...
  - ExportProfilesServiceRequest, ResourceProfiles, ScopeProfiles, ...
  - Common: Resource, KeyValue, AnyValue, InstrumentationScope

  Conversion functions:
  - proto_to_json_traces(bytes) → serde_json::Value
  - proto_to_json_metrics(bytes) → serde_json::Value
  - proto_to_json_logs(bytes) → serde_json::Value
  - proto_to_json_profiles(bytes) → serde_json::Value

  Integration in handlers/otlp.rs:
  - detect_encoding() checks Content-Type header
  - decode_otlp_body() dispatches JSON vs Protobuf
  - Protobuf bodies are decoded via prost then converted to JSON
  - Rest of handler processes identically regardless of encoding

  Feature gate: `otlp-proto` in Cargo.toml
  Dependencies: prost = "0.13", prost-types = "0.13" (workspace)

DB: No changes — same tables, protobuf just decodes to the same JSON structure
```

---

### PROMPT OT-13: gRPC Transport

```
GOAL: Accept OTLP over gRPC on port 4317 (the OTel Collector default) alongside
the HTTP server, so default SDK configurations work without changing protocol.

BACKEND (Rust/axum):
  Module: grpc.rs — tonic-based gRPC service
  - OtlpGrpcService struct with AppState
  - export_traces(Request<ExportTraceServiceRequest>) → Response<ExportTraceServiceResponse>
  - export_metrics, export_logs, export_profiles (same pattern)
  - Auth: resolve org_id from gRPC metadata (authorization, x-zentra-org-id)
  - Decode: prost Message::decode → proto_to_json_*() → shared ingest logic

  Server startup in main.rs:
  - Feature-gated: #[cfg(feature = "grpc")]
  - Spawned via tokio::spawn alongside HTTP server
  - Port: ZENTRA_GRPC_PORT env var or default 4317
  - tonic::transport::Server with all 4 service implementations

  Feature gate: `grpc` in Cargo.toml (implies `otlp-proto`)
  Dependencies: tonic = { version = "0.12", features = ["default"] } (workspace)

DB: No changes — gRPC handlers reuse the same insert logic as HTTP
```

---

## End of Document

This document covers the complete OpenTelemetry module of Zentra:
- **10 UI pages** with frontend components and API endpoints
- **13 workable prompts** for implementing each feature
- **Full database schema** for all OTel tables
- **Architecture diagram** showing the ingest pipeline
- **Semantic convention catalog** with 40+ attributes across 10 domains
