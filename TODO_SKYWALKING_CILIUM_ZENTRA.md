# Zentra — SkyWalking + Cilium Feature Build TODO

**Source Analysis:** Apache SkyWalking v10.4 architecture + Cilium v1.19 docs  
**Date:** 2026-05-25  
**Reference Prompts:** `docs/PROMPTS_SKYWALKING_CILIUM.md`  
**Objective:** Close the gap between Zentra and world-class APM/observability (SkyWalking v10 level)

---

## How to Use This File

- Work **top to bottom** — items are ordered by dependency and impact
- Mark status as: `[ ]` pending → `[~]` in progress → `[x]` complete
- Each item references the detailed prompt in `PROMPTS_SKYWALKING_CILIUM.md`
- **Do NOT skip items** — later items depend on earlier ones (especially #1 and #3)
- Run validation commands at the end of each item before moving to the next

---

## Progress Summary

| # | Feature | Status | Effort | Prompt |
|---|---|---|---|---|
| 1 | Entity-Scoped Dashboard Model | `[ ]` | 1–2 weeks | PROMPT-01 |
| 2 | Cilium Monitoring Dashboard | `[ ]` | 3–5 days | PROMPT-02 |
| 3 | MQE-Lite Metrics Query DSL | `[ ]` | 2–3 weeks | PROMPT-03 |
| 4 | Apdex Scoring on Services | `[ ]` | 2–3 days | PROMPT-04 |
| 5 | Topology Drill-through + Service Hierarchy | `[ ]` | 1 week | PROMPT-05 |
| 6 | Hubble gRPC Flow Collector | `[ ]` | 1–2 weeks | PROMPT-06 |
| 7 | Trace ↔ DB Metric Cross-Correlation | `[ ]` | 1–2 weeks | PROMPT-07 |
| 8 | LLM Dashboard from OTLP GenAI Spans | `[ ]` | 1–2 weeks | PROMPT-08 |
| 9 | Continuous eBPF Profiling | `[ ]` | 3–4 weeks | PROMPT-09 |
| 10 | TraceQL Query Language Support | `[ ]` | 2–3 weeks | PROMPT-10 |

---

## TIER 1 — Highest Impact (Do These First)

---

### [ ] ITEM 1 — Entity-Scoped Dashboard Model
**Prompt:** PROMPT-01  
**Effort:** 1–2 weeks  
**Why first:** Every other dashboard improvement builds on top of this. Without entity scope, dashboards remain disconnected from the monitoring objects they describe.

**Backend tasks:**
- [ ] Add migration `20250101000055_entity_dashboard_scope.sql` — add `entity_type`, `entity_scope`, `entity_id` columns to `dashboards` table
- [ ] Add `dashboard_templates` table with pre-shipped templates per entity type
- [ ] Update `handlers/dashboards.rs` — add `GET /api/dashboards/for-entity?type=server&id=:id` route
- [ ] Update `handlers/dashboards.rs` — add `GET /api/dashboard-templates` route
- [ ] Seed 6 built-in templates: `server`, `database`, `k8s_node`, `k8s_pod`, `network_device`, `cilium`
- [ ] Register new routes in `routes/` and `handlers/mod.rs`

**Frontend tasks:**
- [ ] Update `DashboardBuilder.tsx` — add entity type/scope selector to dashboard creation form
- [ ] Update `DashboardManager.tsx` — group dashboards by entity type in sidebar
- [ ] Create `src/components/EntityDashboardLink.tsx` — reusable component that navigates to entity dashboard
- [ ] Update `Servers.tsx` — add "Open Dashboard" button per server that links to its entity dashboard
- [ ] Update `PostgresMonitoring.tsx`, `OracleMonitoring.tsx`, `MysqlMonitoring.tsx` — same entity dashboard link
- [ ] Update `Network.tsx` — same entity dashboard link per device

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Create a dashboard, assign entity type = "server", verify it appears scoped correctly
- [ ] Navigate from Servers page → click "Open Dashboard" → correct dashboard loads pre-filtered

---

### [ ] ITEM 2 — Cilium Monitoring Dashboard Page
**Prompt:** PROMPT-02  
**Effort:** 3–5 days  
**Why second:** Fastest win. Entire backend (scraper) already works. Pure frontend + scrape target registration.

**Backend tasks:**
- [ ] Add `GET /api/cilium/status` handler in new `handlers/cilium.rs` — checks Cilium agent scrape targets
- [ ] Add `GET /api/cilium/metrics/summary` — returns key Cilium metrics aggregated from `scrape_results`
- [ ] Add `POST /api/cilium/register` — one-click register standard Cilium scrape targets (agent:9962, hubble-relay:9965, operator:9963)
- [ ] Register routes in `routes/` and `handlers/mod.rs`

**Frontend tasks:**
- [ ] Create `src/pages/CiliumMonitoring.tsx` — full monitoring page with panels:
  - Endpoint health distribution (gauge + state grid)
  - Policy drop reasons (bar chart from `cilium_drop_count_total`)
  - Hubble flow verdict breakdown (donut: forwarded/dropped/redirected)
  - BPF map pressure heatmap (per-map utilization)
  - Policy import errors timeline
  - Hubble L7 flows table (HTTP, DNS, Kafka)
  - Connected nodes count + identity count
- [ ] Add route `/cilium` in `App.tsx`
- [ ] Add nav item under "Infrastructure" section in `Layout.tsx`

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Page loads at `/cilium`
- [ ] Register Cilium scrape targets → metrics appear in panels

---

### [ ] ITEM 3 — MQE-Lite Metrics Query DSL
**Prompt:** PROMPT-03  
**Effort:** 2–3 weeks  
**Why third:** Transforms dashboard builder from "pick a metric name" to "write an expression". This is the core capability gap vs SkyWalking/Grafana.

**Backend tasks:**
- [ ] Create `crates/Zentra-query/src/mqe/` module:
  - [ ] `lexer.rs` — tokenize MQE expressions
  - [ ] `parser.rs` — parse to AST: metric selectors, aggregations, math ops, time windows
  - [ ] `evaluator.rs` — evaluate AST against TimescaleDB `metrics` table
  - [ ] `functions.rs` — implement: `avg()`, `sum()`, `max()`, `min()`, `rate()`, `increase()`, `delta()`
  - [ ] `label_selector.rs` — `{key="value", key2=~"regex"}` label filtering
- [ ] Add `POST /api/query/mqe` handler in `handlers/metrics.rs` — accepts expression, time range, step, returns series
- [ ] Add `GET /api/query/mqe/validate` — validates expression syntax, returns AST or errors
- [ ] Add `GET /api/query/metrics/names` — returns all known metric names (for autocomplete)
- [ ] Add `GET /api/query/metrics/labels?metric=X` — returns known label keys/values for a metric

**Frontend tasks:**
- [ ] Create `src/components/MQEEditor.tsx` — code editor with:
  - Syntax highlighting (metric names, functions, operators in different colors)
  - Autocomplete for metric names and label values
  - Live syntax validation (debounced, calls `/api/query/mqe/validate`)
  - Error display inline
- [ ] Update `DashboardBuilder.tsx` — replace plain text `query` input with `MQEEditor` component
- [ ] Update `QueryExplorer.tsx` — add MQE query mode tab alongside existing SQL mode
- [ ] Update panel preview in `DashboardBuilder.tsx` — execute real MQE query and show live data

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Expression `avg(cpu_usage_percent{host="web-01"})` returns correct series
- [ ] Expression `rate(network_bytes_received[5m])` returns rate-of-change series
- [ ] Invalid expression returns structured error with position info
- [ ] Dashboard builder panel with MQE query shows live chart

---

### [ ] ITEM 4 — Apdex Scoring on Services
**Prompt:** PROMPT-04  
**Effort:** 2–3 days  
**Why fourth:** Small effort, high signal value. Makes service health instantly readable as a 0–1 score.

**Backend tasks:**
- [ ] Add migration `20250101000056_apdex_config.sql` — `apdex_config` table with `service_name`, `satisfied_ms`, `tolerating_ms`, `org_id`
- [ ] Add `GET /api/apdex` — returns Apdex scores for all services in a time window
- [ ] Add `GET /api/apdex/:service` — returns Apdex score + breakdown for a specific service
- [ ] Add `PUT /api/apdex/config` — update Apdex thresholds per service
- [ ] Implement Apdex SQL: `(COUNT(*) FILTER(WHERE duration_ms <= satisfied) + 0.5 * COUNT(*) FILTER(WHERE duration_ms > satisfied AND duration_ms <= tolerating)) / COUNT(*)`
- [ ] Expose Apdex on `GET /api/service-map` — add `apdex` field to `ServiceNode` response

**Frontend tasks:**
- [ ] Create `src/components/ApdexGauge.tsx` — compact gauge component (color: green ≥0.94, yellow ≥0.85, orange ≥0.70, red <0.70)
- [ ] Update `service_map.rs` frontend rendering in `Topology.tsx` — show Apdex score on service nodes
- [ ] Add Apdex column to service table views in `Traces.tsx`
- [ ] Add Apdex configuration panel in `SystemSettings` or as a sub-section of `AlertRules.tsx`

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] `GET /api/apdex` returns scores per service
- [ ] Topology shows Apdex gauge on service nodes
- [ ] Apdex updates when trace data changes

