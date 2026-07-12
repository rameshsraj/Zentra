//! OT02: Async OTLP ingest pipeline with DLQ.
//!
//! Handlers decode and enqueue; background workers batch-insert into the DB.
//! On final failure (3 retries), the batch is written to `otlp_dead_letter`.
//! Provides backpressure via bounded channels (503 if full).

use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

// ============================================================================
// Types
// ============================================================================

/// A single ingest batch from a decoded OTLP payload.
#[derive(Debug, Clone)]
pub struct IngestBatch {
    pub org_id: Uuid,
    pub signal: Signal,
    pub payload: Value,
    pub raw_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Traces,
    Metrics,
    Logs,
    Profiles,
}

impl Signal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Signal::Traces => "traces",
            Signal::Metrics => "metrics",
            Signal::Logs => "logs",
            Signal::Profiles => "profiles",
        }
    }
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Channel capacity per signal (bounded for backpressure).
const CHANNEL_CAPACITY: usize = 10_000;

/// Max items per batch insert.
const BATCH_SIZE: usize = 500;

/// Max time to wait before flushing a partial batch.
const BATCH_TIMEOUT_MS: u64 = 200;

/// Max retries before DLQ.
const MAX_RETRIES: u32 = 3;

/// OT08: Receiver-side sampling ratio (0.0 = drop all, 1.0 = keep all).
/// Configurable via ZENTRA_OTLP_SAMPLE_RATIO env var.
fn sample_ratio() -> f64 {
    let ratio: f64 = std::env::var("ZENTRA_OTLP_SAMPLE_RATIO")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    ratio.clamp(0.0, 1.0)
}

/// OT08: Probabilistic sampling — returns true if the item should be kept.
fn should_sample(trace_id: &str) -> bool {
    let ratio = sample_ratio();
    if ratio >= 1.0 { return true; }
    if ratio <= 0.0 { return false; }
    // Deterministic sampling based on trace_id hash for consistency
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    trace_id.hash(&mut hasher);
    let hash = hasher.finish();
    (hash as f64 / u64::MAX as f64) < ratio
}

// ============================================================================
// Ingest Queue (lives in AppState)
// ============================================================================

/// Holds bounded senders for each signal. Handlers enqueue here.
#[derive(Clone)]
pub struct OtlpIngestQueue {
    pub traces_tx: mpsc::Sender<IngestBatch>,
    pub metrics_tx: mpsc::Sender<IngestBatch>,
    pub logs_tx: mpsc::Sender<IngestBatch>,
    pub profiles_tx: mpsc::Sender<IngestBatch>,
}

impl OtlpIngestQueue {
    /// Try to enqueue a batch. Returns Err if channel is full (backpressure → 503).
    pub fn try_send(&self, batch: IngestBatch) -> Result<(), mpsc::error::TrySendError<IngestBatch>> {
        match batch.signal {
            Signal::Traces => self.traces_tx.try_send(batch),
            Signal::Metrics => self.metrics_tx.try_send(batch),
            Signal::Logs => self.logs_tx.try_send(batch),
            Signal::Profiles => self.profiles_tx.try_send(batch),
        }
    }
}

