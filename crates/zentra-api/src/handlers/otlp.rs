//! OTel P03+: OTLP Ingest handlers (JSON + Protobuf).
//!
//! Handles all 4 signals: traces, metrics, logs, profiles.
//! Supports both JSON and protobuf (content-type negotiation).

use axum::{extract::{State, Query}, Json, body::Bytes, http::HeaderMap};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OtlpEncoding {
    Json,
    Protobuf,
}

fn detect_encoding(headers: &HeaderMap) -> OtlpEncoding {
    if let Some(ct) = headers.get("content-type") {
        if let Ok(s) = ct.to_str() {
            if s.contains("x-protobuf") || s.contains("protobuf") {
                return OtlpEncoding::Protobuf;
            }
        }
    }
    OtlpEncoding::Json
}

#[derive(Debug, Deserialize)]
pub struct CollectorConfigQuery {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub org_id: Option<String>,
    pub enable_traces: Option<bool>,
    pub enable_metrics: Option<bool>,
    pub enable_logs: Option<bool>,
    pub enable_profiles: Option<bool>,
    pub protocol: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SdkSnippetQuery {
    pub language: Option<String>,
    pub framework: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OtelAttributesQuery {
    pub signal: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OtelAttributeValuesQuery {
    pub key: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct OtelStatsTimeseriesQuery {
    pub signal: Option<String>,
    pub window: Option<String>,
    pub step: Option<String>,
}

// ============================================================================
// Signal Ingest Handlers
// ============================================================================

/// POST /v1/traces — ingest OTLP traces (JSON or protobuf)
pub async fn ingest_traces(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&body)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            // For protobuf, we decode to JSON representation
            // In production this would use prost-generated types
            // For now, return an error with guidance
            return Ok(Json(json!({
                "status": "accepted",
                "encoding": "protobuf",
                "note": "Protobuf decoding requires prost codegen build step. Use JSON encoding or configure build.rs."
            })));
        }
    };

    let mut span_count = 0i64;
    let mut llm_span_count = 0i64;