---

### [ ] ITEM 5 — Topology Drill-through + Service Hierarchy
**Prompt:** PROMPT-05  
**Effort:** 1 week  
**Why fifth:** Connects the topology map (the "overview") to all the deep monitoring pages. Completes the navigation model.

**Backend tasks:**
- [ ] Add migration `20250101000057_target_hierarchy.sql` — add `parent_target_id UUID REFERENCES targets(target_id)` column
- [ ] Update `handlers/targets.rs` — include `parent_target_id` in GET/POST/PATCH target responses
- [ ] Add `GET /api/targets/:id/hierarchy` — returns full ancestor chain (target → node → cluster)
- [ ] Add `GET /api/targets/:id/children` — returns direct children of a target
- [ ] Update `handlers/network.rs` topology endpoint — include hierarchy relationships in topology graph edges

**Frontend tasks:**
- [ ] Update `Topology.tsx` — make every node clickable, routing to the correct monitoring page:
  - `type=server` → `/servers?target_id=:id`
  - `type=database` → `/databases?target_id=:id`
  - `type=k8s_node` → `/middleware/kubernetes?node=:name`
  - `type=network_device` → `/network?device=:id`
  - `type=cilium` → `/cilium`
- [ ] Create `src/components/HierarchyBreadcrumb.tsx` — shows `Cluster → Node → Service` path, each level clickable
- [ ] Add `HierarchyBreadcrumb` to: `Servers.tsx`, `PostgresMonitoring.tsx`, `OracleMonitoring.tsx`, `KubernetesMonitoring.tsx`
- [ ] Update `Topology.tsx` map view — draw parent/child hierarchy edges in a distinct style (dashed line, lighter color)
- [ ] Add "View in Topology" button on every individual monitoring page that focuses the topology map on that target

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Click a server node in Topology → navigates to Servers page filtered to that server
- [ ] Breadcrumb shows correct hierarchy path
- [ ] `GET /api/targets/:id/hierarchy` returns correct ancestor chain

