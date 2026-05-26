# Zentra — SkyWalking + Cilium Implementation Prompts

**Source Analysis:** Apache SkyWalking v10.4 + Cilium v1.19  
**Date:** 2026-05-25  
**Total Prompts:** 10  
**Track progress in:** `docs/TODO_SKYWALKING_CILIUM.md`

---

## GLOBAL RULES — MANDATORY FOR EVERY PROMPT

1. **Never duplicate code.** Verify with `grep`/`find` before creating anything. If it exists, enhance it.
2. **Enhance, don't replace.** Modify existing files rather than recreating them.
3. **Validation gate.** After every prompt: `cargo check` = 0 errors, `npx tsc --noEmit` = 0 errors.
4. **Complete 100%** before moving to the next prompt.
5. **No third-party names in UI.** No "Grafana", "SkyWalking", "Cilium", "Prometheus" in user-facing text.
6. **No mock data in live mode.** All endpoints return real structured data or empty arrays.
7. **Migration numbering.** Continue from `20250101000055_*`. Use `IF NOT EXISTS` on all DDL.
8. **Route registration.** New handlers → `routes/*.rs` + `handlers/mod.rs`. New pages → `App.tsx` + `Layout.tsx`.
9. **Agentless only.** All collection via SSH exec, SNMP, HTTP scrape, gRPC, REST APIs. No agents on monitored hosts.
10. **Backwards compatible.** Every migration uses `ADD COLUMN IF NOT EXISTS`.

---

## PROMPT 01 — Entity-Scoped Dashboard Model

**Inspired by:** SkyWalking v9/v10 entity-scoped dashboard system  
**Effort:** 1–2 weeks

### Context
SkyWalking's biggest UX insight: every dashboard belongs to an entity type (Server, Database, K8s Pod, Service). Clicking any topology node auto-loads the correct dashboard pre-filtered for that entity. Zentra dashboards are currently global — no entity binding.

### Pre-Flight
```
grep -r "entity_type" migrations/ --include="*.sql"
grep -r "EntityDashboardLink" frontend/src/components/
```

### Migration — `migrations/20250101000055_entity_dashboard_scope.sql`
```sql
ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS entity_type  TEXT;
ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS entity_scope TEXT;
-- entity_type values: 'server'|'database'|'k8s_node'|'k8s_pod'|'network_device'|'cilium'|'service'|'global'
CREATE INDEX IF NOT EXISTS idx_dashboards_entity ON dashboards(entity_type, entity_scope);

CREATE TABLE IF NOT EXISTS dashboard_templates (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    panels      JSONB NOT NULL DEFAULT '[]',
    is_builtin  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dashboard_templates_entity_name
    ON dashboard_templates(entity_type, name) WHERE is_builtin = TRUE;
```

### Backend Tasks
- **New File** `crates/Zentra-api/src/provisioning_templates.rs` — seed 6 builtin templates at startup: `server`, `database`, `k8s_node`, `k8s_pod`, `network_device`, `cilium`. Call from `main.rs` after pool init.
- **Modify** `handlers/dashboards.rs` — add 3 handlers:
  - `GET /api/dashboards/for-entity?entity_type=X&entity_scope=Y` — returns scoped dashboards; falls back to template
  - `GET /api/dashboard-templates` — lists all templates
  - `POST /api/dashboard-templates/:entity_type/instantiate` — creates dashboard from template for a specific entity

### Frontend Tasks
- **Modify** `DashboardBuilder.tsx` — add entity type dropdown + optional scope field; "Load from template" button
- **Modify** `DashboardManager.tsx` — group dashboard list by entity type with collapsible sections
- **New File** `frontend/src/components/EntityDashboardLink.tsx` — queries `/api/dashboards/for-entity`, navigates to dashboard or offers to create from template; renders as "Open Dashboard →" button
- **Modify** `Servers.tsx`, `PostgresMonitoring.tsx`, `OracleMonitoring.tsx`, `MysqlMonitoring.tsx`, `Network.tsx` — add `<EntityDashboardLink>` in each row/header

### Validation
```
GET /api/dashboard-templates → 6 builtin templates
GET /api/dashboards/for-entity?entity_type=server → server template
Dashboard builder → entity type dropdown present
Servers page → "Open Dashboard" link per row
```

---

