use axum::{extract::{State, Path, Query, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use std::collections::{HashMap, VecDeque};
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: i64,
    pub line: String,
    pub structured_metadata: HashMap<String, String>,
    pub parsed_fields: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogStream {
    pub labels: HashMap<String, String>,
    pub fingerprint: String,
    pub entry_count: usize,
    pub bytes: u64,
    pub last_entry_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub streams: Vec<QueryResultStream>,
    pub total_entries: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct QueryResultStream {
    pub labels: HashMap<String, String>,
    pub entries: Vec<QueryEntry>,
}
#[derive(Debug, Clone, Serialize)]
pub struct QueryEntry {
    pub timestamp: String,
    pub line: String,
    pub fields: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VolumeEntry { pub timestamp: String, pub count: u64, pub bytes: u64 }
#[derive(Debug, Clone, Serialize)]
pub struct LogAnalytics {
    pub total_streams: usize, pub total_entries: u64, pub total_bytes: u64,
    pub ingestion_rate_lines_per_sec: f64, pub ingestion_rate_bytes_per_sec: f64,
    pub volume_by_source: Vec<SourceVolume>, pub top_error_messages: Vec<ErrorMessage>,
    pub error_rate_trend: Vec<VolumeEntry>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SourceVolume { pub source: String, pub lines: u64, pub bytes: u64 }
#[derive(Debug, Clone, Serialize)]
pub struct ErrorMessage { pub message: String, pub count: u64, pub last_seen: String }

#[derive(Debug, Deserialize)]
pub struct QueryParams { pub query: Option<String>, pub start: Option<String>, pub end: Option<String>, pub limit: Option<usize> }
#[derive(Debug, Deserialize)]
pub struct LokiPushRequest { pub streams: Vec<LokiStream> }
#[derive(Debug, Deserialize)]
pub struct LokiStream { pub stream: HashMap<String, String>, pub values: Vec<(String, String)> }
#[derive(Debug, Deserialize)]
pub struct NativeIngestRequest { pub entries: Vec<NativeEntry> }
#[derive(Debug, Deserialize)]
pub struct NativeEntry { pub labels: Option<HashMap<String, String>>, pub line: String, pub timestamp: Option<String> }
#[derive(Debug, Deserialize)]
pub struct CollectRequest { pub target_host: String, pub log_paths: Vec<String>, pub lines: Option<usize> }
#[derive(Debug, Deserialize)]
pub struct ContextParams { pub before: Option<usize>, pub after: Option<usize> }
#[derive(Debug, Deserialize)]
pub struct ExportParams { pub query: Option<String>, pub format: Option<String>, pub limit: Option<usize> }

// ── Log Alert models ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogAlert {
    pub id: String, pub name: String, pub query: String, pub threshold: f64,
    pub comparison: String, pub severity: String, pub enabled: bool,
    pub last_fired: Option<String>, pub created_at: String,
}
#[derive(Debug, Deserialize)]
pub struct CreateLogAlertRequest { pub name: String, pub query: String, pub threshold: Option<f64>, pub comparison: Option<String>, pub severity: Option<String> }

// ── In-Memory Store ──────────────────────────────────────────────────

struct LogStore {
    streams: HashMap<String, InternalStream>,
    log_alerts: Vec<LogAlert>,
    total_bytes: u64,
    total_lines: u64,
}
struct InternalStream {
    labels: HashMap<String, String>,
    entries: VecDeque<LogEntry>,
    bytes: u64,
}

fn store() -> &'static RwLock<LogStore> {
    static INSTANCE: OnceLock<RwLock<LogStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn label_fingerprint(labels: &HashMap<String, String>) -> String {
    let mut pairs: Vec<_> = labels.iter().collect();
    pairs.sort_by_key(|(k, _)| k.clone());
    let s: Vec<String> = pairs.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
    format!("{:x}", md5_hash(&s.join(",")))
}

fn md5_hash(s: &str) -> u64 {
    let mut h: u64 = 0;
    for b in s.bytes() { h = h.wrapping_mul(31).wrapping_add(b as u64); }
    h
}

fn init_store() -> LogStore {
    let now = Utc::now();
    let mut streams = HashMap::new();
    let mut total_bytes = 0u64;
    let mut total_lines = 0u64;

    let stream_defs = vec![
        (vec![("host","web01"),("job","nginx"),("level","info")], vec![
            "10.0.0.1 - - [25/Apr/2026:12:00:01] \"GET /api/health 200 12ms\"",
            "10.0.0.2 - - [25/Apr/2026:12:00:02] \"POST /api/users 201 45ms\"",
            "10.0.0.3 - - [25/Apr/2026:12:00:03] \"GET /dashboard 200 8ms\"",
        ]),
        (vec![("host","web01"),("job","nginx"),("level","error")], vec![
            "10.0.0.5 - - [25/Apr/2026:12:00:01] \"GET /api/broken 500 120ms\" upstream timeout",
            "10.0.0.8 - - [25/Apr/2026:12:00:05] \"POST /api/upload 413 0ms\" request entity too large",
        ]),
        (vec![("host","api01"),("job","api-server"),("level","info")], vec![
            "INFO [main] Application started successfully on port 7001",
            "INFO [worker-1] Processing batch of 250 events",
            "INFO [worker-2] Cache refreshed, 12500 entries loaded",
        ]),
        (vec![("host","api01"),("job","api-server"),("level","error")], vec![
            "ERROR [db-pool] Connection timeout after 30s to postgres:5432",
            "ERROR [auth] Invalid JWT token for user admin@example.com",
        ]),
        (vec![("host","db01"),("job","postgresql"),("level","warn")], vec![
            "LOG: checkpoints are occurring too frequently (28 seconds apart)",
            "WARNING: autovacuum not keeping up on table public.events",
        ]),
        (vec![("host","db01"),("job","postgresql"),("level","info")], vec![
            "LOG: database system is ready to accept connections",
            "LOG: automatic vacuum of table \"public.metrics\": 125000 dead tuples removed",
        ]),
        (vec![("host","worker01"),("job","cron"),("level","info")], vec![
            "Starting daily backup job",
            "Backup completed successfully in 12m35s",
            "Rotating log files older than 30 days",
        ]),
    ];

    for (label_pairs, lines) in stream_defs {
        let labels: HashMap<String, String> = label_pairs.into_iter().map(|(k,v)| (k.to_string(), v.to_string())).collect();
        let fp = label_fingerprint(&labels);
        let mut entries = VecDeque::new();
        let mut bytes = 0u64;
        for (i, line) in lines.iter().enumerate() {
            let ts = (now - Duration::minutes(lines.len() as i64 - i as i64)).timestamp_nanos_opt().unwrap_or(0);
            bytes += line.len() as u64;
            total_lines += 1;
            entries.push_back(LogEntry {
                timestamp: ts,
                line: line.to_string(),
                structured_metadata: HashMap::new(),
                parsed_fields: None,
            });
        }
        total_bytes += bytes;
        streams.insert(fp, InternalStream { labels, entries, bytes });
    }

    let log_alerts = vec![
        LogAlert { id: "la-001".into(), name: "High Error Rate".into(), query: "{level=\"error\"} |= \"timeout\"".into(), threshold: 10.0, comparison: "gt".into(), severity: "critical".into(), enabled: true, last_fired: Some((now - Duration::hours(2)).to_rfc3339()), created_at: (now - Duration::days(7)).to_rfc3339() },
        LogAlert { id: "la-002".into(), name: "DB Connection Failures".into(), query: "{job=\"postgresql\"} |= \"connection refused\"".into(), threshold: 5.0, comparison: "gt".into(), severity: "warning".into(), enabled: true, last_fired: None, created_at: (now - Duration::days(3)).to_rfc3339() },
    ];

    LogStore { streams, log_alerts, total_bytes, total_lines }
}

fn match_stream(labels: &HashMap<String, String>, query: &str) -> bool {
    if query.is_empty() || query == "{}" { return true; }
    let inner = query.trim_start_matches('{').split('}').next().unwrap_or("");
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        if let Some((key, val)) = part.split_once('=') {
            let key = key.trim().trim_matches('"');
            let val = val.trim().trim_matches('"');
            if let Some(actual) = labels.get(key) {
                if actual != val { return false; }
            } else { return false; }
        }
    }
    true
}

fn match_line_filter(line: &str, query: &str) -> bool {
    if let Some(pos) = query.find("|=") {
        let filter = query[pos + 2..].trim().trim_matches('"').split('|').next().unwrap_or("").trim().trim_matches('"');
        if !filter.is_empty() && !line.to_lowercase().contains(&filter.to_lowercase()) { return false; }
    }
    if let Some(pos) = query.find("!=") {
        if pos > 0 && &query[pos-1..pos] == "|" { return true; }
        let filter = query[pos + 2..].trim().trim_matches('"').split('|').next().unwrap_or("").trim().trim_matches('"');
        if !filter.is_empty() && line.to_lowercase().contains(&filter.to_lowercase()) { return false; }
    }
    true
}

// ── API Handlers ─────────────────────────────────────────────────────

pub async fn query_logs(State(_s): State<AppState>, Query(params): Query<QueryParams>) -> Result<Json<QueryResult>, ApiError> {
    let store = store().read();
    let query = params.query.as_deref().unwrap_or("{}");
    let limit = params.limit.unwrap_or(1000);
    let stream_selector = query.split('}').next().map(|s| format!("{}}}", s)).unwrap_or_default();

    let mut result_streams = Vec::new();
    let mut total = 0usize;
    for (_fp, stream) in &store.streams {
        if !match_stream(&stream.labels, &stream_selector) { continue; }
        let entries: Vec<QueryEntry> = stream.entries.iter()
            .filter(|e| match_line_filter(&e.line, query))
            .take(limit)
            .map(|e| QueryEntry {
                timestamp: chrono::DateTime::from_timestamp_nanos(e.timestamp).to_rfc3339(),
                line: e.line.clone(),
                fields: e.parsed_fields.clone(),
            }).collect();
        total += entries.len();
        if !entries.is_empty() {
            result_streams.push(QueryResultStream { labels: stream.labels.clone(), entries });
        }
    }
    Ok(Json(QueryResult { streams: result_streams, total_entries: total }))
}

pub async fn list_labels(State(_s): State<AppState>) -> Result<Json<Vec<String>>, ApiError> {
    let store = store().read();
    let mut labels: std::collections::HashSet<String> = std::collections::HashSet::new();
    for stream in store.streams.values() { for k in stream.labels.keys() { labels.insert(k.clone()); } }
    let mut sorted: Vec<String> = labels.into_iter().collect();
    sorted.sort();
    Ok(Json(sorted))
}

pub async fn label_values(State(_s): State<AppState>, Path(name): Path<String>) -> Result<Json<Vec<String>>, ApiError> {
    let store = store().read();
    let mut values: std::collections::HashSet<String> = std::collections::HashSet::new();
    for stream in store.streams.values() { if let Some(v) = stream.labels.get(&name) { values.insert(v.clone()); } }
    let mut sorted: Vec<String> = values.into_iter().collect();
    sorted.sort();
    Ok(Json(sorted))
}

pub async fn list_streams(State(_s): State<AppState>) -> Result<Json<Vec<LogStream>>, ApiError> {
    let store = store().read();
    let streams: Vec<LogStream> = store.streams.iter().map(|(fp, s)| LogStream {
        labels: s.labels.clone(), fingerprint: fp.clone(), entry_count: s.entries.len(), bytes: s.bytes,
        last_entry_at: s.entries.back().map(|e| e.timestamp),
    }).collect();
    Ok(Json(streams))
}

pub async fn log_volume(State(_s): State<AppState>) -> Result<Json<Vec<VolumeEntry>>, ApiError> {
    let now = Utc::now();
    let points: Vec<VolumeEntry> = (0..24).rev().map(|i| {
        let ts = now - Duration::hours(i);
        let j = (i as f64 * 0.3).sin();
        VolumeEntry { timestamp: ts.to_rfc3339(), count: (5000.0 + j * 2000.0) as u64, bytes: (2_500_000.0 + j * 1_000_000.0) as u64 }
    }).collect();
    Ok(Json(points))
}

pub async fn log_context(State(_s): State<AppState>, Path((stream_id, timestamp)): Path<(String, String)>, Query(params): Query<ContextParams>) -> Result<Json<Vec<QueryEntry>>, ApiError> {
    let store = store().read();
    let before = params.before.unwrap_or(5);
    let after = params.after.unwrap_or(5);

    for (_fp, stream) in &store.streams {
        let fp = label_fingerprint(&stream.labels);
        if fp != stream_id { continue; }
        let entries: Vec<QueryEntry> = stream.entries.iter().take(before + after + 1).map(|e| QueryEntry {
            timestamp: chrono::DateTime::from_timestamp_nanos(e.timestamp).to_rfc3339(),
            line: e.line.clone(), fields: e.parsed_fields.clone(),
        }).collect();
        return Ok(Json(entries));
    }
    Ok(Json(vec![]))
}

pub async fn native_ingest(State(_s): State<AppState>, Json(req): Json<NativeIngestRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let mut ingested = 0usize;
    let mut added_bytes = 0u64;
    let mut added_lines = 0u64;
    for entry in req.entries {
        let labels = entry.labels.unwrap_or_else(|| {
            let mut m = HashMap::new(); m.insert("job".into(), "default".into()); m
        });
        let fp = label_fingerprint(&labels);
        let ts = entry.timestamp.and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok().map(|dt| dt.timestamp_nanos_opt().unwrap_or(0))).unwrap_or_else(|| Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let bytes = entry.line.len() as u64;
        let log_entry = LogEntry { timestamp: ts, line: entry.line, structured_metadata: HashMap::new(), parsed_fields: None };
        let stream = store.streams.entry(fp).or_insert_with(|| InternalStream { labels, entries: VecDeque::new(), bytes: 0 });
        stream.entries.push_back(log_entry);
        stream.bytes += bytes;
        added_bytes += bytes;
        added_lines += 1;
        if stream.entries.len() > 100_000 { stream.entries.pop_front(); }
        ingested += 1;
    }
    store.total_bytes += added_bytes;
    store.total_lines += added_lines;
    Ok(Json(serde_json::json!({"ingested": ingested})))
}

pub async fn loki_push(State(_s): State<AppState>, Json(req): Json<LokiPushRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let mut ingested = 0usize;
    let mut added_bytes = 0u64;
    let mut added_lines = 0u64;
    for loki_stream in req.streams {
        let fp = label_fingerprint(&loki_stream.stream);
        for (ts_str, line) in &loki_stream.values {
            let ts: i64 = ts_str.parse().unwrap_or_else(|_| Utc::now().timestamp_nanos_opt().unwrap_or(0));
            let bytes = line.len() as u64;
            let entry = LogEntry { timestamp: ts, line: line.clone(), structured_metadata: HashMap::new(), parsed_fields: None };
            let stream = store.streams.entry(fp.clone()).or_insert_with(|| InternalStream { labels: loki_stream.stream.clone(), entries: VecDeque::new(), bytes: 0 });
            stream.entries.push_back(entry);
            stream.bytes += bytes;
            added_bytes += bytes;
            added_lines += 1;
            if stream.entries.len() > 100_000 { stream.entries.pop_front(); }
            ingested += 1;
        }
    }
    store.total_bytes += added_bytes;
    store.total_lines += added_lines;
    Ok(Json(serde_json::json!({"ingested": ingested})))
}

pub async fn collect_logs(State(_s): State<AppState>, Json(req): Json<CollectRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "status": "collection_triggered",
        "target_host": req.target_host,
        "log_paths": req.log_paths,
        "lines_requested": req.lines.unwrap_or(100),
        "message": "SSH-based log collection would be triggered here. In demo mode, returning status."
    })))
}

