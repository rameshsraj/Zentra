# Zentra Roadmap — Implementation Prompts Playbook

This document contains precise, copy-pasteable implementation prompts and step-by-step playbooks for executing Zentra's core roadmap phases. Each prompt is structured to be immediately actionable by Cascade or any developer.

---

## Prompt 1: DB-Backed gRPC Ingestion Security (OT-SEC)

### Objective
Upgrade the gRPC ingestion server on port 4317 to validate the authentication header (`Authorization: Bearer <key>`) against the live database (`api_tokens` table) rather than relying exclusively on memory-cached checks or unvalidated org IDs.

### Target Files
- `@/crates/zentra-api/src/grpc.rs` (Verification loop and header resolver)
- `@/crates/zentra-api/src/handlers/otlp.rs` (Cache validation and DB query engine)

### Implementation Playbook
1. **Read & Resolve**: Inside `resolve_org_id_from_headers`, extract the Bearer token.
2. **Cache-then-DB Lookup**: Check the local `otlp_key_cache` in `AppState`. If missing:
   - Run an asynchronous database query against `api_tokens` to locate a matching, active, unexpired API token.
   - SQL query:
     ```sql
     SELECT org_id, permissions FROM api_tokens 
     WHERE token_hash = $1 AND is_active = true AND (expires_at IS NULL OR expires_at > NOW())
     ```
3. **Update Cache**: Write the valid `(org_id, permissions)` tuple into the concurrent cache map.
4. **Enforce Hard Rejection**: If the token is invalid or missing, immediately return a gRPC error status code `16` (Unauthenticated) using `grpc_error(16, "Unauthenticated")`.

---

## Prompt 2: High-Availability Background Task Runner (BKG-JOBS)

### Objective
Transition `@/crates/zentra-api/src/background_jobs.rs` from simple uncoordinated tokio loop spawning into a coordinated, HA-safe, distributed background worker using transactional advisory locks.

### Target Files
- `@/crates/zentra-api/src/background_jobs.rs` (Job scheduler loop)
- `@/crates/zentra-api/src/main.rs` (Server initialization)

### Implementation Playbook
1. **Define Task Constants**: Assign a unique 64-bit ID for each task's Postgres advisory lock:
   - `RED_METRIC_MV_LOCK_ID = 48201`
   - `ALERT_EVAL_LOCK_ID = 48202`
2. **Transaction-Gate Executions**: Before running the tick payload, obtain a session-level non-blocking advisory lock:
   ```sql
   SELECT pg_try_advisory_lock($1)
   ```
3. **Safe MV Refresh**: Ensure that if the lock is successfully acquired:
   - Run the concurrent materialized view refresh: `REFRESH MATERIALIZED VIEW CONCURRENTLY span_red_metrics_1m`.
   - If a unique index is missing, capture the failure and fallback to a standard refresh.
4. **Alert Rule Evaluator**: Coordinate evaluation per tenant/org. Lock the alert run task, query alert thresholds from the database, evaluate incoming metrics, and write triggered alerts to `active_alerts`.
5. **Release Locks**: Ensure locks are cleanly released on transactional commit or task panics/timeouts.

---

## Prompt 3: Native Oracle Connection Pooling (ORA-FFI)

### Objective
Eliminate the subprocess-wrapper execution model in `@/plugins/database-plugin/src/oracle_connection.rs` that spawns `sqlplus` scripts over SSH. Replace it with a pure native, multi-threaded connection pool leveraging the `oracle` crate (OCI Instant Client FFI) or pure SQL connection drivers.

### Target Files
- `@/plugins/database-plugin/Cargo.toml` (Add dependencies)
- `@/plugins/database-plugin/src/oracle_connection.rs` (Native driver implementation)
- `@/plugins/database-plugin/src/oracle_metric_executor.rs` (Query pipeline)

### Implementation Playbook
1. **Declare Crate Dependency**: Add `oracle = { version = "0.6", features = [] }` to the plugin's Cargo manifest.
2. **Build Connection Pool Manager**:
   - Initialize the `oracle::Connection` pool during target onboarding.
   - Configure connection limits, idle timeouts, and prefetch buffers.
3. **Refactor Query Execution**:
   - Rewrite the query string builders to pass parsed values natively via parameters:
     ```rust
     let conn = pool.get()?;
     let rows = conn.query("SELECT metric_name, value FROM v$sysmetric", &[])?;
     ```
4. **Map Oracle Results directly to Metric Structs**: Parse table results natively into `zentra_core::models::metric::Metric` vectors, avoiding regex parsing of temporary text file logs on disk.

---

