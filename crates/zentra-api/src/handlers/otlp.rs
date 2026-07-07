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
    pub host: Option<String>,
    pub port: Option<u16>,
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

/// O17: Decompress gzip-encoded body if Content-Encoding header is set
fn maybe_decompress(headers: &HeaderMap, body: &Bytes) -> Result<Vec<u8>, String> {
    let is_gzip = headers.get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("gzip"))
        .unwrap_or(false);

    if is_gzip {
        use std::io::Read;
        let mut decoder = flate2::read::GzDecoder::new(&body[..]);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)
            .map_err(|e| format!("gzip decompression failed: {}", e))?;
        Ok(decompressed)
    } else {
        Ok(body.to_vec())
    }
}

/// Validates an OTLP API key (isok_...) against the cache and database.
/// Returns (org_id, permissions) if valid.
pub async fn validate_otlp_key(
    state: &crate::state::AppState,
    key: &str,
) -> Result<(Uuid, Vec<String>), String> {
    if key.starts_with("isok_") {
        return Err("Org enrollment key cannot ingest telemetry".into());
    }
    if !key.starts_with("isak_") {
        return Err("Invalid key format".into());
    }

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Check cache (OT12: cache verification with metrics)
    if let Some(cached) = state.otlp_key_cache.get(&hash) {
        metrics::counter!("zentra_otlp_key_cache_hit").increment(1);
        return Ok(cached.clone());
    }
    metrics::counter!("zentra_otlp_key_cache_miss").increment(1);

    // Check DB
    let row = sqlx::query(
        "SELECT org_id FROM agent_api_keys
         WHERE key_hash = $1
           AND enabled = true
           AND revoked_at IS NULL
           AND (expires_at > NOW() OR (grace_until IS NOT NULL AND grace_until > NOW()))"
    )
    .bind(&hash)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(r) = row {
        use sqlx::Row;
        let org_id: Uuid = r.get("org_id");
        let perms: Vec<String> = vec!["metrics".to_string(), "logs".to_string()];
        state.otlp_key_cache.insert(hash.clone(), (org_id, perms.clone()));

        // O12: Update last_used_at asynchronously (fire-and-forget)
        let pool = state.tsdb_pool.clone();
        let h = hash.clone();
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE agent_api_keys SET last_used = NOW(), updated_at = NOW() WHERE key_hash = $1")
                .bind(&h)
                .execute(&pool)
                .await;
        });

        Ok((org_id, perms))
    } else {
        Err("Invalid or expired API key".into())
    }
}

// ============================================================================
// Signal Ingest Handlers
// ============================================================================

/// POST /v1/traces — ingest OTLP traces (JSON or protobuf)
#[tracing::instrument(skip_all, fields(otel.kind = "server", signal = "traces"))]
pub async fn ingest_traces(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    // O17: Decompress gzip if Content-Encoding header is set
    let raw = maybe_decompress(&headers, &body)
        .map_err(|e| ApiError::BadRequest(e))?;
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_traces(&raw)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
        }
    };

    // Enqueue for async batch insert (OT02)
    if let Some(ref queue) = state.ingest_queue {
        let batch = crate::ingest_queue::IngestBatch {
            org_id,
            signal: crate::ingest_queue::Signal::Traces,
            payload: payload.clone(),
            raw_bytes: body.to_vec(),
        };
        if queue.try_send(batch).is_err() {
            return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
        }
    }

    // Count spans for the response
    let span_count = payload.get("resourceSpans")
        .and_then(|v| v.as_array())
        .map(|rs| rs.iter()
            .flat_map(|r| r.get("scopeSpans").and_then(|v| v.as_array()).into_iter().flatten())
            .flat_map(|ss| {
                ss.get("spans").or_else(|| ss.get("traces"))
                    .and_then(|v| v.as_array()).into_iter().flatten()
            })
            .count())
        .unwrap_or(0);

    // O20: Spec-compliant ExportTraceServiceResponse
    Ok(Json(json!({
        "partialSuccess": null,
    })))
}

