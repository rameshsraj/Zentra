//! Item 3: Per-query OTEL DB spans for zentra-db
//!
//! Wraps sqlx queries with semantic-convention DB spans so latency,
//! query text, and errors appear in trace waterfalls.

use std::future::Future;

const REDACTED_TABLES: &[&str] = &["users", "secrets", "audit", "user_module_access"];

/// Execute an async DB operation inside an OTel-convention span.
///
/// Sets `db.system=postgresql`, `db.operation`, `db.sql.table`, `db.statement`
/// (truncated to 1KB, redacted for sensitive tables), and `otel.kind=client`.
///
/// Usage:
/// ```ignore
/// traced_query("SELECT", "SELECT * FROM targets WHERE ...", "targets", async {
///     sqlx::query("...").fetch_all(pool).await
/// }).await
/// ```
pub async fn traced_query<F, T, E>(operation: &str, statement: &str, table: &str, fut: F) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let is_sensitive = REDACTED_TABLES.iter().any(|t| table.eq_ignore_ascii_case(t));

    let stmt_display = if is_sensitive {
        "[REDACTED]".to_string()
    } else if statement.len() > 1024 {
        format!("{}...", &statement[..1024])
    } else {
        statement.to_string()
    };

    let span = tracing::info_span!(
        "db.query",
        db.system = "postgresql",
        db.operation = %operation,
        db.sql.table = %table,
        db.statement = %stmt_display,
        db.statement.redacted = is_sensitive,
        otel.kind = "client",
        otel.status_code = tracing::field::Empty,
    );

    let _guard = span.enter();
    drop(_guard); // release synchronous guard before await

    let result = {
        use tracing::Instrument;
        fut.instrument(span.clone()).await
    };

    if let Err(ref e) = result {
        span.record("otel.status_code", "error");
        tracing::error!(parent: &span, error = %e, "DB query failed");
    }

    result
}
