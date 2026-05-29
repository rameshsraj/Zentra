# Zentra OpenTelemetry — Gap Analysis, Prompts & Dated Plan

**Document date:** 2026-05-29
**Target completion:** 2026-07-10 (6-week plan + 1 buffer week)
**Owner:** TBD
**Scope:** OTLP ingestion, self-instrumentation, storage, and OTel pipeline correctness. Builds on the existing `TODO_OTEL_GAP_ANALYSIS.md` — supersedes nothing, but adds the protocol-correctness and reliability gaps that doc omits.

---

## 1. Executive summary

Zentra's OTel surface is broader than expected: all four signals (traces / metrics / logs / profiles), JSON and protobuf encodings, an HTTP receiver and a feature-gated "gRPC" server, real self-instrumentation via `opentelemetry-otlp`, W3C context propagation, GenAI semantic conventions, hashed API-key auth.

It is **not yet production-grade**. Three classes of problem:

- **Protocol correctness.** What is labelled gRPC is HTTP/2 + raw protobuf at the gRPC method paths — no gRPC framing. Stock OTel SDKs using `grpc-tonic` will not interoperate. Hand-written protobuf structs drift from upstream `opentelemetry-proto`.
- **Reliability.** Ingest is synchronous per-row INSERTs in the request handler. Failures use `ON CONFLICT DO NOTHING` silently. No batching, no DLQ, no backpressure. Will collapse at realistic OTel collector volumes.
- **Promised-but-unenforced.** Schema columns for `rate_limit_rpm`, `max_payload_bytes`, and `otlp_ingest_audit` exist but the handlers don't enforce or populate them. Background workers for materialized-view refresh and alert evaluation are referenced in TODO but unimplemented.

The team's existing `TODO_OTEL_GAP_ANALYSIS.md` covers most operational gaps (background workers, sampling, compression). It misses the protocol-correctness and silent-failure issues — those are the ones that bite in production. This document consolidates both and adds dates.

---

## 2. Gap analysis

Legend: 🔴 critical · 🟠 high · 🟡 medium · 🟢 low

| # | Area | Current state | Gap | Severity | Closed by |
|---|---|---|---|---|---|
| O1 | gRPC transport | `crates/zentra-api/src/grpc.rs:179-205` mounts axum POST routes at gRPC method paths and reads body as raw protobuf | Not real gRPC framing — stock SDKs with `grpc-tonic` will not interoperate | 🔴 | OT01 |
| O2 | Ingest error handling | `ON CONFLICT DO NOTHING` silently drops failures (`handlers/otlp.rs:176,293,361,422`) | No DLQ, no retry, no audit of dropped data | 🔴 | OT02 |
| O3 | Ingest concurrency | Synchronous single-row INSERTs in the request handler | Will collapse at 1k–10k spans/sec | 🔴 | OT02 |
| O4 | Rate / payload limits | `otlp_api_keys` has `rate_limit_rpm` and `max_payload_bytes` columns | Not enforced anywhere | 🟠 | OT03 |
| O5 | Ingest audit | `otlp_ingest_audit` table queried by status endpoint | Never written to | 🟠 | OT04 |
| O6 | Migration hygiene | `migrations/.../20250101000068_rename_spans_to_traces.sql` renames a table whose CREATE lives in `deploy/sql/024_*.sql`, not migrations | Fresh `sqlx migrate` against clean DB fails | 🟠 | OT05 |
| O7 | Materialized view refresh | `span_red_metrics_1m`, `otel_derived_metrics` MVs defined, no refresher | RED metrics go stale | 🟠 | OT06 |
| O8 | Alert evaluation | `otel_alert_rules` table + evaluate endpoint exist; no background worker | Rules never fire | 🟠 | OT07 |
| O9 | Protobuf source | Hand-written prost structs mirroring OTel proto v1.3.0 (`otlp_proto.rs`, 555 LOC) | Drift from upstream as OTel proto evolves | 🟡 | OT01 |
| O10 | Receiver-side sampling | None — every span the SDK sends is stored | Scaling cliff when paired with O2/O3 | 🟡 | OT08 |
| O11 | Resource-attribute extraction | `extract_service_name` walks attribute array per-span (`otlp.rs:1340-1358`) | Should cache once per `resourceSpans` batch | 🟡 | OT09 |
| O12 | Span links / events | Unverified in ingest path | Linked operations and exception events may be silently dropped | 🟡 | OT11 |
| O13 | TimescaleDB compression | None on traces/metrics/logs hypertables | Storage cost balloons | 🟢 | OT10 |
| O14 | Licensing gate | Hardcoded prefix list — already tracked under licensing T14 | Closes via licensing plan, not this one | 🟢 | (licensing T14) |
| O15 | gRPC per-org throttling | Auth present, throttling absent | Same as O4 at gRPC layer | 🟡 | OT03 |
| O16 | Profiles signal stability | Schema targets OTel `v1experimental` path | Spec not yet GA — eventual rework | 🟢 | OT16 |
| O17 | gRPC auth cache | `otlp_key_cache` referenced; verify it's actually populated and TTL'd | DB hit per gRPC call | 🟡 | OT12 |
| O18 | Frontend gaps | Continuous-profiling explorer, OTel-native GenAI dashboard, resource discovery accuracy — tracked in existing OT-UI-* items | UI work | 🟡 | OT13, OT14, OT15 |
| O19 | Attribute-level search | `search_traces` does not query JSONB `attributes` | Trace UX gap | 🟡 | OT16 |