pub async fn export_logs(State(_s): State<AppState>, Query(params): Query<ExportParams>) -> Result<Json<serde_json::Value>, ApiError> {
    let query = params.query.as_deref().unwrap_or("{}");
    let format = params.format.as_deref().unwrap_or("json");
    let limit = params.limit.unwrap_or(100);
    let store = store().read();
    let mut lines = Vec::new();
    for stream in store.streams.values() {
        for e in stream.entries.iter().take(limit) {
            lines.push(serde_json::json!({"timestamp": chrono::DateTime::from_timestamp_nanos(e.timestamp).to_rfc3339(), "line": e.line, "labels": stream.labels}));
        }
    }
    Ok(Json(serde_json::json!({"format": format, "query": query, "total": lines.len(), "entries": lines})))
}

// ── Log Alerts ───────────────────────────────────────────────────────

pub async fn list_log_alerts(State(_s): State<AppState>) -> Result<Json<Vec<LogAlert>>, ApiError> { Ok(Json(store().read().log_alerts.clone())) }

pub async fn create_log_alert(State(_s): State<AppState>, Json(req): Json<CreateLogAlertRequest>) -> Result<Json<LogAlert>, ApiError> {
    let alert = LogAlert {
        id: crate::error::short_id("la"),
        name: req.name, query: req.query, threshold: req.threshold.unwrap_or(10.0),
        comparison: req.comparison.unwrap_or("gt".into()), severity: req.severity.unwrap_or("warning".into()),
        enabled: true, last_fired: None, created_at: Utc::now().to_rfc3339(),
    };
    store().write().log_alerts.push(alert.clone());
    Ok(Json(alert))
}

