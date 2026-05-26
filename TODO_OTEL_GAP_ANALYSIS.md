# Zentra OTel Roadmap: Post-Implementation Gaps

This document tracks the missing pieces and recommended improvements for the OpenTelemetry module in Zentra, following the initial feature rollout in May 2026.

## Phase 1: Security & Core Reliability (Critical)
- [ ] **OT-SEC-01: gRPC Auth Validation**
  - Implement actual DB lookup for `isok_` API keys in `grpc.rs`.
  - Cache active keys in-memory (e.g., `DashMap`) to avoid DB hits on every gRPC call.
- [ ] **OT-BKG-01: Materialized View Refresh Worker**
  - Create a background task in `main.rs` that runs every 1 minute.
  - Command: `REFRESH MATERIALIZED VIEW CONCURRENTLY span_red_metrics_1m`.
- [ ] **OT-BKG-02: Alert Evaluation Engine**
  - Implement a background worker that evaluates all `enabled = true` rules in `otel_alert_rules` every 1-5 minutes.
  - Integrate with the existing `notifications` module to send Slack/Email/PagerDuty alerts.

## Phase 2: Feature Parity & UX (High)
- [ ] **OT-UI-01: Continuous Profiling Explorer**
  - Create `src/pages/Profiling.tsx`.
  - Integrate a Flamegraph component (e.g., `react-flame-graph`).
  - Connect to `ingest_profiles` data points.
- [ ] **OT-AI-01: OTel-Native GenAI Dashboard**
  - Update `LLMObservability.tsx` to use `/api/otel/genai` which aggregates standard `gen_ai.*` spans.
  - Deprecate legacy `/llm/*` endpoints if they overlap.
- [ ] **OT-UI-02: Resource Discovery Signal Accuracy**
  - Update `otel_resources` backend to return actual boolean flags for which signals (T, M, L, P) a resource is sending.
  - Wire frontend summary cards to these dynamic counts.

## Phase 3: Advanced Capabilities (Medium)
- [ ] **OT-ADV-01: Attribute-Level Search in Traces**
  - Enhance `search_traces` to allow JSONB queries on the `attributes` column.
- [ ] **OT-ADV-02: Semantic Convention Catalog**
  - Implement `GET /api/otel/semconv` returning a structured list of OTel attributes (HTTP, DB, RPC, etc.).
- [ ] **OT-ADV-03: Sampling Support**
  - Add support for `traceparent` interpretation and head-based sampling headers to reduce ingest volume for high-traffic users.

## Phase 4: Performance & Scaling (Low)
- [ ] **OT-PERF-01: TimescaleDB Compression**
  - Enable compression on `traces`, `metrics`, and `logs` hypertables for data older than 7 days.
- [ ] **OT-PERF-02: Batch Ingest Optimizations**
  - Refactor `INSERT` statements to use multi-row batches for higher throughput during bursts.