## Prompt 4: OTel Distributed Flamegraph Visualizer (PROF-UI)

### Objective
Expose standard REST and gRPC endpoints to retrieve, aggregate, and render hierarchical OpenTelemetry call stack profiles into an interactive Flamegraph visualization.

### Target Files
- `@/crates/zentra-api/src/handlers/profiles.rs` (Profile query engine)
- `@/crates/zentra-query/src/lib.rs` (Stack frame aggregation)

### Implementation Playbook
1. **Query Profiles DB**: Select stack traces matching a specified `service.name` and timeframe from the `profiles` table.
2. **Aggregate Stack Frames**: Build a collapsed, hierarchically prefix-folded stack tree representing function calls:
   - Format: `parent_frame;child_frame;leaf_frame value`
3. **Format JSON Representation**: Generate a nested hierarchical JSON response optimal for rendering with frontend visualizers (e.g., d3-flame-graph or speedscope format):
   ```json
   {
     "name": "root",
     "value": 1500,
     "children": [
       { "name": "main", "value": 1400, "children": [] }
     ]
   }
   ```
4. **Integrate Route**: Route `GET /api/v1/profiles/flamegraph` directly to this handler inside the API router.

---

## Prompt 5: Real-Time Hubble Streaming Client (HUBBLE-STREAM)

### Objective
Replace the static/simulated Cilium network flow loading in `plugins/network-plugin` with an active gRPC streaming client that connects to the local Hubble Relay endpoint and continuously ingests live Cilium network flows.

### Target Files
- `@/plugins/network-plugin/Cargo.toml` (Add tonic/prost gRPC client dependencies)
- `@/plugins/network-plugin/src/hubble_client.rs` (New — streaming gRPC client)
- `@/plugins/network-plugin/src/lib.rs` (Wire the streaming client into the plugin lifecycle)

### Implementation Playbook
1. **Add gRPC Client Dependencies**: Add `tonic`, `prost`, and `tokio-stream` to the network plugin's Cargo manifest.
2. **Generate or Hand-Write Hubble Protobuf Structs**: Define the minimal Hubble `observer.proto` structs needed: `GetFlowsRequest`, `GetFlowsResponse`, `Flow`, `Endpoint`, `IP`, `Layer4`, `Verdict`.
3. **Implement Streaming Client**:
   ```rust
   pub struct HubbleStreamClient {
       endpoint: String, // e.g., "http://localhost:4245" or "unix:///var/run/cilium/hubble.sock"
   }
   ```
   - Connect to Hubble Relay via `tonic::transport::Channel`.
   - Call `observer::Observer::get_flows()` with a streaming request.
   - For each `Flow` received, map it to `zentra_core::models::metric::Metric` or the existing `HubbleFlow` model.
4. **Backpressure & Reconnect**: Implement exponential backoff reconnection if the Hubble Relay is temporarily unavailable. Use a bounded channel (`tokio::sync::mpsc`) to buffer flows before metric conversion.
5. **Graceful Shutdown**: Listen for a `CancellationToken` to cleanly drop the gRPC stream on server shutdown.

---

## Prompt 6: AI Plugin OTel Semantic Convention Alignment (AI-OTEL)

### Objective
Refactor the AI/ML observability plugin (`plugins/ai-plugin`) to emit metrics and spans that conform to the OpenTelemetry Semantic Conventions for Generative AI (`gen_ai.*` namespace), eliminating custom metric names.

### Target Files
- `@/plugins/ai-plugin/src/lib.rs` (Metric name and attribute refactoring)
- `@/plugins/ai-plugin/src/anomaly_engine.rs` (If custom spans exist — align attribute keys)

### Implementation Playbook
1. **Rename Metrics to OTel Conventions**:
   | Current Name | OTel Semantic Name |
   |---|---|
   | `ai_anomaly_score` | `gen_ai.client.operation.duration` (with anomaly attributes) |
   | `ai_model_health` | `gen_ai.server.request.duration` |
   | `ai_inference_latency` | `gen_ai.client.token.usage` |
2. **Add Semantic Attributes**: Every metric must carry:
   - `gen_ai.system` (e.g., `"zentra"`)
   - `gen_ai.request.model` (model name)
   - `gen_ai.operation.name` (e.g., `"anomaly_detection"`, `"inference"`)
3. **Span Convention Alignment**: If the plugin emits trace spans, ensure:
   - Span name follows `{gen_ai.operation.name} {gen_ai.request.model}` pattern.
   - `gen_ai.usage.input_tokens` and `gen_ai.usage.output_tokens` are recorded as span attributes.