/// POST /api/agent/ingest/metrics — agent-key authenticated metric ingest
/// Accepts isak_ bearer tokens (agent operational keys), not JWT.
#[tracing::instrument(skip_all, fields(otel.kind = "server", signal = "metrics"))]
pub async fn agent_ingest_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let bearer = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let (org_id, _perms) = validate_otlp_key(&state, bearer)
        .await
        .map_err(|e| ApiError::Unauthorized(e))?;

    let raw = maybe_decompress(&headers, &body)
        .map_err(|e| ApiError::BadRequest(e))?;
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            return Err(ApiError::BadRequest("Protobuf not supported on agent ingest endpoint".into()));
        }
    };

    if let Some(ref queue) = state.ingest_queue {
        let batch = crate::ingest_queue::IngestBatch {
            org_id,
            signal: crate::ingest_queue::Signal::Metrics,
            payload: payload.clone(),
            raw_bytes: body.to_vec(),
        };
        if queue.try_send(batch).is_err() {
            return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
        }
    }

    Ok(Json(json!({ "partialSuccess": null })))
}

/// POST /api/agent/ingest/logs — agent-key authenticated log ingest
#[tracing::instrument(skip_all, fields(otel.kind = "server", signal = "logs"))]
pub async fn agent_ingest_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let bearer = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let (org_id, _perms) = validate_otlp_key(&state, bearer)
        .await
        .map_err(|e| ApiError::Unauthorized(e))?;

    let raw = maybe_decompress(&headers, &body)
        .map_err(|e| ApiError::BadRequest(e))?;
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            return Err(ApiError::BadRequest("Protobuf not supported on agent ingest endpoint".into()));
        }
    };

    if let Some(ref queue) = state.ingest_queue {
        let batch = crate::ingest_queue::IngestBatch {
            org_id,
            signal: crate::ingest_queue::Signal::Logs,
            payload: payload.clone(),
            raw_bytes: body.to_vec(),
        };
        if queue.try_send(batch).is_err() {
            return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
        }
    }

    Ok(Json(json!({ "partialSuccess": null })))
}

/// POST /v1/metrics — ingest OTLP metrics
#[tracing::instrument(skip_all, fields(otel.kind = "server", signal = "metrics"))]
pub async fn ingest_metrics(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let raw = maybe_decompress(&headers, &body)
        .map_err(|e| ApiError::BadRequest(e))?;
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_metrics(&raw)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
        }
    };

    if let Some(ref queue) = state.ingest_queue {
        let batch = crate::ingest_queue::IngestBatch {
            org_id,
            signal: crate::ingest_queue::Signal::Metrics,
            payload: payload.clone(),
            raw_bytes: body.to_vec(),
        };
        if queue.try_send(batch).is_err() {
            return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
        }
    }

    // O20: Spec-compliant ExportMetricsServiceResponse
    Ok(Json(json!({
        "partialSuccess": null,
    })))
}

/// POST /v1/logs — ingest OTLP logs
#[tracing::instrument(skip_all, fields(otel.kind = "server", signal = "logs"))]
pub async fn ingest_logs(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let raw = maybe_decompress(&headers, &body)
        .map_err(|e| ApiError::BadRequest(e))?;
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_logs(&raw)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
        }
    };

    if let Some(ref queue) = state.ingest_queue {
        let batch = crate::ingest_queue::IngestBatch {
            org_id,
            signal: crate::ingest_queue::Signal::Logs,
            payload: payload.clone(),
            raw_bytes: body.to_vec(),
        };
        if queue.try_send(batch).is_err() {
            return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
        }
    }

    // O20: Spec-compliant ExportLogsServiceResponse
    Ok(Json(json!({
        "partialSuccess": null,
    })))
}

/// POST /v1/profiles — ingest OTLP profiles (4th signal)
#[tracing::instrument(skip_all, fields(otel.kind = "server", signal = "profiles"))]
pub async fn ingest_profiles(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let raw = maybe_decompress(&headers, &body)
        .map_err(|e| ApiError::BadRequest(e))?;
    let encoding = detect_encoding(&headers);
    let payload: Value = match encoding {
        OtlpEncoding::Json => serde_json::from_slice(&raw)
            .map_err(|e| ApiError::BadRequest(format!("Invalid JSON: {}", e)))?,
        OtlpEncoding::Protobuf => {
            #[cfg(feature = "otlp-proto")]
            {
                crate::otlp_proto::proto_to_json_profiles(&raw)
                    .map_err(|e| ApiError::BadRequest(format!("Protobuf decode error: {}", e)))?
            }
            #[cfg(not(feature = "otlp-proto"))]
            {
                return Err(ApiError::BadRequest("Protobuf support not compiled. Enable otlp-proto feature.".into()));
            }
        }
    };

    if let Some(ref queue) = state.ingest_queue {
        let batch = crate::ingest_queue::IngestBatch {
            org_id,
            signal: crate::ingest_queue::Signal::Profiles,
            payload: payload.clone(),
            raw_bytes: body.to_vec(),
        };
        if queue.try_send(batch).is_err() {
            return Err(ApiError::ServiceUnavailable("Ingest queue full, try again later".into()));
        }
    }

    // O20: Spec-compliant ExportProfilesServiceResponse
    Ok(Json(json!({
        "partialSuccess": null,
    })))
}

