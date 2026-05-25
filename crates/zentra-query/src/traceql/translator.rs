//! TraceQL translator: converts parsed AST into SQL WHERE clauses.

use super::parser::{Query, Condition, CompareOp, LogicOp};
use super::{TraceSummary, TraceQlError};
use chrono::{DateTime, Utc};

/// Translate a parsed TraceQL query into a SQL WHERE fragment and bind values.
pub fn translate(query: &Query) -> Result<(String, Vec<BindValue>), TraceQlError> {
    let mut parts = Vec::new();
    let mut binds = Vec::new();

    for (i, cond) in query.conditions.iter().enumerate() {
        let fragment = translate_condition(cond, &mut binds)?;
        if i > 0 {
            let logic = query.logic.get(i - 1).unwrap_or(&LogicOp::And);
            let op = match logic {
                LogicOp::And => "AND",
                LogicOp::Or => "OR",
            };
            parts.push(format!(" {} {}", op, fragment));
        } else {
            parts.push(fragment);
        }
    }

    Ok((parts.join(""), binds))
}

fn translate_condition(cond: &Condition, binds: &mut Vec<BindValue>) -> Result<String, TraceQlError> {
    match cond {
        Condition::StatusMatch { status } => {
            binds.push(BindValue::String(status.clone()));
            Ok(format!("status = ${}", binds.len() + 2)) // +2 because $1=from, $2=to
        }
        Condition::DurationMatch { op, duration_ms } => {
            let sql_op = compare_op_sql(op);
            // duration stored as duration_us in spans table, convert ms to us
            Ok(format!("duration_us {} {}", sql_op, duration_ms * 1000))
        }
        Condition::AttributeMatch { attribute, op, value } => {
            let column = attribute_to_column(attribute);
            let sql_op = compare_op_sql(op);
            binds.push(BindValue::String(value.clone()));
            let bind_num = binds.len() + 2;

            match op {
                CompareOp::RegexMatch => Ok(format!("{} ~ ${}", column, bind_num)),
                CompareOp::RegexNoMatch => Ok(format!("{} !~ ${}", column, bind_num)),
                _ => Ok(format!("{} {} ${}", column, sql_op, bind_num)),
            }
        }
    }
}

fn attribute_to_column(attr: &str) -> String {
    match attr {
        "service.name" => "service_name".to_string(),
        "span.name" => "operation_name".to_string(),
        "status" => "status".to_string(),
        "duration" => "duration_us".to_string(),
        a if a.starts_with("span.") => {
            let key = a.strip_prefix("span.").unwrap_or(a);
            format!("tags->>'{}'", key)
        }
        a if a.starts_with("resource.") => {
            let key = a.strip_prefix("resource.").unwrap_or(a);
            format!("tags->>'{}'", key)
        }
        a => format!("tags->>'{}'", a),
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

#[derive(Debug, Clone)]
pub enum BindValue {
    String(String),
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
            FROM spans
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