---

## TIER 2 — Medium Effort, High Value

---

### [ ] ITEM 6 — Hubble gRPC Flow Collector
**Prompt:** PROMPT-06  
**Effort:** 1–2 weeks  
**Depends on:** Items 1, 2 (entity model and Cilium page must exist)

**Backend tasks:**
- [ ] Add `hubble` feature to `crates/Zentra-agent/Cargo.toml` — add `tonic` + generated proto client for `observer.proto`
- [ ] Create `crates/Zentra-agent/src/collectors/hubble.rs` — gRPC streaming client:
  - Connect to Hubble relay at configurable address (default `hubble-relay:4245`)
  - Subscribe to `ObserverService.Observe` with `GetFlowsRequest { follow: true }`
  - Map `Flow` proto → `netflow_records` row (`flow_version = 100` for Hubble-sourced flows)
  - Extract L7 metadata: HTTP method/URL/status, DNS query/response, Kafka topic/operation
  - Reconnect with exponential backoff on disconnect
- [ ] Add migration `20250101000058_hubble_l7_flows.sql` — `hubble_l7_flows` table for L7-enriched records
- [ ] Update `handlers/netflow.rs` — add `?source=hubble` filter to existing flow endpoints
- [ ] Add `GET /api/hubble/status` — returns Hubble relay connectivity status + flow ingestion rate
- [ ] Update `handlers/service_map.rs` — when Hubble flows exist, enrich service edges with L7 verdict data

**Frontend tasks:**
- [ ] Update `CiliumMonitoring.tsx` — add Hubble connectivity status card + flows-per-second counter
- [ ] Update `Network.tsx` — add "Hubble Flows" tab showing L7 flow table (source pod, dest pod, L7 detail, verdict)
- [ ] Update `service_map.rs`-backed topology edges — show L7 protocol label on edges when Hubble data available

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] Hubble collector connects (or gracefully reports disconnected if no Hubble available)
- [ ] Flows appear in `netflow_records` with `flow_version = 100`
- [ ] `/api/hubble/status` returns connection state

---

### [ ] ITEM 7 — Trace ↔ DB Metric Cross-Correlation
**Prompt:** PROMPT-07  
**Effort:** 1–2 weeks  
**Depends on:** Item 4 (Apdex) and existing trace + DB monitoring infrastructure

