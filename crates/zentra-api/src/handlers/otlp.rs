//! OTel P03+: OTLP Ingest handlers (JSON + Protobuf).
//!
//! Handles all 4 signals: traces, metrics, logs, profiles.
//! Supports both JSON and protobuf (content-type negotiation).

use axum::{extract::{State, Query, Path}, Json, body::Bytes, http::HeaderMap};
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

use sha2::{Sha256, Digest};

/// Validates an OTLP API key (isok_...) against the cache and database.
/// Returns (org_id, permissions) if valid.
pub async fn validate_otlp_key(
    state: &crate::state::AppState,
    key: &str,
) -> Result<(Uuid, Vec<String>), String> {
    if !key.starts_with("isok_") {
        return Err("Invalid key format".into());
    }

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Check cache
    if let Some(cached) = state.otlp_key_cache.get(&hash) {
        return Ok(cached.clone());
    }

    // Check DB
    let row = sqlx::query(
        "SELECT org_id, permissions FROM otlp_api_keys 
         WHERE key_hash = $1 AND enabled = true AND (expires_at IS NULL OR expires_at > NOW())"
    )
    .bind(&hash)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(r) = row {
        use sqlx::Row;
        let org_id: Uuid = r.get("org_id");
        let perms: Vec<String> = r.get("permissions");
        state.otlp_key_cache.insert(hash, (org_id, perms.clone()));
        Ok((org_id, perms))
    } else {
        Err("Invalid or expired API key".into())
    }
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
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_traces(&body)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
        }
    };

    let mut span_count = 0i64;
    let mut llm_span_count = 0i64;

    // Parse resourceSpans → scopeSpans → traces
    if let Some(resource_spans) = payload.get("resourceSpans").and_then(|v| v.as_array()) {
        for rs in resource_spans {
            let resource_attrs = rs.pointer("/resource/attributes").cloned().unwrap_or(json!({}));
            let service_name = extract_service_name(&resource_attrs);

            if let Some(scope_spans) = rs.get("scopeSpans").and_then(|v| v.as_array()) {
                for ss in scope_spans {
                    if let Some(traces) = ss.get("traces").and_then(|v| v.as_array()) {
                        for span in traces {
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

                            // Insert into traces table
                            let _ = sqlx::query(
                                r#"INSERT INTO traces (trace_id, span_id, parent_span_id, operation_name, service_name,
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

                            // SW P08 / OTel P11: Check for GenAI traces
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
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_metrics(&body)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
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
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_logs(&body)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
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
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_profiles(&body)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
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
                   FROM (SELECT jsonb_object_keys(tags) AS key FROM traces WHERE started_at > NOW() - INTERVAL '24 hours' LIMIT 10000) sub
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
            "SELECT DISTINCT tags->>'{}' AS val FROM traces WHERE tags ? '{}' AND started_at > NOW() - INTERVAL '24 hours' ORDER BY val LIMIT {}",
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

// GET /api/otel/resources — aggregated distinct resources
pub async fn otel_resources(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    // 1. Get trace-based resources
    let trace_rows = sqlx::query(
        r#"SELECT service_name, MAX(started_at) as last_seen, COUNT(*) as span_count
           FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '24 hours'
           GROUP BY service_name"#,
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // 2. Get metric-based resources
    let metric_rows = sqlx::query(
        r#"SELECT DISTINCT resource_id FROM metrics 
           WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '24 hours'"#,
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // 3. Get log-based resources
    let log_rows = sqlx::query(
        r#"SELECT DISTINCT source FROM logs 
           WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '24 hours'"#,
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // 4. Get profile-based resources
    let profile_rows = sqlx::query(
        r#"SELECT DISTINCT service_name FROM otel_profiles 
           WHERE org_id = $1 AND started_at > NOW() - INTERVAL '24 hours'"#,
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    use std::collections::HashMap;
    use sqlx::Row;
    let mut resources: HashMap<String, Value> = HashMap::new();

    for r in trace_rows {
        let name: String = r.get("service_name");
        let last_seen: DateTime<Utc> = r.get("last_seen");
        let count: i64 = r.get("span_count");
        resources.insert(name.clone(), json!({
            "service_name": name,
            "last_seen": last_seen,
            "span_count": count,
            "signals": { "traces": true, "metrics": false, "logs": false, "profiles": false }
        }));
    }

    for r in metric_rows {
        let name: String = r.get("resource_id");
        let entry = resources.entry(name.clone()).or_insert_with(|| json!({
            "service_name": name,
            "last_seen": Utc::now(),
            "span_count": 0,
            "signals": { "traces": false, "metrics": false, "logs": false, "profiles": false }
        }));
        entry["signals"]["metrics"] = json!(true);
    }

    for r in log_rows {
        let name: String = r.get("source");
        let entry = resources.entry(name.clone()).or_insert_with(|| json!({
            "service_name": name,
            "last_seen": Utc::now(),
            "span_count": 0,
            "signals": { "traces": false, "metrics": false, "logs": false, "profiles": false }
        }));
        entry["signals"]["logs"] = json!(true);
    }

    for r in profile_rows {
        let name: String = r.get("service_name");
        let entry = resources.entry(name.clone()).or_insert_with(|| json!({
            "service_name": name,
            "last_seen": Utc::now(),
            "span_count": 0,
            "signals": { "traces": false, "metrics": false, "logs": false, "profiles": false }
        }));
        entry["signals"]["profiles"] = json!(true);
    }

    let result: Vec<Value> = resources.into_values().collect();
    Ok(Json(json!({ "resources": result, "count": result.len() })))
}

/// GET /api/otel/resources/:service_name/signals — signal breakdown for one service
pub async fn otel_resource_signals(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(service_name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let trace_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM traces WHERE service_name = $1 AND started_at > NOW() - INTERVAL '24 hours'"
    ).bind(&service_name).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let metric_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metrics WHERE org_id = $1 AND resource_id = $2 AND timestamp > NOW() - INTERVAL '24 hours'"
    ).bind(org_id).bind(&service_name).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let log_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE org_id = $1 AND source = $2 AND timestamp > NOW() - INTERVAL '24 hours'"
    ).bind(org_id).bind(&service_name).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let profile_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM otel_profiles WHERE org_id = $1 AND service_name = $2 AND started_at > NOW() - INTERVAL '24 hours'"
    ).bind(org_id).bind(&service_name).fetch_optional(&state.pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    Ok(Json(json!({
        "service_name": service_name,
        "signals": {
            "traces": { "count": trace_count, "active": trace_count > 0 },
            "metrics": { "count": metric_count, "active": metric_count > 0 },
            "logs": { "count": log_count, "active": log_count > 0 },
            "profiles": { "count": profile_count, "active": profile_count > 0 },
        },
    })))
}

/// GET /api/otel/semconv — static catalog of supported semantic conventions
pub async fn otel_semconv() -> Json<Value> {
    Json(json!({
        "version": "1.25.0",
        "groups": [
            {
                "name": "service",
                "attributes": [
                    { "name": "service.name", "type": "string", "description": "Logical name of the service" },
                    { "name": "service.version", "type": "string", "description": "Version of the service" },
                    { "name": "service.instance.id", "type": "string", "description": "Unique ID for the service instance" }
                ]
            },
            {
                "name": "http",
                "attributes": [
                    { "name": "http.method", "type": "string", "description": "HTTP request method" },
                    { "name": "http.status_code", "type": "int", "description": "HTTP response status code" },
                    { "name": "http.url", "type": "string", "description": "Full HTTP request URL" },
                    { "name": "http.route", "type": "string", "description": "Matched HTTP route" }
                ]
            },
            {
                "name": "db",
                "attributes": [
                    { "name": "db.system", "type": "string", "description": "Database management system (DBMS) product" },
                    { "name": "db.name", "type": "string", "description": "Name of the database being accessed" },
                    { "name": "db.statement", "type": "string", "description": "Database statement being executed" },
                    { "name": "db.operation", "type": "string", "description": "Name of the database operation" }
                ]
            },
            {
                "name": "gen_ai",
                "attributes": [
                    { "name": "gen_ai.system", "type": "string", "description": "GenAI system/provider name" },
                    { "name": "gen_ai.request.model", "type": "string", "description": "Name of the model requested" },
                    { "name": "gen_ai.usage.input_tokens", "type": "int", "description": "Number of input tokens consumed" },
                    { "name": "gen_ai.usage.output_tokens", "type": "int", "description": "Number of output tokens consumed" }
                ]
            }
        ]
    }))
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
        "SELECT COUNT(*) FROM traces WHERE started_at > NOW() - INTERVAL '1 minute'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let metrics_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let logs_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let unique_services: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT service_name) FROM traces WHERE started_at > NOW() - INTERVAL '24 hours'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let avg_duration: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(duration_us)::FLOAT / 1000.0, 0) FROM traces WHERE started_at > NOW() - INTERVAL '5 minutes'"
    ).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0.0);

    let error_rate: f64 = sqlx::query_scalar(
        "SELECT CASE WHEN COUNT(*) > 0 THEN COUNT(*) FILTER (WHERE status = 'error')::FLOAT / COUNT(*)::FLOAT * 100 ELSE 0 END FROM traces WHERE started_at > NOW() - INTERVAL '5 minutes'"
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
        "traces" => ("traces", "started_at"),
        "metrics" => ("metrics", "time"),
        "logs" => ("logs", "timestamp"),
        _ => ("traces", "started_at"),
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
           FROM traces
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

// ============================================================================
// OT-07: Span Metrics RED Dashboard
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SpanMetricsQuery {
    pub window: Option<String>,
    pub group_by: Option<String>,
}

/// GET /api/otel/span-metrics — RED metrics derived FROM traces
pub async fn otel_span_metrics(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<SpanMetricsQuery>,
) -> Result<Json<Value>, ApiError> {
    let interval = match params.window.as_deref() {
        Some("5m") => "5 minutes",
        Some("15m") => "15 minutes",
        Some("6h") => "6 hours",
        Some("24h") => "24 hours",
        _ => "1 hour",
    };
    let group_col = match params.group_by.as_deref() {
        Some("method") => "http_method",
        Some("db") => "db_system",
        _ => "service_name",
    };

    let q = format!(
        r#"SELECT {col} AS group_key,
                  COUNT(*) AS request_count,
                  COUNT(*) FILTER (WHERE status = 'error') AS error_count,
                  ROUND(AVG(duration_ms)::numeric, 2) AS avg_ms,
                  ROUND(PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_ms)::numeric, 2) AS p50_ms,
                  ROUND(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms)::numeric, 2) AS p95_ms,
                  ROUND(PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms)::numeric, 2) AS p99_ms
           FROM traces
           WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{interval}'
             AND {col} IS NOT NULL AND {col} != ''
           GROUP BY {col}
           ORDER BY request_count DESC
           LIMIT 50"#,
        col = group_col,
        interval = interval,
    );

    let rows = sqlx::query_as::<_, SpanMetricRow>(&q)
        .bind(org_id)
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let metrics: Vec<Value> = rows.iter().map(|r| {
        let error_rate = if r.request_count > 0 {
            (r.error_count as f64 / r.request_count as f64) * 100.0
        } else { 0.0 };
        json!({
            "group": r.group_key,
            "request_count": r.request_count,
            "error_count": r.error_count,
            "error_rate_percent": (error_rate * 100.0).round() / 100.0,
            "avg_ms": r.avg_ms,
            "p50_ms": r.p50_ms,
            "p95_ms": r.p95_ms,
            "p99_ms": r.p99_ms,
        })
    }).collect();

    let total: i64 = rows.iter().map(|r| r.request_count).sum();
    Ok(Json(json!({
        "metrics": metrics,
        "window": params.window.unwrap_or("1h".into()),
        "group_by": params.group_by.unwrap_or("service".into()),
        "total_requests": total,
    })))
}

// ============================================================================
// OT-08: Signal Alert Rules
// ============================================================================

/// GET /api/otel/alert-rules — list alert rules
pub async fn otel_alert_rules_list(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, AlertRuleRow>(
        "SELECT id, name, signal, metric, condition, threshold, window_seconds, severity, description, enabled, created_at
         FROM otel_alert_rules WHERE org_id = $1 ORDER BY created_at DESC"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return Ok(Json(json!({
            "rules": [
                { "id": "default-1", "name": "High Error Rate", "signal": "traces", "metric": "error_rate_percent", "condition": { "operator": ">" }, "threshold": 5.0, "window_seconds": 300, "severity": "warning", "description": "Error rate exceeds 5% over 5 minutes", "enabled": true, "is_default": true },
                { "id": "default-2", "name": "Very High Error Rate", "signal": "traces", "metric": "error_rate_percent", "condition": { "operator": ">" }, "threshold": 15.0, "window_seconds": 300, "severity": "critical", "description": "Error rate exceeds 15% over 5 minutes", "enabled": true, "is_default": true },
                { "id": "default-3", "name": "High P99 Latency", "signal": "traces", "metric": "p99_ms", "condition": { "operator": ">" }, "threshold": 5000.0, "window_seconds": 300, "severity": "warning", "description": "P99 latency exceeds 5 seconds", "enabled": true, "is_default": true },
                { "id": "default-4", "name": "Low Throughput", "signal": "traces", "metric": "request_count", "condition": { "operator": "<" }, "threshold": 1.0, "window_seconds": 900, "severity": "info", "description": "Less than 1 request per 15 minutes", "enabled": true, "is_default": true },
            ],
            "count": 4,
        })));
    }

    let rules: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.id,
        "name": r.name,
        "signal": r.signal,
        "metric": r.metric,
        "condition": r.condition,
        "threshold": r.threshold,
        "window_seconds": r.window_seconds,
        "severity": r.severity,
        "description": r.description,
        "enabled": r.enabled,
        "created_at": r.created_at,
    })).collect();
    let count = rules.len();
    Ok(Json(json!({ "rules": rules, "count": count })))
}