pub async fn update_log_alert(State(_s): State<AppState>, Path(id): Path<String>, Json(req): Json<CreateLogAlertRequest>) -> Result<Json<LogAlert>, ApiError> {
    let mut store = store().write();
    let a = store.log_alerts.iter_mut().find(|a| a.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    a.name = req.name; a.query = req.query;
    if let Some(t) = req.threshold { a.threshold = t; }
    if let Some(c) = req.comparison { a.comparison = c; }
    if let Some(s) = req.severity { a.severity = s; }
    Ok(Json(a.clone()))
}

pub async fn delete_log_alert(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let b = store.log_alerts.len(); store.log_alerts.retain(|a| a.id != id);
    if store.log_alerts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}

pub async fn log_analytics(State(_s): State<AppState>) -> Result<Json<LogAnalytics>, ApiError> {
    let store = store().read();
    let now = Utc::now();
    Ok(Json(LogAnalytics {
        total_streams: store.streams.len(),
        total_entries: store.total_lines,
        total_bytes: store.total_bytes,
        ingestion_rate_lines_per_sec: 125.5,
        ingestion_rate_bytes_per_sec: 85_000.0,
        volume_by_source: vec![
            SourceVolume { source: "nginx".into(), lines: 45_000, bytes: 12_500_000 },
            SourceVolume { source: "api-server".into(), lines: 32_000, bytes: 8_200_000 },
            SourceVolume { source: "postgresql".into(), lines: 12_000, bytes: 4_500_000 },
            SourceVolume { source: "cron".into(), lines: 5_000, bytes: 1_200_000 },
        ],
        top_error_messages: vec![
            ErrorMessage { message: "Connection timeout".into(), count: 245, last_seen: (now - Duration::minutes(5)).to_rfc3339() },
            ErrorMessage { message: "Invalid JWT token".into(), count: 128, last_seen: (now - Duration::minutes(12)).to_rfc3339() },
            ErrorMessage { message: "Upstream not responding".into(), count: 85, last_seen: (now - Duration::minutes(30)).to_rfc3339() },
        ],
        error_rate_trend: (0..24).rev().map(|i| {
            let j = (i as f64 * 0.3).sin();
            VolumeEntry { timestamp: (now - Duration::hours(i)).to_rfc3339(), count: (50.0 + j * 30.0).max(0.0) as u64, bytes: 0 }
        }).collect(),
    }))
}

// ── Discovery Rules (P12) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryRule {
    pub id: String, pub name: String, pub rule_type: String, pub target_id: String,
    pub interval_secs: u64, pub enabled: bool, pub config: serde_json::Value,
    pub auto_create_checks: bool, pub template_id: Option<String>, pub last_run: Option<String>,
    pub discovered_items: Vec<DiscoveredItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredItem {
    pub key: String, pub name: String, pub item_type: String,
    pub properties: serde_json::Value, pub discovered_at: String, pub check_id: Option<String>,
}
#[derive(Debug, Deserialize)]
pub struct CreateDiscoveryRuleRequest {
    pub name: String, pub rule_type: String, pub target_id: String,
    pub interval_secs: Option<u64>, pub config: Option<serde_json::Value>,
    pub auto_create_checks: Option<bool>, pub template_id: Option<String>,
}

fn discovery_store() -> &'static RwLock<Vec<DiscoveryRule>> {
    static INSTANCE: OnceLock<RwLock<Vec<DiscoveryRule>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_discovery_store()))
}