/// Create the queue and spawn background workers. Returns the queue for AppState.
pub fn spawn_ingest_workers(
    pool: PgPool,
    tsdb_pool: PgPool,
) -> OtlpIngestQueue {
    let (traces_tx, traces_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (metrics_tx, metrics_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (logs_tx, logs_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (profiles_tx, profiles_rx) = mpsc::channel(CHANNEL_CAPACITY);

    let tsdb = Arc::new(tsdb_pool);
    let pg = Arc::new(pool);

    tokio::spawn(worker_loop(Signal::Traces, traces_rx, tsdb.clone()));
    tokio::spawn(worker_loop(Signal::Metrics, metrics_rx, tsdb.clone()));
    tokio::spawn(worker_loop(Signal::Logs, logs_rx, tsdb.clone()));
    tokio::spawn(worker_loop(Signal::Profiles, profiles_rx, pg));

    info!("OTLP ingest workers spawned (4 signals, batch_size={}, timeout={}ms)", BATCH_SIZE, BATCH_TIMEOUT_MS);

    OtlpIngestQueue {
        traces_tx,
        metrics_tx,
        logs_tx,
        profiles_tx,
    }
}

// ============================================================================
// Background worker
// ============================================================================

async fn worker_loop(
    signal: Signal,
    mut rx: mpsc::Receiver<IngestBatch>,
    pool: Arc<PgPool>,
) {
    let mut buffer: Vec<IngestBatch> = Vec::with_capacity(BATCH_SIZE);

    loop {
        // Wait for first item or channel close
        let timeout = tokio::time::Duration::from_millis(BATCH_TIMEOUT_MS);

        tokio::select! {
            maybe = rx.recv() => {
                match maybe {
                    Some(batch) => buffer.push(batch),
                    None => {
                        // Channel closed — flush remaining and exit
                        if !buffer.is_empty() {
                            flush_batch(signal, &pool, &mut buffer).await;
                        }
                        info!(signal = %signal, "Ingest worker shutting down");
                        return;
                    }
                }
            }
            _ = tokio::time::sleep(timeout), if !buffer.is_empty() => {
                // Timeout with pending items — flush
            }
        }

        // Drain up to BATCH_SIZE
        while buffer.len() < BATCH_SIZE {
            match rx.try_recv() {
                Ok(batch) => buffer.push(batch),
                Err(_) => break,
            }
        }

        if buffer.len() >= BATCH_SIZE || !buffer.is_empty() {
            flush_batch(signal, &pool, &mut buffer).await;
        }
    }
}

async fn flush_batch(
    signal: Signal,
    pool: &PgPool,
    buffer: &mut Vec<IngestBatch>,
) {
    let batch: Vec<IngestBatch> = buffer.drain(..).collect();
    let count = batch.len();
    let total_bytes: usize = batch.iter().map(|b| b.raw_bytes.len()).sum();
    let org_id = batch.first().map(|b| b.org_id).unwrap_or(Uuid::nil());

    for retry in 0..MAX_RETRIES {
        match insert_batch(signal, pool, &batch).await {
            Ok(inserted) => {
                info!(signal = %signal, items = inserted, "Batch inserted");
                metrics::counter!("zentra_otlp_ingest_total", "signal" => signal.as_str())
                    .increment(inserted as u64);
                // OT04: Write audit row (fire-and-forget)
                write_audit(pool, org_id, signal, count as i32, total_bytes as i32, "accepted").await;
                return;
            }
            Err(e) => {
                let backoff = tokio::time::Duration::from_millis(100 * 2u64.pow(retry));
                warn!(
                    signal = %signal,
                    retry = retry + 1,
                    error = %e,
                    backoff_ms = backoff.as_millis() as u64,
                    "Batch insert failed, retrying"
                );
                tokio::time::sleep(backoff).await;
            }
        }
    }

    // Final failure → DLQ
    error!(signal = %signal, items = count, "Batch insert failed after {} retries, writing to DLQ", MAX_RETRIES);
    metrics::counter!("zentra_otlp_dlq_total", "signal" => signal.as_str())
        .increment(count as u64);
    // OT04: Write audit row for DLQ
    write_audit(pool, org_id, signal, count as i32, total_bytes as i32, "dlq").await;

    for item in &batch {
        let _ = sqlx::query(
            "INSERT INTO otlp_dead_letter (org_id, signal, payload, error_message, retry_count) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(item.org_id)
        .bind(signal.as_str())
        .bind(&item.raw_bytes)
        .bind("Max retries exceeded")
        .bind(MAX_RETRIES as i32)
        .execute(pool)
        .await;
    }
}

/// OT04: Write an audit row to otlp_ingest_audit (fire-and-forget).
async fn write_audit(pool: &PgPool, org_id: Uuid, signal: Signal, item_count: i32, byte_size: i32, status: &str) {
    let _ = sqlx::query(
        "INSERT INTO otlp_ingest_audit (org_id, signal, endpoint, status_code, span_count, payload_bytes, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW())"
    )
    .bind(org_id)
    .bind(signal.as_str())
    .bind(format!("/v1/{}", signal.as_str()))
    .bind(if status == "accepted" { 200i32 } else { 500 })
    .bind(item_count)
    .bind(byte_size)
    .execute(pool)
    .await;
}

// ============================================================================
// Batch INSERT logic per signal
// ============================================================================

async fn insert_batch(
    signal: Signal,
    pool: &PgPool,
    batch: &[IngestBatch],
) -> Result<usize, sqlx::Error> {
    match signal {
        Signal::Traces => insert_traces_batch(pool, batch).await,
        Signal::Metrics => insert_metrics_batch(pool, batch).await,
        Signal::Logs => insert_logs_batch(pool, batch).await,
        Signal::Profiles => insert_profiles_batch(pool, batch).await,
    }
}

async fn insert_traces_batch(pool: &PgPool, batch: &[IngestBatch]) -> Result<usize, sqlx::Error> {
    use chrono::DateTime;

    let mut count = 0usize;

    // Collect all rows, then do a single multi-row INSERT
    let mut org_ids: Vec<uuid::Uuid> = Vec::new();
    let mut trace_ids = Vec::new();
    let mut span_ids = Vec::new();
    let mut parent_span_ids: Vec<Option<String>> = Vec::new();
    let mut names = Vec::new();
    let mut service_names = Vec::new();
    let mut started_ats = Vec::new();
    let mut duration_us_vals = Vec::new();
    let mut statuses = Vec::new();
    let mut tags_vals: Vec<serde_json::Value> = Vec::new();

    // OT11: Span events and links
    let mut evt_trace_ids = Vec::new();
    let mut evt_span_ids = Vec::new();
    let mut evt_names = Vec::new();
    let mut evt_timestamps: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    let mut evt_attrs: Vec<serde_json::Value> = Vec::new();

    let mut link_trace_ids = Vec::new();
    let mut link_span_ids = Vec::new();
    let mut link_linked_trace_ids = Vec::new();
    let mut link_linked_span_ids = Vec::new();
    let mut link_attrs: Vec<serde_json::Value> = Vec::new();

    for item in batch {
        if let Some(resource_spans) = item.payload.get("resourceSpans").and_then(|v| v.as_array()) {
            for rs in resource_spans {
                let resource_attrs = rs.pointer("/resource/attributes").cloned().unwrap_or(serde_json::json!({}));
                let svc = extract_service_name(&resource_attrs);

                if let Some(scope_spans) = rs.get("scopeSpans").and_then(|v| v.as_array()) {
                    for ss in scope_spans {
                        let spans_key = if ss.get("spans").is_some() { "spans" } else { "traces" };
                        if let Some(spans) = ss.get(spans_key).and_then(|v| v.as_array()) {
                            for span in spans {
                                let tid = span.get("traceId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                // OT08: Receiver-side sampling
                                if !should_sample(&tid) {
                                    metrics::counter!("zentra_otlp_sampled_out_total", "signal" => "traces").increment(1);
                                    continue;
                                }
                                let sid = span.get("spanId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let psid = span.get("parentSpanId").and_then(|v| v.as_str()).map(|s| s.to_string());
                                let name = span.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                                let start_ns = span.get("startTimeUnixNano")
                                    .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
                                    .unwrap_or(0);
                                let end_ns = span.get("endTimeUnixNano")
                                    .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
                                    .unwrap_or(0);
                                let dur = (end_ns - start_ns) / 1000;
                                let status_code = span.pointer("/status/code").and_then(|v| v.as_i64()).unwrap_or(0);
                                let status = if status_code == 2 { "error" } else { "ok" };
                                let attributes = span.get("attributes").cloned().unwrap_or(serde_json::json!([]));
                                let tags = otlp_attrs_to_map(&attributes);
                                let started_at = DateTime::from_timestamp_nanos(start_ns);

                                // OT11: Collect span events
                                if let Some(events) = span.get("events").and_then(|v| v.as_array()) {
                                    for evt in events {
                                        let etime_ns = evt.get("timeUnixNano")
                                            .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
                                            .unwrap_or(start_ns);
                                        let ename = evt.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let eattrs = otlp_attrs_to_map(&evt.get("attributes").cloned().unwrap_or(serde_json::json!([])));
                                        evt_trace_ids.push(tid.clone());
                                        evt_span_ids.push(sid.clone());
                                        evt_names.push(ename);
                                        evt_timestamps.push(DateTime::from_timestamp_nanos(etime_ns));
                                        evt_attrs.push(eattrs);
                                    }
                                }

                                // OT11: Collect span links
                                if let Some(links) = span.get("links").and_then(|v| v.as_array()) {
                                    for lnk in links {
                                        let ltid = lnk.get("traceId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let lsid = lnk.get("spanId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let lattrs = otlp_attrs_to_map(&lnk.get("attributes").cloned().unwrap_or(serde_json::json!([])));
                                        link_trace_ids.push(tid.clone());
                                        link_span_ids.push(sid.clone());
                                        link_linked_trace_ids.push(ltid);
                                        link_linked_span_ids.push(lsid);
                                        link_attrs.push(lattrs);
                                    }
                                }

                                org_ids.push(item.org_id);
                                trace_ids.push(tid);
                                span_ids.push(sid);
                                parent_span_ids.push(psid);
                                names.push(name);
                                service_names.push(svc.clone());
                                started_ats.push(started_at);
                                duration_us_vals.push(dur);
                                statuses.push(status.to_string());
                                tags_vals.push(tags);
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        return Ok(0);
    }

    // Multi-row INSERT using UNNEST for efficiency — O02: includes org_id
    sqlx::query(
        r#"INSERT INTO traces (org_id, trace_id, span_id, parent_span_id, operation_name, service_name,
            started_at, duration_us, status, tags)
           SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
            $7::timestamptz[], $8::bigint[], $9::text[], $10::jsonb[])
           ON CONFLICT DO NOTHING"#
    )
    .bind(&org_ids)
    .bind(&trace_ids)
    .bind(&span_ids)
    .bind(&parent_span_ids)
    .bind(&names)
    .bind(&service_names)
    .bind(&started_ats)
    .bind(&duration_us_vals)
    .bind(&statuses)
    .bind(&tags_vals)
    .execute(pool)
    .await?;

    // OT11: Insert span events
    if !evt_trace_ids.is_empty() {
        let _ = sqlx::query(
            "INSERT INTO span_events (trace_id, span_id, event_name, timestamp, attributes) \
             SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::timestamptz[], $5::jsonb[])"
        )
        .bind(&evt_trace_ids)
        .bind(&evt_span_ids)
        .bind(&evt_names)
        .bind(&evt_timestamps)
        .bind(&evt_attrs)
        .execute(pool)
        .await;
    }

    // OT11: Insert span links
    if !link_trace_ids.is_empty() {
        let _ = sqlx::query(
            "INSERT INTO span_links (trace_id, span_id, linked_trace_id, linked_span_id, attributes) \
             SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[], $4::text[], $5::jsonb[])"
        )
        .bind(&link_trace_ids)
        .bind(&link_span_ids)
        .bind(&link_linked_trace_ids)
        .bind(&link_linked_span_ids)
        .bind(&link_attrs)
        .execute(pool)
        .await;
    }

    Ok(count)
}

async fn insert_metrics_batch(pool: &PgPool, batch: &[IngestBatch]) -> Result<usize, sqlx::Error> {
    use chrono::DateTime;

    let mut count = 0usize;
    let mut org_ids = Vec::new();
    let mut target_ids_vec: Vec<uuid::Uuid> = Vec::new();
    let mut metric_names = Vec::new();
    let mut values = Vec::new();
    let mut times: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    let mut resource_ids = Vec::new();
    let mut labels_vals: Vec<serde_json::Value> = Vec::new();

    for item in batch {
        if let Some(resource_metrics) = item.payload.get("resourceMetrics").and_then(|v| v.as_array()) {
            for rm in resource_metrics {
                let resource_attrs = rm.pointer("/resource/attributes").cloned().unwrap_or(serde_json::json!({}));
                let rid = extract_service_name(&resource_attrs);
                // Extract zentra.target_id from resource attributes; fall back to nil UUID
                let target_id = extract_attr_str(&resource_attrs, "zentra.target_id")
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .unwrap_or(uuid::Uuid::nil());

                if let Some(scope_metrics) = rm.get("scopeMetrics").and_then(|v| v.as_array()) {
                    for sm in scope_metrics {
                        if let Some(metrics) = sm.get("metrics").and_then(|v| v.as_array()) {
                            for metric in metrics {
                                let name = metric.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();

                                // Helper closure to push a single data point
                                let mut push_dp = |mname: String, val: f64, time_ns: i64, dp_attrs: &serde_json::Value| {
                                    let time = DateTime::from_timestamp_nanos(time_ns);
                                    let labels = otlp_attrs_to_map(dp_attrs);
                                    org_ids.push(item.org_id);
                                    target_ids_vec.push(target_id);
                                    metric_names.push(mname);
                                    values.push(val);
                                    times.push(time);
                                    resource_ids.push(rid.clone());
                                    labels_vals.push(labels);
                                    count += 1;
                                };

                                let extract_time = |dp: &serde_json::Value| -> i64 {
                                    dp.get("timeUnixNano")
                                        .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
                                        .unwrap_or(0)
                                };
                                let extract_value = |dp: &serde_json::Value| -> f64 {
                                    dp.get("asDouble").and_then(|v| v.as_f64())
                                        .or_else(|| dp.get("asInt").and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_i64().map(|n| n as f64))))
                                        .unwrap_or(0.0)
                                };
                                let empty_attrs = serde_json::json!([]);

                                // Gauge & Sum data points
                                let gauge_sum_dps = metric.get("gauge").and_then(|g| g.get("dataPoints"))
                                    .or_else(|| metric.get("sum").and_then(|s| s.get("dataPoints")))
                                    .and_then(|v| v.as_array());
                                if let Some(dps) = gauge_sum_dps {
                                    for dp in dps {
                                        let attrs = dp.get("attributes").cloned().unwrap_or(empty_attrs.clone());
                                        push_dp(name.clone(), extract_value(dp), extract_time(dp), &attrs);
                                    }
                                }

                                // O05: Histogram data points
                                if let Some(hist) = metric.get("histogram").and_then(|h| h.get("dataPoints")).and_then(|v| v.as_array()) {
                                    for dp in hist {
                                        let tns = extract_time(dp);
                                        let attrs = dp.get("attributes").cloned().unwrap_or(empty_attrs.clone());
                                        if let Some(c) = dp.get("count").and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_u64().map(|n| n as f64))) {
                                            push_dp(format!("{}_count", name), c, tns, &attrs);
                                        }
                                        if let Some(s) = dp.get("sum").and_then(|v| v.as_f64()) {
                                            push_dp(format!("{}_sum", name), s, tns, &attrs);
                                        }
                                        if let Some(bounds) = dp.get("explicitBounds").and_then(|v| v.as_array()) {
                                            if let Some(counts) = dp.get("bucketCounts").and_then(|v| v.as_array()) {
                                                for (i, bc) in counts.iter().enumerate() {
                                                    let le = if i < bounds.len() {
                                                        bounds[i].as_f64().unwrap_or(f64::INFINITY)
                                                    } else { f64::INFINITY };
                                                    let bval = bc.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| bc.as_u64().map(|n| n as f64)).unwrap_or(0.0);
                                                    let mut bucket_attrs = attrs.clone();
                                                    if let Some(arr) = bucket_attrs.as_array_mut() {
                                                        arr.push(serde_json::json!({"key": "le", "value": {"stringValue": format!("{}", le)}}));
                                                    }
                                                    push_dp(format!("{}_bucket", name), bval, tns, &bucket_attrs);
                                                }
                                            }
                                        }
                                    }
                                }

                                // O05: Exponential histogram data points
                                if let Some(ehist) = metric.get("exponentialHistogram").and_then(|h| h.get("dataPoints")).and_then(|v| v.as_array()) {
                                    for dp in ehist {
                                        let tns = extract_time(dp);
                                        let attrs = dp.get("attributes").cloned().unwrap_or(empty_attrs.clone());
                                        if let Some(c) = dp.get("count").and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_u64().map(|n| n as f64))) {
                                            push_dp(format!("{}_count", name), c, tns, &attrs);
                                        }
                                        if let Some(s) = dp.get("sum").and_then(|v| v.as_f64()) {
                                            push_dp(format!("{}_sum", name), s, tns, &attrs);
                                        }
                                    }
                                }

                                // O05: Summary data points
                                if let Some(summary) = metric.get("summary").and_then(|h| h.get("dataPoints")).and_then(|v| v.as_array()) {
                                    for dp in summary {
                                        let tns = extract_time(dp);
                                        let attrs = dp.get("attributes").cloned().unwrap_or(empty_attrs.clone());
                                        if let Some(c) = dp.get("count").and_then(|v| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_u64().map(|n| n as f64))) {
                                            push_dp(format!("{}_count", name), c, tns, &attrs);
                                        }
                                        if let Some(s) = dp.get("sum").and_then(|v| v.as_f64()) {
                                            push_dp(format!("{}_sum", name), s, tns, &attrs);
                                        }
                                        if let Some(qvs) = dp.get("quantileValues").and_then(|v| v.as_array()) {
                                            for qv in qvs {
                                                let quantile = qv.get("quantile").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                                let qval = qv.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                                let mut q_attrs = attrs.clone();
                                                if let Some(arr) = q_attrs.as_array_mut() {
                                                    arr.push(serde_json::json!({"key": "quantile", "value": {"stringValue": format!("{}", quantile)}}));
                                                }
                                                push_dp(name.clone(), qval, tns, &q_attrs);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        return Ok(0);
    }

    // O03: Fix column names to match real schema — timestamp (not time), tags (not labels)
    // plugin_id/resource_type default to 'otlp' for OTLP-sourced metrics
    // target_id is extracted from zentra.target_id resource attribute (nil UUID if absent)
    let plugin_ids: Vec<String> = vec!["otlp".to_string(); count];
    let resource_types: Vec<String> = vec!["otlp".to_string(); count];
    let units: Vec<Option<String>> = vec![None; count];
    let metadata_vals: Vec<serde_json::Value> = vec![serde_json::json!({}); count];

    sqlx::query(
        "INSERT INTO metrics (timestamp, org_id, target_id, plugin_id, resource_type, resource_id, metric_name, value, unit, tags, metadata) \
         SELECT * FROM UNNEST($1::timestamptz[], $2::uuid[], $3::uuid[], $4::text[], $5::text[], $6::text[], $7::text[], $8::float8[], $9::text[], $10::jsonb[], $11::jsonb[]) \
         ON CONFLICT DO NOTHING"
    )
    .bind(&times)
    .bind(&org_ids)
    .bind(&target_ids_vec)
    .bind(&plugin_ids)
    .bind(&resource_types)
    .bind(&resource_ids)
    .bind(&metric_names)
    .bind(&values)
    .bind(&units)
    .bind(&labels_vals)
    .bind(&metadata_vals)
    .execute(pool)
    .await?;

    Ok(count)
}

async fn insert_logs_batch(pool: &PgPool, batch: &[IngestBatch]) -> Result<usize, sqlx::Error> {
    use chrono::DateTime;

    let mut count = 0usize;
    let mut org_ids = Vec::new();
    let mut log_ids = Vec::new();
    let mut target_ids = Vec::new();
    let mut timestamps: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    let mut sources = Vec::new();
    let mut levels = Vec::new();
    let mut messages = Vec::new();
    let mut metadata_vals: Vec<serde_json::Value> = Vec::new();

    for item in batch {
        if let Some(resource_logs) = item.payload.get("resourceLogs").and_then(|v| v.as_array()) {
            for rl in resource_logs {
                let resource_attrs = rl.pointer("/resource/attributes").cloned().unwrap_or(serde_json::json!({}));
                let resource_map = otlp_attrs_to_map(&resource_attrs);
                let source = extract_service_name(&resource_attrs);
                let target_id = extract_attr_str(&resource_attrs, "zentra.target_id")
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .or_else(|| extract_attr_str(&resource_attrs, "zentra.agent_id").and_then(|s| uuid::Uuid::parse_str(s).ok()))
                    .unwrap_or(uuid::Uuid::nil());

                if let Some(scope_logs) = rl.get("scopeLogs").and_then(|v| v.as_array()) {
                    for sl in scope_logs {
                        if let Some(log_records) = sl.get("logRecords").and_then(|v| v.as_array()) {
                            for lr in log_records {
                                let time_ns = lr.get("timeUnixNano")
                                    .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
                                    .unwrap_or(0);
                                let ts = DateTime::from_timestamp_nanos(time_ns);
                                let severity = lr.get("severityText").and_then(|v| v.as_str()).unwrap_or("INFO").to_string();
                                let body = lr.get("body")
                                    .and_then(|b| b.get("stringValue"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("").to_string();
                                let mut attrs = match otlp_attrs_to_map(&lr.get("attributes").cloned().unwrap_or(serde_json::json!([]))) {
                                    serde_json::Value::Object(map) => map,
                                    _ => serde_json::Map::new(),
                                };
                                if let serde_json::Value::Object(resource_obj) = resource_map.clone() {
                                    for (key, value) in resource_obj {
                                        attrs.entry(key).or_insert(value);
                                    }
                                }

                                // O18: Extract trace context so logs can be correlated with traces
                                if let Some(tid) = lr.get("traceId").and_then(|v| v.as_str()) {
                                    if !tid.is_empty() {
                                        attrs.insert("trace_id".to_string(), serde_json::Value::String(tid.to_string()));
                                    }
                                }
                                if let Some(sid) = lr.get("spanId").and_then(|v| v.as_str()) {
                                    if !sid.is_empty() {
                                        attrs.insert("span_id".to_string(), serde_json::Value::String(sid.to_string()));
                                    }
                                }

                                log_ids.push(uuid::Uuid::new_v4());
                                target_ids.push(target_id);
                                org_ids.push(item.org_id);
                                timestamps.push(ts);
                                sources.push(source.clone());
                                levels.push(severity);
                                messages.push(body);
                                metadata_vals.push(serde_json::Value::Object(attrs));
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        return Ok(0);
    }

    let raw_logs = messages.clone();
    let parsed_fields = metadata_vals.clone();
    let tags_vals = metadata_vals;

    sqlx::query(
        "INSERT INTO logs (log_id, timestamp, org_id, target_id, source, severity, message, raw_log, parsed_fields, tags) \
         SELECT * FROM UNNEST($1::uuid[], $2::timestamptz[], $3::uuid[], $4::uuid[], $5::text[], $6::text[], $7::text[], $8::text[], $9::jsonb[], $10::jsonb[])"
    )
    .bind(&log_ids)
    .bind(&timestamps)
    .bind(&org_ids)
    .bind(&target_ids)
    .bind(&sources)
    .bind(&levels)
    .bind(&messages)
    .bind(&raw_logs)
    .bind(&parsed_fields)
    .bind(&tags_vals)
    .execute(pool)
    .await?;

    Ok(count)
}

async fn insert_profiles_batch(pool: &PgPool, batch: &[IngestBatch]) -> Result<usize, sqlx::Error> {
    let mut count = 0usize;
    let mut org_ids = Vec::new();
    let mut profile_ids = Vec::new();
    let mut service_names = Vec::new();
    let mut resource_attrs_vals: Vec<serde_json::Value> = Vec::new();
    let mut sample_counts = Vec::new();
    let mut flamegraph_vals: Vec<serde_json::Value> = Vec::new();
    let mut attrs_vals: Vec<serde_json::Value> = Vec::new();

    for item in batch {
        if let Some(resource_profiles) = item.payload.get("resourceProfiles").and_then(|v| v.as_array()) {
            for rp in resource_profiles {
                let resource_attrs = rp.pointer("/resource/attributes").cloned().unwrap_or(serde_json::json!({}));
                let svc = extract_service_name(&resource_attrs);

                if let Some(scope_profiles) = rp.get("scopeProfiles").and_then(|v| v.as_array()) {
                    for sp in scope_profiles {
                        if let Some(profiles) = sp.get("profiles").and_then(|v| v.as_array()) {
                            for profile in profiles {
                                let pid = profile.get("profileId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let samples = profile.get("samples").and_then(|v| v.as_array());
                                let sc = samples.map(|a| a.len() as i32).unwrap_or(0);

                                // O09: Build flamegraph data from samples
                                // Extract string_table, function_table, location_table, samples
                                let string_table = profile.get("stringTable").and_then(|v| v.as_array());
                                let function_table = profile.get("function").and_then(|v| v.as_array())
                                    .or_else(|| profile.get("functionTable").and_then(|v| v.as_array()));
                                let location_table = profile.get("location").and_then(|v| v.as_array())
                                    .or_else(|| profile.get("locationTable").and_then(|v| v.as_array()));

                                let get_string = |idx: i64| -> String {
                                    string_table
                                        .and_then(|st| st.get(idx as usize))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string()
                                };

                                let get_func_name = |func_idx: i64| -> String {
                                    function_table
                                        .and_then(|ft| ft.get(func_idx as usize))
                                        .and_then(|f| f.get("name").and_then(|n| n.as_i64()))
                                        .map(|name_idx| get_string(name_idx))
                                        .unwrap_or_else(|| format!("func_{}", func_idx))
                                };

                                let mut flamegraph_frames: Vec<serde_json::Value> = Vec::new();
                                if let Some(samps) = samples {
                                    for sample in samps {
                                        let location_indices = sample.get("locationIndex")
                                            .or_else(|| sample.get("locationId"))
                                            .and_then(|v| v.as_array());
                                        let value = sample.get("value")
                                            .and_then(|v| v.as_array())
                                            .and_then(|a| a.first())
                                            .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                                            .unwrap_or(1);

                                        let mut stack: Vec<String> = Vec::new();
                                        if let Some(locs) = location_indices {
                                            for loc_ref in locs.iter().rev() {
                                                let loc_idx = loc_ref.as_i64().unwrap_or(0);
                                                let loc = location_table.and_then(|lt| lt.get(loc_idx as usize));
                                                if let Some(l) = loc {
                                                    let lines = l.get("line").and_then(|v| v.as_array());
                                                    if let Some(ls) = lines {
                                                        for line in ls {
                                                            let func_idx = line.get("functionIndex")
                                                                .or_else(|| line.get("functionId"))
                                                                .and_then(|v| v.as_i64())
                                                                .unwrap_or(0);
                                                            stack.push(get_func_name(func_idx));
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if !stack.is_empty() {
                                            flamegraph_frames.push(serde_json::json!({
                                                "stack": stack,
                                                "value": value,
                                            }));
                                        }
                                    }
                                }

                                let profile_attrs = otlp_attrs_to_map(
                                    &profile.get("attributes").cloned().unwrap_or(serde_json::json!([]))
                                );

                                org_ids.push(item.org_id);
                                profile_ids.push(pid);
                                service_names.push(svc.clone());
                                resource_attrs_vals.push(resource_attrs.clone());
                                sample_counts.push(sc);
                                flamegraph_vals.push(serde_json::json!(flamegraph_frames));
                                attrs_vals.push(profile_attrs);
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if count == 0 {
        return Ok(0);
    }

    sqlx::query(
        r#"INSERT INTO otel_profiles (org_id, profile_id, service_name, resource_attributes,
            sample_count, flamegraph_data, attributes)
           SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::jsonb[],
            $5::int[], $6::jsonb[], $7::jsonb[])"#
    )
    .bind(&org_ids)
    .bind(&profile_ids)
    .bind(&service_names)
    .bind(&resource_attrs_vals)
    .bind(&sample_counts)
    .bind(&flamegraph_vals)
    .bind(&attrs_vals)
    .execute(pool)
    .await?;

    Ok(count)
}

// ============================================================================
// Helpers (duplicated from otlp.rs to avoid circular deps)
// ============================================================================

fn extract_service_name(attrs: &Value) -> String {
    if let Some(arr) = attrs.as_array() {
        for attr in arr {
            if attr.get("key").and_then(|k| k.as_str()) == Some("service.name") {
                if let Some(sv) = attr.pointer("/value/stringValue").and_then(|v| v.as_str()) {
                    return sv.to_string();
                }
            }
        }
    }
    if let Some(obj) = attrs.as_object() {
        if let Some(name) = obj.get("service.name").and_then(|v| v.as_str()) {
            return name.to_string();
        }
    }
    "unknown".to_string()
}

fn extract_attr_str<'a>(attrs: &'a Value, key: &str) -> Option<&'a str> {
    if let Some(arr) = attrs.as_array() {
        for attr in arr {
            if attr.get("key").and_then(|k| k.as_str()) == Some(key) {
                return attr.pointer("/value/stringValue").and_then(|v| v.as_str());
            }
        }
    }
    None
}

fn otlp_attrs_to_map(attrs: &Value) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(arr) = attrs.as_array() {
        for attr in arr {
            if let Some(key) = attr.get("key").and_then(|k| k.as_str()) {
                let val = attr.get("value")
                    .and_then(|v| {
                        v.get("stringValue").cloned()
                            .or_else(|| v.get("intValue").cloned())
                            .or_else(|| v.get("doubleValue").cloned())
                            .or_else(|| v.get("boolValue").cloned())
                    })
                    .unwrap_or(serde_json::json!(null));
                map.insert(key.to_string(), val);
            }
        }
    }
    Value::Object(map)
}