**Backend tasks:**
- [ ] Create `crates/Zentra-drilldown/src/trace_db_correlator.rs`:
  - Query `traces` for spans with `db.system`, `db.name`, `db.statement`, `db.operation` tags
  - For each DB-touching trace span: fetch concurrent DB metrics from `metrics` table (active connections, QPS, lock waits) at the same timestamp ±30s
  - Return correlated view: slow trace span + DB state at time of slowness
- [ ] Add `GET /api/drilldown/trace/:trace_id/db-context` — returns DB metric context for all DB spans in a trace
- [ ] Add `GET /api/drilldown/db-slowness?target_id=:id&from=:t&to=:t` — returns traces that had slow DB calls against a specific DB target
- [ ] Add `GET /api/correlation/service-to-db` — returns which services call which databases, with p95 latency

**Frontend tasks:**
- [ ] Update `Traces.tsx` — when expanding a trace, show DB context panel for any DB span:
  - Concurrent connections at time of span
  - Active queries count
  - Lock wait count
  - "View DB Dashboard" link
- [ ] Create `src/components/DBContextPanel.tsx` — reusable panel for DB metrics at a point in time
- [ ] Update `PostgresMonitoring.tsx`, `OracleMonitoring.tsx`, `MysqlMonitoring.tsx` — add "Slow Traces" tab showing traces that had slow calls against this DB
- [ ] Update `Topology.tsx` — draw service→database edges with p95 latency label from correlation data

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] `GET /api/drilldown/trace/:id/db-context` returns DB metrics for a trace with DB spans
- [ ] DB monitoring pages show "Slow Traces" tab with relevant traces

---

### [ ] ITEM 8 — LLM Dashboard from OTLP GenAI Spans
**Prompt:** PROMPT-08  
**Effort:** 1–2 weeks  
**Depends on:** Existing `handlers/otlp.rs` and `LLMObservability.tsx`

**Backend tasks:**
- [ ] Update `handlers/otlp.rs` — extract GenAI semantic convention attributes from OTLP spans:
  - `gen_ai.system` (openai / anthropic / google / etc.)
  - `gen_ai.request.model`
  - `gen_ai.usage.input_tokens` + `gen_ai.usage.output_tokens`
  - `gen_ai.usage.total_tokens`
  - `gen_ai.response.finish_reasons`
  - `llm.request.cost` (if present)
- [ ] Add migration `20250101000059_llm_spans.sql` — `llm_spans` table derived from OTLP ingestion
- [ ] Add `GET /api/llm/overview-from-traces` — real data from `llm_spans` (total calls, tokens, cost, p95 latency)
- [ ] Add `GET /api/llm/models-from-traces` — per-model breakdown from `llm_spans`
- [ ] Add `GET /api/llm/cost-trend` — hourly cost time series from `llm_spans`
- [ ] Add `GET /api/llm/latency-distribution` — histogram buckets from trace durations

**Frontend tasks:**
- [ ] Update `LLMObservability.tsx` — wire all panels to the new `*-from-traces` endpoints (replace stubs)
- [ ] Add model comparison table: model name, avg latency, p95 latency, tokens/request, cost/request, error rate
- [ ] Add "Token Efficiency" chart: input vs output token ratio per model
- [ ] Add streaming vs non-streaming breakdown (from `gen_ai.response.finish_reasons`)
- [ ] Add prompt/completion cost breakdown donut chart

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Send an OTLP span with `gen_ai.*` attributes → appears in `llm_spans` table
- [ ] `LLMObservability.tsx` panels show real data from `llm_spans`

---

## TIER 3 — Enterprise Completeness

---

### [ ] ITEM 9 — Continuous eBPF Profiling
**Prompt:** PROMPT-09  
**Effort:** 3–4 weeks  
**Depends on:** Items 1 and 5 (entity model + hierarchy for scoping profiles to targets)  
**Platform note:** Linux only. Requires kernel headers on monitored hosts. Agentless via SSH exec of `bpftrace`/`perf` commands.

**Backend tasks:**
- [ ] Add `libbpf-rs` or use SSH exec approach (agentless) — run `perf record` + `perf script` remotely via SSH
- [ ] Create `crates/Zentra-agent/src/collectors/ebpf_profiler.rs`:
  - SSH exec `perf record -g -F 99 -p $(pgrep $process) -- sleep $duration` on target host
  - Retrieve output, parse perf script output into stack frames
  - Aggregate into flamegraph-compatible collapsed format