fn init_discovery_store() -> Vec<DiscoveryRule> {
    let now = Utc::now();
    vec![
        DiscoveryRule {
            id: "dr-001".into(), name: "Filesystem Discovery".into(), rule_type: "filesystem".into(), target_id: "10.88.11.162".into(),
            interval_secs: 3600, enabled: true, config: serde_json::json!({"min_size_gb": 1}),
            auto_create_checks: true, template_id: None, last_run: Some((now - Duration::minutes(30)).to_rfc3339()),
            discovered_items: vec![
                DiscoveredItem { key: "/".into(), name: "Root filesystem".into(), item_type: "mount_point".into(), properties: serde_json::json!({"size_gb": 50, "used_pct": 62, "fs_type": "ext4"}), discovered_at: (now - Duration::minutes(30)).to_rfc3339(), check_id: Some("hc-disk-root".into()) },
                DiscoveredItem { key: "/data".into(), name: "Data filesystem".into(), item_type: "mount_point".into(), properties: serde_json::json!({"size_gb": 500, "used_pct": 78, "fs_type": "xfs"}), discovered_at: (now - Duration::minutes(30)).to_rfc3339(), check_id: Some("hc-disk-data".into()) },
                DiscoveredItem { key: "/boot".into(), name: "Boot partition".into(), item_type: "mount_point".into(), properties: serde_json::json!({"size_gb": 1, "used_pct": 35, "fs_type": "ext4"}), discovered_at: (now - Duration::minutes(30)).to_rfc3339(), check_id: None },
            ],
        },
        DiscoveryRule {
            id: "dr-002".into(), name: "Port Scan".into(), rule_type: "port_scan".into(), target_id: "10.88.11.162".into(),
            interval_secs: 7200, enabled: true, config: serde_json::json!({"port_range": "1-65535"}),
            auto_create_checks: false, template_id: None, last_run: Some((now - Duration::hours(1)).to_rfc3339()),
            discovered_items: vec![
                DiscoveredItem { key: "22".into(), name: "SSH".into(), item_type: "listening_port".into(), properties: serde_json::json!({"protocol": "tcp", "program": "sshd"}), discovered_at: (now - Duration::hours(1)).to_rfc3339(), check_id: None },
                DiscoveredItem { key: "80".into(), name: "HTTP".into(), item_type: "listening_port".into(), properties: serde_json::json!({"protocol": "tcp", "program": "nginx"}), discovered_at: (now - Duration::hours(1)).to_rfc3339(), check_id: None },
                DiscoveredItem { key: "5432".into(), name: "PostgreSQL".into(), item_type: "listening_port".into(), properties: serde_json::json!({"protocol": "tcp", "program": "postgres"}), discovered_at: (now - Duration::hours(1)).to_rfc3339(), check_id: None },
                DiscoveredItem { key: "7001".into(), name: "Zentra API".into(), item_type: "listening_port".into(), properties: serde_json::json!({"protocol": "tcp", "program": "zentra-server"}), discovered_at: (now - Duration::hours(1)).to_rfc3339(), check_id: None },
            ],
        },
        DiscoveryRule {
            id: "dr-003".into(), name: "Docker Containers".into(), rule_type: "docker_container".into(), target_id: "10.88.11.162".into(),
            interval_secs: 1800, enabled: true, config: serde_json::json!({}),
            auto_create_checks: true, template_id: None, last_run: Some((now - Duration::minutes(15)).to_rfc3339()),
            discovered_items: vec![
                DiscoveredItem { key: "a1b2c3d4".into(), name: "nginx-proxy".into(), item_type: "container".into(), properties: serde_json::json!({"image": "nginx:1.25", "status": "running"}), discovered_at: (now - Duration::minutes(15)).to_rfc3339(), check_id: None },
                DiscoveredItem { key: "b2c3d4e5".into(), name: "api-server".into(), item_type: "container".into(), properties: serde_json::json!({"image": "Zentra/api:latest", "status": "running"}), discovered_at: (now - Duration::minutes(15)).to_rfc3339(), check_id: None },
            ],
        },
        DiscoveryRule {
            id: "dr-004".into(), name: "Network Interfaces".into(), rule_type: "network_interface".into(), target_id: "10.88.11.162".into(),
            interval_secs: 3600, enabled: false, config: serde_json::json!({}),
            auto_create_checks: false, template_id: None, last_run: None, discovered_items: vec![],
        },
    ]
}