---

## 3. Phased timeline

Today is 2026-05-29. One engineer at ~80% capacity, 5-day weeks. Dates = target completion (end of day).

| Phase | Window | Theme | Tasks | Exit criteria |
|---|---|---|---|---|
| **A — Make it real** | 2026-06-01 → 2026-06-12 | Protocol correctness + async ingest | OT01–OT04 | Stock OTel collector with grpc-tonic transport ingests successfully; ingest survives 5k spans/sec sustained without dropping silently; rate limits enforced |
| **B — Foundations** | 2026-06-15 → 2026-06-19 | Migrations + background workers | OT05–OT07 | Fresh DB bootstraps from `migrations/` alone; RED MVs refresh every minute; alerts actually fire |
| **C — Correctness** | 2026-06-22 → 2026-07-03 | Sampling, perf, compression | OT08–OT11 | Receiver-side sampling configurable; per-batch resource caching; 7-day compression policy live |
| **D — UX & search** | 2026-07-06 → 2026-07-10 | Frontend + verification | OT12–OT16 | UI consumes new endpoints; attribute search; profiles signal flagged for GA tracking |
| **Buffer** | 2026-07-13 → 2026-07-17 | Contingency, hardening | — | Reserve for slippage, OTel SDK interop testing, load test |

---

## 4. Detailed prompts (OT01–OT16)

Every prompt is self-contained — handoff-ready. The "Done when" criteria are concrete checks an engineer can sign off against.

---

### OT01 — Fix gRPC: switch to `tonic-build` codegen + proper framing

**Target:** 2026-06-04 (Thu) · **Closes:** O1, O9 · **Severity:** 🔴

**Prompt.**
> The "gRPC" server in `crates/zentra-api/src/grpc.rs:179-205` is not real gRPC. It mounts axum POST routes at the gRPC method paths and reads the body as raw protobuf, but stock OTel SDKs using `opentelemetry-otlp` with `grpc-tonic` transport will not interoperate because they send gRPC-framed messages (compression flag + 4-byte length prefix + protobuf body) over HTTP/2 with `application/grpc` content type and a `grpc-status` trailer.
>
> Switch to `tonic`:
> - Add `tonic-build` to `crates/zentra-api/build.rs`. Generate Rust types from the upstream `opentelemetry-proto` repo (vendor under `crates/zentra-api/proto/opentelemetry/`). Generates `*ServiceServer` traits for trace, metrics, logs, profiles.
> - Implement each `*ServiceServer` trait on a new `OtlpGrpcService` struct that wraps `AppState`. Reuse the existing conversion helpers in `otlp_proto.rs` (or, better, delete `otlp_proto.rs` entirely — codegen replaces it, closing O9).
> - Run the tonic server on port 4317 in parallel with axum's HTTP server in `main.rs:225-232` (already feature-gated behind `feature = "grpc"`). Use `tonic::transport::Server::builder().add_service(...)`.
> - Preserve the API-key auth in `resolve_org_id_from_headers` — port to tonic's `tonic::Request::metadata()`.
> - Delete `otlp_proto.rs` and `grpc.rs`'s hand-rolled handlers once parity is reached.
>
> **Done when:** `opentelemetry-otlp = { features = ["grpc-tonic"] }` from a fresh Rust binary successfully exports 100 spans to `http://localhost:4317` and they appear in the `traces` table.