## PROMPT 02 — Cilium / eBPF Network Monitoring Dashboard

**Inspired by:** SkyWalking v10 Cilium dashboard + Cilium Prometheus metrics  
**Effort:** 3–5 days

### Context
When Cilium is the K8s CNI, Zentra needs a dedicated page for eBPF policy health, endpoint states, drop reasons, BPF map pressure, and Hubble flow stats. All data is scraped from Cilium's native `/metrics` endpoints — the existing `Zentra-scrape` crate handles this already. This is frontend + scrape registration.

### Key Metrics
| Metric | Panel |
|---|---|
| `cilium_endpoint_state` by `state` | State grid |
| `cilium_drop_count_total` by `reason` | Bar chart |
| `cilium_bpf_map_pressure` by `map_name` | Heatmap |
| `hubble_flows_processed_total` by `verdict` | Donut |
| `cilium_policy_import_errors_total` | Stat + timeline |
| `cilium_identity_count` | Stat |
| `hubble_drop_total` by `reason`,`protocol` | Bar chart |

### Backend Tasks — New File `handlers/cilium.rs`
- `GET /api/cilium/status` — scrape target list + last scrape time
- `GET /api/cilium/metrics/summary` — aggregated key metrics from `scrape_results` (last 5m)
- `POST /api/cilium/register-scrape-targets` — body: `{agent_host, agent_port:9962, hubble_host, hubble_port:9965, operator_host, operator_port:9963}` — creates scrape targets

### Frontend Tasks
- **New File** `frontend/src/pages/CiliumMonitoring.tsx` — dark card grid with: endpoint state grid, flow verdict donut, drop reasons bar chart, BPF map pressure heatmap, policy error timeline, register-targets form when no targets configured
- Add route `/cilium` in `App.tsx`
- Add nav item "eBPF Network" under Infrastructure in `Layout.tsx`

### Validation
```
Navigate /cilium → page loads
POST /api/cilium/register-scrape-targets → targets appear in scrape_targets
After scrape: panels populate with cilium_* metric values
```

---

## PROMPT 03 — MQE-Lite Metrics Query DSL

**Inspired by:** SkyWalking MQE (Metrics Query Engine)  
**Effort:** 2–3 weeks

### Context
SkyWalking panels use expressions like `avg(cpu_usage_percent{host="web-01"})` or `rate(network_bytes_received[5m])`. Zentra dashboard panels currently accept only a raw metric name string — no aggregation, no label filtering, no math. This prompt adds an expression evaluator inside `Zentra-query`.

### Language (MQE-lite)
```
Expression = AggFunc '(' Selector Window? ')' (MathOp Expression)?
           | Selector

AggFunc    = avg | sum | max | min | count | rate | increase | delta
Selector   = MetricName ('{' LabelMatcher (',' LabelMatcher)* '}')?
LabelMatcher = LabelName ('='|'!='|'=~'|'!~') StringLiteral
Window     = '[' Duration ']'   -- e.g. [5m], [1h]
MathOp     = '+' | '-' | '*' | '/'

Examples:
  avg(cpu_usage_percent{host="web-01"})
  rate(network_bytes_received[5m])
  sum(http_errors{status="500"}) / sum(http_requests)
  max(disk_io_util{device!="sda"})
```

### Backend Tasks — New directory `crates/Zentra-query/src/mqe/`
- `lexer.rs` — tokenize identifiers, strings, operators, durations
- `parser.rs` — parse to AST; `ParseError` must include `position: usize` + `message: String`
- `evaluator.rs` — translate AST → SQL against `metrics` table using `time_bucket` + aggregations; `rate()` uses LAG window function
- `functions.rs` — SQL fragment generators for each AggFunc
- `mod.rs` — `pub async fn run(query: MqeQuery, pool: &PgPool) -> Result<Vec<TimeSeries>, MqeError>`

**Modify** `handlers/metrics.rs` — add 4 handlers:
- `POST /api/query/mqe` — body: `{expr, from, to, step_seconds}` → returns `Vec<TimeSeries>`
- `GET /api/query/mqe/validate?expr=X` → `{valid, error?{position,message}}`
- `GET /api/query/metrics/names` → distinct metric names (last 24h)
- `GET /api/query/metrics/labels?metric=X` → `{label_keys, label_values}`