4. **Backward Compatibility**: Emit both the legacy metric name and the OTel name for one release cycle, controlled by a feature flag `otel-semconv` (default on).

---

## Prompt 7: Database Schema Standardization — Spans vs Traces (DB-SCHEMA)

### Objective
Resolve the inconsistent use of `spans` and `traces` table names across SQL migrations, Rust handlers, and frontend API calls. Standardize on a single naming convention (`traces` as the primary table, `spans` as child rows within a trace).

### Target Files
- `@/deploy/sql/` (Migration files referencing both `spans` and `traces`)
- `@/crates/zentra-api/src/handlers/traces.rs` (Query handlers)
- `@/crates/zentra-api/src/handlers/otlp.rs` (Ingestion handlers)
- `@/frontend/src/pages/Traces.tsx` (Frontend API calls)

### Implementation Playbook
1. **Audit All Table References**: Search for `FROM spans`, `FROM traces`, `INSERT INTO spans`, `INSERT INTO traces` across all SQL and Rust files.
2. **Define Canonical Schema**:
   - `traces` — one row per trace (trace_id, root span service, total duration, span count).
   - `spans` — one row per span (span_id, trace_id FK, parent_span_id, service.name, operation, duration, status, attributes JSONB).
   - `span_red_metrics_1m` — materialized view aggregating from `spans`.
3. **Create Migration**: Write a new migration (`deploy/sql/0XX_standardize_spans_traces.sql`) that:
   - Renames any misnamed tables.
   - Adds missing foreign key constraints.
   - Recreates materialized views to point at the corrected schema.
4. **Update Handlers**: Ensure all Rust query builders reference the canonical table names.
5. **Update Frontend**: Verify that all API response field names match the new schema.

---

## Prompt 8: Offline License Signer CLI Tool (LIC-SIGNER)

### Objective
Build a standalone Rust CLI binary (`zentra-license-gen`) that generates Ed25519 keypairs, signs license payloads, and emits production-ready `license.json` files conforming to the versioned envelope format documented in `docs/licensing/SIGNING_CONTRACT.md`.

### Target Files
- `@/tools/zentra-license-gen/Cargo.toml` (New crate)
- `@/tools/zentra-license-gen/src/main.rs` (CLI entry point)

### Implementation Playbook
1. **Create Crate**: Initialize a new binary crate at `tools/zentra-license-gen` with dependencies: `ring`, `clap`, `serde_json`, `chrono`, `hex`, `uuid`.
2. **Subcommands**:
   ```
   zentra-license-gen keygen --output keys/
   zentra-license-gen sign --private-key keys/private.pem --config license-config.json --output license.json
   zentra-license-gen verify --public-key keys/public.hex --license license.json
   ```
3. **`keygen` Command**:
   - Generate an Ed25519 keypair using `ring::signature::Ed25519KeyPair`.
   - Write the PKCS#8 private key to `private.pem` and the raw 32-byte public key hex to `public.hex`.
   - Print the `key_id` (e.g., `"zentra-prod-YYYY-MM"`).
4. **`sign` Command**:
   - Read a JSON config with fields: `org_name`, `org_id`, `product`, `tier`, `seat_count`, `onboarded_at`, `trial_duration_days`.
   - Serialize the payload as compact JSON (no trailing whitespace).
   - Sign the raw payload bytes with the Ed25519 private key.
   - Wrap in versioned envelope: `{ "format_version": 1, "algorithm": "ed25519", "key_id": "...", "payload": <raw>, "signature_hex": "..." }`.
   - Write to output file.
5. **`verify` Command**:
   - Load the license file and public key.
   - Verify the signature matches the raw payload bytes.
   - Print payload details and expiry status.

---

## Prompt 9: Security Plugin Live Activation (SEC-LIVE)

### Objective
Wire the security plugin's sub-module engines (IDS, DNS, SSL, Firewall) into the metric collection pipeline so that real data flows through instead of hardcoded zero values. Implement a Suricata EVE JSON log watcher.

### Target Files
- `@/plugins/security-plugin/src/lib.rs` (Metric collection pipeline)
- `@/plugins/security-plugin/src/ids_engine.rs` (EVE JSON watcher)
- `@/plugins/security-plugin/src/ssl_analyzer.rs` (Live certificate scanning)
- `@/plugins/security-plugin/src/firewall.rs` (iptables/nftables rule counter)

