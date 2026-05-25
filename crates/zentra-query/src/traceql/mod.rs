//! TraceQL: Trace query language (inspired by Grafana Tempo TraceQL).
//!
//! Supports queries like:
//!   {service.name="order-service"}
//!   {status=error}
//!   {duration > 1s}
//!   {service.name="order-service" && duration > 500ms}

pub mod lexer;
pub mod parser;
pub mod translator;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    pub trace_id: String,
    pub root_service: String,
    pub root_operation: String,
    pub duration_ms: f64,
    pub span_count: i64,
    pub has_errors: bool,
    pub services: Vec<String>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TraceQlQuery {
    pub expr: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceQlValidationResult {
    pub valid: bool,
    pub error: Option<TraceQlValidationError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceQlValidationError {
    pub position: usize,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum TraceQlError {
    #[error("parse error at position {position}: {message}")]
    Parse { position: usize, message: String },
    #[error("translation error: {0}")]
    Translation(String),
    #[error("database error: {0}")]
    Database(String),
}

/// Validate a TraceQL expression without executing it.
pub fn validate(expr: &str) -> TraceQlValidationResult {
    let tokens = match lexer::tokenize(expr) {
        Ok(t) => t,
        Err(e) => return TraceQlValidationResult {
            valid: false,
            error: Some(TraceQlValidationError { position: e.position, message: e.message }),
        },
    };
    match parser::parse(&tokens) {
        Ok(_) => TraceQlValidationResult { valid: true, error: None },
        Err(e) => TraceQlValidationResult {
            valid: false,
            error: Some(TraceQlValidationError { position: e.position, message: e.message }),
        },
    }
}

/// Execute a TraceQL query against the database.
pub async fn run(
    query: &TraceQlQuery,
    pool: &sqlx::PgPool,
) -> Result<Vec<TraceSummary>, TraceQlError> {
    let tokens = lexer::tokenize(&query.expr).map_err(|e| TraceQlError::Parse {
        position: e.position,
        message: e.message,
    })?;
    let ast = parser::parse(&tokens).map_err(|e| TraceQlError::Parse {
        position: e.position,
        message: e.message,
    })?;

    let from = query.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = query.to.unwrap_or_else(Utc::now);
    let limit = query.limit.unwrap_or(50).min(1000);

    let (where_clause, binds) = translator::translate(&ast)?;

    translator::execute_query(&where_clause, &binds, from, to, limit, pool).await
}
