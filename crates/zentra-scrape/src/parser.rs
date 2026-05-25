//! Prometheus text format parser (OpenMetrics compatible) for Zentra.
//! Reference: prometheus-main/scrape/scrape.go text format parsing.
//!
//! Parses the standard Prometheus exposition format:
//!   # HELP metric_name Description
//!   # TYPE metric_name counter|gauge|histogram|summary
//!   metric_name{label="value"} 123.45 timestamp

use std::collections::HashMap;
use tracing::debug;

/// A single parsed metric sample from Prometheus text format.
#[derive(Debug, Clone)]
pub struct PrometheusSample {
    pub metric_name: String,
    pub labels: HashMap<String, String>,
    pub value: f64,
    pub timestamp: Option<i64>,
}

/// Metric type from TYPE comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
    Untyped,
}

/// Metadata about a metric family.
#[derive(Debug, Clone)]
pub struct MetricFamily {
    pub name: String,
    pub help: String,
    pub metric_type: MetricType,
}

/// Parse Prometheus text exposition format into samples.
pub fn parse_prometheus_text(body: &str) -> Vec<PrometheusSample> {
    let mut samples = Vec::new();

    for line in body.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(sample) = parse_sample_line(line) {
            samples.push(sample);
        }
    }

    debug!(count = samples.len(), "Parsed Prometheus text format");
    samples
}

/// Parse a single sample line: metric_name{labels} value [timestamp]
fn parse_sample_line(line: &str) -> Option<PrometheusSample> {
    let (name_labels, rest) = if let Some(brace_start) = line.find('{') {
        let brace_end = line.find('}')?;
        let name = &line[..brace_start];
        let label_str = &line[brace_start + 1..brace_end];
        let rest = line[brace_end + 1..].trim();
        let labels = parse_labels(label_str);
        (
            (name.to_string(), labels),
            rest.to_string(),
        )
    } else {
        // No labels
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.is_empty() {
            return None;
        }
        (
            (parts[0].to_string(), HashMap::new()),
            parts.get(1).unwrap_or(&"").to_string(),
        )
    };

    let (metric_name, labels) = name_labels;
    let rest = rest.trim();

    // Parse value and optional timestamp
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let value = match parts[0] {
        "+Inf" => f64::INFINITY,
        "-Inf" => f64::NEG_INFINITY,
        "NaN" => f64::NAN,
        s => s.parse::<f64>().ok()?,
    };

    let timestamp = if parts.len() > 1 {
        parts[1].parse::<i64>().ok()
    } else {
        None
    };

    Some(PrometheusSample {
        metric_name,
        labels,
        value,
        timestamp,
    })
}

/// Parse label pairs: key="value",key2="value2"
fn parse_labels(label_str: &str) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    if label_str.is_empty() {
        return labels;
    }

    let mut remaining = label_str;
    while !remaining.is_empty() {
        // Find key=
        let eq_pos = match remaining.find('=') {
            Some(p) => p,
            None => break,
        };
        let key = remaining[..eq_pos].trim().to_string();
        remaining = &remaining[eq_pos + 1..];

        // Find quoted value
        if !remaining.starts_with('"') {
            break;
        }
        remaining = &remaining[1..]; // skip opening quote

        let mut value = String::new();
        let mut escaped = false;
        let mut consumed = 0;
        for ch in remaining.chars() {
            consumed += ch.len_utf8();
            if escaped {
                match ch {
                    'n' => value.push('\n'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    _ => {
                        value.push('\\');
                        value.push(ch);
                    }
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                break;
            } else {
                value.push(ch);
            }
        }

        labels.insert(key, value);
        remaining = &remaining[consumed..];

        // Skip comma separator
        remaining = remaining.trim_start_matches(',').trim();
    }

    labels
}

/// Parse TYPE and HELP comments to extract metadata.
pub fn parse_metadata(body: &str) -> Vec<MetricFamily> {
    let mut families: HashMap<String, MetricFamily> = HashMap::new();

    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("# HELP ") {
            let rest = &line[7..];
            if let Some(space) = rest.find(' ') {
                let name = rest[..space].to_string();
                let help = rest[space + 1..].to_string();
                families.entry(name.clone()).or_insert_with(|| MetricFamily {
                    name,
                    help: String::new(),
                    metric_type: MetricType::Untyped,
                }).help = help;
            }
        } else if line.starts_with("# TYPE ") {
            let rest = &line[7..];
            if let Some(space) = rest.find(' ') {
                let name = rest[..space].to_string();
                let type_str = &rest[space + 1..];
                let mt = match type_str {
                    "counter" => MetricType::Counter,
                    "gauge" => MetricType::Gauge,
                    "histogram" => MetricType::Histogram,
                    "summary" => MetricType::Summary,
                    _ => MetricType::Untyped,
                };
                families.entry(name.clone()).or_insert_with(|| MetricFamily {
                    name,
                    help: String::new(),
                    metric_type: MetricType::Untyped,
                }).metric_type = mt;
            }
        }
    }

    families.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_gauge() {
        let text = r#"
# HELP go_goroutines Number of goroutines.
# TYPE go_goroutines gauge
go_goroutines 42
"#;
        let samples = parse_prometheus_text(text);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].metric_name, "go_goroutines");
        assert_eq!(samples[0].value, 42.0);
        assert!(samples[0].labels.is_empty());
    }

    #[test]
    fn test_parse_with_labels() {
        let text = r#"http_requests_total{method="GET",code="200"} 1027 1395066363000"#;
        let samples = parse_prometheus_text(text);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].metric_name, "http_requests_total");
        assert_eq!(samples[0].value, 1027.0);
        assert_eq!(samples[0].labels.get("method"), Some(&"GET".to_string()));
        assert_eq!(samples[0].labels.get("code"), Some(&"200".to_string()));
        assert_eq!(samples[0].timestamp, Some(1395066363000));
    }

    #[test]
    fn test_parse_histogram() {
        let text = r#"
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le="0.05"} 24054
http_request_duration_seconds_bucket{le="0.1"} 33444
http_request_duration_seconds_bucket{le="+Inf"} 144320
http_request_duration_seconds_sum 53423
http_request_duration_seconds_count 144320
"#;
        let samples = parse_prometheus_text(text);
        assert_eq!(samples.len(), 5);
        assert_eq!(samples[2].value, f64::INFINITY); // le="+Inf"
    }

    #[test]
    fn test_parse_metadata() {
        let text = r#"
# HELP process_cpu_seconds_total Total user and system CPU time.
# TYPE process_cpu_seconds_total counter
process_cpu_seconds_total 123.45
"#;
        let families = parse_metadata(text);
        assert_eq!(families.len(), 1);
        assert_eq!(families[0].metric_type, MetricType::Counter);
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_prometheus_text("").is_empty());
        assert!(parse_prometheus_text("# just comments\n# HELP foo bar").is_empty());
    }
}