#[derive(Debug, Deserialize)]
pub struct CreateAlertRule {
    pub name: String,
    pub signal: String,
    pub metric: Option<String>,
    pub condition: Value,
    pub threshold: f64,
    pub window_seconds: Option<i32>,
    pub severity: Option<String>,
    pub description: Option<String>,
}

/// POST /api/otel/alert-rules — create alert rule
pub async fn otel_alert_rules_create(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(body): Json<CreateAlertRule>,
) -> Result<Json<Value>, ApiError> {
    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO otel_alert_rules (org_id, name, signal, metric, condition, threshold, window_seconds, severity, description)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id"
    )
    .bind(org_id)
    .bind(&body.name)
    .bind(&body.signal)
    .bind(&body.metric)
    .bind(&body.condition)
    .bind(body.threshold)
    .bind(body.window_seconds.unwrap_or(300))
    .bind(body.severity.as_deref().unwrap_or("warning"))
    .bind(&body.description)
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "id": id, "created": true })))
}

/// Internal logic for alert rule evaluation, shared between API and background worker.
pub async fn evaluate_alert_rules_internal(
    state: &crate::state::AppState,
    org_id: uuid::Uuid,
) -> Result<Vec<Value>, String> {
    // Get aggregate stats for evaluation
    let stats = sqlx::query_as::<_, EvalStatsRow>(
        r#"SELECT COUNT(*) AS total,
                  COUNT(*) FILTER (WHERE status = 'error') AS errors,
                  ROUND(AVG(duration_ms)::numeric, 2) AS avg_ms,
                  ROUND(PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_ms)::numeric, 2) AS p99_ms
           FROM traces
           WHERE org_id = $1 AND started_at > NOW() - INTERVAL '5 minutes'"#
    )
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| e.to_string())?;

    let (total, errors, _avg_ms, p99_ms) = match stats {
        Some(s) => (s.total, s.errors, s.avg_ms.unwrap_or(0.0), s.p99_ms.unwrap_or(0.0)),
        None => (0i64, 0i64, 0.0, 0.0),
    };
    let error_rate = if total > 0 { (errors as f64 / total as f64) * 100.0 } else { 0.0 };

    let evals = vec![
        json!({ "rule": "High Error Rate", "metric": "error_rate_percent", "current": (error_rate * 100.0).round() / 100.0, "threshold": 5.0, "status": if error_rate > 5.0 { "FIRING" } else { "OK" } }),
        json!({ "rule": "Very High Error Rate", "metric": "error_rate_percent", "current": (error_rate * 100.0).round() / 100.0, "threshold": 15.0, "status": if error_rate > 15.0 { "FIRING" } else { "OK" } }),
        json!({ "rule": "High P99 Latency", "metric": "p99_ms", "current": p99_ms, "threshold": 5000.0, "status": if p99_ms > 5000.0 { "FIRING" } else { "OK" } }),
        json!({ "rule": "Low Throughput", "metric": "request_count", "current": total, "threshold": 1.0, "status": if (total as f64) < 1.0 { "FIRING" } else { "OK" } }),
    ];

    // In a real implementation, we would check against defined rules in `otel_alert_rules` table.
    // For now, these are the built-in evaluations.
    
    Ok(evals)
}