---

### OT02 — Async ingest pipeline with DLQ

**Target:** 2026-06-09 (Tue) · **Closes:** O2, O3 · **Severity:** 🔴

**Prompt.**
> Current OTLP HTTP handlers (`handlers/otlp.rs`, all four signals) decode the payload and then INSERT each span/metric/log row directly inside the request handler. Failures are swallowed by `ON CONFLICT DO NOTHING`. This is the single biggest scaling and reliability gap.
>
> Refactor:
> 1. Introduce a `OtlpIngestQueue` in `AppState` holding bounded `tokio::sync::mpsc::Sender<IngestBatch>` channels — one per signal.
> 2. Handlers do only: decode → validate → enqueue. Return 200 on enqueue, 503 if the channel is full (proper backpressure).
> 3. Spawn one worker per signal in `main.rs` that batches up to N items (or T ms) and issues a single multi-row INSERT.
> 4. On INSERT failure, retry with exponential backoff (3 attempts). On final failure, write the batch to a new `otlp_dead_letter` table with the error and the original payload bytes for forensics. Emit metric `zentra_otlp_dlq_total{signal=...}`.
> 5. Add a migration `2026MMDD_otlp_dead_letter.sql` for the DLQ table.
>
> **Done when:** a load test of 5k spans/sec for 5 minutes shows zero silent drops (every span either landed in `traces` or in `otlp_dead_letter`), and p99 ingest latency stays under 100ms.

---

### OT03 — Enforce rate_limit_rpm and max_payload_bytes

**Target:** 2026-06-11 (Thu) · **Closes:** O4, O15 · **Severity:** 🟠

**Prompt.**
> `otlp_api_keys` (`migrations/.../20250101000065_otlp_api_keys.sql`) defines `rate_limit_rpm` and `max_payload_bytes` per key but neither is enforced.
>
> Add a tower layer `OtlpQuota` that wraps the OTLP HTTP routes and the tonic gRPC service (after OT01):
> - Extract the API key, resolve to `(org_id, rate_limit_rpm, max_payload_bytes)` from the existing `otlp_key_cache`.
> - Reject body before deserialisation if `content-length` > `max_payload_bytes` → 413 Payload Too Large (HTTP) / `RESOURCE_EXHAUSTED` (gRPC).
> - Token-bucket rate limit per key using `governor` (already a dependency, see `main.rs:215`). On exceed → 429 / `RESOURCE_EXHAUSTED`.
> - Emit metrics: `zentra_otlp_rate_limited_total{key_id=...}`, `zentra_otlp_payload_oversize_total`.
>
> **Done when:** integration test confirms a key with `rate_limit_rpm=60` rejects the 61st request in a 60s window, and a 2MB payload to a `max_payload_bytes=1048576` key is rejected before parsing.

---

### OT04 — Populate `otlp_ingest_audit` on every ingest

**Target:** 2026-06-12 (Fri) · **Closes:** O5 · **Severity:** 🟠

**Prompt.**
> The status endpoint queries `otlp_ingest_audit` to report "what was ingested recently" but the ingest path never writes to it.
>
> In each signal worker (after OT02), emit one audit row per batch:
> ```sql
> INSERT INTO otlp_ingest_audit
>   (audit_id, org_id, signal, encoding, item_count, byte_size, source_ip, api_key_id, received_at, status)
> VALUES (...)
> ```
> `status` is one of `accepted`, `dlq`, `rate_limited`, `payload_too_large`. Keep this fire-and-forget (non-blocking) — the worker continues even if audit insert fails.
>
> **Done when:** every successful and failed ingest produces an audit row visible at `GET /api/otel/ingest-audit?limit=20`.

---

### OT05 — Migration hygiene: move traces base schema into `migrations/`

**Target:** 2026-06-16 (Tue) · **Closes:** O6 · **Severity:** 🟠

