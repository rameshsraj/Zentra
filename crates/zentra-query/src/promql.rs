//! PromQL-Subset Query Engine for Zentra
//!
//! Supports a useful subset of PromQL syntax:
//! - Instant vectors:  `metric_name{label="value"}`
//! - Range vectors:    `metric_name{label="value"}[5m]`
//! - Aggregations:     `avg(metric_name{label="value"}[5m])`
//! - Functions:        `rate(metric_name[5m])`, `increase()`, `delta()`
//! - Binary ops:       `metric_a / metric_b`, `metric > 90`
//! - Label matchers:   `=`, `!=`, `=~`, `!~`
//!
//! Queries are compiled into SQL against TimescaleDB hypertables/aggregates.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::debug;

// ── AST ──

/// Top-level PromQL expression
#[derive(Debug, Clone)]
pub enum Expr {
    /// metric_name{label="value"}
    VectorSelector(VectorSelector),
    /// metric_name{label="value"}[5m]
    MatrixSelector(MatrixSelector),
    /// avg(expr), sum(expr), etc.
    Aggregation(AggregationExpr),
    /// rate(expr), increase(expr), delta(expr), abs(expr)
    FunctionCall(FunctionCallExpr),
    /// expr <op> expr  or  expr <op> scalar
    BinaryOp(BinaryOpExpr),
    /// Literal number
    NumberLiteral(f64),
}

#[derive(Debug, Clone)]
pub struct VectorSelector {
    pub metric_name: String,
    pub label_matchers: Vec<LabelMatcher>,
}

#[derive(Debug, Clone)]
pub struct MatrixSelector {
    pub vector: VectorSelector,
    pub range: Duration,
}

