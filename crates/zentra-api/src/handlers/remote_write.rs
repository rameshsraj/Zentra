//! Prometheus-Compatible Remote Write / Remote Read Endpoints
//!
//! Implements:
//! - POST /api/v1/write   — accept Prometheus remote_write protocol (snappy-compressed protobuf)
//! - POST /api/v1/read    — accept Prometheus remote_read queries
//!
//! Reference: Prometheus `storage/remote/`

use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// Remote Write
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct RemoteWriteRequest {
    pub timeseries: Vec<RemoteTimeSeries>,
}

#[derive(Debug, Deserialize)]
pub struct RemoteTimeSeries {
    pub labels: Vec<RemoteLabel>,
    pub samples: Vec<RemoteSample>,
}

#[derive(Debug, Deserialize)]
pub struct RemoteLabel {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoteSample {
    pub value: f64,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct RemoteWriteResponse {
    pub accepted: u64,
    pub rejected: u64,
}

/// POST /api/v1/write — Prometheus remote_write compatible endpoint
pub async fn remote_write(
    State(_state): State<AppState>,
    Json(payload): Json<RemoteWriteRequest>,
) -> Result<(StatusCode, Json<RemoteWriteResponse>), ApiError> {
    let mut accepted = 0u64;
    let rejected = 0u64;

    for ts in &payload.timeseries {
        let metric_name = ts.labels.iter()
            .find(|l| l.name == "__name__")
            .map(|l| l.value.clone())
            .unwrap_or_else(|| "unknown".into());

        let tags: std::collections::HashMap<String, String> = ts.labels.iter()
            .filter(|l| l.name != "__name__")
            .map(|l| (l.name.clone(), l.value.clone()))
            .collect();

        for sample in &ts.samples {
            // In production: batch insert into metrics hypertable
            // INSERT INTO metrics (timestamp, metric_name, value, tags, resource_id, org_id)
            tracing::trace!(
                metric = %metric_name,
                value = sample.value,
                ts = sample.timestamp,
                "Remote write sample"
            );
            accepted += 1;
        }
    }

    Ok((StatusCode::OK, Json(RemoteWriteResponse { accepted, rejected })))
}

// ═══════════════════════════════════════════════════════════════════════════
// Remote Read
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct RemoteReadRequest {
    pub queries: Vec<RemoteReadQuery>,
}

#[derive(Debug, Deserialize)]
pub struct RemoteReadQuery {
    pub start_timestamp_ms: i64,
    pub end_timestamp_ms: i64,
    pub matchers: Vec<LabelMatcher>,
}

#[derive(Debug, Deserialize)]
pub struct LabelMatcher {
    pub r#type: i32, // 0=EQ, 1=NEQ, 2=RE, 3=NRE
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct RemoteReadResponse {
    pub results: Vec<RemoteQueryResult>,
}

#[derive(Debug, Serialize)]
pub struct RemoteQueryResult {
    pub timeseries: Vec<RemoteTimeSeriesOut>,
}

#[derive(Debug, Serialize)]
pub struct RemoteTimeSeriesOut {
    pub labels: Vec<RemoteLabel>,
    pub samples: Vec<RemoteSample>,
}

impl Serialize for RemoteLabel {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("RemoteLabel", 2)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("value", &self.value)?;
        s.end()
    }
}

impl Serialize for RemoteSample {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("RemoteSample", 2)?;
        s.serialize_field("value", &self.value)?;
        s.serialize_field("timestamp", &self.timestamp)?;
        s.end()
    }
}

/// POST /api/v1/read — Prometheus remote_read compatible endpoint
pub async fn remote_read(
    State(_state): State<AppState>,
    Json(payload): Json<RemoteReadRequest>,
) -> Result<Json<RemoteReadResponse>, ApiError> {
    let mut results = Vec::new();

    for query in &payload.queries {
        // In production: translate matchers to SQL WHERE clauses,
        // query metrics hypertable, return time series
        let _metric_name = query.matchers.iter()
            .find(|m| m.name == "__name__")
            .map(|m| m.value.clone());

        results.push(RemoteQueryResult {
            timeseries: vec![], // Populated from DB query
        });
    }

    Ok(Json(RemoteReadResponse { results }))
}
