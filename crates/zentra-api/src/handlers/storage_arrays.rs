use axum::{
    extract::{Path, Query, State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;
use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::ssh_utils;

use crate::error::ApiError;

// ============================================================================
// Enterprise Storage Arrays — DB-backed with live SNMP polling
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageArray {
    pub id: String,
    pub name: String,
    pub array_type: String,
    pub vendor: String,
    pub model: String,
    pub firmware: String,
    pub ip: String,
    pub status: String,
    pub total_tb: f64,
    pub used_tb: f64,
    pub free_tb: f64,
    pub utilization_pct: f64,
    pub iops: u64,
    pub throughput_mb: f64,
    pub latency_ms: f64,
    pub data_reduction_ratio: f64,
    pub snapshot_count: u32,
    pub volume_count: u32,
    pub protocol: String,
    pub replication_enabled: bool,
    pub replication_lag_secs: Option<f64>,
    pub last_polled: String,
    pub location: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageVolume {
    pub id: String,
    pub array_id: String,
    pub name: String,
    pub size_tb: f64,
    pub used_tb: f64,
    pub protocol: String,
    pub status: String,
    pub iops: u64,
    pub latency_ms: f64,
    pub snapshot_count: u32,
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageReplication {
    pub id: String,
    pub source_array_id: String,
    pub target_array_id: String,
    pub relationship_type: String,
    pub status: String,
    pub lag_secs: f64,
    pub last_transfer_size_gb: f64,
    pub last_transfer_duration_secs: f64,
    pub transfer_rate_mb: f64,
    pub last_updated: String,
}

/// Fetch storage targets from DB and poll SNMP for live status
async fn fetch_storage_arrays(state: &AppState, org_id: Uuid) -> Vec<StorageArray> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, i32, String, Option<String>)>(
        "SELECT target_id, name, host, port, target_subtype, description \
         FROM targets WHERE org_id = $1 AND target_type = 'storage' AND is_active = true ORDER BY name"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let ssh_host = std::env::var("ZENTRA_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let ssh_user = std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into());
    let now = Utc::now().to_rfc3339();

    // Build a single SSH script that polls ALL arrays in parallel.
    // Each job writes to its own temp file to avoid interleaved output.
    let sq = crate::validation::shell_quote;
    let mut script_parts = vec!["TMPDIR=$(mktemp -d)".to_string()];
    for (i, (_, _, host, port, _, _)) in rows.iter().enumerate() {
        let snmp_target = if *port != 161 { format!("{}:{}", host, port) } else { host.clone() };
        script_parts.push(format!(
            "{{ echo '@@ARRAY_{}@@'; \
               snmpget -v2c -c public -t2 -r0 {} .1.3.6.1.2.1.1.1.0 2>&1; \
               echo '@@EXTENDS_{}@@'; \
               snmpwalk -v2c -c public -t2 -r0 {} NET-SNMP-EXTEND-MIB::nsExtendOutputFull 2>&1; \
               echo '@@END_{}@@'; }} > $TMPDIR/arr_{} 2>&1 &",
            i, sq(&snmp_target), i, sq(&snmp_target), i, i
        ));
    }
    script_parts.push("wait".to_string());
    // Cat all files in order
    for (i, _) in rows.iter().enumerate() {
        script_parts.push(format!("cat $TMPDIR/arr_{}", i));
    }
    script_parts.push("rm -rf $TMPDIR".to_string());
    let full_script = script_parts.join("\n");

    let bulk_res = zentra_core::ssh_pool::pooled_ssh_exec(&ssh_host, &ssh_user, &full_script).await;
    let bulk_out = bulk_res.stdout;

    let mut result = Vec::new();
    for (i, (target_id, name, host, port, subtype, _description)) in rows.iter().enumerate() {
        let array_marker = format!("@@ARRAY_{}@@", i);
        let extends_marker = format!("@@EXTENDS_{}@@", i);
        let end_marker = format!("@@END_{}@@", i);

        // Extract sysDescr section
        let sys_section = bulk_out.split(&array_marker).nth(1)
            .and_then(|s| s.split(&extends_marker).next())
            .unwrap_or("");
        let snmp_ok = sys_section.contains("STRING:");
        let sys_descr = if snmp_ok {
            sys_section.split("STRING:").nth(1).map(|s| s.trim().lines().next().unwrap_or("").to_string())
        } else { None };

        let (vendor, model, firmware, array_type) = parse_storage_info(&sys_descr, &subtype, name);

        // Extract extends section
        let extends_section = bulk_out.split(&extends_marker).nth(1)
            .and_then(|s| s.split(&end_marker).next())
            .unwrap_or("");
        let m = parse_extends_output(extends_section);

        // Determine status
        let status = if !snmp_ok {
            "offline".to_string()
        } else if m.overall_health.to_lowercase().contains("ok") || m.overall_health.to_lowercase().contains("normal") || m.overall_health.to_lowercase().contains("optimal") {
            if m.alert_count > 0 { "warning".to_string() } else { "healthy".to_string() }
        } else if m.overall_health.to_lowercase().contains("degrade") || m.overall_health.to_lowercase().contains("faulted") {
            "degraded".to_string()
        } else if m.overall_health.is_empty() && snmp_ok {
            "healthy".to_string()
        } else {
            "warning".to_string()
        };

        // Update target status in DB
        let new_status = if snmp_ok { "healthy" } else { "unreachable" };
        let _ = sqlx::query("UPDATE targets SET status = $1, last_check_at = NOW() WHERE target_id = $2")
            .bind(new_status).bind(target_id).execute(&state.pool).await;

        // Persist metrics snapshot for historical analysis
        if snmp_ok {
            let raw_json = serde_json::to_value(&m.raw_extends).ok();
            let _ = sqlx::query(
                "INSERT INTO storage_array_metrics \
                 (org_id, target_id, array_name, vendor, model, status, \
                  total_tb, used_tb, free_tb, utilization_pct, \
                  iops, throughput_mb, latency_ms, data_reduction_ratio, \
                  snapshot_count, volume_count, overall_health, alert_count, \
                  replication_enabled, raw_extends) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)"
            )
            .bind(state.org_id).bind(target_id).bind(name).bind(&vendor).bind(&model).bind(&status)
            .bind(m.total_tb).bind(m.used_tb).bind(m.free_tb).bind(m.util_pct)
            .bind(m.iops as i64).bind(m.throughput_mb).bind(m.latency_ms).bind(m.data_reduction_ratio)
            .bind(m.snapshot_count as i32).bind(m.volume_count as i32)
            .bind(&m.overall_health).bind(m.alert_count as i32)
            .bind(m.replication_enabled).bind(&raw_json)
            .execute(&state.pool).await;
        }

        result.push(StorageArray {
            id: target_id.to_string(),
            name: name.clone(),
            array_type,
            vendor,
            model,
            firmware,
            ip: host.clone(),
            status,
            total_tb: m.total_tb, used_tb: m.used_tb, free_tb: m.free_tb,
            utilization_pct: m.util_pct,
            iops: m.iops, throughput_mb: m.throughput_mb, latency_ms: m.latency_ms,
            data_reduction_ratio: m.data_reduction_ratio,
            snapshot_count: m.snapshot_count, volume_count: m.volume_count,
            protocol: detect_protocol(subtype),
            replication_enabled: m.replication_enabled, replication_lag_secs: None,
            last_polled: now.clone(),
            location: String::new(),
            tags: vec!["snmp-monitored".into()],
        });
    }
    result
}

pub async fn list_arrays(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Vec<StorageArray>>, ApiError> {
    Ok(Json(fetch_storage_arrays(&state, org_id).await))
}

pub async fn get_array(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<String>,
) -> Result<Json<StorageArray>, ApiError> {
    let arrays = fetch_storage_arrays(&state, org_id).await;
    arrays.into_iter().find(|a| a.id == id).map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Array not found".into())))
}

pub async fn list_volumes(
    State(_s): State<AppState>,
    Path(_array_id): Path<String>,
) -> Result<Json<Vec<StorageVolume>>, ApiError> {
    Ok(Json(vec![]))
}

pub async fn list_replications(
    State(_s): State<AppState>,
) -> Result<Json<Vec<StorageReplication>>, ApiError> {
    Ok(Json(vec![]))
}

pub async fn storage_summary(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let arrays = fetch_storage_arrays(&state, org_id).await;

    let total = arrays.len();
    let healthy = arrays.iter().filter(|a| a.status == "healthy").count();
    let warning = arrays.iter().filter(|a| a.status == "warning").count();
    let critical = arrays.iter().filter(|a| a.status == "critical").count();
    let total_tb: f64 = arrays.iter().map(|a| a.total_tb).sum();
    let used_tb: f64 = arrays.iter().map(|a| a.used_tb).sum();
    let total_iops: u64 = arrays.iter().map(|a| a.iops).sum();
    let total_throughput: f64 = arrays.iter().map(|a| a.throughput_mb).sum();

    let mut by_type: HashMap<String, usize> = HashMap::new();
    for a in &arrays {
        *by_type.entry(a.array_type.clone()).or_insert(0) += 1;
    }

    Ok(Json(serde_json::json!({
        "total_arrays": total,
        "healthy": healthy,
        "warning": warning,
        "critical": critical,
        "total_tb": total_tb,
        "used_tb": used_tb,
        "utilization_pct": if total_tb > 0.0 { (used_tb / total_tb * 100.0).round() } else { 0.0 },
        "total_iops": total_iops,
        "total_throughput_mb": total_throughput,
        "arrays_by_type": by_type,
        "replication_relationships": 0,
    })))
}

/// Rich metrics result from SNMP extend polling
struct StorageMetrics {
    total_tb: f64,
    used_tb: f64,
    free_tb: f64,
    util_pct: f64,
    iops: u64,
    throughput_mb: f64,
    latency_ms: f64,
    data_reduction_ratio: f64,
    snapshot_count: u32,
    volume_count: u32,
    replication_enabled: bool,
    overall_health: String,
    alert_count: u32,
    raw_extends: HashMap<String, String>,
}

/// Parse pre-fetched NET-SNMP-EXTEND-MIB output into StorageMetrics.
fn parse_extends_output(raw: &str) -> StorageMetrics {
    // Build a key→value map from extend output
    // Lines like: NET-SNMP-EXTEND-MIB::nsExtendOutputFull."totalRawCapacityTB" = STRING: 35.84
    let mut extends: HashMap<String, String> = HashMap::new();
    for line in raw.lines() {
        if let Some(kv) = line.split("nsExtendOutputFull.\"").nth(1) {
            if let Some(key_end) = kv.find('"') {
                let key = kv[..key_end].to_string();
                if let Some(val) = kv.split("STRING:").nth(1) {
                    extends.insert(key, val.trim().to_string());
                }
            }
        }
    }

    // Helper: parse float from extends map
    let get_f = |k: &str| -> f64 {
        extends.get(k).and_then(|v| v.trim_end_matches('%').parse::<f64>().ok()).unwrap_or(0.0)
    };
    let get_u = |k: &str| -> u64 {
        extends.get(k).and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0)
    };
    let get_u32 = |k: &str| -> u32 {
        extends.get(k).and_then(|v| v.trim().parse::<u32>().ok()).unwrap_or(0)
    };
    let get_s = |k: &str| -> String {
        extends.get(k).cloned().unwrap_or_default()
    };

    // Capacity — use totalUsableCapacityTB if available, else totalRawCapacityTB
    let total_tb = {
        let usable = get_f("totalUsableCapacityTB");
        if usable > 0.0 { usable } else {
            let raw = get_f("totalRawCapacityTB");
            if raw > 0.0 { raw } else { get_f("totalCapacityTB") }
        }
    };
    let used_tb = get_f("usedCapacityTB");
    let free_tb = {
        let f = get_f("freeCapacityTB");
        if f > 0.0 { f } else if total_tb > used_tb { total_tb - used_tb } else { 0.0 }
    };
    let util_pct = {
        let p = get_f("usagePct");
        if p > 0.0 { p } else if total_tb > 0.0 { (used_tb / total_tb * 100.0 * 10.0).round() / 10.0 } else { 0.0 }
    };

    // IOPS
    let iops_read = get_u("iopsRead");
    let iops_write = get_u("iopsWrite");
    let iops_other = get_u("iopsOther");
    let iops_mirrored = get_u("iopsMirrored");
    let total_iops = iops_read + iops_write + iops_other + iops_mirrored;

    // Throughput
    let tput_read = get_f("throughputReadMBs");
    let tput_write = get_f("throughputWriteMBs");
    let throughput_mb = tput_read + tput_write;

    // Latency — some arrays use ms, Pure uses microseconds via separate extends
    let lat_read = get_f("latencyReadMs");
    let lat_write = get_f("latencyWriteMs");
    let lat_read_us = get_f("latencyReadUs");
    let lat_write_us = get_f("latencyWriteUs");
    let latency_ms = if lat_read > 0.0 || lat_write > 0.0 {
        if lat_read > 0.0 && lat_write > 0.0 { (lat_read + lat_write) / 2.0 } else { lat_read + lat_write }
    } else if lat_read_us > 0.0 || lat_write_us > 0.0 {
        ((lat_read_us + lat_write_us) / 2.0) / 1000.0 // microseconds → ms
    } else { 0.0 };

    // Data reduction
    let dr_ratio = get_f("dataReductionRatio");
    let data_reduction_ratio = if dr_ratio > 0.0 { dr_ratio } else {
        let dedup = get_f("dedupSavingsPct");
        let comp = get_f("compressionSavingsPct");
        if dedup > 0.0 || comp > 0.0 {
            1.0 / (1.0 - (dedup + comp) / 100.0).max(0.01)
        } else { 0.0 }
    };

    // Counts
    let snapshot_count = get_u32("snapshotCount");
    let volume_count = {
        let v = get_u32("volumeCount");
        if v > 0 { v } else { get_u32("lunCount") }
    };

    // Health
    let overall_health = get_s("overallHealth");
    let alert_count = get_u32("alertCount");

    // Replication
    let repl_status = get_s("replicationStatus");
    let replication_enabled = !repl_status.is_empty()
        && repl_status != "None"
        && repl_status != "Disabled";

    StorageMetrics {
        total_tb: (total_tb * 100.0).round() / 100.0,
        used_tb: (used_tb * 100.0).round() / 100.0,
        free_tb: (free_tb * 100.0).round() / 100.0,
        util_pct,
        iops: total_iops,
        throughput_mb: (throughput_mb * 100.0).round() / 100.0,
        latency_ms: (latency_ms * 1000.0).round() / 1000.0,
        data_reduction_ratio: (data_reduction_ratio * 10.0).round() / 10.0,
        snapshot_count,
        volume_count,
        replication_enabled,
        overall_health,
        alert_count,
        raw_extends: extends,
    }
}

fn parse_storage_info(sys_descr: &Option<String>, subtype: &str, name: &str) -> (String, String, String, String) {
    if let Some(desc) = sys_descr {
        let dl = desc.to_lowercase();
        if dl.contains("dell") && dl.contains("powerstore") {
            return ("Dell EMC".into(), extract_model(desc, "PowerStore"), extract_version(desc), "dellemc_powerstore".into());
        } else if dl.contains("dell") && dl.contains("unity") {
            return ("Dell EMC".into(), extract_model(desc, "Unity"), extract_version(desc), "dellemc".into());
        } else if dl.contains("3par") || dl.contains("hp") {
            return ("HP".into(), extract_model(desc, "3PAR"), extract_version(desc), "hp3par".into());
        } else if dl.contains("netapp") {
            return ("NetApp".into(), extract_model(desc, "FAS"), extract_version(desc), "netapp_ontap".into());
        } else if dl.contains("pure") && dl.contains("flash") {
            return ("Pure Storage".into(), extract_model(desc, "FlashArray"), extract_version(desc), "pure_storage".into());
        }
    }
    match subtype {
        "netapp" => ("NetApp".into(), name.to_string(), String::new(), "netapp_ontap".into()),
        "purestorage" => ("Pure Storage".into(), name.to_string(), String::new(), "pure_storage".into()),
        "hp3par" => ("HP".into(), name.to_string(), String::new(), "hp3par".into()),
        "san" => {
            if name.contains("dell") || name.contains("powerstore") { ("Dell EMC".into(), name.to_string(), String::new(), "dellemc".into()) }
            else { ("SAN".into(), name.to_string(), String::new(), "san".into()) }
        },
        _ => ("Unknown".into(), name.to_string(), String::new(), subtype.to_string()),
    }
}

fn extract_model(desc: &str, keyword: &str) -> String {
    if let Some(pos) = desc.to_lowercase().find(&keyword.to_lowercase()) {
        let chunk = &desc[pos..];
        let end = chunk.find(',').unwrap_or(chunk.len()).min(40);
        chunk[..end].trim().to_string()
    } else {
        keyword.to_string()
    }
}

fn extract_version(desc: &str) -> String {
    for part in desc.split(',') {
        let trimmed = part.trim().to_lowercase();
        if trimmed.contains("version") || trimmed.contains("os ") || trimmed.contains("purity") || trimmed.contains("ontap") {
            return part.trim().to_string();
        }
    }
    String::new()
}

fn detect_protocol(subtype: &str) -> String {
    match subtype {
        "netapp" => "NFS/iSCSI".into(),
        "purestorage" => "FC/NVMe-oF".into(),
        "hp3par" => "FC/iSCSI".into(),
        "san" => "iSCSI/FC".into(),
        _ => "iSCSI".into(),
    }
}

// ============================================================================
// Background Poller — persists storage metrics every 5 minutes
// ============================================================================

/// Start a background task that polls all storage arrays every 5 minutes
/// and writes snapshots to storage_array_metrics for historical analysis.
/// Data persists until the target is removed from monitoring by the user.
pub fn start_storage_poller(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        // Skip the first tick so we don't double-poll on startup
        interval.tick().await;
        tracing::info!("Storage array background poller started (every 5 min)");
        loop {
            interval.tick().await;
            tracing::debug!("Storage poller: running scheduled poll");
            let _ = fetch_storage_arrays(&state, state.org_id).await;
        }
    });
}

// ============================================================================
// Historical Metrics API
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct MetricsHistoryQuery {
    /// ISO 8601 start time (default: 24h ago)
    pub from: Option<String>,
    /// ISO 8601 end time (default: now)
    pub to: Option<String>,
    /// Rollup: "raw", "hourly", "daily" (default: auto based on range)
    pub rollup: Option<String>,
    /// Max rows to return
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct MetricPoint {
    pub timestamp: String,
    pub total_tb: f64,
    pub used_tb: f64,
    pub free_tb: f64,
    pub utilization_pct: f64,
    pub iops: i64,
    pub throughput_mb: f64,
    pub latency_ms: f64,
    pub data_reduction_ratio: f64,
    pub snapshot_count: i32,
    pub volume_count: i32,
    pub overall_health: Option<String>,
    pub alert_count: i32,
}

/// GET /api/storage-arrays/:id/metrics/history
/// Returns historical metric snapshots for a single storage array.
pub async fn get_array_metrics_history(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<String>,
    Query(q): Query<MetricsHistoryQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let target_id: Uuid = id.parse().map_err(|_| ApiError::from((StatusCode::BAD_REQUEST, "Invalid target ID".into())))?;

    let now = Utc::now();
    let from = q.from.as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| now - chrono::Duration::hours(24));
    let to = q.to.as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(now);
    let limit = q.limit.unwrap_or(1000).min(5000);

    // Auto-select rollup based on range
    let range_hours = (to - from).num_hours();
    let rollup = q.rollup.as_deref().unwrap_or(
        if range_hours <= 6 { "raw" }
        else if range_hours <= 72 { "hourly" }
        else { "daily" }
    );

    let rows: Vec<MetricPoint> = match rollup {
        "hourly" => {
            let rows = sqlx::query_as::<_, (chrono::DateTime<Utc>, f64, f64, f64, f64, f64, f64, f64, f64, i64)>(
                "SELECT bucket, avg_total_tb, avg_used_tb, \
                 avg_total_tb - avg_used_tb, avg_utilization_pct, \
                 avg_iops, avg_throughput_mb, avg_latency_ms, avg_data_reduction, \
                 COALESCE(max_alerts, 0) \
                 FROM storage_array_metrics_hourly \
                 WHERE org_id = $1 AND target_id = $2 AND bucket >= $3 AND bucket < $4 \
                 ORDER BY bucket DESC LIMIT $5"
            )
            .bind(org_id).bind(target_id).bind(from).bind(to).bind(limit)
            .fetch_all(&state.pool).await.unwrap_or_default();

            rows.into_iter().map(|r| MetricPoint {
                timestamp: r.0.to_rfc3339(),
                total_tb: r.1, used_tb: r.2, free_tb: r.3, utilization_pct: r.4,
                iops: r.5 as i64, throughput_mb: r.6, latency_ms: r.7,
                data_reduction_ratio: r.8, snapshot_count: 0, volume_count: 0,
                overall_health: None, alert_count: r.9 as i32,
            }).collect()
        }
        "daily" => {
            let rows = sqlx::query_as::<_, (chrono::DateTime<Utc>, f64, f64, f64, f64, f64, f64, f64, f64, i32, i32, i64)>(
                "SELECT bucket, avg_total_tb, avg_used_tb, \
                 avg_total_tb - avg_used_tb, avg_utilization_pct, \
                 avg_iops, avg_throughput_mb, avg_latency_ms, avg_data_reduction, \
                 COALESCE(max_snapshots, 0), COALESCE(max_volumes, 0), COALESCE(max_alerts, 0) \
                 FROM storage_array_metrics_daily \
                 WHERE org_id = $1 AND target_id = $2 AND bucket >= $3 AND bucket < $4 \
                 ORDER BY bucket DESC LIMIT $5"
            )
            .bind(org_id).bind(target_id).bind(from).bind(to).bind(limit)
            .fetch_all(&state.pool).await.unwrap_or_default();

            rows.into_iter().map(|r| MetricPoint {
                timestamp: r.0.to_rfc3339(),
                total_tb: r.1, used_tb: r.2, free_tb: r.3, utilization_pct: r.4,
                iops: r.5 as i64, throughput_mb: r.6, latency_ms: r.7,
                data_reduction_ratio: r.8, snapshot_count: r.9, volume_count: r.10,
                overall_health: None, alert_count: r.11 as i32,
            }).collect()
        }
        _ => { // "raw"
            let rows = sqlx::query_as::<_, (chrono::DateTime<Utc>, f64, f64, f64, f64, i64, f64, f64, f64, i32, i32, Option<String>, i32)>(
                "SELECT timestamp, total_tb, used_tb, free_tb, utilization_pct, \
                 iops, throughput_mb, latency_ms, data_reduction_ratio, \
                 snapshot_count, volume_count, overall_health, alert_count \
                 FROM storage_array_metrics \
                 WHERE org_id = $1 AND target_id = $2 AND timestamp >= $3 AND timestamp < $4 \
                 ORDER BY timestamp DESC LIMIT $5"
            )
            .bind(org_id).bind(target_id).bind(from).bind(to).bind(limit)
            .fetch_all(&state.pool).await.unwrap_or_default();

            rows.into_iter().map(|r| MetricPoint {
                timestamp: r.0.to_rfc3339(),
                total_tb: r.1, used_tb: r.2, free_tb: r.3, utilization_pct: r.4,
                iops: r.5, throughput_mb: r.6, latency_ms: r.7,
                data_reduction_ratio: r.8, snapshot_count: r.9, volume_count: r.10,
                overall_health: r.11, alert_count: r.12,
            }).collect()
        }
    };

    Ok(Json(serde_json::json!({
        "target_id": id,
        "from": from.to_rfc3339(),
        "to": to.to_rfc3339(),
        "rollup": rollup,
        "count": rows.len(),
        "data": rows,
    })))
}

/// GET /api/storage-arrays/:id/metrics/latest
/// Returns the most recent N snapshots from the DB (no live poll).
pub async fn get_array_metrics_latest(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let target_id: Uuid = id.parse().map_err(|_| ApiError::from((StatusCode::BAD_REQUEST, "Invalid target ID".into())))?;

    let rows = sqlx::query_as::<_, (chrono::DateTime<Utc>, f64, f64, f64, f64, i64, f64, f64, f64, i32, i32, Option<String>, i32, Option<serde_json::Value>)>(
        "SELECT timestamp, total_tb, used_tb, free_tb, utilization_pct, \
         iops, throughput_mb, latency_ms, data_reduction_ratio, \
         snapshot_count, volume_count, overall_health, alert_count, raw_extends \
         FROM storage_array_metrics \
         WHERE org_id = $1 AND target_id = $2 \
         ORDER BY timestamp DESC LIMIT 1"
    )
    .bind(org_id).bind(target_id)
    .fetch_all(&state.pool).await.unwrap_or_default();

    if let Some(r) = rows.into_iter().next() {
        Ok(Json(serde_json::json!({
            "target_id": id,
            "timestamp": r.0.to_rfc3339(),
            "total_tb": r.1, "used_tb": r.2, "free_tb": r.3, "utilization_pct": r.4,
            "iops": r.5, "throughput_mb": r.6, "latency_ms": r.7,
            "data_reduction_ratio": r.8, "snapshot_count": r.9, "volume_count": r.10,
            "overall_health": r.11, "alert_count": r.12, "raw_extends": r.13,
        })))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "No metrics found for this array".into())))
    }
}