#[derive(Debug, Clone)]
pub struct AggregationExpr {
    pub op: AggOp,
    pub expr: Box<Expr>,
    pub by_labels: Vec<String>,
    pub without_labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionCallExpr {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct BinaryOpExpr {
    pub op: BinOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

#[derive(Debug, Clone)]
pub struct LabelMatcher {
    pub name: String,
    pub op: MatchOp,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchOp {
    Equal,       // =
    NotEqual,    // !=
    RegexMatch,  // =~
    RegexNoMatch,// !~
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggOp {
    Sum, Avg, Min, Max, Count, Stddev, Stdvar, Topk, Bottomk, Quantile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, Ne, Gt, Lt, Gte, Lte,
    And, Or, Unless,
}

/// Duration like `[5m]`, `[1h]`, `[30s]`
#[derive(Debug, Clone, Copy)]
pub struct Duration {
    pub seconds: i64,
}

// ── Parser ──

/// Parse a PromQL expression string into an AST
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError("Empty query".into()));
    }

    // Try to parse as aggregation: avg(...), sum(...) etc.
    if let Some(expr) = try_parse_aggregation(input)? {
        return Ok(expr);
    }

    // Try to parse as function: rate(...), increase(...), etc.
    if let Some(expr) = try_parse_function(input)? {
        return Ok(expr);
    }

    // Try to parse as binary op: expr <op> expr
    if let Some(expr) = try_parse_binary(input)? {
        return Ok(expr);
    }

    // Try to parse as number literal
    if let Ok(n) = input.parse::<f64>() {
        return Ok(Expr::NumberLiteral(n));
    }

    // Try to parse as matrix selector: metric{...}[5m]
    if let Some(expr) = try_parse_matrix(input)? {
        return Ok(expr);
    }

    // Parse as vector selector: metric{...}
    parse_vector_selector(input).map(Expr::VectorSelector)
}

fn try_parse_aggregation(input: &str) -> Result<Option<Expr>, ParseError> {
    let agg_ops = [
        ("sum", AggOp::Sum), ("avg", AggOp::Avg), ("min", AggOp::Min),
        ("max", AggOp::Max), ("count", AggOp::Count), ("stddev", AggOp::Stddev),
        ("stdvar", AggOp::Stdvar), ("topk", AggOp::Topk), ("bottomk", AggOp::Bottomk),
        ("quantile", AggOp::Quantile),
    ];

    for (name, op) in &agg_ops {
        if input.starts_with(name) {
            let rest = &input[name.len()..].trim_start();

            // Check for "by" or "without" clause before parens
            let (by_labels, without_labels, rest) = parse_grouping(rest);

            if let Some(inner) = extract_parens(rest) {
                let inner_expr = parse(inner)?;
                return Ok(Some(Expr::Aggregation(AggregationExpr {
                    op: *op,
                    expr: Box::new(inner_expr),
                    by_labels,
                    without_labels,
                })));
            }
        }
    }
    Ok(None)
}

fn try_parse_function(input: &str) -> Result<Option<Expr>, ParseError> {
    let funcs = ["rate", "increase", "delta", "irate", "idelta", "abs", "ceil", "floor",
                 "round", "deriv", "predict_linear", "histogram_quantile", "label_replace"];

    for func_name in &funcs {
        if input.starts_with(func_name) {
            let rest = &input[func_name.len()..];
            if let Some(inner) = extract_parens(rest) {
                // Split args by comma (respecting nested parens)
                let args = split_args(inner);
                let mut parsed_args = Vec::new();
                for arg in args {
                    parsed_args.push(parse(arg.trim())?);
                }
                return Ok(Some(Expr::FunctionCall(FunctionCallExpr {
                    name: func_name.to_string(),
                    args: parsed_args,
                })));
            }
        }
    }
    Ok(None)
}

fn try_parse_binary(input: &str) -> Result<Option<Expr>, ParseError> {
    // Find binary operator at the top level (not inside parens/brackets)
    let ops = [
        (" unless ", BinOp::Unless), (" and ", BinOp::And), (" or ", BinOp::Or),
        (" >= ", BinOp::Gte), (" <= ", BinOp::Lte), (" != ", BinOp::Ne),
        (" == ", BinOp::Eq), (" > ", BinOp::Gt), (" < ", BinOp::Lt),
        (" + ", BinOp::Add), (" - ", BinOp::Sub), (" * ", BinOp::Mul),
        (" / ", BinOp::Div), (" % ", BinOp::Mod), (" ^ ", BinOp::Pow),
    ];

    for (op_str, op) in &ops {
        if let Some(pos) = find_top_level(input, op_str) {
            let lhs_str = &input[..pos];
            let rhs_str = &input[pos + op_str.len()..];
            if !lhs_str.trim().is_empty() && !rhs_str.trim().is_empty() {
                let lhs = parse(lhs_str.trim())?;
                let rhs = parse(rhs_str.trim())?;
                return Ok(Some(Expr::BinaryOp(BinaryOpExpr {
                    op: *op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                })));
            }
        }
    }
    Ok(None)
}

fn try_parse_matrix(input: &str) -> Result<Option<Expr>, ParseError> {
    if !input.ends_with(']') {
        return Ok(None);
    }
    if let Some(bracket_start) = input.rfind('[') {
        let range_str = &input[bracket_start + 1..input.len() - 1];
        let duration = parse_duration(range_str)?;
        let vector_str = &input[..bracket_start];
        let vector = parse_vector_selector(vector_str)?;
        return Ok(Some(Expr::MatrixSelector(MatrixSelector { vector, range: duration })));
    }
    Ok(None)
}

fn parse_vector_selector(input: &str) -> Result<VectorSelector, ParseError> {
    let input = input.trim();
    if let Some(brace_start) = input.find('{') {
        let metric_name = input[..brace_start].trim().to_string();
        let brace_end = input.rfind('}').ok_or_else(|| ParseError("Unclosed brace".into()))?;
        let matchers_str = &input[brace_start + 1..brace_end];
        let label_matchers = parse_label_matchers(matchers_str)?;
        Ok(VectorSelector { metric_name, label_matchers })
    } else {
        Ok(VectorSelector {
            metric_name: input.to_string(),
            label_matchers: vec![],
        })
    }
}

fn parse_label_matchers(input: &str) -> Result<Vec<LabelMatcher>, ParseError> {
    if input.trim().is_empty() {
        return Ok(vec![]);
    }
    let mut matchers = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }

        let (name, op, value) = if let Some(pos) = part.find("=~") {
            (&part[..pos], MatchOp::RegexMatch, &part[pos + 2..])
        } else if let Some(pos) = part.find("!~") {
            (&part[..pos], MatchOp::RegexNoMatch, &part[pos + 2..])
        } else if let Some(pos) = part.find("!=") {
            (&part[..pos], MatchOp::NotEqual, &part[pos + 2..])
        } else if let Some(pos) = part.find('=') {
            (&part[..pos], MatchOp::Equal, &part[pos + 1..])
        } else {
            return Err(ParseError(format!("Invalid label matcher: {}", part)));
        };

        let value = value.trim().trim_matches('"').trim_matches('\'').to_string();
        matchers.push(LabelMatcher {
            name: name.trim().to_string(),
            op,
            value,
        });
    }
    Ok(matchers)
}