### Frontend Tasks
- **New File** `frontend/src/components/MQEEditor.tsx` — textarea with:
  - Syntax highlighting (metric=blue, functions=purple, label keys=amber, strings=green)
  - Debounced validation (500ms) calling `/api/query/mqe/validate`
  - Inline error with position underline
  - Autocomplete for metric names and label values
  - Example expression chips
- **Modify** `DashboardBuilder.tsx` — replace plain query input with `<MQEEditor>`; `PanelPreview` calls `POST /api/query/mqe` for live preview when expression is valid
- **Modify** `QueryExplorer.tsx` — add "Metrics Expression" tab with `MQEEditor` + time range + step + line chart result

### Validation
```
POST /api/query/mqe {expr:"avg(cpu_usage_percent)"} → Vec<TimeSeries>
GET /api/query/mqe/validate?expr=rate(net[5m]) → {valid:true}
GET /api/query/mqe/validate?expr=avg({ → {valid:false, error:{position:6,...}}
Dashboard builder panel → MQEEditor present, live preview works
```

---

## PROMPT 04 — Apdex Scoring on Services

**Inspired by:** SkyWalking standard Apdex metric  
**Effort:** 2–3 days

### Context
Apdex (Application Performance Index) is a 0–1 score: `(satisfied + tolerating×0.5) / total`. Satisfied = duration ≤ T, Tolerating = T < duration ≤ 4T. Gives instant human-readable service health without interpreting raw percentiles.

### Migration — `migrations/20250101000056_apdex_config.sql`
```sql
CREATE TABLE IF NOT EXISTS apdex_config (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id         UUID NOT NULL,
    service_name   TEXT NOT NULL,
    satisfied_ms   INTEGER NOT NULL DEFAULT 200,
    tolerating_ms  INTEGER NOT NULL DEFAULT 800,
    UNIQUE(org_id, service_name)
);
```

### Backend Tasks
- **Modify** `handlers/service_map.rs` — add:
  - `GET /api/apdex?window=1h` — score per service (SQL: count satisfied/tolerating/frustrated from `traces`)
  - `GET /api/apdex/:service?window=1h` — score + breakdown for one service
  - `PUT /api/apdex/config` — upsert thresholds
- Update `ServiceNode` struct — add `apdex: Option<f64>`
- Update `get_service_map` — JOIN apdex into each node

### Frontend Tasks
- **New File** `frontend/src/components/ApdexGauge.tsx` — SVG circular gauge, color bands: ≥0.94 emerald / ≥0.85 blue / ≥0.70 amber / ≥0.50 orange / <0.50 red; rating label: Excellent/Good/Fair/Poor/Unacceptable
- **Modify** `Traces.tsx` — add Apdex column to services list
- **Modify** `Topology.tsx` — show `<ApdexGauge size="sm">` in selected service node detail panel
- **Modify** `Dashboard.tsx` — add Apdex scores in service health section

### Validation
```
GET /api/apdex?window=1h → [{service_name, apdex, satisfied, tolerating, frustrated}]
PUT /api/apdex/config {service_name:"X",satisfied_ms:300} → 200 OK
Topology → click service node → Apdex gauge visible in detail panel
```

---

## PROMPT 05 — Topology Drill-through + Service Hierarchy

**Inspired by:** SkyWalking Service Hierarchy + entity navigation  
**Effort:** 1 week

### Context
SkyWalking's topology map is a navigation hub — every node click navigates into the correct monitoring view for that entity. Zentra `Topology.tsx` nodes are not clickable. This prompt makes every node a contextual navigation entry point and adds parent-child target relationships.

### Migration — `migrations/20250101000057_target_hierarchy.sql`
```sql
ALTER TABLE targets ADD COLUMN IF NOT EXISTS parent_target_id UUID REFERENCES targets(target_id) ON DELETE SET NULL;
ALTER TABLE targets ADD COLUMN IF NOT EXISTS hierarchy_depth  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE targets ADD COLUMN IF NOT EXISTS hierarchy_path   TEXT;
CREATE INDEX IF NOT EXISTS idx_targets_parent ON targets(parent_target_id);
```

### Backend Tasks
- **Modify** `handlers/targets.rs` — add:
  - `GET /api/targets/:id/hierarchy` — recursive CTE walking parent chain upward → `Vec<TargetSummary>`
  - `GET /api/targets/:id/children` — direct children
  - `PATCH /api/targets/:id/parent` — set/clear parent; recompute `hierarchy_depth` + `hierarchy_path`