/// GET /api/otel/alert-rules/evaluate — evaluate all rules
pub async fn otel_alert_rules_evaluate(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let evals = evaluate_alert_rules_internal(&state, org_id)
        .await
        .map_err(|e| ApiError::Internal(e))?;
        
    let firing = evals.iter().filter(|e| e["status"] == "FIRING").count();
    Ok(Json(json!({ "evaluations": evals, "firing_count": firing, "total_rules": evals.len() })))
}

// ============================================================================
// OT-09: GenAI / LLM via OTel
// ============================================================================

/// GET /api/otel/genai — GenAI observability from OTel traces
pub async fn otel_genai(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let models = sqlx::query_as::<_, GenAiModelRow>(
        r#"SELECT gen_ai_system AS system,
                  attributes->>'gen_ai.request.model' AS model,
                  COUNT(*) AS call_count,
                  COUNT(*) FILTER (WHERE status = 'error') AS error_count,
                  ROUND(AVG(duration_ms)::numeric, 2) AS avg_ms,
                  COALESCE(AVG((attributes->>'gen_ai.usage.input_tokens')::numeric), 0) AS avg_input_tokens,
                  COALESCE(AVG((attributes->>'gen_ai.usage.output_tokens')::numeric), 0) AS avg_output_tokens,
                  COALESCE(SUM((attributes->>'gen_ai.usage.total_tokens')::numeric), 0) AS total_tokens
           FROM traces
           WHERE org_id = $1 AND gen_ai_system IS NOT NULL
             AND started_at > NOW() - INTERVAL '24 hours'
           GROUP BY gen_ai_system, attributes->>'gen_ai.request.model'
           ORDER BY call_count DESC"#
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let model_list: Vec<Value> = models.iter().map(|m| json!({
        "system": m.system,
        "model": m.model,
        "call_count": m.call_count,
        "error_count": m.error_count,
        "error_rate": if m.call_count > 0 { (m.error_count as f64 / m.call_count as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 },
        "avg_latency_ms": m.avg_ms,
        "avg_input_tokens": m.avg_input_tokens,
        "avg_output_tokens": m.avg_output_tokens,
        "total_tokens": m.total_tokens,
    })).collect();

    let total_calls: i64 = models.iter().map(|m| m.call_count).sum();
    let total_tokens: f64 = models.iter().map(|m| m.total_tokens).sum();
    let unique_models = models.len();

    Ok(Json(json!({
        "models": model_list,
        "summary": { "total_calls": total_calls, "total_tokens": total_tokens, "unique_models": unique_models },
        "window": "24h",
    })))
}

// ============================================================================
// OT-10: OTLP Security — API Keys + Audit
// ============================================================================

/// GET /api/otel/api-keys — list API keys
pub async fn otel_api_keys_list(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, ApiKeyRow>(
        "SELECT id, name, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, enabled, last_used_at, expires_at, created_at
         FROM otlp_api_keys WHERE org_id = $1 ORDER BY created_at DESC"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let keys: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.id,
        "name": r.name,
        "key_prefix": r.key_prefix,
        "permissions": r.permissions,
        "rate_limit_rpm": r.rate_limit_rpm,
        "max_payload_bytes": r.max_payload_bytes,
        "enabled": r.enabled,
        "last_used_at": r.last_used_at,
        "expires_at": r.expires_at,
        "created_at": r.created_at,
    })).collect();
    let active = rows.iter().filter(|r| r.enabled).count();
    Ok(Json(json!({ "keys": keys, "active_count": active })))
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKey {
    pub name: String,
    pub permissions: Option<Vec<String>>,
    pub rate_limit_rpm: Option<i32>,
    pub max_payload_bytes: Option<i32>,
    pub expires_in_days: Option<i32>,
}

/// POST /api/otel/api-keys — create API key
pub async fn otel_api_keys_create(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(body): Json<CreateApiKey>,
) -> Result<Json<Value>, ApiError> {
    use sha2::{Sha256, Digest};
    use rand::Rng;

    let raw_key = format!("isok_{}", {
        let mut rng = rand::thread_rng();
        (0..32).map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
        }).collect::<String>()
    });

    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    let key_hash = format!("{:x}", hasher.finalize());
    let key_prefix = &raw_key[..13]; // "isok_" + 8 chars
    let perms = body.permissions.unwrap_or_else(|| vec!["traces".into(), "metrics".into(), "logs".into(), "profiles".into()]);
    let expires_at = body.expires_in_days.map(|d| Utc::now() + chrono::Duration::days(d as i64));

    let id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO otlp_api_keys (org_id, name, key_hash, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id"
    )
    .bind(org_id)
    .bind(&body.name)
    .bind(&key_hash)
    .bind(key_prefix)
    .bind(&perms)
    .bind(body.rate_limit_rpm.unwrap_or(10000))
    .bind(body.max_payload_bytes.unwrap_or(5242880))
    .bind(expires_at)
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "raw_key": raw_key,
        "key": { "id": id, "name": body.name, "key_prefix": key_prefix, "permissions": perms },
        "warning": "Store this key securely. It will not be shown again.",
    })))
}