fn parse_duration(s: &str) -> Result<Duration, ParseError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ParseError("Empty duration".into()));
    }

    let mut total_secs: i64 = 0;
    let mut num_buf = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else {
            let n: i64 = num_buf.parse().map_err(|_| ParseError(format!("Invalid duration: {}", s)))?;
            num_buf.clear();
            match c {
                's' => total_secs += n,
                'm' => total_secs += n * 60,
                'h' => total_secs += n * 3600,
                'd' => total_secs += n * 86400,
                'w' => total_secs += n * 604800,
                'y' => total_secs += n * 31536000,
                _ => return Err(ParseError(format!("Unknown duration unit: {}", c))),
            }
        }
    }

    if total_secs == 0 {
        return Err(ParseError(format!("Zero duration: {}", s)));
    }

    Ok(Duration { seconds: total_secs })
}

fn parse_grouping(input: &str) -> (Vec<String>, Vec<String>, &str) {
    // Parse optional `by(label1, label2)` or `without(label1, label2)` clause
    let input = input.trim();
    if input.starts_with("by") {
        let rest = &input[2..].trim_start();
        if let Some(inner) = extract_parens(rest) {
            let labels: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let after = &rest[rest.find(')').unwrap() + 1..];
            return (labels, vec![], after.trim());
        }
    }
    if input.starts_with("without") {
        let rest = &input[7..].trim_start();
        if let Some(inner) = extract_parens(rest) {
            let labels: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let after = &rest[rest.find(')').unwrap() + 1..];
            return (vec![], labels, after.trim());
        }
    }
    (vec![], vec![], input)
}

fn extract_parens(s: &str) -> Option<&str> {
    let s = s.trim();
    if !s.starts_with('(') {
        return None;
    }
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[1..i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_args(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                result.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < s.len() {
        result.push(&s[start..]);
    }
    result
}

fn find_top_level(s: &str, needle: &str) -> Option<usize> {
    let mut depth = 0;
    let bytes = s.as_bytes();
    let needle_bytes = needle.as_bytes();

    for i in 0..s.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }
        if depth == 0 && i + needle_bytes.len() <= s.len() {
            if &bytes[i..i + needle_bytes.len()] == needle_bytes {
                return Some(i);
            }
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "PromQL parse error: {}", self.0)
    }
}
impl std::error::Error for ParseError {}

// ── SQL Compiler ──

/// Query evaluation context
#[derive(Debug, Clone)]
pub struct QueryContext {
    pub org_id: uuid::Uuid,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub step_seconds: Option<i64>,
}