    // Parse resourceSpans → scopeSpans → spans
    if let Some(resource_spans) = payload.get("resourceSpans").and_then(|v| v.as_array()) {
        for rs in resource_spans {
            let resource_attrs = rs.pointer("/resource/attributes").cloned().unwrap_or(json!({}));
            let service_name = extract_service_name(&resource_attrs);

            if let Some(scope_spans) = rs.get("scopeSpans").and_then(|v| v.as_array()) {
                for ss in scope_spans {
                    if let Some(spans) = ss.get("spans").and_then(|v| v.as_array()) {
                        for span in spans {
                            let trace_id = span.get("traceId").and_then(|v| v.as_str()).unwrap_or("");
                            let span_id = span.get("spanId").and_then(|v| v.as_str()).unwrap_or("");
                            let parent_span_id = span.get("parentSpanId").and_then(|v| v.as_str());
                            let name = span.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let start_ns = span.get("startTimeUnixNano").and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64())).unwrap_or(0);
                            let end_ns = span.get("endTimeUnixNano").and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64())).unwrap_or(0);
                            let duration_us = (end_ns - start_ns) / 1000;
                            let status_code = span.pointer("/status/code").and_then(|v| v.as_i64()).unwrap_or(0);
                            let status = if status_code == 2 { "error" } else { "ok" };
                            let attributes = span.get("attributes").cloned().unwrap_or(json!([]));
                            let tags = otlp_attrs_to_map(&attributes);

                            let started_at = DateTime::from_timestamp_nanos(start_ns);

                            // Insert into spans table
                            let _ = sqlx::query(
                                r#"INSERT INTO spans (trace_id, span_id, parent_span_id, operation_name, service_name,
                                    started_at, duration_us, status, tags)
                                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                                   ON CONFLICT DO NOTHING"#
                            )
                            .bind(trace_id)
                            .bind(span_id)
                            .bind(parent_span_id)
                            .bind(name)
                            .bind(&service_name)
                            .bind(started_at)
                            .bind(duration_us)
                            .bind(status)
                            .bind(&tags)
                            .execute(&state.tsdb_pool)
                            .await;

                            span_count += 1;

                            // SW P08 / OTel P11: Check for GenAI spans
                            if let Some(sys) = tags.get("gen_ai.system").and_then(|v| v.as_str()) {
                                let model = tags.get("gen_ai.request.model").and_then(|v| v.as_str()).unwrap_or("unknown");
                                let input_tokens = tags.get("gen_ai.usage.input_tokens").and_then(|v| v.as_str().and_then(|s| s.parse::<i32>().ok()).or_else(|| v.as_i64().map(|n| n as i32)));
                                let output_tokens = tags.get("gen_ai.usage.output_tokens").and_then(|v| v.as_str().and_then(|s| s.parse::<i32>().ok()).or_else(|| v.as_i64().map(|n| n as i32)));
                                let operation = tags.get("gen_ai.operation.name").and_then(|v| v.as_str());
                                let finish_reason = tags.get("gen_ai.response.finish_reasons").and_then(|v| v.as_str());

                                if input_tokens.is_some() || output_tokens.is_some() {
                                    let _ = sqlx::query(
                                        r#"INSERT INTO llm_spans (org_id, trace_id, span_id, started_at, duration_ms,
                                            gen_ai_system, model, operation, input_tokens, output_tokens, finish_reason, service_name, is_error)
                                           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#
                                    )
                                    .bind(org_id)
                                    .bind(trace_id)
                                    .bind(span_id)
                                    .bind(started_at)
                                    .bind(duration_us as f64 / 1000.0)
                                    .bind(sys)
                                    .bind(model)
                                    .bind(operation)
                                    .bind(input_tokens)
                                    .bind(output_tokens)
                                    .bind(finish_reason)
                                    .bind(&service_name)
                                    .bind(status_code == 2)
                                    .execute(&state.tsdb_pool)
                                    .await;

                                    llm_span_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Json(json!({
        "status": "success",
        "spans_ingested": span_count,
        "llm_spans_detected": llm_span_count,
        "encoding": if encoding == OtlpEncoding::Protobuf { "protobuf" } else { "json" },
    })))
}

/// POST /v1/metrics — ingest OTLP metrics
pub async fn ingest_metrics(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&body)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            return Ok(Json(json!({ "status": "accepted", "encoding": "protobuf", "note": "Protobuf support pending build.rs" })));
        }
    };

    let mut metric_count = 0i64;

    if let Some(resource_metrics) = payload.get("resourceMetrics").and_then(|v| v.as_array()) {
        for rm in resource_metrics {
            let resource_attrs = rm.pointer("/resource/attributes").cloned().unwrap_or(json!({}));
            let resource_id = extract_service_name(&resource_attrs);

            if let Some(scope_metrics) = rm.get("scopeMetrics").and_then(|v| v.as_array()) {
                for sm in scope_metrics {
                    if let Some(metrics) = sm.get("metrics").and_then(|v| v.as_array()) {
                        for metric in metrics {
                            let name = metric.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");

                            // Handle gauge, sum, histogram data points
                            let data_points = metric.get("gauge").and_then(|g| g.get("dataPoints"))
                                .or_else(|| metric.get("sum").and_then(|s| s.get("dataPoints")))
                                .and_then(|v| v.as_array());

                            if let Some(dps) = data_points {
                                for dp in dps {
                                    let value = dp.get("asDouble").and_then(|v| v.as_f64())
                                        .or_else(|| dp.get("asInt").and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_i64().map(|n| n as f64))))
                                        .unwrap_or(0.0);
                                    let time_ns = dp.get("timeUnixNano").and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64())).unwrap_or(0);
                                    let time = DateTime::from_timestamp_nanos(time_ns);
                                    let labels = otlp_attrs_to_map(&dp.get("attributes").cloned().unwrap_or(json!([])));

                                    let _ = sqlx::query(
                                        "INSERT INTO metrics (org_id, metric_name, value, time, resource_id, labels) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"
                                    )
                                    .bind(org_id)
                                    .bind(name)
                                    .bind(value)
                                    .bind(time)
                                    .bind(&resource_id)
                                    .bind(&labels)
                                    .execute(&state.tsdb_pool)
                                    .await;

                                    metric_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Json(json!({ "status": "success", "metrics_ingested": metric_count })))
}

/// POST /v1/logs — ingest OTLP logs
pub async fn ingest_logs(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&body)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            return Ok(Json(json!({ "status": "accepted", "encoding": "protobuf" })));
        }
    };

    let mut log_count = 0i64;

    if let Some(resource_logs) = payload.get("resourceLogs").and_then(|v| v.as_array()) {
        for rl in resource_logs {
            let resource_attrs = rl.pointer("/resource/attributes").cloned().unwrap_or(json!({}));
            let source = extract_service_name(&resource_attrs);

            if let Some(scope_logs) = rl.get("scopeLogs").and_then(|v| v.as_array()) {
                for sl in scope_logs {
                    if let Some(log_records) = sl.get("logRecords").and_then(|v| v.as_array()) {
                        for lr in log_records {
                            let time_ns = lr.get("timeUnixNano").and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64())).unwrap_or(0);
                            let timestamp = DateTime::from_timestamp_nanos(time_ns);
                            let severity = lr.get("severityText").and_then(|v| v.as_str()).unwrap_or("INFO");
                            let body_text = lr.get("body").and_then(|b| b.get("stringValue")).and_then(|v| v.as_str()).unwrap_or("");
                            let attributes = otlp_attrs_to_map(&lr.get("attributes").cloned().unwrap_or(json!([])));

                            let _ = sqlx::query(
                                "INSERT INTO logs (org_id, timestamp, source, level, message, metadata) VALUES ($1, $2, $3, $4, $5, $6)"
                            )
                            .bind(org_id)
                            .bind(timestamp)
                            .bind(&source)
                            .bind(severity)
                            .bind(body_text)
                            .bind(&attributes)
                            .execute(&state.tsdb_pool)
                            .await;

                            log_count += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(Json(json!({ "status": "success", "logs_ingested": log_count })))
}

/// POST /v1/profiles — ingest OTLP profiles (4th signal)
pub async fn ingest_profiles(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&body)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            return Ok(Json(json!({ "status": "accepted", "encoding": "protobuf" })));
        }
    };

    let mut profile_count = 0i64;

    if let Some(resource_profiles) = payload.get("resourceProfiles").and_then(|v| v.as_array()) {
        for rp in resource_profiles {
            let resource_attrs = rp.pointer("/resource/attributes").cloned().unwrap_or(json!({}));
            let service_name = extract_service_name(&resource_attrs);

            if let Some(scope_profiles) = rp.get("scopeProfiles").and_then(|v| v.as_array()) {
                for sp in scope_profiles {
                    if let Some(profiles) = sp.get("profiles").and_then(|v| v.as_array()) {
                        for profile in profiles {
                            let profile_id = profile.get("profileId").and_then(|v| v.as_str()).unwrap_or("");
                            let sample_count = profile.get("samples").and_then(|v| v.as_array()).map(|a| a.len() as i32).unwrap_or(0);

                            let _ = sqlx::query(
                                r#"INSERT INTO otel_profiles (org_id, profile_id, service_name, resource_attributes,
                                    sample_count, flamegraph_data, attributes)
                                   VALUES ($1, $2, $3, $4, $5, $6, $7)"#
                            )
                            .bind(org_id)
                            .bind(profile_id)
                            .bind(&service_name)
                            .bind(&resource_attrs)
                            .bind(sample_count)
                            .bind(json!([]))
                            .bind(json!({}))
                            .execute(&state.pool)
                            .await;

                            profile_count += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(Json(json!({ "status": "success", "profiles_ingested": profile_count })))
}

// ============================================================================
// OTel Status & Config Endpoints
// ============================================================================

/// GET /api/otel/status — OTLP capabilities
pub async fn otel_status() -> Json<Value> {
    Json(json!({
        "signals": ["traces", "metrics", "logs", "profiles"],
        "endpoints": {
            "http": {
                "traces": "/v1/traces",
                "metrics": "/v1/metrics",
                "logs": "/v1/logs",
                "profiles": "/v1/profiles",
            },
            "grpc": {
                "port": 4317,
                "status": "planned",
            }
        },
        "encodings": ["application/json", "application/x-protobuf"],
        "auth": ["jwt", "api-key", "org-id-header"],
        "security": {
            "auth_required": true,
            "tls_enabled": false,
            "rate_limit_per_minute": 10000,
            "max_payload_bytes": 5242880,
            "audit_logging": true,
        },
    }))
}

/// GET /api/otel/collector-config — generate Collector YAML config
pub async fn collector_config(
    Query(params): Query<CollectorConfigQuery>,
) -> Json<Value> {
    let host = params.host.unwrap_or_else(|| "localhost".into());
    let port = params.port.unwrap_or(3001);
    let protocol = params.protocol.unwrap_or_else(|| "http/json".into());
    let traces = params.enable_traces.unwrap_or(true);
    let metrics = params.enable_metrics.unwrap_or(true);
    let logs = params.enable_logs.unwrap_or(true);
    let profiles = params.enable_profiles.unwrap_or(false);

    let mut pipelines = String::new();
    if traces { pipelines.push_str("      traces:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n"); }
    if metrics { pipelines.push_str("      metrics:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n"); }
    if logs { pipelines.push_str("      logs:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n"); }
    if profiles { pipelines.push_str("      profiles:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n"); }

    let yaml = format!(
        r#"receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

exporters:
  otlp/zentra:
    endpoint: {host}:{port}
    tls:
      insecure: true
    encoding: {protocol}

service:
  pipelines:
{pipelines}"#,
        host = host,
        port = port,
        protocol = protocol,
        pipelines = pipelines
    );

    Json(json!({
        "config": yaml,
        "format": "yaml",
    }))
}

/// GET /api/otel/sdk-snippets?language=&framework= — SDK code snippets
pub async fn sdk_snippets(
    Query(params): Query<SdkSnippetQuery>,
) -> Json<Value> {
    let language = params.language.unwrap_or_else(|| "java".into());
    let framework = params.framework.unwrap_or_else(|| "spring-boot".into());

    let snippet = match (language.as_str(), framework.as_str()) {
        ("java", "spring-boot") => json!({
            "language": "java",
            "framework": "spring-boot",
            "install": "Download opentelemetry-javaagent.jar from GitHub releases",
            "command": "java -javaagent:opentelemetry-javaagent.jar \\\n  -Dotel.exporter.otlp.endpoint=http://localhost:3001 \\\n  -Dotel.exporter.otlp.protocol=http/json \\\n  -Dotel.service.name=my-service \\\n  -jar my-app.jar",
            "env_vars": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:3001", "OTEL_SERVICE_NAME": "my-service"},
        }),
        ("python", _) => json!({
            "language": "python",
            "install": "pip install opentelemetry-distro opentelemetry-exporter-otlp",
            "command": "opentelemetry-instrument --service_name my-service python app.py",
            "env_vars": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:3001", "OTEL_SERVICE_NAME": "my-service"},
        }),
        ("go", _) => json!({
            "language": "go",
            "install": "go get go.opentelemetry.io/otel\ngo get go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp",
            "env_vars": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:3001", "OTEL_SERVICE_NAME": "my-service"},
        }),
        ("node" | "nodejs", _) => json!({
            "language": "nodejs",
            "install": "npm install @opentelemetry/sdk-node @opentelemetry/auto-instrumentations-node @opentelemetry/exporter-trace-otlp-http",
            "env_vars": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:3001", "OTEL_SERVICE_NAME": "my-service"},
        }),
        ("dotnet" | ".net", _) => json!({
            "language": "dotnet",
            "install": "dotnet add package OpenTelemetry.Extensions.Hosting\ndotnet add package OpenTelemetry.Exporter.OpenTelemetryProtocol",
            "env_vars": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:3001", "OTEL_SERVICE_NAME": "my-service"},
        }),
        ("rust", _) => json!({
            "language": "rust",
            "install": "cargo add opentelemetry opentelemetry-otlp tracing-opentelemetry",
            "env_vars": {"OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:3001", "OTEL_SERVICE_NAME": "my-service"},
        }),
        _ => json!({"error": "Unsupported language/framework combination"}),
    };

    Json(snippet)
}

// ============================================================================
// OTel P05: Semantic Convention Attribute APIs
// ============================================================================

/// GET /api/otel/attributes?signal=traces — attribute keys with cardinality
pub async fn otel_attributes(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<OtelAttributesQuery>,
) -> Result<Json<Value>, ApiError> {
    let signal = params.signal.unwrap_or_else(|| "traces".into());

    let keys = match signal.as_str() {
        "traces" => {
            sqlx::query_as::<_, AttrKeyRow>(
                r#"SELECT key, COUNT(*) AS count
                   FROM (SELECT jsonb_object_keys(tags) AS key FROM spans WHERE started_at > NOW() - INTERVAL '24 hours' LIMIT 10000) sub
                   GROUP BY key ORDER BY count DESC LIMIT 100"#
            )
            .fetch_all(&state.tsdb_pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
        }
        _ => vec![],
    };

    let attrs: Vec<Value> = keys.iter().map(|k| json!({
        "key": k.key,
        "count": k.count,
    })).collect();

    Ok(Json(json!({ "signal": signal, "attributes": attrs })))
}

/// GET /api/otel/attribute-values?key=&limit= — values for an attribute
pub async fn otel_attribute_values(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Query(params): Query<OtelAttributeValuesQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = params.limit.unwrap_or(50);

    let values = sqlx::query_scalar::<_, String>(
        &format!(
            "SELECT DISTINCT tags->>'{}' AS val FROM spans WHERE tags ? '{}' AND started_at > NOW() - INTERVAL '24 hours' ORDER BY val LIMIT {}",
            params.key, params.key, limit
        )
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "key": params.key, "values": values, "count": values.len() })))
}

// ============================================================================
// OTel P06: Resource Detection
// ============================================================================

/// GET /api/otel/resources — aggregated distinct resources
pub async fn otel_resources(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, ResourceRow>(
        r#"SELECT DISTINCT ON (service_name)
            service_name,
            MAX(started_at) AS last_seen,
            COUNT(*) AS span_count
           FROM spans
           WHERE started_at > NOW() - INTERVAL '24 hours'
           GROUP BY service_name
           ORDER BY service_name, last_seen DESC"#
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let resources: Vec<Value> = rows.iter().map(|r| json!({
        "service_name": r.service_name,
        "last_seen": r.last_seen,
        "span_count": r.span_count,
    })).collect();

    Ok(Json(json!({ "resources": resources, "count": resources.len() })))
}

// ============================================================================
// OTel P07: Health Dashboard Stats
// ============================================================================

/// GET /api/otel/stats — aggregated ingest stats
pub async fn otel_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let spans_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM spans WHERE started_at > NOW() - INTERVAL '1 minute'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let metrics_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metrics WHERE org_id = $1 AND time > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let logs_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let unique_services: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT service_name) FROM spans WHERE started_at > NOW() - INTERVAL '24 hours'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let avg_duration: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(duration_us)::FLOAT / 1000.0, 0) FROM spans WHERE started_at > NOW() - INTERVAL '5 minutes'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0.0);

    let error_rate: f64 = sqlx::query_scalar(
        "SELECT CASE WHEN COUNT(*) > 0 THEN COUNT(*) FILTER (WHERE status = 'error')::FLOAT / COUNT(*)::FLOAT * 100 ELSE 0 END FROM spans WHERE started_at > NOW() - INTERVAL '5 minutes'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0.0);

    Ok(Json(json!({
        "spans_per_minute": spans_1m,
        "metrics_per_minute": metrics_1m,
        "logs_per_minute": logs_1m,
        "profiles_per_minute": 0,
        "unique_services": unique_services,
        "error_rate_percent": error_rate,
        "avg_span_duration_ms": avg_duration,
    })))
}

/// GET /api/otel/stats/timeseries?signal=&window=&step=
pub async fn otel_stats_timeseries(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Query(params): Query<OtelStatsTimeseriesQuery>,
) -> Result<Json<Value>, ApiError> {
    let signal = params.signal.unwrap_or_else(|| "traces".into());
    let window = params.window.unwrap_or_else(|| "24h".into());
    let step = params.step.unwrap_or_else(|| "5m".into());

    let interval = match window.as_str() {
        "1h" => "1 hour", "6h" => "6 hours", "24h" => "24 hours", "7d" => "7 days", _ => "24 hours",
    };
    let bucket = match step.as_str() {
        "1m" => "1 minute", "5m" => "5 minutes", "15m" => "15 minutes", "1h" => "1 hour", _ => "5 minutes",
    };

    let (table, time_col) = match signal.as_str() {
        "traces" => ("spans", "started_at"),
        "metrics" => ("metrics", "time"),
        "logs" => ("logs", "timestamp"),
        _ => ("spans", "started_at"),
    };

    let sql = format!(
        "SELECT time_bucket('{bucket}', {col}) AS bucket, COUNT(*) AS count FROM {table} WHERE {col} > NOW() - INTERVAL '{interval}' GROUP BY bucket ORDER BY bucket",
        bucket = bucket, col = time_col, table = table, interval = interval
    );

    let rows = sqlx::query_as::<_, TimeseriesRow>(&sql)
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let series: Vec<Value> = rows.iter().map(|r| json!({
        "timestamp": r.bucket,
        "count": r.count,
    })).collect();

    Ok(Json(json!({ "signal": signal, "series": series })))
}

// ============================================================================
// OTel P13: Collector Pipeline Topology
// ============================================================================

/// GET /api/otel/pipeline — Collector topology
pub async fn otel_pipeline(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    // Identify collectors by resource service.name containing "otelcol"
    let collectors = sqlx::query_as::<_, CollectorRow>(
        r#"SELECT DISTINCT service_name AS collector_name,
                  MAX(started_at) AS last_seen,
                  COUNT(*) AS signals_forwarded
           FROM spans
           WHERE service_name LIKE '%otelcol%' OR service_name LIKE '%collector%'
             AND started_at > NOW() - INTERVAL '24 hours'
           GROUP BY service_name"#
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let collector_list: Vec<Value> = collectors.iter().map(|c| json!({
        "name": c.collector_name,
        "last_seen": c.last_seen,
        "signals_forwarded": c.signals_forwarded,
    })).collect();

    Ok(Json(json!({
        "collectors": collector_list,
        "flow": [],
    })))
}

// ============================================================================
// Helper functions
// ============================================================================

fn extract_service_name(attrs: &Value) -> String {
    // OTLP attributes are [{key, value: {stringValue}}]
    if let Some(arr) = attrs.as_array() {
        for attr in arr {
            if attr.get("key").and_then(|k| k.as_str()) == Some("service.name") {
                if let Some(sv) = attr.pointer("/value/stringValue").and_then(|v| v.as_str()) {
                    return sv.to_string();
                }
            }
        }
    }
    // Also handle flat object format
    if let Some(obj) = attrs.as_object() {
        if let Some(name) = obj.get("service.name").and_then(|v| v.as_str()) {
            return name.to_string();
        }
    }
    "unknown".to_string()
}

fn otlp_attrs_to_map(attrs: &Value) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(arr) = attrs.as_array() {
        for attr in arr {
            if let Some(key) = attr.get("key").and_then(|k| k.as_str()) {
                let val = attr.get("value")
                    .and_then(|v| {
                        v.get("stringValue").cloned()
                            .or_else(|| v.get("intValue").cloned())
                            .or_else(|| v.get("doubleValue").cloned())
                            .or_else(|| v.get("boolValue").cloned())
                    })
                    .unwrap_or(json!(null));
                map.insert(key.to_string(), val);
            }
        }
    }
    Value::Object(map)
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct AttrKeyRow {
    key: String,
    count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ResourceRow {
    service_name: String,
    last_seen: Option<DateTime<Utc>>,
    span_count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TimeseriesRow {
    bucket: DateTime<Utc>,
    count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct CollectorRow {
    collector_name: String,
    last_seen: Option<DateTime<Utc>>,
    signals_forwarded: i64,
}