- Update topology endpoint in `handlers/network.rs` — include hierarchy edges (`edge_type:"hierarchy"`) as dashed edges

### Frontend Tasks
- **New File** `frontend/src/components/HierarchyBreadcrumb.tsx` — calls `GET /api/targets/:id/hierarchy`; renders "Cluster > Node > Target" with each level as a clickable link to the correct monitoring page
- **Modify** `Topology.tsx` — make every node clickable with routing:
  ```
  server        → /servers?target_id=:id
  database      → /targets?type=database&id=:id
  k8s_node      → /middleware/kubernetes?node=:label
  network_device→ /network?device=:id
  cilium        → /cilium
  service       → /traces?service=:label
  ```
  Draw hierarchy edges as dashed lighter lines. Show "Navigate →" tooltip on hover.
- Add `<HierarchyBreadcrumb>` to: `Servers.tsx`, `PostgresMonitoring.tsx`, `OracleMonitoring.tsx`, `MysqlMonitoring.tsx`, `KubernetesMonitoring.tsx`, `CiliumMonitoring.tsx`
- Add "View in Topology" button on each monitoring page → `navigate('/topology')` with target pre-selected

### Validation
```
GET /api/targets/:id/hierarchy → ancestor chain JSON
Topology: click server node → navigates to /servers?target_id=X
Servers page: HierarchyBreadcrumb visible below page title
Topology: hierarchy edges rendered as dashed lines
```

---

## PROMPT 06 — Hubble gRPC Flow Collector

**Inspired by:** Cilium Hubble ObserverService gRPC API  
**Effort:** 1–2 weeks  
**Depends on:** PROMPT-01, PROMPT-02

### Context
Cilium Hubble exposes a gRPC stream (`ObserverService.Observe`) emitting real-time L4/L7 network flows from the Linux kernel — HTTP method/path/status, DNS queries, Kafka topics — with zero pcap overhead. Zentra's agent subscribes to this stream and writes flows into the existing `netflow_records` table (`flow_version=100` for Hubble-sourced flows).

### Migration — `migrations/20250101000058_hubble_l7_flows.sql`
```sql
CREATE INDEX IF NOT EXISTS idx_netflow_hubble ON netflow_records(flow_version, received_at DESC) WHERE flow_version = 100;

CREATE OR REPLACE VIEW hubble_l7_flows AS
SELECT received_at, src_ip, src_port, dst_ip, dst_port, protocol_name,
    extra->>'verdict'           AS verdict,
    extra->>'drop_reason'       AS drop_reason,
    extra->'l7'->>'type'        AS l7_type,
    extra->'l7'->>'method'      AS http_method,
    extra->'l7'->>'url'         AS http_url,
    extra->'l7'->>'status_code' AS http_status,
    extra->'l7'->>'query'       AS dns_query,
    extra->'l7'->>'topic'       AS kafka_topic
FROM netflow_records WHERE flow_version = 100;
```

### Backend Tasks
- **Modify** `crates/Zentra-agent/Cargo.toml` — add `tonic = "0.12"`, `prost = "0.13"`, `prost-types = "0.13"`
- **New File** `crates/Zentra-agent/src/collectors/hubble.rs`:
  - Connect to `hubble-relay:4245` via tonic gRPC
  - Subscribe to `ObserverService.Observe` with `follow:true`
  - Map each `Flow` proto → INSERT into `netflow_records` (`flow_version=100`, verdict/drop_reason/L7 detail in `extra` JSONB)
  - Reconnect with exponential backoff (max 60s) on disconnect
- **Modify** `handlers/cilium.rs` (from PROMPT-02) — add:
  - `GET /api/hubble/status` — `{connected, flows_per_second, total_flows_1h, l7_flows_1h}`
  - `GET /api/hubble/flows?from&to&verdict&l7_type&limit` — returns from `hubble_l7_flows` view

### Frontend Tasks
- **Modify** `CiliumMonitoring.tsx` — add Hubble connectivity card + L7 flows table tab
- **Modify** `Network.tsx` — add "eBPF Flows" tab (visible when Hubble connected) with L7 flows table

