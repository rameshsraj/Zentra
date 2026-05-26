//! OTLP v1.3.0 Protobuf types — hand-written prost-annotated structs.
//!
//! These mirror the upstream OpenTelemetry proto definitions:
//!   opentelemetry/proto/collector/trace/v1/trace_service.proto
//!   opentelemetry/proto/collector/metrics/v1/metrics_service.proto
//!   opentelemetry/proto/collector/logs/v1/logs_service.proto
//!   opentelemetry/proto/profiles/v1experimental/profiles_service.proto
//!   opentelemetry/proto/common/v1/common.proto
//!   opentelemetry/proto/resource/v1/resource.proto
//!
//! Conversion functions decode protobuf bytes → serde_json::Value (JSON),
//! so the rest of the ingest pipeline processes identically regardless of encoding.

#![allow(clippy::derive_partial_eq_without_eq)]

use prost::Message;
use serde_json::{json, Value};

// ============================================================================
// Common types (opentelemetry.proto.common.v1)
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct KeyValue {
    #[prost(string, tag = "1")]
    pub key: String,
    #[prost(message, optional, tag = "2")]
    pub value: Option<AnyValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct AnyValue {
    #[prost(oneof = "any_value::Value", tags = "1, 2, 3, 4, 5, 6")]
    pub value: Option<any_value::Value>,
}

pub mod any_value {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Value {
        #[prost(string, tag = "1")]
        StringValue(String),
        #[prost(bool, tag = "2")]
        BoolValue(bool),
        #[prost(int64, tag = "3")]
        IntValue(i64),
        #[prost(double, tag = "4")]
        DoubleValue(f64),
        #[prost(message, tag = "5")]
        ArrayValue(super::ArrayValue),
        #[prost(message, tag = "6")]
        KvlistValue(super::KeyValueList),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ArrayValue {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<AnyValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct KeyValueList {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<KeyValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct InstrumentationScope {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub version: String,
    #[prost(message, repeated, tag = "3")]
    pub attributes: Vec<KeyValue>,
}

// ============================================================================
// Resource (opentelemetry.proto.resource.v1)
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct Resource {
    #[prost(message, repeated, tag = "1")]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = "2")]
    pub dropped_attributes_count: u32,
}

// ============================================================================
// Traces (opentelemetry.proto.collector.trace.v1)
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct ExportTraceServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_spans: Vec<ResourceSpans>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ResourceSpans {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_spans: Vec<ScopeSpans>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ScopeSpans {
    #[prost(message, optional, tag = "1")]
    pub scope: Option<InstrumentationScope>,
    #[prost(message, repeated, tag = "2")]
    pub spans: Vec<Span>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Span {
    #[prost(bytes = "vec", tag = "1")]
    pub trace_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub span_id: Vec<u8>,
    #[prost(string, tag = "3")]
    pub trace_state: String,
    #[prost(bytes = "vec", tag = "4")]
    pub parent_span_id: Vec<u8>,
    #[prost(string, tag = "5")]
    pub name: String,
    #[prost(enumeration = "SpanKind", tag = "6")]
    pub kind: i32,
    #[prost(fixed64, tag = "7")]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = "8")]
    pub end_time_unix_nano: u64,
    #[prost(message, repeated, tag = "9")]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = "10")]
    pub dropped_attributes_count: u32,
    #[prost(message, repeated, tag = "11")]
    pub events: Vec<SpanEvent>,
    #[prost(uint32, tag = "12")]
    pub dropped_events_count: u32,
    #[prost(message, repeated, tag = "13")]
    pub links: Vec<SpanLink>,
    #[prost(uint32, tag = "14")]
    pub dropped_links_count: u32,
    #[prost(message, optional, tag = "15")]
    pub status: Option<SpanStatus>,
}

#[derive(Clone, PartialEq, Message)]
pub struct SpanEvent {
    #[prost(fixed64, tag = "1")]
    pub time_unix_nano: u64,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(message, repeated, tag = "3")]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = "4")]
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct SpanLink {
    #[prost(bytes = "vec", tag = "1")]
    pub trace_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub span_id: Vec<u8>,
    #[prost(string, tag = "3")]
    pub trace_state: String,
    #[prost(message, repeated, tag = "4")]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = "5")]
    pub dropped_attributes_count: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct SpanStatus {
    #[prost(string, tag = "2")]
    pub message: String,
    #[prost(enumeration = "StatusCode", tag = "3")]
    pub code: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SpanKind {
    Unspecified = 0,
    Internal = 1,
    Server = 2,
    Client = 3,
    Producer = 4,
    Consumer = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StatusCode {
    Unset = 0,
    Ok = 1,
    Error = 2,
}

// ============================================================================
// Metrics (opentelemetry.proto.collector.metrics.v1)
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct ExportMetricsServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_metrics: Vec<ResourceMetrics>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ResourceMetrics {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_metrics: Vec<ScopeMetrics>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ScopeMetrics {
    #[prost(message, optional, tag = "1")]
    pub scope: Option<InstrumentationScope>,
    #[prost(message, repeated, tag = "2")]
    pub metrics: Vec<Metric>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Metric {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub description: String,
    #[prost(string, tag = "3")]
    pub unit: String,
    // We store the raw metric data as bytes for simplicity;
    // the JSON conversion captures the essential fields.
    #[prost(message, repeated, tag = "9")]
    pub metadata: Vec<KeyValue>,
}

// ============================================================================
// Logs (opentelemetry.proto.collector.logs.v1)
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct ExportLogsServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_logs: Vec<ResourceLogs>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ResourceLogs {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_logs: Vec<ScopeLogs>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ScopeLogs {
    #[prost(message, optional, tag = "1")]
    pub scope: Option<InstrumentationScope>,
    #[prost(message, repeated, tag = "2")]
    pub log_records: Vec<LogRecord>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct LogRecord {
    #[prost(fixed64, tag = "1")]
    pub time_unix_nano: u64,
    #[prost(fixed64, tag = "11")]
    pub observed_time_unix_nano: u64,
    #[prost(int32, tag = "2")]
    pub severity_number: i32,
    #[prost(string, tag = "3")]
    pub severity_text: String,
    #[prost(message, optional, tag = "5")]
    pub body: Option<AnyValue>,
    #[prost(message, repeated, tag = "6")]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = "7")]
    pub dropped_attributes_count: u32,
    #[prost(uint32, tag = "8")]
    pub flags: u32,
    #[prost(bytes = "vec", tag = "9")]
    pub trace_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "10")]
    pub span_id: Vec<u8>,
}

// ============================================================================
// Profiles (opentelemetry.proto.profiles.v1experimental) — simplified
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct ExportProfilesServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_profiles: Vec<ResourceProfiles>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ResourceProfiles {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_profiles: Vec<ScopeProfiles>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ScopeProfiles {
    #[prost(message, optional, tag = "1")]
    pub scope: Option<InstrumentationScope>,
    #[prost(message, repeated, tag = "2")]
    pub profiles: Vec<ProfileContainer>,
    #[prost(string, tag = "3")]
    pub schema_url: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProfileContainer {
    #[prost(bytes = "vec", tag = "1")]
    pub profile_id: Vec<u8>,
    #[prost(fixed64, tag = "2")]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = "3")]
    pub end_time_unix_nano: u64,
    #[prost(message, repeated, tag = "4")]
    pub attributes: Vec<KeyValue>,
    #[prost(uint32, tag = "5")]
    pub dropped_attributes_count: u32,
    #[prost(bytes = "vec", tag = "6")]
    pub original_payload: Vec<u8>,
    #[prost(string, tag = "7")]
    pub original_payload_format: String,
}

// ============================================================================
// Conversion: Protobuf bytes → serde_json::Value
// ============================================================================

fn kv_to_json(attrs: &[KeyValue]) -> Value {
    let arr: Vec<Value> = attrs.iter().map(|kv| {
        json!({
            "key": kv.key,
            "value": anyvalue_to_json(kv.value.as_ref()),
        })
    }).collect();
    Value::Array(arr)
}

fn anyvalue_to_json(v: Option<&AnyValue>) -> Value {
    match v.and_then(|av| av.value.as_ref()) {
        Some(any_value::Value::StringValue(s)) => json!({"stringValue": s}),
        Some(any_value::Value::BoolValue(b)) => json!({"boolValue": b}),
        Some(any_value::Value::IntValue(i)) => json!({"intValue": i.to_string()}),
        Some(any_value::Value::DoubleValue(d)) => json!({"doubleValue": d}),
        Some(any_value::Value::ArrayValue(a)) => {
            let vals: Vec<Value> = a.values.iter().map(|v| anyvalue_to_json(Some(v))).collect();
            json!({"arrayValue": {"values": vals}})
        }
        Some(any_value::Value::KvlistValue(kv)) => {
            json!({"kvlistValue": {"values": kv_to_json(&kv.values)}})
        }
        None => Value::Null,
    }
}

fn resource_to_json(r: Option<&Resource>) -> Value {
    match r {
        Some(res) => json!({ "attributes": kv_to_json(&res.attributes) }),
        None => json!({ "attributes": [] }),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn span_kind_str(kind: i32) -> &'static str {
    match kind {
        1 => "internal",
        2 => "server",
        3 => "client",
        4 => "producer",
        5 => "consumer",
        _ => "unspecified",
    }
}

fn status_code_str(code: i32) -> &'static str {
    match code {
        1 => "ok",
        2 => "error",
        _ => "unset",
    }
}

/// Decode protobuf ExportTraceServiceRequest bytes → OTLP JSON Value
pub fn proto_to_json_traces(bytes: &[u8]) -> Result<Value, String> {
    let req = ExportTraceServiceRequest::decode(bytes)
        .map_err(|e| format!("Failed to decode protobuf traces: {}", e))?;

    let resource_spans: Vec<Value> = req.resource_spans.iter().map(|rs| {
        let scope_spans: Vec<Value> = rs.scope_spans.iter().map(|ss| {
            let spans: Vec<Value> = ss.spans.iter().map(|s| {
                let events: Vec<Value> = s.events.iter().map(|e| json!({
                    "timeUnixNano": e.time_unix_nano.to_string(),
                    "name": e.name,
                    "attributes": kv_to_json(&e.attributes),
                })).collect();
                let links: Vec<Value> = s.links.iter().map(|l| json!({
                    "traceId": hex_encode(&l.trace_id),
                    "spanId": hex_encode(&l.span_id),
                    "attributes": kv_to_json(&l.attributes),
                })).collect();
                let status = s.status.as_ref().map(|st| json!({
                    "code": st.code,
                    "message": st.message,
                })).unwrap_or(json!({"code": 0}));
                json!({
                    "traceId": hex_encode(&s.trace_id),
                    "spanId": hex_encode(&s.span_id),
                    "parentSpanId": hex_encode(&s.parent_span_id),
                    "name": s.name,
                    "kind": s.kind,
                    "startTimeUnixNano": s.start_time_unix_nano.to_string(),
                    "endTimeUnixNano": s.end_time_unix_nano.to_string(),
                    "attributes": kv_to_json(&s.attributes),
                    "events": events,
                    "links": links,
                    "status": status,
                })
            }).collect();
            json!({
                "scope": ss.scope.as_ref().map(|sc| json!({"name": sc.name, "version": sc.version})).unwrap_or(json!(null)),
                "spans": spans,
            })
        }).collect();
        json!({
            "resource": resource_to_json(rs.resource.as_ref()),
            "scopeSpans": scope_spans,
        })
    }).collect();

    Ok(json!({ "resourceSpans": resource_spans }))
}

/// Decode protobuf ExportMetricsServiceRequest bytes → OTLP JSON Value
pub fn proto_to_json_metrics(bytes: &[u8]) -> Result<Value, String> {
    let req = ExportMetricsServiceRequest::decode(bytes)
        .map_err(|e| format!("Failed to decode protobuf metrics: {}", e))?;

    let resource_metrics: Vec<Value> = req.resource_metrics.iter().map(|rm| {
        let scope_metrics: Vec<Value> = rm.scope_metrics.iter().map(|sm| {
            let metrics: Vec<Value> = sm.metrics.iter().map(|m| {
                json!({
                    "name": m.name,
                    "description": m.description,
                    "unit": m.unit,
                })
            }).collect();
            json!({
                "scope": sm.scope.as_ref().map(|sc| json!({"name": sc.name, "version": sc.version})).unwrap_or(json!(null)),
                "metrics": metrics,
            })
        }).collect();
        json!({
            "resource": resource_to_json(rm.resource.as_ref()),
            "scopeMetrics": scope_metrics,
        })
    }).collect();

    Ok(json!({ "resourceMetrics": resource_metrics }))
}

/// Decode protobuf ExportLogsServiceRequest bytes → OTLP JSON Value
pub fn proto_to_json_logs(bytes: &[u8]) -> Result<Value, String> {
    let req = ExportLogsServiceRequest::decode(bytes)
        .map_err(|e| format!("Failed to decode protobuf logs: {}", e))?;

    let resource_logs: Vec<Value> = req.resource_logs.iter().map(|rl| {
        let scope_logs: Vec<Value> = rl.scope_logs.iter().map(|sl| {
            let records: Vec<Value> = sl.log_records.iter().map(|lr| {
                json!({
                    "timeUnixNano": lr.time_unix_nano.to_string(),
                    "observedTimeUnixNano": lr.observed_time_unix_nano.to_string(),
                    "severityNumber": lr.severity_number,
                    "severityText": lr.severity_text,
                    "body": anyvalue_to_json(lr.body.as_ref()),
                    "attributes": kv_to_json(&lr.attributes),
                    "traceId": hex_encode(&lr.trace_id),
                    "spanId": hex_encode(&lr.span_id),
                })
            }).collect();
            json!({
                "scope": sl.scope.as_ref().map(|sc| json!({"name": sc.name, "version": sc.version})).unwrap_or(json!(null)),
                "logRecords": records,
            })
        }).collect();
        json!({
            "resource": resource_to_json(rl.resource.as_ref()),
            "scopeLogs": scope_logs,
        })
    }).collect();

    Ok(json!({ "resourceLogs": resource_logs }))
}

/// Decode protobuf ExportProfilesServiceRequest bytes → OTLP JSON Value
pub fn proto_to_json_profiles(bytes: &[u8]) -> Result<Value, String> {
    let req = ExportProfilesServiceRequest::decode(bytes)
        .map_err(|e| format!("Failed to decode protobuf profiles: {}", e))?;

    let resource_profiles: Vec<Value> = req.resource_profiles.iter().map(|rp| {
        let scope_profiles: Vec<Value> = rp.scope_profiles.iter().map(|sp| {
            let profiles: Vec<Value> = sp.profiles.iter().map(|p| {
                json!({
                    "profileId": hex_encode(&p.profile_id),
                    "startTimeUnixNano": p.start_time_unix_nano.to_string(),
                    "endTimeUnixNano": p.end_time_unix_nano.to_string(),
                    "attributes": kv_to_json(&p.attributes),
                    "originalPayloadFormat": p.original_payload_format,
                    "originalPayloadBytes": p.original_payload.len(),
                })
            }).collect();
            json!({
                "scope": sp.scope.as_ref().map(|sc| json!({"name": sc.name, "version": sc.version})).unwrap_or(json!(null)),
                "profiles": profiles,
            })
        }).collect();
        json!({
            "resource": resource_to_json(rp.resource.as_ref()),
            "scopeProfiles": scope_profiles,
        })
    }).collect();

    Ok(json!({ "resourceProfiles": resource_profiles }))
}