### Implementation Playbook
1. **EVE JSON File Watcher**: In `ids_engine.rs`, implement a `notify`-based file watcher that tails the Suricata EVE JSON log (default: `/var/log/suricata/eve.json`). Parse each new line as `EveJsonEvent` and emit IDS alerts.
2. **SSL Certificate Scanner**: In `ssl_analyzer.rs`, connect to target host:port via TLS, extract the peer certificate chain, and calculate `security_ssl_cert_expiry_days` from the `Not After` field.
3. **Firewall Rule Counter**: In `firewall.rs`, execute `iptables -L -v -n --line-numbers` (Linux) or `netsh advfirewall show allprofiles` (Windows) and parse rule hit counters.
4. **Wire into `collect_metrics`**: Replace the hardcoded metric values in `lib.rs` with calls to each sub-module:
   ```rust
   let ids_count = self.ids_engine.alert_count_since_last_poll();
   metrics.push(Metric::new("security_ids_alerts_total", ids_count as f64, ...));
   ```
5. **Alert Pipeline Integration**: Route critical IDS alerts (severity >= `high`) to the notification router via `zentra_stream::notification_router::dispatch_alert()`.

---

## Prompt 10: Cloud Monitoring Frontend & RBAC (CLOUD-UI)

### Objective
Build dedicated frontend pages for AWS, Azure, and GCP cloud monitoring data, wire them into the sidebar navigation, and register a new `cloud_monitoring` RBAC module.

### Target Files
- `@/frontend/src/pages/CloudDashboard.tsx` (New — unified cloud overview)
- `@/frontend/src/pages/CloudAws.tsx` (New — AWS-specific detail page)
- `@/frontend/src/pages/CloudAzure.tsx` (New — Azure-specific detail page)
- `@/frontend/src/pages/CloudGcp.tsx` (New — GCP-specific detail page)
- `@/frontend/src/components/Layout.tsx` (Sidebar navigation registration)
- `@/deploy/sql/0XX_cloud_rbac.sql` (New — RBAC module registration)

### Implementation Playbook
1. **Register RBAC Module**: Create a SQL migration that inserts `cloud_monitoring` as a module ID. Add sub-module entries: `cloud_monitoring.aws`, `cloud_monitoring.azure`, `cloud_monitoring.gcp`.
2. **Sidebar Navigation**: In `Layout.tsx`, add a "Cloud Monitoring" section under `NAV_MODULE_MAP` and `NAV_CHILD_MAP`:
   ```ts
   cloud_monitoring: { label: 'Cloud Monitoring', icon: Cloud, path: '/cloud' },
   ```
   Children: AWS, Azure, GCP detail pages.
3. **Cloud Dashboard Page**: Build a unified overview showing:
   - Active cloud targets by provider (pie chart or card grid).
   - Latest metrics per provider (CPU, cost, request count).
   - Health status badges per cloud account.
4. **Provider Detail Pages**: Each page (AWS, Azure, GCP) shows:
   - Target list for that provider with last-seen timestamps.
   - Key metric time-series charts (fetched from `/api/v1/metrics?resource_type=cloud&subtype=aws_*`).
   - Alert status and recent events.
5. **API Backend**: No new backend handlers needed — cloud metrics are already ingested into the standard metrics pipeline by the cloud plugin. Frontend queries the existing `/api/v1/metrics` endpoint with appropriate filters.

---

## Prompt 11: Storage Deep-Dive Dashboard (STORAGE-UI)

### Objective
Build a dedicated storage monitoring page with per-volume IOPS, latency heatmaps, and capacity forecasting. Wire into the existing Infrastructure sidebar section.

### Target Files
- `@/frontend/src/pages/StorageMonitor.tsx` (New — deep-dive storage page)
- `@/frontend/src/components/Layout.tsx` (Register under infrastructure children)

### Implementation Playbook
1. **Register Menu Item**: Add `infrastructure.storage_monitor` to the RBAC sub-module items and register it in `NAV_CHILD_MAP` under the infrastructure module.
2. **Storage Overview Panel**:
   - Card grid showing each storage target (NetApp, Ceph, Dell EMC, etc.) with health badge, total/used/free capacity.
   - Aggregate capacity pie chart across all storage backends.
3. **Per-Volume Detail Table**:
   - Sortable table of volumes with columns: Name, Backend, Total, Used, Used%, IOPS (read/write), Latency (read/write).
   - Data fetched from `/api/v1/metrics?resource_type=storage`.
4. **Capacity Forecast Chart**:
   - Line chart showing historical disk usage over time with a linear regression projection line.
   - Highlight the estimated "days until full" for each volume.
5. **Latency Heatmap** (stretch goal):
   - Matrix visualization showing latency percentiles (p50, p95, p99) per volume over time.
   - Color-coded: green (<1ms), yellow (1-5ms), red (>5ms).