// ============================================================================
// OTel Status & Config Endpoints
// ============================================================================

/// GET /api/otel/status — OTLP capabilities (O15: reflect real running config)
pub async fn otel_status() -> Json<Value> {
    let grpc_enabled = cfg!(feature = "grpc");
    let proto_enabled = cfg!(feature = "otlp-proto");

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
                "enabled": grpc_enabled,
                "status": if grpc_enabled { "active" } else { "disabled" },
            }
        },
        "encodings": {
            "json": true,
            "protobuf": proto_enabled,
            "gzip": true,
        },
        "auth": ["jwt", "api-key", "org-id-header"],
        "features": {
            "otlp_proto": proto_enabled,
            "grpc": grpc_enabled,
            "histogram_metrics": true,
            "exp_histogram_metrics": true,
            "summary_metrics": true,
            "span_events": true,
            "span_links": true,
            "receiver_sampling": true,
            "dead_letter_queue": true,
            "quota_enforcement": true,
        },
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

    // O21: Use correct reachable endpoint (port 5010 for Docker/nginx, 5006 for direct)
    let host = params.host.unwrap_or_else(|| "localhost".into());
    let port = params.port.unwrap_or(5006);
    let base_url = format!("http://{}:{}", host, port);

    let snippet = match (language.as_str(), framework.as_str()) {
        ("java", "spring-boot") => json!({
            "language": "java",
            "framework": "spring-boot",
            "install": "Download opentelemetry-javaagent.jar from GitHub releases",
            "command": format!("java -javaagent:opentelemetry-javaagent.jar \\\n  -Dotel.exporter.otlp.endpoint={} \\\n  -Dotel.exporter.otlp.protocol=http/json \\\n  -Dotel.exporter.otlp.headers=Authorization=Bearer\\ isok_YOUR_API_KEY \\\n  -Dotel.service.name=my-service \\\n  -jar my-app.jar", base_url),
            "env_vars": {
                "OTEL_EXPORTER_OTLP_ENDPOINT": base_url,
                "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
                "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer isok_YOUR_API_KEY",
                "OTEL_SERVICE_NAME": "my-service",
            },
        }),
        ("python", _) => json!({
            "language": "python",
            "install": "pip install opentelemetry-distro opentelemetry-exporter-otlp",
            "command": "opentelemetry-instrument --service_name my-service python app.py",
            "env_vars": {
                "OTEL_EXPORTER_OTLP_ENDPOINT": base_url,
                "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
                "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer isok_YOUR_API_KEY",
                "OTEL_SERVICE_NAME": "my-service",
            },
        }),
        ("go", _) => json!({
            "language": "go",
            "install": "go get go.opentelemetry.io/otel\ngo get go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp",
            "env_vars": {
                "OTEL_EXPORTER_OTLP_ENDPOINT": base_url,
                "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
                "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer isok_YOUR_API_KEY",
                "OTEL_SERVICE_NAME": "my-service",
            },
        }),
        ("node" | "nodejs", _) => json!({
            "language": "nodejs",
            "install": "npm install @opentelemetry/sdk-node @opentelemetry/auto-instrumentations-node @opentelemetry/exporter-trace-otlp-http",
            "env_vars": {
                "OTEL_EXPORTER_OTLP_ENDPOINT": base_url,
                "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
                "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer isok_YOUR_API_KEY",
                "OTEL_SERVICE_NAME": "my-service",
            },
        }),
        ("dotnet" | ".net", _) => json!({
            "language": "dotnet",
            "install": "dotnet add package OpenTelemetry.Extensions.Hosting\ndotnet add package OpenTelemetry.Exporter.OpenTelemetryProtocol",
            "env_vars": {
                "OTEL_EXPORTER_OTLP_ENDPOINT": base_url,
                "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
                "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer isok_YOUR_API_KEY",
                "OTEL_SERVICE_NAME": "my-service",
            },
        }),
        ("rust", _) => json!({
            "language": "rust",
            "install": "cargo add opentelemetry opentelemetry-otlp tracing-opentelemetry",
            "env_vars": {
                "OTEL_EXPORTER_OTLP_ENDPOINT": base_url,
                "OTEL_EXPORTER_OTLP_PROTOCOL": "http/json",
                "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer isok_YOUR_API_KEY",
                "OTEL_SERVICE_NAME": "my-service",
            },
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

    // O13: All attribute reads are org-scoped
    let keys = match signal.as_str() {
        "traces" => {
            sqlx::query_as::<_, AttrKeyRow>(
                r#"SELECT key, COUNT(*) AS count
                   FROM (SELECT jsonb_object_keys(tags) AS key FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '24 hours' LIMIT 10000) sub
                   GROUP BY key ORDER BY count DESC LIMIT 100"#
            )
            .bind(org_id)
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
    OrgId(org_id): OrgId,
    Query(params): Query<OtelAttributeValuesQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = params.limit.unwrap_or(50).min(500) as i64;

    // O07: Use bound parameters to prevent SQL injection — never interpolate user input
    let values = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT tags->>$1 AS val FROM traces \
         WHERE org_id = $2 AND tags ? $1 AND started_at > NOW() - INTERVAL '24 hours' \
         AND tags->>$1 IS NOT NULL ORDER BY val LIMIT $3"
    )
    .bind(&params.key)
    .bind(org_id)
    .bind(limit)
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
        "SELECT COUNT(*) FROM traces WHERE org_id = $1 AND service_name = $2 AND started_at > NOW() - INTERVAL '24 hours'"
    ).bind(org_id).bind(&service_name).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

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
        "SELECT COUNT(*) FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let metrics_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let logs_1m: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM logs WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 minute'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let unique_services: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT service_name) FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '24 hours'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0);

    let avg_duration: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(duration_us)::FLOAT / 1000.0, 0) FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '5 minutes'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0.0);

    let error_rate: f64 = sqlx::query_scalar(
        "SELECT CASE WHEN COUNT(*) > 0 THEN COUNT(*) FILTER (WHERE status = 'error')::FLOAT / COUNT(*)::FLOAT * 100 ELSE 0 END FROM traces WHERE org_id = $1 AND started_at > NOW() - INTERVAL '5 minutes'"
    ).bind(org_id).fetch_optional(&state.tsdb_pool).await.map_err(|e| ApiError::Internal(e.to_string()))?.unwrap_or(0.0);

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

/// GET /api/otel/pipeline — Collector topology (O11: real edges, O13: org-scoped)
pub async fn otel_pipeline(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    // Identify collectors by resource service.name containing "otelcol"
    let collectors = sqlx::query_as::<_, CollectorRow>(
        r#"SELECT DISTINCT service_name AS collector_name,
                  MAX(started_at) AS last_seen,
                  COUNT(*) AS signals_forwarded
           FROM traces
           WHERE org_id = $1
             AND (service_name LIKE '%otelcol%' OR service_name LIKE '%collector%')
             AND started_at > NOW() - INTERVAL '24 hours'
           GROUP BY service_name"#
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let collector_list: Vec<Value> = collectors.iter().map(|c| json!({
        "name": c.collector_name,
        "last_seen": c.last_seen,
        "signals_forwarded": c.signals_forwarded,
    })).collect();

    // O11: Produce real pipeline edges from parent→child span relationships
    let edges = sqlx::query(
        r#"SELECT DISTINCT
              p.service_name AS source,
              c.service_name AS target,
              COUNT(*) AS call_count
           FROM traces c
           JOIN traces p ON c.parent_span_id = p.span_id
              AND c.trace_id = p.trace_id
              AND p.started_at > NOW() - INTERVAL '24 hours'
           WHERE c.org_id = $1
             AND c.started_at > NOW() - INTERVAL '24 hours'
             AND c.service_name IS DISTINCT FROM p.service_name
           GROUP BY p.service_name, c.service_name
           ORDER BY call_count DESC
           LIMIT 100"#
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let flow: Vec<Value> = edges.iter().map(|r| {
        use sqlx::Row;
        json!({
            "source": r.get::<String, _>("source"),
            "target": r.get::<String, _>("target"),
            "call_count": r.get::<i64, _>("call_count"),
        })
    }).collect();

    Ok(Json(json!({
        "collectors": collector_list,
        "flow": flow,
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
/// O06: Evaluates user-defined rules from the otel_alert_rules table.
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

    let (total, errors, avg_ms, p99_ms) = match stats {
        Some(s) => (s.total, s.errors, s.avg_ms.unwrap_or(0.0), s.p99_ms.unwrap_or(0.0)),
        None => (0i64, 0i64, 0.0, 0.0),
    };
    let error_rate = if total > 0 { (errors as f64 / total as f64) * 100.0 } else { 0.0 };

    // O06: Fetch user-defined rules from otel_alert_rules table
    let user_rules = sqlx::query(
        "SELECT id, name, signal, metric, condition, threshold, window_seconds, severity, enabled \
         FROM otel_alert_rules WHERE org_id = $1 AND enabled = true"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut evals = Vec::new();

    // Evaluate each user-defined rule
    for rule in &user_rules {
        use sqlx::Row;
        let rule_name: String = rule.get("name");
        let metric_name: String = rule.get("metric");
        let condition: String = rule.get("condition");
        let threshold: f64 = rule.get("threshold");
        let severity: String = rule.get("severity");
        let window_secs: i32 = rule.get("window_seconds");

        // Resolve current value for known metrics
        let current_value = match metric_name.as_str() {
            "error_rate_percent" | "error_rate" => error_rate,
            "p99_ms" | "p99_latency_ms" => p99_ms,
            "avg_ms" | "avg_latency_ms" => avg_ms,
            "request_count" | "throughput" => total as f64,
            "error_count" => errors as f64,
            _ => {
                // For custom metric names, query the metrics table
                let mval = sqlx::query_scalar::<_, Option<f64>>(
                    &format!(
                        "SELECT AVG(value) FROM metrics WHERE org_id = $1 AND metric_name = $2 \
                         AND timestamp > NOW() - INTERVAL '{} seconds'",
                        window_secs
                    )
                )
                .bind(org_id)
                .bind(&metric_name)
                .fetch_one(&state.tsdb_pool)
                .await
                .ok()
                .flatten()
                .unwrap_or(0.0);
                mval
            }
        };

        let firing = match condition.as_str() {
            ">" | "gt" => current_value > threshold,
            "<" | "lt" => current_value < threshold,
            ">=" | "gte" => current_value >= threshold,
            "<=" | "lte" => current_value <= threshold,
            "==" | "eq" => (current_value - threshold).abs() < f64::EPSILON,
            "!=" | "ne" => (current_value - threshold).abs() >= f64::EPSILON,
            _ => current_value > threshold,
        };

        evals.push(json!({
            "rule": rule_name,
            "metric": metric_name,
            "condition": condition,
            "current": (current_value * 100.0).round() / 100.0,
            "threshold": threshold,
            "severity": severity,
            "status": if firing { "FIRING" } else { "OK" },
        }));
    }

    // If no user-defined rules exist, provide built-in defaults
    if evals.is_empty() {
        evals = vec![
            json!({ "rule": "High Error Rate (default)", "metric": "error_rate_percent", "current": (error_rate * 100.0).round() / 100.0, "threshold": 5.0, "status": if error_rate > 5.0 { "FIRING" } else { "OK" } }),
            json!({ "rule": "High P99 Latency (default)", "metric": "p99_ms", "current": p99_ms, "threshold": 5000.0, "status": if p99_ms > 5000.0 { "FIRING" } else { "OK" } }),
        ];
    }

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

// ============================================================================
// O14: Single-Trace Waterfall Endpoint
// ============================================================================

/// GET /api/otel/traces/:trace_id — full waterfall for a single trace
pub async fn otel_trace_detail(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(trace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let spans = sqlx::query(
        r#"SELECT span_id, parent_span_id, operation_name, service_name,
                  started_at, duration_us, status, tags
           FROM traces
           WHERE org_id = $1 AND trace_id = $2
           ORDER BY started_at ASC"#
    )
    .bind(org_id)
    .bind(&trace_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let span_list: Vec<Value> = spans.iter().map(|r| {
        use sqlx::Row;
        json!({
            "span_id": r.get::<String, _>("span_id"),
            "parent_span_id": r.get::<Option<String>, _>("parent_span_id"),
            "operation_name": r.get::<String, _>("operation_name"),
            "service_name": r.get::<String, _>("service_name"),
            "started_at": r.get::<DateTime<Utc>, _>("started_at"),
            "duration_us": r.get::<i64, _>("duration_us"),
            "status": r.get::<String, _>("status"),
            "tags": r.get::<Value, _>("tags"),
        })
    }).collect();

    // O14: Also fetch span events for this trace
    let events = sqlx::query(
        "SELECT span_id, event_name, timestamp, attributes FROM span_events WHERE trace_id = $1 ORDER BY timestamp"
    )
    .bind(&trace_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let event_list: Vec<Value> = events.iter().map(|r| {
        use sqlx::Row;
        json!({
            "span_id": r.get::<String, _>("span_id"),
            "event_name": r.get::<String, _>("event_name"),
            "timestamp": r.get::<DateTime<Utc>, _>("timestamp"),
            "attributes": r.get::<Value, _>("attributes"),
        })
    }).collect();

    Ok(Json(json!({
        "trace_id": trace_id,
        "span_count": span_list.len(),
        "spans": span_list,
        "events": event_list,
    })))
}

// ============================================================================
// O18: Correlate Logs with Traces
// ============================================================================

/// GET /api/otel/logs-by-trace/:trace_id — logs correlated with a trace
pub async fn otel_logs_by_trace(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(trace_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let logs = sqlx::query(
        r#"SELECT timestamp, source, severity AS level, message, parsed_fields AS metadata
           FROM logs
           WHERE org_id = $1
             AND (parsed_fields->>'trace_id' = $2 OR parsed_fields->>'traceId' = $2)
           ORDER BY timestamp ASC
           LIMIT 500"#
    )
    .bind(org_id)
    .bind(&trace_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let log_list: Vec<Value> = logs.iter().map(|r| {
        use sqlx::Row;
        json!({
            "timestamp": r.get::<DateTime<Utc>, _>("timestamp"),
            "source": r.get::<String, _>("source"),
            "level": r.get::<Option<String>, _>("level"),
            "message": r.get::<String, _>("message"),
            "metadata": r.get::<Value, _>("metadata"),
        })
    }).collect();

    Ok(Json(json!({
        "trace_id": trace_id,
        "log_count": log_list.len(),
        "logs": log_list,
    })))
}

// ============================================================================
// Row structs
// ============================================================================

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

// ============================================================================
// Item 16: Trace Aggregates (p50/p95/p99)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TraceAggregatesQuery {
    pub service: Option<String>,
    pub operation: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[tracing::instrument(skip(state))]
pub async fn otel_trace_aggregates(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(q): Query<TraceAggregatesQuery>,
) -> Result<Json<Value>, ApiError> {
    let from = q.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = q.to.unwrap_or_else(Utc::now);

    let mut sql = String::from(
        r#"SELECT
             service_name,
             operation_name,
             COUNT(*) AS request_count,
             COUNT(*) FILTER (WHERE status = 'error') AS error_count,
             ROUND(COUNT(*) FILTER (WHERE status = 'error')::numeric / NULLIF(COUNT(*), 0) * 100, 2) AS error_rate,
             percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_us) AS p50_us,
             percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_us) AS p95_us,
             percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_us) AS p99_us,
             AVG(duration_us)::bigint AS avg_us,
             MAX(duration_us) AS max_us
           FROM traces
           WHERE org_id = $1 AND started_at >= $2 AND started_at < $3"#
    );
    let mut param_idx = 4;

    if q.service.is_some() {
        sql.push_str(&format!(" AND service_name = ${}", param_idx));
        param_idx += 1;
    }
    if q.operation.is_some() {
        sql.push_str(&format!(" AND operation_name = ${}", param_idx));
    }
    sql.push_str(" GROUP BY service_name, operation_name ORDER BY request_count DESC LIMIT 100");

    let mut query = sqlx::query_as::<_, TraceAggRow>(&sql)
        .bind(org_id)
        .bind(from)
        .bind(to);

    if let Some(ref svc) = q.service {
        query = query.bind(svc);
    }
    if let Some(ref op) = q.operation {
        query = query.bind(op);
    }

    let rows = query.fetch_all(&state.pool).await
        .map_err(|e| ApiError::Internal(format!("Trace aggregates query failed: {}", e)))?;

    let results: Vec<Value> = rows.iter().map(|r| json!({
        "service_name": r.service_name,
        "operation_name": r.operation_name,
        "request_count": r.request_count,
        "error_count": r.error_count,
        "error_rate": r.error_rate,
        "p50_us": r.p50_us,
        "p95_us": r.p95_us,
        "p99_us": r.p99_us,
        "avg_us": r.avg_us,
        "max_us": r.max_us,
    })).collect();

    Ok(Json(json!({ "aggregates": results, "from": from, "to": to })))
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TraceAggRow {
    service_name: String,
    operation_name: String,
    request_count: i64,
    error_count: i64,
    error_rate: Option<f64>,
    p50_us: Option<f64>,
    p95_us: Option<f64>,
    p99_us: Option<f64>,
    avg_us: Option<i64>,
    max_us: Option<i64>,
}
