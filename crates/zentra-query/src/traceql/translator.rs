//! TraceQL translator: converts parsed AST into SQL WHERE clauses.
//!
//! Supports:
//! - Native column matching (service_name, operation_name, status)
//! - Duration comparisons (stored as duration_us, input as milliseconds)
//! - Arbitrary JSONB tag queries via `tags ->> 'key'` with parameterised binds
//! - AND / OR logic with correct parenthesisation
//! - Regex match (`=~`) and regex-no-match (`!~`) operators

use super::parser::{Query, Condition, CompareOp, LogicOp};
use super::{TraceSummary, TraceQlError};
use chrono::{DateTime, Utc};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Translate a parsed TraceQL query into a SQL WHERE fragment and bind values.
///
/// Bind numbering starts at `$3` because the caller reserves `$1` = from and
/// `$2` = to for the time-range predicate.
pub fn translate(query: &Query) -> Result<(String, Vec<BindValue>), TraceQlError> {
    if query.conditions.is_empty() {
        return Ok(("TRUE".to_string(), vec![]));
    }

    let mut binds = Vec::new();

    // When the query mixes AND and OR we must group them to avoid ambiguity.
    // Strategy: translate each condition into a fragment, then combine
    // respecting the logic operators with parentheses around OR groups.
    let mut fragments: Vec<String> = Vec::with_capacity(query.conditions.len());
    for cond in &query.conditions {
        fragments.push(translate_condition(cond, &mut binds)?);
    }

    // Build the combined expression.
    // If there is only one condition, no grouping needed.
    if fragments.len() == 1 {
        return Ok((fragments.remove(0), binds));
    }

    // Check whether we have mixed AND/OR.
    let has_and = query.logic.iter().any(|l| matches!(l, LogicOp::And));
    let has_or  = query.logic.iter().any(|l| matches!(l, LogicOp::Or));

    if has_and && has_or {
        // Mixed operators: wrap OR-connected groups in parentheses so AND
        // binds more tightly (standard precedence).
        let result = build_mixed_logic(&fragments, &query.logic);
        Ok((result, binds))
    } else {
        // Uniform logic – simple join.
        let op_str = if has_or { " OR " } else { " AND " };
        Ok((fragments.join(op_str), binds))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build SQL for queries that mix AND and OR.
///
/// Groups consecutive AND-connected conditions together and parenthesises
/// them so that `A && B || C && D` becomes `(A AND B) OR (C AND D)`.
fn build_mixed_logic(fragments: &[String], logic: &[LogicOp]) -> String {
    let mut groups: Vec<Vec<&str>> = vec![vec![&fragments[0]]];

    for (i, op) in logic.iter().enumerate() {
        match op {
            LogicOp::And => {
                groups.last_mut().unwrap().push(&fragments[i + 1]);
            }
            LogicOp::Or => {
                groups.push(vec![&fragments[i + 1]]);
            }
        }
    }

    let group_strs: Vec<String> = groups.iter().map(|g| {
        if g.len() == 1 {
            g[0].to_string()
        } else {
            format!("({})", g.join(" AND "))
        }
    }).collect();

    group_strs.join(" OR ")
}

fn translate_condition(cond: &Condition, binds: &mut Vec<BindValue>) -> Result<String, TraceQlError> {
    match cond {
        Condition::StatusMatch { status } => {
            binds.push(BindValue::String(status.clone()));
            Ok(format!("status = ${}", binds.len() + 2))
        }
        Condition::DurationMatch { op, duration_ms } => {
            let sql_op = compare_op_sql(op);
            binds.push(BindValue::Int(*duration_ms * 1000));
            Ok(format!("duration_us {} ${}", sql_op, binds.len() + 2))
        }
        Condition::AttributeMatch { attribute, op, value } => {
            let col = resolve_column(attribute);
            let sql_op = compare_op_sql(op);
            binds.push(BindValue::String(value.clone()));
            let bind_num = binds.len() + 2;

            match op {
                CompareOp::RegexMatch => Ok(format!("{} ~ ${}", col, bind_num)),
                CompareOp::RegexNoMatch => Ok(format!("{} !~ ${}", col, bind_num)),
                _ => Ok(format!("{} {} ${}", col, sql_op, bind_num)),
            }
        }
    }
}

/// Map a TraceQL attribute name to a SQL column expression.
///
/// Well-known semconv attributes are mapped to native indexed columns.
/// Everything else becomes a JSONB `tags ->> 'key'` lookup so that
/// arbitrary user-defined tags are queryable.
fn resolve_column(attr: &str) -> String {
    match attr {
        // Native indexed columns
        "service.name"    => "service_name".to_string(),
        "span.name"       => "operation_name".to_string(),
        "name"            => "operation_name".to_string(),
        "status"          => "status".to_string(),
        "duration"        => "duration_us".to_string(),
        "trace_id"        => "trace_id".to_string(),
        "span_id"         => "span_id".to_string(),

        // Semconv shortcuts  (OTel semconv → native columns if available)
        "http.method"     => "tags->>'http.method'".to_string(),
        "http.status_code" => "(tags->>'http.status_code')::int".to_string(),

        // span.* and resource.* prefixes → JSONB lookup
        a if a.starts_with("span.") => {
            let key = a.strip_prefix("span.").unwrap_or(a);
            format!("tags->>'{}'" , key)
        }
        a if a.starts_with("resource.") => {
            let key = a.strip_prefix("resource.").unwrap_or(a);
            format!("tags->>'{}'" , key)
        }

        // Fallback: treat as a tag key
        a => format!("tags->>'{}'" , a),
    }
}

fn compare_op_sql(op: &CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "=",
        CompareOp::Neq => "!=",
        CompareOp::Gt => ">",
        CompareOp::Gte => ">=",
        CompareOp::Lt => "<",
        CompareOp::Lte => "<=",
        CompareOp::RegexMatch => "~",
        CompareOp::RegexNoMatch => "!~",
    }
}

// ---------------------------------------------------------------------------
// Bind values
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum BindValue {
    String(String),
    Int(i64),
}

/// Execute the translated query against the database.
pub async fn execute_query(
    where_clause: &str,
    binds: &[BindValue],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    limit: u32,
    pool: &sqlx::PgPool,
) -> Result<Vec<TraceSummary>, TraceQlError> {
    let sql = format!(
        r#"WITH matching_spans AS (
            SELECT trace_id, service_name, operation_name, duration_us, started_at, status
            FROM traces
            WHERE started_at >= $1 AND started_at <= $2 AND {}
            ORDER BY started_at DESC
            LIMIT {}
        )
        SELECT
            trace_id,
            MIN(service_name) AS root_service,
            MIN(operation_name) AS root_operation,
            MAX(duration_us)::FLOAT / 1000.0 AS duration_ms,
            COUNT(*) AS span_count,
            BOOL_OR(status = 'error') AS has_errors,
            ARRAY_AGG(DISTINCT service_name) AS services,
            MIN(started_at) AS started_at
        FROM matching_spans
        GROUP BY trace_id
        ORDER BY started_at DESC"#,
        where_clause,
        limit
    );

    let mut query = sqlx::query_as::<_, TraceResultRow>(&sql)
        .bind(from)
        .bind(to);

    for bind in binds {
        match bind {
            BindValue::String(s) => { query = query.bind(s.clone()); }
            BindValue::Int(n) => { query = query.bind(*n); }
        }
    }

    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| TraceQlError::Database(e.to_string()))?;

    Ok(rows.into_iter().map(|r| TraceSummary {
        trace_id: r.trace_id,
        root_service: r.root_service.unwrap_or_default(),
        root_operation: r.root_operation.unwrap_or_default(),
        duration_ms: r.duration_ms.unwrap_or(0.0),
        span_count: r.span_count,
        has_errors: r.has_errors.unwrap_or(false),
        services: r.services.unwrap_or_default(),
        started_at: r.started_at,
    }).collect())
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TraceResultRow {
    trace_id: String,
    root_service: Option<String>,
    root_operation: Option<String>,
    duration_ms: Option<f64>,
    span_count: i64,
    has_errors: Option<bool>,
    services: Option<Vec<String>>,
    started_at: DateTime<Utc>,
}
