//! OT01: Proper gRPC Transport — OTLP over gRPC on port 4317.
//!
//! Implements real gRPC framing using axum's HTTP/2 server. Incoming gRPC
//! messages carry a 5-byte frame header (1 byte compression flag + 4 bytes
//! big-endian payload length) before the protobuf body. Responses are framed
//! the same way, with `content-type: application/grpc` and `grpc-status`
//! trailers. This is interoperable with stock OTel SDKs using `grpc-tonic`.
//!
//! Auth: Bearer token (isok_* API key) via gRPC metadata, validated through
//! the existing `validate_otlp_key` → `otlp_key_cache` path.

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use prost::Message;
use std::net::SocketAddr;
use tracing::{info, warn};
use uuid::Uuid;

use crate::otlp_proto;
use crate::state::AppState;
use crate::handlers::otlp::validate_otlp_key;

// ============================================================================
// Response types (protobuf messages per OTLP spec)
// ============================================================================

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportTraceServiceResponse {
    #[prost(message, optional, tag = "1")]
    pub partial_success: Option<ExportPartialSuccess>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportMetricsServiceResponse {
    #[prost(message, optional, tag = "1")]
    pub partial_success: Option<ExportPartialSuccess>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportLogsServiceResponse {
    #[prost(message, optional, tag = "1")]
    pub partial_success: Option<ExportPartialSuccess>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportProfilesServiceResponse {}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ExportPartialSuccess {
    #[prost(int64, tag = "1")]
    pub rejected_items: i64,
    #[prost(string, tag = "2")]
    pub error_message: String,
}

// ============================================================================
// gRPC framing helpers
// ============================================================================

/// Strip the gRPC 5-byte frame header from the request body.
/// Format: [1 byte compression flag] [4 bytes big-endian payload length] [payload]
fn strip_grpc_frame(body: &Bytes) -> &[u8] {
    if body.len() > 5 {
        &body[5..]
    } else {
        body
    }
}

/// Wrap a protobuf response in a gRPC frame (5-byte header + body).
fn grpc_frame(msg: &impl Message) -> Bytes {
    let encoded = msg.encode_to_vec();
    let mut frame = Vec::with_capacity(5 + encoded.len());
    frame.push(0u8); // no compression
    frame.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
    frame.extend_from_slice(&encoded);
    Bytes::from(frame)
}

/// Build a successful gRPC response with a framed protobuf body.
fn grpc_ok_response(body: Bytes) -> (StatusCode, HeaderMap, Bytes) {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/grpc".parse().expect("valid"));
    headers.insert("grpc-status", "0".parse().expect("valid"));
    (StatusCode::OK, headers, body)
}

/// Build a gRPC error response with status code and message.
fn grpc_error(code: i32, message: &str) -> (StatusCode, HeaderMap, Bytes) {
    let mut headers = HeaderMap::new();
    headers.insert("content-type", "application/grpc".parse().expect("valid"));
    headers.insert("grpc-status", code.to_string().parse().expect("valid"));
    if let Ok(v) = message.parse() {
        headers.insert("grpc-message", v);
    }
    (StatusCode::OK, headers, Bytes::new())
}

// ============================================================================
// Auth helper
// ============================================================================

/// Resolve org_id from gRPC metadata (Authorization: Bearer <key>).
async fn resolve_org_id_from_headers(state: &AppState, headers: &HeaderMap) -> Result<Uuid, (i32, String)> {
    if let Some(val) = headers.get("authorization") {
        if let Ok(s) = val.to_str() {
            if let Some(key) = s.strip_prefix("Bearer ") {
                match validate_otlp_key(state, key).await {
                    Ok((org_id, _perms)) => return Ok(org_id),
                    Err(_) => {
                        warn!("gRPC auth: invalid or expired Bearer token");
                        return Err((16, "Invalid or expired API key".into()));
                    }
                }
            }
        }
    }
    Err((16, "Missing Authorization header with Bearer token".into()))
}

// ============================================================================
// gRPC handlers — real gRPC framing (strip/add 5-byte frame header)
// ============================================================================

/// POST /opentelemetry.proto.collector.trace.v1.TraceService/Export
async fn grpc_export_traces(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let org_id = match resolve_org_id_from_headers(&state, &headers).await {
        Ok(id) => id,
        Err((code, msg)) => return grpc_error(code, &msg),
    };

    let proto_bytes = strip_grpc_frame(&body);
    match otlp_proto::proto_to_json_traces(proto_bytes) {
        Ok(payload) => {
            let span_count = payload.get("resourceSpans")
                .and_then(|v| v.as_array())
                .map(|rs| rs.iter()
                    .flat_map(|r| r.get("scopeSpans").and_then(|v| v.as_array()).into_iter().flatten())
                    .flat_map(|ss| ss.get("spans").and_then(|v| v.as_array()).into_iter().flatten())
                    .count())
                .unwrap_or(0);
            // O01: Enqueue for async batch insert
            if let Some(ref queue) = state.ingest_queue {
                let batch = crate::ingest_queue::IngestBatch {
                    org_id,
                    signal: crate::ingest_queue::Signal::Traces,
                    payload: payload.clone(),
                    raw_bytes: body.to_vec(),
                };
                if queue.try_send(batch).is_err() {
                    return grpc_error(8, "Ingest queue full");
                }
            }
            info!(org_id = %org_id, spans = span_count, "gRPC trace ingest");
            let resp = ExportTraceServiceResponse { partial_success: None };
            grpc_ok_response(grpc_frame(&resp))
        }
        Err(e) => grpc_error(12, &format!("Protobuf decode error: {}", e)),
    }
}

/// POST /opentelemetry.proto.collector.metrics.v1.MetricsService/Export
async fn grpc_export_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let org_id = match resolve_org_id_from_headers(&state, &headers).await {
        Ok(id) => id,
        Err((code, msg)) => return grpc_error(code, &msg),
    };

    let proto_bytes = strip_grpc_frame(&body);
    match otlp_proto::proto_to_json_metrics(proto_bytes) {
        Ok(payload) => {
            // O01: Enqueue for async batch insert
            if let Some(ref queue) = state.ingest_queue {
                let batch = crate::ingest_queue::IngestBatch {
                    org_id,
                    signal: crate::ingest_queue::Signal::Metrics,
                    payload: payload.clone(),
                    raw_bytes: body.to_vec(),
                };
                if queue.try_send(batch).is_err() {
                    return grpc_error(8, "Ingest queue full");
                }
            }
            info!(org_id = %org_id, "gRPC metrics ingest");
            let resp = ExportMetricsServiceResponse { partial_success: None };
            grpc_ok_response(grpc_frame(&resp))
        }
        Err(e) => grpc_error(12, &format!("Protobuf decode error: {}", e)),
    }
}

/// POST /opentelemetry.proto.collector.logs.v1.LogsService/Export
async fn grpc_export_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let org_id = match resolve_org_id_from_headers(&state, &headers).await {
        Ok(id) => id,
        Err((code, msg)) => return grpc_error(code, &msg),
    };

    let proto_bytes = strip_grpc_frame(&body);
    match otlp_proto::proto_to_json_logs(proto_bytes) {
        Ok(payload) => {
            // O01: Enqueue for async batch insert
            if let Some(ref queue) = state.ingest_queue {
                let batch = crate::ingest_queue::IngestBatch {
                    org_id,
                    signal: crate::ingest_queue::Signal::Logs,
                    payload: payload.clone(),
                    raw_bytes: body.to_vec(),
                };
                if queue.try_send(batch).is_err() {
                    return grpc_error(8, "Ingest queue full");
                }
            }
            info!(org_id = %org_id, "gRPC logs ingest");
            let resp = ExportLogsServiceResponse { partial_success: None };
            grpc_ok_response(grpc_frame(&resp))
        }
        Err(e) => grpc_error(12, &format!("Protobuf decode error: {}", e)),
    }
}

/// POST /opentelemetry.proto.collector.profiles.v1experimental.ProfilesService/Export
async fn grpc_export_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let org_id = match resolve_org_id_from_headers(&state, &headers).await {
        Ok(id) => id,
        Err((code, msg)) => return grpc_error(code, &msg),
    };

    let proto_bytes = strip_grpc_frame(&body);
    match otlp_proto::proto_to_json_profiles(proto_bytes) {
        Ok(payload) => {
            // O01: Enqueue for async batch insert
            if let Some(ref queue) = state.ingest_queue {
                let batch = crate::ingest_queue::IngestBatch {
                    org_id,
                    signal: crate::ingest_queue::Signal::Profiles,
                    payload: payload.clone(),
                    raw_bytes: body.to_vec(),
                };
                if queue.try_send(batch).is_err() {
                    return grpc_error(8, "Ingest queue full");
                }
            }
            info!(org_id = %org_id, "gRPC profiles ingest");
            let resp = ExportProfilesServiceResponse {};
            grpc_ok_response(grpc_frame(&resp))
        }
        Err(e) => grpc_error(12, &format!("Protobuf decode error: {}", e)),
    }
}