**Prompt.**
> `migrations/20250101000068_rename_spans_to_traces.sql` renames a table whose `CREATE TABLE` lives in `deploy/sql/024_standardize_spans_traces.sql`. A fresh `sqlx migrate run` against a clean database fails at the rename step because the source table doesn't exist yet.
>
> Find every base-table CREATE referenced by the OTel pipeline (`traces`, `metrics`, `logs`, `profiles`, supporting hypertables, materialized views) currently living under `deploy/sql/`. Re-author them as new sequential migrations under `migrations/`, dated *before* `20250101000068`. Then either:
> - Renumber `20250101000068` so it follows the new base CREATEs, or
> - Make it idempotent (`ALTER TABLE IF EXISTS spans RENAME TO traces;`).
>
> Test: drop the local Postgres DB, recreate it, run `sqlx migrate run` from scratch — must succeed end-to-end with no `deploy/sql/` involvement.

---

### OT06 — Materialized view refresh worker (existing OT-BKG-01)

**Target:** 2026-06-17 (Wed) · **Closes:** O7 · **Severity:** 🟠

**Prompt.**
> Spawn a background tokio task in `crates/zentra-api/src/main.rs` (alongside the gRPC spawn at line 228) that every 60s runs:
> ```sql
> REFRESH MATERIALIZED VIEW CONCURRENTLY span_red_metrics_1m;
> REFRESH MATERIALIZED VIEW CONCURRENTLY otel_derived_metrics;
> ```
> Make the interval configurable via `ZENTRA_OTEL_MV_REFRESH_SECS` (default 60). Log warnings if a refresh takes > 30s; emit metric `zentra_otel_mv_refresh_duration_seconds`. On failure, log and continue (do not crash the worker).
>
> **Done when:** `pg_stat_user_tables` shows `last_analyze` advancing every minute and the RED metrics dashboard data is at most ~60s stale.

---

### OT07 — Alert evaluation worker (existing OT-BKG-02)

**Target:** 2026-06-19 (Fri) · **Closes:** O8 · **Severity:** 🟠

**Prompt.**
> `otel_alert_rules` table exists. `POST /api/otel/alert-rules/{id}/evaluate` exists. Nothing schedules evaluations.
>
> Add a worker that every 60s (configurable via `ZENTRA_OTEL_ALERT_EVAL_SECS`) selects all rows where `enabled = true`, runs each rule's `otel_query` against the appropriate signal, and on threshold breach forwards to the existing notifications module. Persist the last-firing state in a new column `last_fired_at` and `last_status` (use a small migration) so flapping can be detected.
>
> **Done when:** a test rule "spans where http.status_code >= 500 in last 5m > 10" fires within 60s of conditions being met and sends a Slack notification through the existing notifications path.

---

### OT08 — Receiver-side sampling

**Target:** 2026-06-24 (Wed) · **Closes:** O10 · **Severity:** 🟡

**Prompt.**
> Add configurable head sampling at the OTLP receiver (post-OT02 pipeline). Per `otlp_api_keys`, add a `sample_ratio FLOAT8` column (default 1.0). In the ingest worker, hash `trace_id` to a uniform [0,1] and drop spans where the value exceeds the key's `sample_ratio`. Whole traces are kept or dropped together (consistent sampling).
>
> Expose: `GET /api/otel/sampling/preview?ratio=0.1` returning expected volume reduction based on the last hour's ingest.
>
> Document trade-off in `docs/otel/SAMPLING.md`: probabilistic sampling preserves throughput stats but distorts rare-event detection — tail sampling is a future improvement.

---

### OT09 — Per-batch resource attribute caching

**Target:** 2026-06-26 (Fri) · **Closes:** O11 · **Severity:** 🟡

**Prompt.**
> `handlers/otlp.rs:1340-1358` walks the OTLP attribute array per-span to extract `service.name`. Resource attrs are constant for a whole `resourceSpans` batch — extract once.
>
> Refactor the ingest loop to parse resource attrs (service.name, service.version, service.namespace, deployment.environment, host.name) once per `resourceSpans`, then attach the cached tuple to every span in that batch. Measure CPU saving with a 1000-span payload before/after.

---

### OT10 — TimescaleDB compression policies (existing OT-PERF-01)

**Target:** 2026-07-01 (Wed) · **Closes:** O13 · **Severity:** 🟢

