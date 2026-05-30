# OpenTelemetry Conformance Test Matrix (O22)

This matrix documents the expected behavior for every combination of
**pillar × transport × encoding** that Zentra accepts.

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Implemented and expected to pass |
| 🔧 | Requires `otlp-proto` / `grpc` feature flag |

---

## Signal × Transport × Encoding Matrix

| Signal   | Transport | Encoding   | Endpoint                                                                 | Status |
|----------|-----------|------------|--------------------------------------------------------------------------|--------|
| Traces   | HTTP      | JSON       | `POST /v1/traces` Content-Type: application/json                         | ✅     |
| Traces   | HTTP      | Protobuf   | `POST /v1/traces` Content-Type: application/x-protobuf                   | 🔧     |
| Traces   | HTTP      | JSON+gzip  | `POST /v1/traces` Content-Encoding: gzip                                 | ✅     |
| Traces   | HTTP      | Proto+gzip | `POST /v1/traces` Content-Encoding: gzip, Content-Type: x-protobuf      | 🔧     |
| Traces   | gRPC      | Protobuf   | `TraceService/Export` on port 4317                                       | 🔧     |
| Metrics  | HTTP      | JSON       | `POST /v1/metrics` Content-Type: application/json                        | ✅     |
| Metrics  | HTTP      | Protobuf   | `POST /v1/metrics` Content-Type: application/x-protobuf                  | 🔧     |
| Metrics  | HTTP      | JSON+gzip  | `POST /v1/metrics` Content-Encoding: gzip                                | ✅     |
| Metrics  | HTTP      | Proto+gzip | `POST /v1/metrics` Content-Encoding: gzip, Content-Type: x-protobuf     | 🔧     |
| Metrics  | gRPC      | Protobuf   | `MetricsService/Export` on port 4317                                     | 🔧     |
| Logs     | HTTP      | JSON       | `POST /v1/logs` Content-Type: application/json                           | ✅     |
| Logs     | HTTP      | Protobuf   | `POST /v1/logs` Content-Type: application/x-protobuf                     | 🔧     |
| Logs     | HTTP      | JSON+gzip  | `POST /v1/logs` Content-Encoding: gzip                                   | ✅     |
| Logs     | HTTP      | Proto+gzip | `POST /v1/logs` Content-Encoding: gzip, Content-Type: x-protobuf        | 🔧     |
| Logs     | gRPC      | Protobuf   | `LogsService/Export` on port 4317                                        | 🔧     |
| Profiles | HTTP      | JSON       | `POST /v1/profiles` Content-Type: application/json                       | ✅     |
| Profiles | HTTP      | Protobuf   | `POST /v1/profiles` Content-Type: application/x-protobuf                 | 🔧     |
| Profiles | HTTP      | JSON+gzip  | `POST /v1/profiles` Content-Encoding: gzip                               | ✅     |
| Profiles | HTTP      | Proto+gzip | `POST /v1/profiles` Content-Encoding: gzip, Content-Type: x-protobuf    | 🔧     |
| Profiles | gRPC      | Protobuf   | `ProfilesService/Export` on port 4317                                    | 🔧     |

**Total cells: 20** (4 signals × 5 transport/encoding combos)

---

## Metric Sub-Types

| Metric Type            | Ingest Support | Storage Format          |
|------------------------|----------------|------------------------|
| Gauge                  | ✅             | Single value per point  |
| Sum (monotonic)        | ✅             | Single value per point  |
| Sum (non-monotonic)    | ✅             | Single value per point  |
| Histogram              | ✅             | _count, _sum, _bucket   |
| Exponential Histogram  | ✅             | _count, _sum            |
| Summary                | ✅             | _count, _sum, quantiles |

---

## Auth Methods

| Method            | Supported | Notes                              |
|-------------------|-----------|-------------------------------------|
| Bearer API Key    | ✅        | `isok_*` prefix, SHA-256 hashed     |
| JWT               | ✅        | Standard org-scoped JWT             |
| X-Org-Id header   | ✅        | For internal/dev use                |

---

## Response Conformance (O20)

| Scenario             | HTTP Status | Response Body                          |
|----------------------|-------------|----------------------------------------|
| Success              | 200         | `{"partialSuccess": null}`             |
| Bad encoding         | 400         | `{"error": "..."}`                     |
| Queue full           | 503         | `{"error": "Ingest queue full..."}`    |
| Auth failure (HTTP)  | 401         | `{"error": "..."}`                     |
| Auth failure (gRPC)  | gRPC 16     | UNAUTHENTICATED                        |
| Quota exceeded       | 429         | Rate limit exceeded                    |

---

## Feature Flags

| Feature      | Cargo Flag    | Enables                                  |
|--------------|---------------|-------------------------------------------|
| otlp-proto   | `otlp-proto`  | Protobuf decode (prost) for all signals   |
| grpc         | `grpc`        | gRPC server on port 4317 (implies proto)  |

Both are enabled by default (`default = ["otlp-proto", "grpc"]`).

---

## CI Test Commands (curl-based)

```bash
# JSON traces
curl -X POST http://localhost:5006/v1/traces \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer isok_TEST_KEY" \
  -d '{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"test"}}]},"scopeSpans":[{"spans":[{"traceId":"abc123","spanId":"def456","name":"test-span","startTimeUnixNano":"1000000000","endTimeUnixNano":"2000000000","status":{"code":0}}]}]}]}'

# JSON metrics
curl -X POST http://localhost:5006/v1/metrics \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer isok_TEST_KEY" \
  -d '{"resourceMetrics":[{"resource":{"attributes":[]},"scopeMetrics":[{"metrics":[{"name":"test.counter","sum":{"dataPoints":[{"asDouble":42.0,"timeUnixNano":"1000000000"}]}}]}]}]}'

# JSON logs
curl -X POST http://localhost:5006/v1/logs \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer isok_TEST_KEY" \
  -d '{"resourceLogs":[{"resource":{"attributes":[]},"scopeLogs":[{"logRecords":[{"timeUnixNano":"1000000000","severityText":"INFO","body":{"stringValue":"test log"},"traceId":"abc123","spanId":"def456"}]}]}]}'

# Gzip traces
echo '{"resourceSpans":[]}' | gzip | curl -X POST http://localhost:5006/v1/traces \
  -H "Content-Type: application/json" \
  -H "Content-Encoding: gzip" \
  -H "Authorization: Bearer isok_TEST_KEY" \
  --data-binary @-

# Verify trace stored with org_id
psql -c "SELECT count(*) FROM traces WHERE org_id IS NOT NULL AND trace_id = 'abc123'"

# Verify log has trace correlation
psql -c "SELECT metadata->>'trace_id' FROM logs WHERE metadata->>'trace_id' = 'abc123'"
```