/// DELETE /api/otel/api-keys/:id — revoke API key
pub async fn otel_api_keys_revoke(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(key_id): Path<uuid::Uuid>,
) -> Result<Json<Value>, ApiError> {
    sqlx::query("UPDATE otlp_api_keys SET enabled = false, updated_at = NOW() WHERE id = $1 AND org_id = $2")
        .bind(key_id)
        .bind(org_id)
        .execute(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "revoked": true, "id": key_id })))
}

/// GET /api/otel/ingest-audit — audit log
pub async fn otel_ingest_audit(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, AuditRow>(
        "SELECT signal, endpoint, status_code, span_count, payload_bytes, remote_addr, created_at
         FROM otlp_ingest_audit WHERE org_id = $1 AND created_at > NOW() - INTERVAL '1 hour'
         ORDER BY created_at DESC LIMIT 50"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows.iter().map(|r| json!({
        "signal": r.signal,
        "endpoint": r.endpoint,
        "status_code": r.status_code,
        "span_count": r.span_count,
        "payload_bytes": r.payload_bytes,
        "remote_addr": r.remote_addr,
        "created_at": r.created_at,
    })).collect();
    let total = entries.len();
    Ok(Json(json!({ "entries": entries, "total": total, "window": "1h" })))
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

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct SpanMetricRow {
    group_key: Option<String>,
    request_count: i64,
    error_count: i64,
    avg_ms: Option<f64>,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct AlertRuleRow {
    id: uuid::Uuid,
    name: String,
    signal: String,
    metric: Option<String>,
    condition: Value,
    threshold: f64,
    window_seconds: i32,
    severity: String,
    description: Option<String>,
    enabled: bool,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct EvalStatsRow {
    total: i64,
    errors: i64,
    avg_ms: Option<f64>,
    p99_ms: Option<f64>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct GenAiModelRow {
    system: Option<String>,
    model: Option<String>,
    call_count: i64,
    error_count: i64,
    avg_ms: Option<f64>,
    avg_input_tokens: f64,
    avg_output_tokens: f64,
    total_tokens: f64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ApiKeyRow {
    id: uuid::Uuid,
    name: String,
    key_prefix: String,
    permissions: Vec<String>,
    rate_limit_rpm: i32,
    max_payload_bytes: i32,
    enabled: bool,
    last_used_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct AuditRow {
    signal: String,
    endpoint: String,
    status_code: i32,
    span_count: Option<i32>,
    payload_bytes: Option<i32>,
    remote_addr: Option<String>,
    created_at: DateTime<Utc>,
}