**Prompt.**
> Add migrations enabling compression on `traces`, `metrics`, `logs`, `profiles` hypertables for chunks older than 7 days:
> ```sql
> ALTER TABLE traces SET (
>   timescaledb.compress,
>   timescaledb.compress_segmentby = 'service_name, org_id'
> );
> SELECT add_compression_policy('traces', INTERVAL '7 days');
> ```
> Validate `compress_segmentby` choice by querying access patterns — `service_name + org_id` is the working assumption. Repeat for the other three tables with appropriate segment-by keys.
>
> **Done when:** `SELECT hypertable_name, compression_status FROM timescaledb_information.hypertables` shows all four enabled and a chunk older than 7 days is observed compressed.

---

### OT11 — Verify span links and span events ingest

**Target:** 2026-07-03 (Fri) · **Closes:** O12 · **Severity:** 🟡

**Prompt.**
> OTLP spans carry `links` (array of linked spans) and `events` (array of timestamped events including exceptions). Confirm via test whether `handlers/otlp.rs` ingest path persists these or silently drops them.
>
> If dropped:
> - Add `links JSONB`, `events JSONB` columns to `traces` (new migration).
> - Populate from the OTLP message in the ingest worker.
> - Expose in trace detail API responses.
>
> If already persisted, write a regression test that asserts both fields round-trip a known payload.

---

### OT12 — gRPC API-key cache verification (existing OT-SEC-01)

**Target:** 2026-07-06 (Mon) · **Closes:** O17 · **Severity:** 🟡

**Prompt.**
> `grpc.rs:46` calls `validate_otlp_key` which the comment claims uses a cache (`otlp_key_cache`). Verify:
> - The cache is actually populated (not a stub).
> - It has a TTL (recommend 60s).
> - It is invalidated on `DELETE /api/otlp-keys/{id}` so revocation propagates within TTL.
>
> If any of the three is missing, implement with `dashmap::DashMap<String, (Uuid, Vec<String>, Instant)>`. Add metric `zentra_otlp_key_cache_hit_total` and `..._miss_total`.

---

### OT13 — Continuous Profiling Explorer UI (existing OT-UI-01)

**Target:** 2026-07-07 (Tue) · **Closes:** O18 (part) · **Severity:** 🟡

**Prompt.**
> Create `frontend/src/pages/Profiling.tsx` consuming the `/v1/profiles` ingest data. Integrate `react-flame-graph` (or similar). Filter by `service.name`, time range, profile type (CPU / heap). Link from the main service detail page when profiles are available for that service.

---

### OT14 — OTel-native GenAI dashboard (existing OT-AI-01)

**Target:** 2026-07-08 (Wed) · **Closes:** O18 (part) · **Severity:** 🟡

**Prompt.**
> `LLMObservability.tsx` currently uses the legacy `/llm/*` endpoints. Switch to `/api/otel/genai` which aggregates from spans matching `gen_ai.*` semantic conventions. Deprecate the legacy endpoints (add `Sunset` header pointing at the cutover date). Confirm metrics rendered match within 1% of the legacy view.

---

### OT15 — Resource discovery signal accuracy (existing OT-UI-02)

**Target:** 2026-07-09 (Thu) · **Closes:** O18 (part) · **Severity:** 🟡

**Prompt.**
> `GET /api/otel/resources` currently returns boolean flags for which signals (T/M/L/P) a resource is sending. Verify the booleans reflect ground truth by querying each signal table per resource within the last 15 minutes. Wire to dashboard summary cards.

---

### OT16 — Attribute-level search in traces (existing OT-ADV-01) + profiles GA tracking

**Target:** 2026-07-10 (Fri) · **Closes:** O19, O16 · **Severity:** 🟡

**Prompt.**
> Extend `search_traces` to accept JSONB queries on the `attributes` column. Example: `GET /api/otel/traces?attr.http.method=POST&attr.http.status_code=500`. Add a GIN index on `attributes` if not already present (migration).
>
> Separately, add a tracking item / docs note that the `profiles` signal currently targets OTel `v1experimental` paths. Subscribe to OTel proto release notes; revisit when profiles signal goes GA (re-evaluate the schema at that point).

---

## 5. Out of scope (parked)