### Validation
```
With Hubble: netflow_records gains rows with flow_version=100
Without Hubble: GET /api/hubble/status → {connected:false} (graceful, no error page)
Cilium page: Hubble tab shows flow table
```

---

## PROMPT 07 — Trace ↔ DB Metric Cross-Correlation

**Inspired by:** SkyWalking Virtual Database correlation  
**Effort:** 1–2 weeks  
**Depends on:** PROMPT-04

### Context
When a trace contains a DB span (`db.system`, `db.statement` span attributes), Zentra correlates it with actual DB metrics at the exact time of the slowness. From a slow trace you can jump to see what the database was doing — active connections, lock waits, cache hit ratio — at that exact moment.

### Backend Tasks
- **New File** `crates/Zentra-drilldown/src/trace_db_correlator.rs`:
  - `correlate_trace(trace_id, org_id)` — find DB spans in trace; for each, query `metrics` table in window `[span.started_at - 30s, span.started_at + duration + 30s]` for matching DB target; return `Vec<DbSpanContext>`
  - `find_slow_traces_for_db(target_id, from, to, threshold_ms, org_id)` — traces with slow DB calls against this target
  - `service_to_db_map(org_id)` — which services call which DBs, with p95 latency from spans
- **Modify** `handlers/drilldown.rs` — add:
  - `GET /api/drilldown/trace/:trace_id/db-context`
  - `GET /api/drilldown/db-slowness?target_id&from&to&threshold_ms`
  - `GET /api/correlation/service-to-db`

### Frontend Tasks
- **New File** `frontend/src/components/DBContextPanel.tsx` — shows DB metrics at time of span: active connections, active queries, lock waits, cache hit ratio; "View DB Dashboard →" link
- **Modify** `Traces.tsx` — show `<DBContextPanel>` in span detail when span has `db.system` tag; add DB icon on DB spans in waterfall
- **Modify** `PostgresMonitoring.tsx`, `OracleMonitoring.tsx`, `MysqlMonitoring.tsx`, `MongodbMonitoring.tsx` — add "Slow Traces" tab calling `/api/drilldown/db-slowness?target_id=:id`
- **Modify** `Topology.tsx` — draw service→database edges with p95 latency label from `/api/correlation/service-to-db`

### Validation
```
GET /api/drilldown/trace/:id/db-context → DB span contexts with metric values
GET /api/drilldown/db-slowness?target_id=:id → slow traces list
Traces page: DB span selected → DBContextPanel shows correlated DB metrics
PostgresMonitoring: "Slow Traces" tab present and populated
```

---

## PROMPT 08 — LLM Dashboard from OTLP GenAI Spans

**Inspired by:** SkyWalking v10.4 Virtual GenAI dashboard  
**Effort:** 1–2 weeks  
**Depends on:** Existing `handlers/otlp.rs` + `LLMObservability.tsx`

### Context
SkyWalking 10.4 derives all LLM metrics from OTLP traces with `gen_ai.*` span attributes (OpenTelemetry GenAI semantic conventions). Zentra's `LLMObservability.tsx` exists but uses stubs. This wires it to real data from OTLP ingestion.

**GenAI span attributes:** `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.response.finish_reasons`, `gen_ai.operation.name`

### Migration — `migrations/20250101000059_llm_spans.sql`
```sql
CREATE TABLE IF NOT EXISTS llm_spans (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID NOT NULL,
    trace_id      TEXT NOT NULL,
    span_id       TEXT NOT NULL,
    started_at    TIMESTAMPTZ NOT NULL,
    duration_ms   DOUBLE PRECISION NOT NULL,
    gen_ai_system TEXT,
    model         TEXT,
    operation     TEXT,
    input_tokens  INTEGER,
    output_tokens INTEGER,
    finish_reason TEXT,
    service_name  TEXT,
    is_error      BOOLEAN NOT NULL DEFAULT FALSE
);
SELECT create_hypertable('llm_spans', 'started_at', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS idx_llm_spans_org_time ON llm_spans(org_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_spans_model    ON llm_spans(model, started_at DESC);
SELECT add_retention_policy('llm_spans', INTERVAL '90 days', if_not_exists => TRUE);
```