/// Compiled SQL query ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledQuery {
    pub sql: String,
    pub params: QueryParams,
    pub result_type: ResultType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParams {
    pub org_id: uuid::Uuid,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub metric_name: String,
    pub bucket_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResultType {
    Scalar,
    InstantVector,
    RangeVector,
}

/// Compile a parsed PromQL expression into a SQL query
pub fn compile(expr: &Expr, ctx: &QueryContext) -> Result<CompiledQuery, ParseError> {
    match expr {
        Expr::VectorSelector(vs) => compile_vector(vs, ctx),
        Expr::MatrixSelector(ms) => compile_matrix(ms, ctx),
        Expr::Aggregation(agg) => compile_aggregation(agg, ctx),
        Expr::FunctionCall(func) => compile_function(func, ctx),
        Expr::BinaryOp(bin) => compile_binary(bin, ctx),
        Expr::NumberLiteral(n) => Ok(CompiledQuery {
            sql: format!("SELECT {} AS value", n),
            params: QueryParams {
                org_id: ctx.org_id,
                start: ctx.start,
                end: ctx.end,
                metric_name: String::new(),
                bucket_seconds: None,
            },
            result_type: ResultType::Scalar,
        }),
    }
}

fn compile_vector(vs: &VectorSelector, ctx: &QueryContext) -> Result<CompiledQuery, ParseError> {
    let label_filter = compile_label_filters(&vs.label_matchers);

    let sql = format!(
        r#"SELECT timestamp, value, resource_id, tags
           FROM metrics
           WHERE org_id = $1 AND metric_name = $2 AND timestamp >= $3 AND timestamp < $4
           {}
           ORDER BY timestamp DESC
           LIMIT 10000"#,
        label_filter,
    );

    Ok(CompiledQuery {
        sql,
        params: QueryParams {
            org_id: ctx.org_id,
            start: ctx.start,
            end: ctx.end,
            metric_name: vs.metric_name.clone(),
            bucket_seconds: None,
        },
        result_type: ResultType::InstantVector,
    })
}

fn compile_matrix(ms: &MatrixSelector, ctx: &QueryContext) -> Result<CompiledQuery, ParseError> {
    let label_filter = compile_label_filters(&ms.vector.label_matchers);
    let bucket = ctx.step_seconds.unwrap_or(ms.range.seconds);

    // Pick the best continuous aggregate source
    let (source, time_col) = pick_aggregate_source(bucket);

    let sql = format!(
        r#"SELECT
             time_bucket(make_interval(secs => $5), {time_col}) AS bucket,
             avg(avg_value) AS avg_value,
             max(max_value) AS max_value,
             min(min_value) AS min_value,
             sum(sample_count) AS count
           FROM {source}
           WHERE org_id = $1 AND metric_name = $2 AND {time_col} >= $3 AND {time_col} < $4
           {label_filter}
           GROUP BY 1
           ORDER BY 1"#,
    );

    Ok(CompiledQuery {
        sql,
        params: QueryParams {
            org_id: ctx.org_id,
            start: ctx.start,
            end: ctx.end,
            metric_name: ms.vector.metric_name.clone(),
            bucket_seconds: Some(bucket as f64),
        },
        result_type: ResultType::RangeVector,
    })
}

fn compile_aggregation(agg: &AggregationExpr, ctx: &QueryContext) -> Result<CompiledQuery, ParseError> {
    let inner = compile(&agg.expr, ctx)?;
    let agg_fn = match agg.op {
        AggOp::Sum => "sum",
        AggOp::Avg => "avg",
        AggOp::Min => "min",
        AggOp::Max => "max",
        AggOp::Count => "count",
        AggOp::Stddev => "stddev",
        AggOp::Stdvar => "variance",
        _ => "avg",
    };

    let group_by = if !agg.by_labels.is_empty() {
        format!(", {}", agg.by_labels.join(", "))
    } else {
        String::new()
    };

    let sql = format!(
        r#"SELECT bucket{group_by}, {agg_fn}(avg_value) AS value
           FROM ({inner_sql}) AS sub
           GROUP BY bucket{group_by}
           ORDER BY bucket"#,
        inner_sql = inner.sql,
    );

    Ok(CompiledQuery {
        sql,
        params: inner.params,
        result_type: ResultType::RangeVector,
    })
}

fn compile_function(func: &FunctionCallExpr, ctx: &QueryContext) -> Result<CompiledQuery, ParseError> {
    if func.args.is_empty() {
        return Err(ParseError(format!("Function '{}' requires arguments", func.name)));
    }

    let inner = compile(&func.args[0], ctx)?;

    let sql = match func.name.as_str() {
        "rate" => {
            // rate = (last - first) / range_seconds
            format!(
                r#"SELECT bucket,
                     (last_value(avg_value) OVER w - first_value(avg_value) OVER w)
                     / EXTRACT(EPOCH FROM (max(bucket) OVER w - min(bucket) OVER w)) AS value
                   FROM ({}) AS sub
                   WINDOW w AS (ORDER BY bucket ROWS BETWEEN 1 PRECEDING AND CURRENT ROW)
                   ORDER BY bucket"#,
                inner.sql
            )
        }
        "increase" | "delta" => {
            format!(
                r#"SELECT bucket,
                     avg_value - lag(avg_value) OVER (ORDER BY bucket) AS value
                   FROM ({}) AS sub
                   ORDER BY bucket"#,
                inner.sql
            )
        }
        "abs" => {
            format!(
                r#"SELECT bucket, abs(avg_value) AS value FROM ({}) AS sub ORDER BY bucket"#,
                inner.sql
            )
        }
        "ceil" => {
            format!(
                r#"SELECT bucket, ceil(avg_value) AS value FROM ({}) AS sub ORDER BY bucket"#,
                inner.sql
            )
        }
        "floor" => {
            format!(
                r#"SELECT bucket, floor(avg_value) AS value FROM ({}) AS sub ORDER BY bucket"#,
                inner.sql
            )
        }
        other => {
            // Passthrough — just return inner
            debug!(func = other, "Unknown function, returning inner query");
            inner.sql.clone()
        }
    };

    Ok(CompiledQuery {
        sql,
        params: inner.params,
        result_type: ResultType::RangeVector,
    })
}

fn compile_binary(bin: &BinaryOpExpr, ctx: &QueryContext) -> Result<CompiledQuery, ParseError> {
    let lhs = compile(&bin.lhs, ctx)?;
    let rhs = compile(&bin.rhs, ctx)?;

    let op_str = match bin.op {
        BinOp::Add => "+", BinOp::Sub => "-", BinOp::Mul => "*", BinOp::Div => "/",
        BinOp::Mod => "%", BinOp::Pow => "^",
        BinOp::Eq => "=", BinOp::Ne => "!=", BinOp::Gt => ">", BinOp::Lt => "<",
        BinOp::Gte => ">=", BinOp::Lte => "<=",
        BinOp::And => "AND", BinOp::Or => "OR", BinOp::Unless => "AND NOT",
    };

    // If RHS is a scalar, apply as filter/transform
    if rhs.result_type == ResultType::Scalar {
        let sql = format!(
            r#"SELECT bucket, avg_value {op_str} ({rhs_sql}) AS value
               FROM ({lhs_sql}) AS sub
               ORDER BY bucket"#,
            lhs_sql = lhs.sql,
            rhs_sql = rhs.sql,
        );
        return Ok(CompiledQuery {
            sql,
            params: lhs.params,
            result_type: ResultType::RangeVector,
        });
    }

    // Both vectors: join on bucket
    let sql = format!(
        r#"SELECT COALESCE(l.bucket, r.bucket) AS bucket,
             l.avg_value {op_str} r.avg_value AS value
           FROM ({lhs_sql}) AS l
           FULL OUTER JOIN ({rhs_sql}) AS r ON l.bucket = r.bucket
           ORDER BY bucket"#,
        lhs_sql = lhs.sql,
        rhs_sql = rhs.sql,
    );

    Ok(CompiledQuery {
        sql,
        params: lhs.params,
        result_type: ResultType::RangeVector,
    })
}

fn compile_label_filters(matchers: &[LabelMatcher]) -> String {
    if matchers.is_empty() {
        return String::new();
    }
    let mut filters = Vec::new();
    for m in matchers {
        let filter = match (&m.name[..], m.op) {
            ("resource_id", MatchOp::Equal) => format!("AND resource_id = '{}'", m.value),
            ("resource_id", MatchOp::NotEqual) => format!("AND resource_id != '{}'", m.value),
            ("resource_id", MatchOp::RegexMatch) => format!("AND resource_id ~ '{}'", m.value),
            ("resource_id", MatchOp::RegexNoMatch) => format!("AND resource_id !~ '{}'", m.value),
            ("resource_type", MatchOp::Equal) => format!("AND resource_type = '{}'", m.value),
            ("target_id", MatchOp::Equal) => format!("AND target_id = '{}'", m.value),
            (name, MatchOp::Equal) => format!("AND tags->>'{}' = '{}'", name, m.value),
            (name, MatchOp::NotEqual) => format!("AND (tags->>'{}' IS NULL OR tags->>'{}' != '{}')", name, name, m.value),
            (name, MatchOp::RegexMatch) => format!("AND tags->>'{}' ~ '{}'", name, m.value),
            (name, MatchOp::RegexNoMatch) => format!("AND (tags->>'{}' IS NULL OR tags->>'{}' !~ '{}')", name, name, m.value),
        };
        filters.push(filter);
    }
    filters.join(" ")
}

fn pick_aggregate_source(bucket_seconds: i64) -> (&'static str, &'static str) {
    match bucket_seconds {
        s if s >= 86400 => ("metrics_1d", "bucket"),
        s if s >= 3600 => ("metrics_1h", "bucket"),
        s if s >= 300 => ("metrics_5m", "bucket"),
        s if s >= 60 => ("metrics_1m", "bucket"),
        _ => ("metrics", "timestamp"),
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_metric() {
        let expr = parse("cpu_usage").unwrap();
        match expr {
            Expr::VectorSelector(vs) => {
                assert_eq!(vs.metric_name, "cpu_usage");
                assert!(vs.label_matchers.is_empty());
            }
            _ => panic!("Expected VectorSelector"),
        }
    }

    #[test]
    fn test_parse_metric_with_labels() {
        let expr = parse(r#"http_requests{method="GET", code="200"}"#).unwrap();
        match expr {
            Expr::VectorSelector(vs) => {
                assert_eq!(vs.metric_name, "http_requests");
                assert_eq!(vs.label_matchers.len(), 2);
                assert_eq!(vs.label_matchers[0].name, "method");
                assert_eq!(vs.label_matchers[0].value, "GET");
                assert_eq!(vs.label_matchers[1].name, "code");
                assert_eq!(vs.label_matchers[1].value, "200");
            }
            _ => panic!("Expected VectorSelector"),
        }
    }

    #[test]
    fn test_parse_range_vector() {
        let expr = parse("cpu_usage[5m]").unwrap();
        match expr {
            Expr::MatrixSelector(ms) => {
                assert_eq!(ms.vector.metric_name, "cpu_usage");
                assert_eq!(ms.range.seconds, 300);
            }
            _ => panic!("Expected MatrixSelector"),
        }
    }

    #[test]
    fn test_parse_aggregation() {
        let expr = parse("avg(cpu_usage[5m])").unwrap();
        match expr {
            Expr::Aggregation(agg) => {
                assert_eq!(agg.op, AggOp::Avg);
            }
            _ => panic!("Expected Aggregation"),
        }
    }

    #[test]
    fn test_parse_function() {
        let expr = parse("rate(http_requests[5m])").unwrap();
        match expr {
            Expr::FunctionCall(f) => {
                assert_eq!(f.name, "rate");
                assert_eq!(f.args.len(), 1);
            }
            _ => panic!("Expected FunctionCall"),
        }
    }

    #[test]
    fn test_parse_binary_scalar() {
        let expr = parse("cpu_usage > 90").unwrap();
        match expr {
            Expr::BinaryOp(b) => {
                assert_eq!(b.op, BinOp::Gt);
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5m").unwrap().seconds, 300);
        assert_eq!(parse_duration("1h").unwrap().seconds, 3600);
        assert_eq!(parse_duration("30s").unwrap().seconds, 30);
        assert_eq!(parse_duration("1h30m").unwrap().seconds, 5400);
        assert_eq!(parse_duration("1d").unwrap().seconds, 86400);
    }

    #[test]
    fn test_compile_vector() {
        let ctx = QueryContext {
            org_id: uuid::Uuid::nil(),
            start: Utc::now(),
            end: Utc::now(),
            step_seconds: None,
        };
        let expr = parse("cpu_usage").unwrap();
        let compiled = compile(&expr, &ctx).unwrap();
        assert!(compiled.sql.contains("FROM metrics"));
        assert_eq!(compiled.params.metric_name, "cpu_usage");
    }

    #[test]
    fn test_compile_rate() {
        let ctx = QueryContext {
            org_id: uuid::Uuid::nil(),
            start: Utc::now(),
            end: Utc::now(),
            step_seconds: Some(300),
        };
        let expr = parse("rate(http_requests[5m])").unwrap();
        let compiled = compile(&expr, &ctx).unwrap();
        assert!(compiled.sql.contains("first_value") || compiled.sql.contains("last_value"));
    }

    #[test]
    fn test_label_regex_match() {
        let expr = parse(r#"cpu_usage{host=~"web-.*"}"#).unwrap();
        match expr {
            Expr::VectorSelector(vs) => {
                assert_eq!(vs.label_matchers[0].op, MatchOp::RegexMatch);
                assert_eq!(vs.label_matchers[0].value, "web-.*");
            }
            _ => panic!("Expected VectorSelector"),
        }
    }
}