- **Tail-based sampling.** Complex; revisit after head sampling (OT08) is bedded in.
- **Multi-region OTLP ingest.** Single-region for now.
- **OTel logs full-text indexing.** Today logs ingest is structural; full-text search needs a separate decision (e.g., add a column with `tsvector` or move to a search backend).
- **Native OTLP/Arrow.** Not on the SDK side yet; revisit 2027.

---

## 6. Sequencing rules

- **OT01 unblocks OT02's gRPC ingest path** — but OT02 can start on HTTP first, then absorb gRPC after OT01 lands.
- **OT02 → OT03, OT04, OT08:** quota, audit, and sampling all hook into the new pipeline.
- **OT05 must land before any new migration in this plan** (OT02 DLQ, OT07 alert state, OT10 compression, OT11 links/events, OT16 GIN index).
- **OT06 / OT07 are independent** and can run in parallel.
- **OT13–OT15** depend only on existing endpoints; UI work can begin anytime.

---

## 7. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Existing customers already point collectors at the fake gRPC endpoint and rely on its behaviour | Unknown — confirm before OT01 | High | Keep the legacy HTTP-over-gRPC-paths handler under a deprecation flag for one release; warn in logs |
| `tonic-build` pulls in heavy build-time deps | Low | Low | Already have `prost` and `tonic` in `Cargo.toml:54-57` — incremental cost is small |
| DLQ table grows unbounded | Medium | Medium | Add retention policy (drop after 30 days) in OT02 migration |
| MV refresh worker holds locks under load | Medium | Medium | `CONCURRENTLY` already specified; monitor `pg_stat_activity` for lock waits |
| Compression policy breaks existing queries | Low | Medium | TimescaleDB allows transparent reads of compressed chunks; run query suite post-OT10 to confirm |

---

## 8. Tracking

- **Update cadence:** weekly, end-of-Friday.
- **PR titles:** must reference `OT##` for the task they close.
- **Sign-off gates:** Phase A exit (OT01-OT04) requires load test sign-off (5k spans/sec for 10 min, zero silent drops). Phase B exit (OT05-OT07) requires fresh-DB bootstrap test.

### Status board

| Task | Target | Closes | Status |
|---|---|---|---|
| OT01 tonic-build gRPC | 2026-06-04 | O1, O9 | ☐ |
| OT02 async ingest + DLQ | 2026-06-09 | O2, O3 | ☐ |
| OT03 quota enforcement | 2026-06-11 | O4, O15 | ☐ |
| OT04 ingest audit writes | 2026-06-12 | O5 | ☐ |
| OT05 migration hygiene | 2026-06-16 | O6 | ☐ |
| OT06 MV refresh worker | 2026-06-17 | O7 | ☐ |
| OT07 alert evaluator | 2026-06-19 | O8 | ☐ |
| OT08 receiver sampling | 2026-06-24 | O10 | ☐ |
| OT09 per-batch resource cache | 2026-06-26 | O11 | ☐ |
| OT10 Timescale compression | 2026-07-01 | O13 | ☐ |
| OT11 verify span links/events | 2026-07-03 | O12 | ☐ |
| OT12 gRPC key cache verify | 2026-07-06 | O17 | ☐ |
| OT13 profiling explorer UI | 2026-07-07 | O18 | ☐ |
| OT14 GenAI dashboard cutover | 2026-07-08 | O18 | ☐ |
| OT15 resource discovery accuracy | 2026-07-09 | O18 | ☐ |
| OT16 attribute search + profiles GA tracker | 2026-07-10 | O19, O16 | ☐ |

Status legend: ☐ not started · ◐ in progress · ✅ done · ⛔ blocked

---

## 9. Verdict

The OTel surface is more thoughtfully implemented than the licensing system was: real semantic conventions, working self-instrumentation, four signals. But it is a **demo-grade receiver**, not a production OTel ingestion service.

The highest-leverage fix is **OT01 + OT02 + OT03 together** — they turn the ingest from a toy into something that can sit behind a customer-facing collector endpoint. After that, the team's existing OTel TODO items (background workers, compression, batch inserts) close most remaining gaps.

End state by 2026-07-10: a tonic-based gRPC server interoperable with stock OTel SDKs, an async batched ingest pipeline with DLQ and enforced quotas, populated audit trail, self-contained migrations, working RED metric refresh and alert evaluation, head sampling, compression policy, and a frontend that consumes the new GenAI and profiling endpoints.