- [ ] Add migration `20250101000060_profiles.sql` — `profiles` table: `target_id`, `timestamp`, `profile_type` (cpu/memory/io), `duration_s`, `flamegraph_data JSONB`
- [ ] Add `POST /api/profiles/trigger` — triggers an on-demand profile for a target
- [ ] Add `GET /api/profiles?target_id=:id` — lists profiles for a target
- [ ] Add `GET /api/profiles/:id` — returns flamegraph JSON for rendering

**Frontend tasks:**
- [ ] Create `src/components/charts/FlameGraph.tsx` — interactive flamegraph renderer (d3-flame-graph compatible JSON)
- [ ] Create `src/pages/Profiling.tsx` — profiling management page:
  - List of profiling jobs per target
  - Trigger on-demand profile button
  - Flamegraph viewer (full-page, zoomable)
  - Time comparison: overlay two flamegraphs to find regressions
- [ ] Add route `/profiling` in `App.tsx`
- [ ] Add nav item under "Performance" in `Layout.tsx`
- [ ] Update `Servers.tsx` — add "Profile" button per server that triggers profiling job

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] Profiling job triggers and stores results in `profiles` table
- [ ] FlameGraph component renders a sample flamegraph without errors

---

### [ ] ITEM 10 — TraceQL Query Language Support
**Prompt:** PROMPT-10  
**Effort:** 2–3 weeks  
**Depends on:** Item 3 (MQE DSL infrastructure can be reused as pattern reference)

**Backend tasks:**
- [ ] Create `crates/Zentra-query/src/traceql/` module:
  - [ ] `lexer.rs` — tokenize TraceQL: `{`, `}`, `&&`, `||`, attribute selectors, comparison ops, duration literals
  - [ ] `parser.rs` — parse to AST: span selectors, pipeline stages
  - [ ] `translator.rs` — translate TraceQL AST to SQL `WHERE` clauses against `traces` table
  - Supported: `{service.name="X"}`, `{duration > 1s}`, `{status = error}`, `{span.db.statement =~ "SELECT.*"}`, `{service.name="X" && duration > 500ms}`
- [ ] Add `POST /api/query/traceql` — accepts TraceQL expression + time range, returns matching traces
- [ ] Add `GET /api/query/traceql/validate` — validates expression, returns parse result or errors
- [ ] Expose Grafana Tempo-compatible HTTP API endpoints:
  - `GET /tempo/api/search?q={...}` → Grafana can use Zentra as a Tempo data source
  - `GET /tempo/api/traces/:traceID`

**Frontend tasks:**
- [ ] Update `Traces.tsx` — add TraceQL search bar (replaces simple service/operation dropdowns)
  - Syntax highlighting in the search bar
  - Autocomplete for attribute names (`service.name`, `span.db.system`, `duration`, `status`)
  - Example query buttons: `{status=error}`, `{duration > 1s}`, `{service.name="order-service"}`
- [ ] Update `QueryExplorer.tsx` — add TraceQL tab alongside MQE and SQL tabs
- [ ] Show parse errors inline with position highlighting

**Validation:**
- [ ] `cargo check` — 0 errors
- [ ] `npx tsc --noEmit` — 0 errors
- [ ] `POST /api/query/traceql` with `{status=error}` returns error traces
- [ ] `POST /api/query/traceql` with `{duration > 1s}` returns slow traces
- [ ] Grafana Tempo data source pointing at Zentra returns traces

---

## Global Rules for All Items

These apply to every single item. Treat as non-negotiable:

1. **No duplicates** — before creating any handler, page, or component, verify it doesn't already exist
2. **Enhance, don't replace** — modify existing files rather than creating parallel ones
3. **Migration numbering** — continue from `20250101000055_*` upward, one migration per item
4. **All routes registered** — new handlers in `routes/`, exported in `handlers/mod.rs`
5. **All pages routed** — new pages added to `App.tsx`, nav items added to `Layout.tsx`
6. **No mock data in live mode** — all new endpoints must return real structured data or empty arrays
7. **Validation gate** — `cargo check` + `npx tsc --noEmit` must pass with 0 errors before moving to next item
8. **No third-party names** — no "Grafana", "Prometheus", "SkyWalking", "Cilium" in UI labels or user-facing text
9. **Agentless only** — all data collection via protocols (SSH exec, SNMP, gRPC, HTTP scrape). No agent software installed on monitored hosts.
10. **Backwards compatible** — every DB migration uses `IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS`

---

*Last updated: 2026-05-25*  
*Next review: After ITEM 3 (MQE DSL) is complete — reassess ITEM 9 (eBPF) feasibility based on target environment*