pub async fn list_discovery_rules(State(_s): State<AppState>) -> Result<Json<Vec<DiscoveryRule>>, ApiError> { Ok(Json(discovery_store().read().clone())) }

pub async fn create_discovery_rule(State(_s): State<AppState>, Json(req): Json<CreateDiscoveryRuleRequest>) -> Result<Json<DiscoveryRule>, ApiError> {
    let rule = DiscoveryRule {
        id: crate::error::short_id("dr"),
        name: req.name, rule_type: req.rule_type, target_id: req.target_id,
        interval_secs: req.interval_secs.unwrap_or(3600), enabled: true,
        config: req.config.unwrap_or(serde_json::json!({})),
        auto_create_checks: req.auto_create_checks.unwrap_or(false), template_id: req.template_id,
        last_run: None, discovered_items: vec![],
    };
    discovery_store().write().push(rule.clone());
    Ok(Json(rule))
}

pub async fn update_discovery_rule(State(_s): State<AppState>, Path(id): Path<String>, Json(req): Json<CreateDiscoveryRuleRequest>) -> Result<Json<DiscoveryRule>, ApiError> {
    let mut store = discovery_store().write();
    let r = store.iter_mut().find(|r| r.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    r.name = req.name; r.rule_type = req.rule_type; r.target_id = req.target_id;
    if let Some(i) = req.interval_secs { r.interval_secs = i; }
    if let Some(c) = req.config { r.config = c; }
    if let Some(a) = req.auto_create_checks { r.auto_create_checks = a; }
    r.template_id = req.template_id;
    Ok(Json(r.clone()))
}

pub async fn delete_discovery_rule(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = discovery_store().write();
    let b = store.len(); store.retain(|r| r.id != id);
    if store.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}

pub async fn run_discovery_rule(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<DiscoveryRule>, ApiError> {
    let mut store = discovery_store().write();
    let r = store.iter_mut().find(|r| r.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    r.last_run = Some(Utc::now().to_rfc3339());
    Ok(Json(r.clone()))
}

pub async fn discovery_rule_items(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DiscoveredItem>>, ApiError> {
    let store = discovery_store().read();
    let r = store.iter().find(|r| r.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(r.discovered_items.clone()))
}

// ── Live Tail SSE ─────────────────────────────────────────────────────

use axum::response::sse::{Event, Sse};
use futures::Stream;
use std::{convert::Infallible, time::Duration as StdDuration};
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::StreamExt;

#[derive(Debug, Deserialize)]
pub struct TailParams {
    pub query: Option<String>,
    pub limit: Option<usize>,
}

/// Server-Sent Events live tail endpoint.
/// Streams new log entries matching the query every second.
pub async fn live_tail(
    State(_s): State<AppState>,
    Query(params): Query<TailParams>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let query = params.query.unwrap_or_else(|| "{}".into());
    let limit = params.limit.unwrap_or(50);

    // Track last seen timestamp to emit only new entries
    let last_ts = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));

    let interval = tokio::time::interval(StdDuration::from_secs(1));
    let stream = IntervalStream::new(interval).map(move |_| {
        let cutoff = last_ts.load(std::sync::atomic::Ordering::Relaxed);
        let store = store().read();
        let stream_selector = query.split('}').next()
            .map(|s| format!("{}}}", s)).unwrap_or_default();

        let mut new_entries: Vec<serde_json::Value> = Vec::new();
        let mut max_ts = cutoff;

        for stream in store.streams.values() {
            if !match_stream(&stream.labels, &stream_selector) { continue; }
            for entry in stream.entries.iter().rev().take(limit) {
                if entry.timestamp <= cutoff { break; }
                if !match_line_filter(&entry.line, &query) { continue; }
                if entry.timestamp > max_ts { max_ts = entry.timestamp; }
                new_entries.push(serde_json::json!({
                    "timestamp": chrono::DateTime::from_timestamp_nanos(entry.timestamp).to_rfc3339(),
                    "line": entry.line,
                    "labels": stream.labels,
                    "fields": entry.parsed_fields,
                }));
            }
        }

        if max_ts > cutoff {
            last_ts.store(max_ts, std::sync::atomic::Ordering::Relaxed);
        }

        let count = new_entries.len();
        let data = serde_json::json!({ "entries": new_entries, "count": count });
        Ok(Event::default().data(data.to_string()))
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(StdDuration::from_secs(15))
            .text("ping"),
    )
}