### Backend Tasks
- **Modify** `handlers/otlp.rs` — in span ingestion loop: if any attribute starts with `gen_ai.` and `input_tokens` or `output_tokens` present → extract all GenAI attributes and INSERT into `llm_spans`
- **Modify** `handlers/llm_observability.rs` — replace stub implementations with real SQL on `llm_spans`:
  - `GET /api/llm/overview?window=` — total requests, tokens, avg/p95 latency, error rate, estimated cost
  - `GET /api/llm/models?window=` — per-model breakdown
  - `GET /api/llm/cost-trend?window&step` — hourly cost time series
  - `GET /api/llm/latency-distribution?window` — histogram buckets
  - `GET /api/llm/token-efficiency?window` — input vs output token ratio per model

### Frontend Tasks
- **Modify** `LLMObservability.tsx` — wire all panels to updated endpoints; add: model comparison table, token efficiency grouped bar chart, system distribution donut, operation breakdown (chat/embeddings/completions), streaming vs non-streaming indicator from `finish_reason`

### Validation
```
Send OTLP span with gen_ai.system="openai", gen_ai.request.model="gpt-4o",
  gen_ai.usage.input_tokens=100, gen_ai.usage.output_tokens=200
→ llm_spans gains a row
GET /api/llm/overview → real counts (not zeros)
LLMObservability page → all panels show real data
```

---

## PROMPT 09 — Continuous eBPF Profiling (Agentless via SSH)

**Inspired by:** SkyWalking Rover eBPF profiling  
**Effort:** 3–4 weeks  
**Platform:** Linux target hosts only. Collection via SSH exec — fully agentless.  
**Depends on:** PROMPT-01, PROMPT-05

### Context
SkyWalking added CPU flame graph profiling via its Rover eBPF agent. Zentra implements this agentlessly by SSH-executing `perf record` + `perf script` on the monitored Linux host (using existing SSH infrastructure), retrieving output, and rendering an interactive flamegraph.

### Migration — `migrations/20250101000060_profiles.sql`
```sql
CREATE TABLE IF NOT EXISTS profiling_jobs (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id        UUID NOT NULL,
    target_id     UUID NOT NULL REFERENCES targets(target_id),
    profile_type  TEXT NOT NULL DEFAULT 'cpu',
    status        TEXT NOT NULL DEFAULT 'pending',
    duration_s    INTEGER NOT NULL DEFAULT 30,
    pid_filter    TEXT,
    triggered_by  UUID,
    triggered_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at  TIMESTAMPTZ,
    error_message TEXT
);
CREATE TABLE IF NOT EXISTS profiles (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    org_id          UUID NOT NULL,
    job_id          UUID NOT NULL REFERENCES profiling_jobs(id),
    target_id       UUID NOT NULL REFERENCES targets(target_id),
    profile_type    TEXT NOT NULL,
    collected_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_s      INTEGER NOT NULL,
    sample_count    INTEGER,
    flamegraph_data JSONB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_profiles_target ON profiles(target_id, collected_at DESC);
```

### Backend Tasks
- **New File** `crates/Zentra-agent/src/collectors/ebpf_profiler.rs`:
  - SSH exec: `perf record -g -F 99 -a -o /tmp/is_perf_$JOB_ID.data -- sleep $DURATION`
  - SSH exec: `perf script -i /tmp/is_perf_$JOB_ID.data`
  - Parse perf script output → collapsed stack format `"func1;func2;func3 42"`
  - Convert collapsed → d3-flame-graph JSON `{name, value, children: [...]}`
  - SSH exec: `rm /tmp/is_perf_$JOB_ID.data`
  - INSERT result into `profiles` table; UPDATE `profiling_jobs` status
- **New File** `handlers/profiling.rs`:
  - `POST /api/profiles/trigger` — creates job, queues async task
  - `GET /api/profiles?target_id=:id` — recent jobs list
  - `GET /api/profiles/:id` — full flamegraph JSON
  - `GET /api/profiles/jobs/:job_id/status` — poll for completion

### Frontend Tasks
- **New File** `frontend/src/components/charts/FlameGraph.tsx` — pure SVG React flamegraph renderer: width-proportional frames, depth as Y axis, hover tooltips, click-to-zoom, search/highlight, reset zoom button
- **New File** `frontend/src/pages/Profiling.tsx` — trigger panel (target dropdown, type, duration, PID filter), recent profiles table with status polling (3s interval), flamegraph viewer panel
- Add route `/profiling` in `App.tsx`; nav item "CPU Profiling" under Performance in `Layout.tsx`
- **Modify** `Servers.tsx` — add "Profile" button per server row

