//! MQE-Lite: Metrics Query Expression language (inspired by SkyWalking MQE).
//!
//! Supports expressions like:
//!   avg(cpu_usage_percent{host="web-01"})
//!   rate(network_bytes_received[5m])
//!   sum(http_errors{status="500"}) / sum(http_requests)

pub mod lexer;
pub mod parser;
pub mod evaluator;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    pub metric_name: String,
    pub labels: serde_json::Value,
    pub values: Vec<TimeSeriesPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MqeQuery {
    pub expr: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub step_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MqeValidationResult {
    pub valid: bool,
    pub error: Option<MqeValidationError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MqeValidationError {
    pub position: usize,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum MqeError {
    #[error("parse error at position {position}: {message}")]
    Parse { position: usize, message: String },
    #[error("evaluation error: {0}")]
    Eval(String),
    #[error("database error: {0}")]
    Database(String),
}

/// Validate an MQE expression without executing it.
pub fn validate(expr: &str) -> MqeValidationResult {
    let tokens = match lexer::tokenize(expr) {
        Ok(t) => t,
        Err(e) => return MqeValidationResult {
            valid: false,
            error: Some(MqeValidationError { position: e.position, message: e.message }),
        },
    };
    match parser::parse(&tokens) {
        Ok(_) => MqeValidationResult { valid: true, error: None },
        Err(e) => MqeValidationResult {
            valid: false,
            error: Some(MqeValidationError { position: e.position, message: e.message }),
        },
    }
}

/// Execute an MQE expression against the database.
pub async fn run(
    query: &MqeQuery,
    org_id: uuid::Uuid,
    pool: &sqlx::PgPool,
) -> Result<Vec<TimeSeries>, MqeError> {
    let tokens = lexer::tokenize(&query.expr).map_err(|e| MqeError::Parse {
        position: e.position,
        message: e.message,
    })?;
    let ast = parser::parse(&tokens).map_err(|e| MqeError::Parse {
        position: e.position,
        message: e.message,
    })?;
    let step = query.step_seconds.unwrap_or_else(|| {
        let dur = query.to.signed_duration_since(query.from).num_seconds();
        match dur {
            0..=3600 => 10,
            3601..=21600 => 60,
            21601..=86400 => 300,
            86401..=604800 => 3600,
            _ => 86400,
        }
    });
    evaluator::evaluate(&ast, org_id, query.from, query.to, step, pool).await
}
