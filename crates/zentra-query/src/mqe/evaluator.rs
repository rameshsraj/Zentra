//! MQE-Lite evaluator: translates parsed AST into SQL and executes against TimescaleDB.

use super::{TimeSeries, TimeSeriesPoint, MqeError};
use super::parser::{Expr, AggFunc, MathOp, MatchOp};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Evaluate a parsed MQE expression against the database.
pub async fn evaluate(
    expr: &Expr,
    org_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    step_seconds: i64,
    pool: &sqlx::PgPool,
) -> Result<Vec<TimeSeries>, MqeError> {
    match expr {
        Expr::Selector { metric, matchers } => {
            let mut where_clauses = vec![
                "org_id = $1".to_string(),
                "metric_name = $2".to_string(),
                "time >= $3".to_string(),
                "time <= $4".to_string(),
            ];
            let mut bind_idx = 5u32;
            let mut extra_binds: Vec<String> = Vec::new();

            for m in matchers {
                let clause = match m.op {
                    MatchOp::Eq => format!("labels->>'{}' = ${}", m.key, bind_idx),
                    MatchOp::Neq => format!("labels->>'{}' != ${}", m.key, bind_idx),
                    MatchOp::RegexMatch => format!("labels->>'{}' ~ ${}", m.key, bind_idx),
                    MatchOp::RegexNoMatch => format!("labels->>'{}' !~ ${}", m.key, bind_idx),
                };
                where_clauses.push(clause);
                extra_binds.push(m.value.clone());
                bind_idx += 1;
            }

            let sql = format!(
                r#"SELECT time_bucket('{step} seconds', time) AS bucket, AVG(value) AS val
                   FROM metrics
                   WHERE {where_clause}
                   GROUP BY bucket
                   ORDER BY bucket"#,
                step = step_seconds,
                where_clause = where_clauses.join(" AND "),
            );

            let mut query = sqlx::query_as::<_, BucketRow>(&sql)
                .bind(org_id)
                .bind(metric)
                .bind(from)
                .bind(to);

            for val in &extra_binds {
                query = query.bind(val);
            }

            let rows = query
                .fetch_all(pool)
                .await
                .map_err(|e| MqeError::Database(e.to_string()))?;

            Ok(vec![TimeSeries {
                metric_name: metric.clone(),
                labels: serde_json::json!({}),
                values: rows.into_iter().map(|r| TimeSeriesPoint {
                    timestamp: r.bucket,
                    value: r.val.unwrap_or(0.0),
                }).collect(),
            }])
        }

        Expr::FuncCall { func, inner, window_seconds } => {
            let (metric, matchers) = extract_selector(inner)?;
            let agg_sql = match func {
                AggFunc::Rate | AggFunc::Increase | AggFunc::Delta => {
                    // Rate uses window function: (last - first) / window
                    let _window = window_seconds.unwrap_or(step_seconds);
                    // For rate, we compute the difference between last and first value in each bucket
                    "rate".to_string()
                }
                other => other.as_sql().to_string(),
            };

            let mut where_clauses = vec![
                "org_id = $1".to_string(),
                "metric_name = $2".to_string(),
                "time >= $3".to_string(),
                "time <= $4".to_string(),
            ];
            let mut bind_idx = 5u32;
            let mut extra_binds: Vec<String> = Vec::new();

            for m in matchers {
                let clause = match m.op {
                    MatchOp::Eq => format!("labels->>'{}' = ${}", m.key, bind_idx),
                    MatchOp::Neq => format!("labels->>'{}' != ${}", m.key, bind_idx),
                    MatchOp::RegexMatch => format!("labels->>'{}' ~ ${}", m.key, bind_idx),
                    MatchOp::RegexNoMatch => format!("labels->>'{}' !~ ${}", m.key, bind_idx),
                };
                where_clauses.push(clause);
                extra_binds.push(m.value.clone());
                bind_idx += 1;
            }

            let sql = if matches!(func, AggFunc::Rate | AggFunc::Increase | AggFunc::Delta) {
                let window = window_seconds.unwrap_or(step_seconds);
                let divisor = if matches!(func, AggFunc::Rate) {
                    format!(" / {}", window)
                } else {
                    String::new()
                };
                format!(
                    r#"WITH bucketed AS (
                        SELECT time_bucket('{step} seconds', time) AS bucket,
                               LAST(value, time) - FIRST(value, time) AS diff
                        FROM metrics
                        WHERE {where_clause}
                        GROUP BY bucket
                    )
                    SELECT bucket, diff{divisor} AS val
                    FROM bucketed
                    ORDER BY bucket"#,
                    step = step_seconds,
                    where_clause = where_clauses.join(" AND "),
                    divisor = divisor,
                )
            } else {
                format!(
                    r#"SELECT time_bucket('{step} seconds', time) AS bucket,
                              {agg}(value) AS val
                       FROM metrics
                       WHERE {where_clause}
                       GROUP BY bucket
                       ORDER BY bucket"#,
                    step = step_seconds,
                    agg = agg_sql,
                    where_clause = where_clauses.join(" AND "),
                )
            };

            let mut query = sqlx::query_as::<_, BucketRow>(&sql)
                .bind(org_id)
                .bind(&metric)
                .bind(from)
                .bind(to);

            for val in &extra_binds {
                query = query.bind(val);
            }

            let rows = query
                .fetch_all(pool)
                .await
                .map_err(|e| MqeError::Database(e.to_string()))?;

            Ok(vec![TimeSeries {
                metric_name: format!("{}({})", func.as_sql().to_lowercase(), metric),
                labels: serde_json::json!({}),
                values: rows.into_iter().map(|r| TimeSeriesPoint {
                    timestamp: r.bucket,
                    value: r.val.unwrap_or(0.0),
                }).collect(),
            }])
        }

        Expr::BinaryOp { left, op, right } => {
            let left_series = Box::pin(evaluate(left, org_id, from, to, step_seconds, pool)).await?;
            let right_series = Box::pin(evaluate(right, org_id, from, to, step_seconds, pool)).await?;

            if left_series.is_empty() || right_series.is_empty() {
                return Ok(vec![]);
            }

            let l = &left_series[0];
            let r = &right_series[0];

            // Align timestamps and compute binary operation
            let mut result_values = Vec::new();
            let r_map: std::collections::HashMap<i64, f64> = r.values.iter()
                .map(|p| (p.timestamp.timestamp(), p.value))
                .collect();

            for lp in &l.values {
                if let Some(&rv) = r_map.get(&lp.timestamp.timestamp()) {
                    let val = match op {
                        MathOp::Add => lp.value + rv,
                        MathOp::Sub => lp.value - rv,
                        MathOp::Mul => lp.value * rv,
                        MathOp::Div => if rv != 0.0 { lp.value / rv } else { 0.0 },
                    };
                    result_values.push(TimeSeriesPoint {
                        timestamp: lp.timestamp,
                        value: val,
                    });
                }
            }

            let op_str = match op {
                MathOp::Add => "+",
                MathOp::Sub => "-",
                MathOp::Mul => "*",
                MathOp::Div => "/",
            };

            Ok(vec![TimeSeries {
                metric_name: format!("{} {} {}", l.metric_name, op_str, r.metric_name),
                labels: serde_json::json!({}),
                values: result_values,
            }])
        }
    }
}

fn extract_selector(expr: &Expr) -> Result<(String, &[super::parser::LabelMatcher]), MqeError> {
    match expr {
        Expr::Selector { metric, matchers } => Ok((metric.clone(), matchers)),
        _ => Err(MqeError::Eval("function argument must be a metric selector".into())),
    }
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct BucketRow {
    bucket: DateTime<Utc>,
    val: Option<f64>,
}