### Validation
```
POST /api/profiles/trigger → {job_id, status:"pending"}
GET /api/profiles/jobs/:id/status → polling works
FlameGraph component: renders sample JSON as SVG without errors
Navigate /profiling → page loads correctly
(With real SSH+perf): full cycle completes, flamegraph stored and rendered
```

---

## PROMPT 10 — TraceQL Query Language Support

**Inspired by:** SkyWalking v10.4 TraceQL + Grafana Tempo spec  
**Effort:** 2–3 weeks  
**Depends on:** PROMPT-03 (reuse lexer/parser patterns)

### Context
SkyWalking 10.4 added TraceQL (Grafana Tempo's trace query language) enabling expressive queries like `{service.name="order-service" && duration > 1s && status=error}` and making SkyWalking a Grafana Tempo data source. Zentra implements the same — better trace search UX + Grafana compatibility.

### TraceQL Subset to Implement
```
Query      = '{' Selector '}'
Selector   = Condition (('&&'|'||') Condition)*

Condition  = Attribute Op StringLiteral
           | 'duration' DurationOp Duration
           | 'status' '=' ('ok'|'error'|'unset')

Attribute  = 'service.name' | 'span.name' | 'status'
           | 'span.' AttrName | 'resource.' AttrName | AttrName

Op         = '=' | '!=' | '=~' | '!~'
DurationOp = '>' | '>=' | '<' | '<='
Duration   = [0-9]+ ('ms'|'s'|'m'|'h')

Examples:
  {service.name="order-service"}
  {status=error}
  {duration > 1s}
  {service.name="order-service" && duration > 500ms}
  {span.db.system="postgresql" && status=error}
```

### Backend Tasks — New directory `crates/Zentra-query/src/traceql/`
- `lexer.rs` — tokenize `{`, `}`, `&&`, `||`, `!`, attribute names, comparison ops, duration literals, strings
- `parser.rs` — parse to AST; `ParseError` includes `position` + `message` (same pattern as MQE)
- `translator.rs` — translate AST → SQL `WHERE` clauses against `traces` table:
  - `service.name` → `service_name`
  - `span.name` → `operation_name`
  - `duration` → `duration_ms` (with unit conversion)
  - `status` → `status` (`ok`→`'ok'`, `error`→`'error'`)
  - Custom `span.*` attributes → `tags->>'key'`
- `mod.rs` — `pub async fn run(expr: &str, from: DateTime, to: DateTime, limit: u32, pool: &PgPool) -> Result<Vec<TraceSummary>, TraceQlError>`

**Modify** `handlers/traces.rs` — add:
- `POST /api/query/traceql` — body: `{expr, from, to, limit}` → `Vec<TraceSummary>`
- `GET /api/query/traceql/validate?expr=X` → `{valid, error?{position,message}}`

**Add Grafana Tempo-compatible endpoints** (new route group `/tempo`):
- `GET /tempo/api/search?q={...}&start=&end=&limit=` → Grafana Tempo search format
- `GET /tempo/api/traces/:traceID` → Grafana Tempo trace format

### Frontend Tasks
- **Modify** `Traces.tsx` — replace service/operation dropdowns with a TraceQL search bar:
  - Monospace input with live syntax validation (debounced 500ms)
  - Inline error display with position highlight
  - Autocomplete for known attribute names
  - Example query chips: `{status=error}`, `{duration > 1s}`, `{service.name="..."}` (clickable)
  - Calls `POST /api/query/traceql` on submit
- **Modify** `QueryExplorer.tsx` — add "Trace Query" tab with TraceQL editor, time range, results table

### Validation
```
POST /api/query/traceql {expr:"{status=error}"} → error traces
POST /api/query/traceql {expr:"{duration > 1s}"} → slow traces
POST /api/query/traceql {expr:"{service.name=\"X\" && duration > 500ms}"} → filtered traces
GET /api/query/traceql/validate?expr={status=error} → {valid:true}
GET /tempo/api/search?q={status=error} → Grafana Tempo format response
Traces page: TraceQL search bar present, submits and shows results
```

---

*Last updated: 2026-05-25*  
*See `docs/TODO_SKYWALKING_CILIUM.md` to track completion status of each prompt*