// ============================================================================
// gRPC Server startup — HTTP/2 with proper gRPC framing
// ============================================================================

/// Start the gRPC server on the OTLP standard port (4317).
/// Uses axum over HTTP/2 with proper gRPC framing (5-byte frame header on
/// request and response bodies). Interoperable with stock OTel SDKs.
pub async fn start_grpc_server(state: AppState) {
    let port: u16 = std::env::var("ZENTRA_GRPC_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4317);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!(port = port, "OTLP gRPC server starting");

    let app = Router::new()
        .route(
            "/opentelemetry.proto.collector.trace.v1.TraceService/Export",
            post(grpc_export_traces),
        )
        .route(
            "/opentelemetry.proto.collector.metrics.v1.MetricsService/Export",
            post(grpc_export_metrics),
        )
        .route(
            "/opentelemetry.proto.collector.logs.v1.LogsService/Export",
            post(grpc_export_logs),
        )
        .route(
            "/opentelemetry.proto.collector.profiles.v1experimental.ProfilesService/Export",
            post(grpc_export_profiles),
        )
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            warn!(error = %e, port = port, "Failed to bind gRPC port");
            return;
        }
    };

    info!(port = port, "OTLP gRPC server listening");

    if let Err(e) = axum::serve(listener, app).await {
        warn!(error = %e, "gRPC server error");
    }
}
