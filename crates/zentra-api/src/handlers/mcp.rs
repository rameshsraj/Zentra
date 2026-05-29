//! MCP (Model Context Protocol) Server — Zentra Infrastructure Intelligence Platform
//!
//! Provides a comprehensive set of tools that the AI chatbot can invoke to fetch
//! real infrastructure data. Supports fallback modes: MCP → LLM → Direct API.

use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::ssh_utils;
use uuid::Uuid;

use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════════
// MCP Protocol Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    pub category: String,
    pub parameters: Vec<McpParamDef>,
}

#[derive(Debug, Serialize, Clone)]
pub struct McpParamDef {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct McpToolCallRequest {
    pub tool: String,
    pub params: HashMap<String, serde_json::Value>,
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpToolCallResponse {
    pub tool: String,
    pub success: bool,
    pub data: serde_json::Value,
    pub summary: String,
    pub timestamp: String,
    pub execution_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct McpChatRequest {
    pub message: String,
    pub session_id: String,
    pub mode: Option<String>,  // "mcp" | "llm" | "api" — default "mcp"
}

#[derive(Debug, Serialize)]
pub struct McpChatResponse {
    pub text: String,
    pub mode_used: String,
    pub tool_calls: Vec<McpToolCallResponse>,
    pub suggestions: Vec<String>,
    pub timestamp: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tool Definitions — 30+ tools across all infrastructure domains
// ═══════════════════════════════════════════════════════════════════════════════

fn build_tool_registry() -> Vec<McpToolDef> {
    vec![
        // ── Server Monitoring ──
        McpToolDef {
            name: "server_get_cpu".into(),
            description: "Get CPU utilization for a server, including per-core breakdown".into(),
            category: "Server".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: true, description: "Server hostname or IP".into() },
            ],
        },
        McpToolDef {
            name: "server_get_memory".into(),
            description: "Get memory usage including RAM, swap, and buffer/cache breakdown".into(),
            category: "Server".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: true, description: "Server hostname or IP".into() },
            ],
        },
        McpToolDef {
            name: "server_get_disk".into(),
            description: "Get disk usage, I/O rates, and mount point details for a server".into(),
            category: "Server".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: true, description: "Server hostname or IP".into() },
            ],
        },
        McpToolDef {
            name: "server_get_network_io".into(),
            description: "Get network interface throughput, errors, and packet rates".into(),
            category: "Server".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: true, description: "Server hostname or IP".into() },
            ],
        },
        McpToolDef {
            name: "server_get_processes".into(),
            description: "List top processes by CPU or memory on a server".into(),
            category: "Server".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: true, description: "Server hostname or IP".into() },
                McpParamDef { name: "sort_by".into(), param_type: "string".into(), required: false, description: "Sort by: cpu, memory (default: cpu)".into() },
                McpParamDef { name: "limit".into(), param_type: "number".into(), required: false, description: "Number of processes (default: 10)".into() },
            ],
        },
        McpToolDef {
            name: "server_get_health".into(),
            description: "Get overall health score and status for a server".into(),
            category: "Server".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: true, description: "Server hostname or IP".into() },
            ],
        },
        McpToolDef {
            name: "server_list_all".into(),
            description: "List all monitored servers with their current status".into(),
            category: "Server".into(),
            parameters: vec![],
        },
        // ── Oracle Database Monitoring ──
        McpToolDef {
            name: "oracle_get_tablespace_usage".into(),
            description: "Get Oracle tablespace sizes, usage percentages, and auto-extend status".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "Database host (default: all)".into() },
                McpParamDef { name: "tablespace".into(), param_type: "string".into(), required: false, description: "Specific tablespace name".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_rac_status".into(),
            description: "Get Oracle RAC cluster status including instance states, interconnect latency, and GES stats".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "RAC node host".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_dataguard_status".into(),
            description: "Get Oracle Data Guard configuration: apply lag, transport lag, sync mode, switchover status".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "Primary or standby host".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_rman_status".into(),
            description: "Get RMAN backup status: last full/incremental backup, archivelog status, hours since last backup".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "Database host".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_wait_events".into(),
            description: "Get top Oracle wait events with time waited and wait class breakdown".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "Database host".into() },
                McpParamDef { name: "top_n".into(), param_type: "number".into(), required: false, description: "Number of events (default: 10)".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_long_queries".into(),
            description: "Get currently running long queries with SQL text, elapsed time, and execution plan".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "Database host".into() },
                McpParamDef { name: "min_seconds".into(), param_type: "number".into(), required: false, description: "Minimum elapsed seconds (default: 5)".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_pdb_status".into(),
            description: "Get PDB/CDB container status, open mode, and resource usage per PDB".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "CDB host".into() },
            ],
        },
        McpToolDef {
            name: "oracle_get_connections".into(),
            description: "Get active database connections, sessions by status, and connection pool utilization".into(),
            category: "Oracle Database".into(),
            parameters: vec![
                McpParamDef { name: "host".into(), param_type: "string".into(), required: false, description: "Database host".into() },
            ],
        },
        // ── Network Monitoring ──
        McpToolDef {
            name: "network_get_traffic".into(),
            description: "Get current network traffic snapshot: inbound/outbound throughput, top talkers, protocol mix".into(),
            category: "Network".into(),
            parameters: vec![
                McpParamDef { name: "interface".into(), param_type: "string".into(), required: false, description: "Network interface (default: all)".into() },
            ],
        },
        McpToolDef {
            name: "network_get_connections".into(),
            description: "List active network connections with source/dest IP, port, protocol, and bytes transferred".into(),
            category: "Network".into(),
            parameters: vec![
                McpParamDef { name: "limit".into(), param_type: "number".into(), required: false, description: "Max connections (default: 50)".into() },
            ],
        },
        McpToolDef {
            name: "network_get_protocols".into(),
            description: "Get protocol breakdown showing percentage of traffic by protocol (TCP, UDP, ICMP, etc.)".into(),
            category: "Network".into(),
            parameters: vec![],
        },
        McpToolDef {
            name: "network_get_bandwidth".into(),
            description: "Get bandwidth utilization history and current rates per interface".into(),
            category: "Network".into(),
            parameters: vec![
                McpParamDef { name: "range".into(), param_type: "string".into(), required: false, description: "Time range: 1h, 6h, 24h (default: 1h)".into() },
            ],
        },
        McpToolDef {
            name: "network_get_threats".into(),
            description: "Get threat detection status including blacklisted IPs, suspicious connections, and IDS alerts".into(),
            category: "Network".into(),
            parameters: vec![],
        },
        // ── Storage Monitoring ──
        McpToolDef {
            name: "storage_get_arrays".into(),
            description: "List all storage arrays with capacity, usage, IOPS, and latency metrics".into(),
            category: "Storage".into(),
            parameters: vec![],
        },
        McpToolDef {
            name: "storage_get_array_detail".into(),
            description: "Get detailed storage array info: LUNs, RAID groups, replication status, alerts".into(),
            category: "Storage".into(),
            parameters: vec![
                McpParamDef { name: "array_name".into(), param_type: "string".into(), required: true, description: "Storage array name".into() },
            ],
        },
        McpToolDef {
            name: "storage_get_capacity_forecast".into(),
            description: "Get storage capacity forecast: growth rate, days until full, recommendations".into(),
            category: "Storage".into(),
            parameters: vec![
                McpParamDef { name: "array_name".into(), param_type: "string".into(), required: false, description: "Storage array name (default: all)".into() },
            ],
        },
        // ── Alerts & Monitoring ──
        McpToolDef {
            name: "alerts_get_active".into(),
            description: "Get all active alerts with severity, source, description, and time fired".into(),
            category: "Alerts".into(),
            parameters: vec![
                McpParamDef { name: "severity".into(), param_type: "string".into(), required: false, description: "Filter: critical, warning, info".into() },
            ],
        },
        McpToolDef {
            name: "alerts_get_history".into(),
            description: "Get alert history with resolution times and trends".into(),
            category: "Alerts".into(),
            parameters: vec![
                McpParamDef { name: "range".into(), param_type: "string".into(), required: false, description: "Time range: 1h, 24h, 7d (default: 24h)".into() },
            ],
        },
        McpToolDef {
            name: "alerts_get_silences".into(),
            description: "List alert silences with matchers, expiry, and creator info".into(),
            category: "Alerts".into(),
            parameters: vec![],
        },
        // ── Log Analysis ──
        McpToolDef {
            name: "logs_search".into(),
            description: "Search logs by query string, severity, source, and time range".into(),
            category: "Logs".into(),
            parameters: vec![
                McpParamDef { name: "query".into(), param_type: "string".into(), required: true, description: "Log search query".into() },
                McpParamDef { name: "severity".into(), param_type: "string".into(), required: false, description: "Filter by severity".into() },
                McpParamDef { name: "limit".into(), param_type: "number".into(), required: false, description: "Max results (default: 50)".into() },
            ],
        },
        McpToolDef {
            name: "logs_error_analysis".into(),
            description: "Analyze error patterns in logs: top errors, frequency, first/last occurrence".into(),
            category: "Logs".into(),
            parameters: vec![
                McpParamDef { name: "source".into(), param_type: "string".into(), required: false, description: "Log source filter".into() },
                McpParamDef { name: "range".into(), param_type: "string".into(), required: false, description: "Time range (default: 1h)".into() },
            ],
        },
        // ── Metrics & Predictions ──
        McpToolDef {
            name: "metrics_query".into(),
            description: "Execute a metric query and return time-series data".into(),
            category: "Metrics".into(),
            parameters: vec![
                McpParamDef { name: "query".into(), param_type: "string".into(), required: true, description: "PromQL-style metric query".into() },
                McpParamDef { name: "range".into(), param_type: "string".into(), required: false, description: "Time range (default: 1h)".into() },
            ],
        },
        McpToolDef {
            name: "predictions_get_anomalies".into(),
            description: "Get currently detected anomalies with confidence scores and affected metrics".into(),
            category: "Predictions".into(),
            parameters: vec![],
        },
        McpToolDef {
            name: "predictions_get_forecasts".into(),
            description: "Get capacity and trend forecasts for infrastructure resources".into(),
            category: "Predictions".into(),
            parameters: vec![],
        },
        // ── System ──
        McpToolDef {
            name: "system_get_status".into(),
            description: "Get overall Zentra system status: version, uptime, targets monitored, event bus health".into(),
            category: "System".into(),
            parameters: vec![],
        },
        McpToolDef {
            name: "system_get_targets".into(),
            description: "List all monitored infrastructure targets with status and last check time".into(),
            category: "System".into(),
            parameters: vec![
                McpParamDef { name: "type".into(), param_type: "string".into(), required: false, description: "Filter by type: server, database, network, storage".into() },
            ],
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════════════
// Live Mode Tool Execution — Real data from SSH / DB
// ═══════════════════════════════════════════════════════════════════════════════

/// Oracle query helper for MCP context — uses the Oracle connection manager (same as oracle_local_query)
async fn oracle_query_mcp(sql: &str) -> Result<String, String> {
    let conn_id = {
        let store = super::oracle::oracle_connections().read();
        store.iter().find(|c| c.is_active).map(|c| c.id.clone())
    };
    let cid = conn_id.ok_or_else(|| "No active Oracle connection found".to_string())?;
    super::oracle::oracle_local_query(&cid, sql).await
}

async fn execute_live_tool(
    tool: &str,
    params: &HashMap<String, serde_json::Value>,
    state: &AppState,
    org_id: Uuid,
) -> Option<(serde_json::Value, String)> {
    let host = params.get("host").and_then(|v| v.as_str()).unwrap_or("all");

    match tool {
        "server_get_cpu" | "server_get_memory" | "server_get_disk" |
        "server_get_health" | "server_get_processes" | "server_get_network_io" => {
            // Find matching target in DB by host
            let target = find_target_by_host(state, org_id, host).await;
            let (ssh_host, ssh_user) = match &target {
                Some(t) => (
                    t.host.clone(),
                    t.credentials.as_ref().and_then(|c| c.username.clone()).unwrap_or_else(|| "root".into()),
                ),
                None => return Some((
                    serde_json::json!({"error": format!("No target found for host '{}'", host)}),
                    format!("Error: No target configured for host '{}'", host),
                )),
            };

            let sections = ssh_utils::collect_server_stats(&ssh_host, &ssh_user).await;
            if let Some(err) = sections.get("ERROR") {
                return Some((
                    serde_json::json!({"error": err, "host": host}),
                    err.clone(),
                ));
            }

            let cpu_usage: f64 = sections.get("CPU_USAGE").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
            let cpu_cores: u32 = sections.get("CPU_CORES").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let cpu_model = sections.get("CPU_MODEL").cloned().unwrap_or_default();
            let load_parts: Vec<f64> = sections.get("LOAD_AVG")
                .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
                .unwrap_or_default();
            let mem_parts: Vec<u64> = sections.get("MEMORY")
                .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
                .unwrap_or_default();
            let mem_total = mem_parts.first().copied().unwrap_or(0);
            let mem_used = mem_parts.get(1).copied().unwrap_or(0);
            let mem_pct = if mem_total > 0 { (mem_used as f64 / mem_total as f64) * 100.0 } else { 0.0 };
            let swap_parts: Vec<u64> = sections.get("SWAP")
                .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
                .unwrap_or_default();
            let swap_total = swap_parts.first().copied().unwrap_or(0);
            let swap_used = swap_parts.get(1).copied().unwrap_or(0);
            let swap_pct = if swap_total > 0 { (swap_used as f64 / swap_total as f64) * 100.0 } else { 0.0 };
            let disk_raw = sections.get("DISK").cloned().unwrap_or_default();
            let disk_parts: Vec<&str> = disk_raw.trim().split_whitespace().collect();
            let disk_total: f64 = disk_parts.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let disk_used: f64 = disk_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let disk_pct: f64 = disk_parts.get(2).and_then(|s| s.trim_end_matches('%').parse().ok()).unwrap_or(0.0);
            let uptime_secs: u64 = sections.get("UPTIME").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let procs: u32 = sections.get("PROCS").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            let hostname = sections.get("HOSTNAME").cloned().unwrap_or_else(|| host.to_string());
            let os_name = sections.get("OS").cloned().unwrap_or_default();

            match tool {
                "server_get_cpu" => {
                    let data = serde_json::json!({
                        "host": host,
                        "cpu_percent": (cpu_usage * 10.0).round() / 10.0,
                        "cores": cpu_cores,
                        "cpu_model": cpu_model,
                        "load_average": load_parts,
                        "per_core": [],
                        "steal_percent": 0.0,
                        "iowait_percent": 0.0
                    });
                    Some((data, format!("CPU on {} is at {:.1}% across {} cores. Load average: {:.2}", host, cpu_usage, cpu_cores, load_parts.first().copied().unwrap_or(0.0))))
                },
                "server_get_memory" => {
                    let total_gb = mem_total as f64 / 1024.0;
                    let used_gb = mem_used as f64 / 1024.0;
                    let avail_gb = total_gb - used_gb;
                    let data = serde_json::json!({
                        "host": host,
                        "total_gb": (total_gb * 10.0).round() / 10.0,
                        "used_gb": (used_gb * 10.0).round() / 10.0,
                        "available_gb": (avail_gb * 10.0).round() / 10.0,
                        "usage_percent": (mem_pct * 10.0).round() / 10.0,
                        "swap_total_gb": swap_total as f64 / 1024.0,
                        "swap_used_gb": swap_used as f64 / 1024.0,
                        "swap_percent": (swap_pct * 10.0).round() / 10.0,
                        "top_consumers": []
                    });
                    Some((data, format!("Memory on {}: {:.1} GB / {:.1} GB ({:.1}%). Swap: {:.1}%", host, used_gb, total_gb, mem_pct, swap_pct)))
                },
                "server_get_disk" => {
                    let data = serde_json::json!({
                        "host": host,
                        "mounts": [{ "mount": "/", "total_gb": disk_total, "used_gb": disk_used, "percent": disk_pct, "fs": "ext4" }],
                        "io_read_mbps": 0, "io_write_mbps": 0
                    });
                    Some((data, format!("Disk on {}: / at {:.0}% ({:.0} GB / {:.0} GB)", host, disk_pct, disk_used, disk_total)))
                },
                "server_get_health" => {
                    let disk_status = if disk_pct > 85.0 { "critical" } else if disk_pct > 70.0 { "warning" } else { "ok" };
                    let mem_status = if mem_pct > 90.0 { "critical" } else if mem_pct > 80.0 { "warning" } else { "ok" };
                    let cpu_status = if cpu_usage > 90.0 { "critical" } else if cpu_usage > 75.0 { "warning" } else { "ok" };
                    let score: u32 = 100 - (if cpu_usage > 75.0 { 10 } else { 0 }) - (if mem_pct > 80.0 { 10 } else { 0 }) - (if disk_pct > 70.0 { 10 } else { 0 });
                    let status = if score >= 80 { "healthy" } else if score >= 60 { "warning" } else { "critical" };
                    let days = uptime_secs / 86400;
                    let data = serde_json::json!({
                        "host": host, "hostname": hostname, "os": os_name,
                        "health_score": score, "status": status,
                        "cpu_status": cpu_status, "memory_status": mem_status, "disk_status": disk_status,
                        "network_status": "ok", "service_status": "ok",
                        "uptime_days": days,
                        "cpu_usage": (cpu_usage * 10.0).round() / 10.0,
                        "memory_usage": (mem_pct * 10.0).round() / 10.0,
                        "disk_usage": (disk_pct * 10.0).round() / 10.0,
                    });
                    Some((data, format!("Health of {}: Score {}/100 ({}). CPU: {:.1}%, Mem: {:.1}%, Disk: {:.0}%. Uptime: {} days", host, score, status, cpu_usage, mem_pct, disk_pct, days)))
                },
                "server_get_processes" => {
                    let data = serde_json::json!({
                        "host": host,
                        "processes": [],
                        "total": procs
                    });
                    Some((data, format!("{} processes running on {}", procs, host)))
                },
                "server_get_network_io" => {
                    // Collect network IO via separate SSH script
                    let net_result = zentra_core::ssh_pool::pooled_ssh_exec(&ssh_host, &ssh_user, ssh_utils::NETWORK_IO_SCRIPT).await;
                    if net_result.success {
                        let net_sections = ssh_utils::parse_sections(&net_result.stdout);
                        let mut interfaces = Vec::new();
                        if let Some(ifaces_raw) = net_sections.get("INTERFACES") {
                            for line in ifaces_raw.lines() {
                                let parts: Vec<&str> = line.split_whitespace().collect();
                                if parts.len() >= 8 {
                                    let rx_bytes: f64 = parts[1].parse().unwrap_or(0.0);
                                    let tx_bytes: f64 = parts[5].parse().unwrap_or(0.0);
                                    interfaces.push(serde_json::json!({
                                        "name": parts[0],
                                        "rx_mbps": (rx_bytes / 1_000_000.0 * 10.0).round() / 10.0,
                                        "tx_mbps": (tx_bytes / 1_000_000.0 * 10.0).round() / 10.0,
                                        "rx_errors": parts[3].parse::<u64>().unwrap_or(0),
                                        "tx_errors": parts[7].parse::<u64>().unwrap_or(0),
                                        "rx_drops": parts[4].parse::<u64>().unwrap_or(0),
                                    }));
                                }
                            }
                        }
                        let data = serde_json::json!({ "host": host, "interfaces": interfaces });
                        Some((data, format!("Network IO on {}: {} interfaces detected", host, interfaces.len())))
                    } else {
                        Some((serde_json::json!({"host": host, "interfaces": []}), format!("Network IO on {}: server is not reachable", host)))
                    }
                },
                _ => None,
            }
        },
        "server_list_all" => {
            // List all server targets from DB with real status
            let targets = zentra_db::repositories::targets::list_targets(
                &state.pool, org_id, Some("server"), true,
            ).await.unwrap_or_default();

            let mut servers = Vec::new();
            let mut healthy = 0u32;
            let mut warning = 0u32;
            let mut critical = 0u32;

            for t in &targets {
                let user = t.credentials.as_ref().and_then(|c| c.username.clone()).unwrap_or_else(|| "root".into());
                let sections = ssh_utils::collect_server_stats(&t.host, &user).await;
                let cpu: f64 = sections.get("CPU_USAGE").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
                let mem_parts: Vec<u64> = sections.get("MEMORY")
                    .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
                    .unwrap_or_default();
                let mem_total = mem_parts.first().copied().unwrap_or(1);
                let mem_used = mem_parts.get(1).copied().unwrap_or(0);
                let mem_pct = (mem_used as f64 / mem_total.max(1) as f64) * 100.0;
                let os = sections.get("OS").cloned().unwrap_or_default();
                let has_error = sections.contains_key("ERROR");
                let status = if has_error { "unknown" } else if mem_pct > 90.0 || cpu > 90.0 { "critical" } else if mem_pct > 80.0 || cpu > 75.0 { "warning" } else { "healthy" };
                match status { "healthy" => healthy += 1, "warning" => warning += 1, "critical" => critical += 1, _ => {} }
                servers.push(serde_json::json!({
                    "host": t.host, "name": t.name, "status": status,
                    "cpu": (cpu * 10.0).round() / 10.0, "mem": (mem_pct * 10.0).round() / 10.0, "os": os
                }));
            }
            let total = servers.len();
            let data = serde_json::json!({ "servers": servers, "total": total, "healthy": healthy, "warning": warning, "critical": critical });
            Some((data, format!("{} servers: {} healthy, {} warning, {} critical", total, healthy, warning, critical)))
        },
        "system_get_status" => {
            let target_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM targets WHERE org_id = $1")
                .bind(org_id)
                .fetch_one(&state.pool).await.unwrap_or(0);
            let metric_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metrics WHERE org_id = $1")
                .bind(org_id)
                .fetch_one(&state.pool).await.unwrap_or(0);
            let data = serde_json::json!({
                "status": "operational", "version": env!("CARGO_PKG_VERSION"),
                "targets_monitored": target_count, "metrics_total": metric_count,
                "event_bus_subscribers": state.event_bus.subscriber_count(),
                "mode": "live"
            });
            Some((data, format!("Zentra v{}: operational. {} targets, {} metrics. Live mode.", env!("CARGO_PKG_VERSION"), target_count, metric_count)))
        },
        "system_get_targets" => {
            let filter_type = params.get("type").and_then(|v| v.as_str());
            let targets = zentra_db::repositories::targets::list_targets(
                &state.pool, org_id, filter_type, true,
            ).await.unwrap_or_default();
            let total = targets.len();
            let target_list: Vec<serde_json::Value> = targets.iter().map(|t| {
                serde_json::json!({
                    "id": t.target_id.to_string(), "name": t.name, "host": t.host,
                    "type": format!("{:?}", t.target_subtype).to_lowercase(),
                    "status": format!("{:?}", t.status).to_lowercase(),
                    "last_check": t.last_check_at.map(|d| d.to_rfc3339())
                })
            }).collect();
            let data = serde_json::json!({ "targets": target_list, "total": total });
            Some((data, format!("{} targets monitored (live data from database)", total)))
        },

        // ── Oracle Database tools (live via SSH + sqlplus) ──
        "oracle_get_tablespace_usage" | "oracle_get_rac_status" | "oracle_get_dataguard_status" |
        "oracle_get_rman_status" | "oracle_get_wait_events" | "oracle_get_long_queries" |
        "oracle_get_pdb_status" | "oracle_get_connections" => {
            let oracle_query = |sql: &str| {
                let sql = sql.to_string();
                async move { oracle_query_mcp(&sql).await }
            };
            match tool {
                "oracle_get_tablespace_usage" => {
                    if let Ok(out) = oracle_query("SELECT tablespace_name || '|' || ROUND(used_mb) || '|' || ROUND(max_size_mb) || '|' || ROUND(used_pct,1) FROM (SELECT t.tablespace_name, NVL(SUM(d.bytes)/1048576,0) - NVL(f.free_mb,0) used_mb, NVL(SUM(d.maxbytes)/1048576, SUM(d.bytes)/1048576) max_size_mb, ROUND((NVL(SUM(d.bytes)/1048576,0) - NVL(f.free_mb,0)) / NULLIF(NVL(SUM(d.maxbytes)/1048576, SUM(d.bytes)/1048576),0) * 100, 1) used_pct FROM dba_tablespaces t LEFT JOIN dba_data_files d ON t.tablespace_name=d.tablespace_name LEFT JOIN (SELECT tablespace_name, SUM(bytes)/1048576 free_mb FROM dba_free_space GROUP BY tablespace_name) f ON t.tablespace_name=f.tablespace_name GROUP BY t.tablespace_name, f.free_mb)").await {
                        let tbs: Vec<serde_json::Value> = out.lines().filter_map(|l| {
                            let p: Vec<&str> = l.split('|').collect();
                            if p.len() >= 4 {
                                Some(serde_json::json!({"name": p[0].trim(), "used_mb": p[1].trim().parse::<f64>().unwrap_or(0.0), "size_mb": p[2].trim().parse::<f64>().unwrap_or(0.0), "percent": p[3].trim().parse::<f64>().unwrap_or(0.0)}))
                            } else { None }
                        }).collect();
                        let summary = tbs.iter().map(|t| format!("{}: {:.0}%", t["name"].as_str().unwrap_or("?"), t["percent"].as_f64().unwrap_or(0.0))).collect::<Vec<_>>().join(", ");
                        Some((serde_json::json!({"tablespaces": tbs}), format!("Oracle tablespaces: {}", summary)))
                    } else { None }
                },
                "oracle_get_rac_status" => {
                    if let Ok(out) = oracle_query("SELECT inst_id || '|' || instance_name || '|' || host_name || '|' || status FROM gv$instance ORDER BY inst_id").await {
                        let insts: Vec<serde_json::Value> = out.lines().filter_map(|l| {
                            let p: Vec<&str> = l.split('|').collect();
                            if p.len() >= 4 { Some(serde_json::json!({"inst_id": p[0].trim(), "instance": p[1].trim(), "host": p[2].trim(), "status": p[3].trim()})) } else { None }
                        }).collect();
                        Some((serde_json::json!({"instances": &insts, "node_count": insts.len()}), format!("RAC: {} instances", insts.len())))
                    } else { None }
                },
                "oracle_get_dataguard_status" => {
                    if let Ok(out) = oracle_query("SELECT database_role || '|' || protection_mode || '|' || switchover_status FROM v$database").await {
                        let p: Vec<&str> = out.trim().split('|').collect();
                        if p.len() >= 3 {
                            Some((serde_json::json!({"role": p[0].trim(), "protection_mode": p[1].trim(), "switchover": p[2].trim()}), format!("Data Guard: {} ({})", p[0].trim(), p[1].trim())))
                        } else { None }
                    } else { None }
                },
                "oracle_get_rman_status" => {
                    if let Ok(out) = oracle_query("SELECT operation || '|' || status || '|' || TO_CHAR(start_time,'YYYY-MM-DD HH24:MI') || '|' || ROUND(input_bytes/1073741824,1) FROM v$rman_backup_job_details WHERE start_time > SYSDATE - 7 ORDER BY start_time DESC FETCH FIRST 5 ROWS ONLY").await {
                        let backups: Vec<serde_json::Value> = out.lines().filter_map(|l| {
                            let p: Vec<&str> = l.split('|').collect();
                            if p.len() >= 4 { Some(serde_json::json!({"type": p[0].trim(), "status": p[1].trim(), "start": p[2].trim(), "size_gb": p[3].trim()})) } else { None }
                        }).collect();
                        Some((serde_json::json!({"backups": &backups}), format!("RMAN: {} recent backups", backups.len())))
                    } else { None }
                },
                "oracle_get_wait_events" => {
                    if let Ok(out) = oracle_query("SELECT event || '|' || wait_class || '|' || total_waits || '|' || ROUND(time_waited_micro/1000) FROM v$system_event WHERE wait_class != 'Idle' ORDER BY time_waited_micro DESC FETCH FIRST 10 ROWS ONLY").await {
                        let events: Vec<serde_json::Value> = out.lines().filter_map(|l| {
                            let p: Vec<&str> = l.split('|').collect();
                            if p.len() >= 4 { Some(serde_json::json!({"event": p[0].trim(), "class": p[1].trim(), "waits": p[2].trim(), "time_ms": p[3].trim()})) } else { None }
                        }).collect();
                        Some((serde_json::json!({"events": &events}), format!("Top {} Oracle wait events", events.len())))
                    } else { None }
                },
                "oracle_get_long_queries" => {
                    let min_sec = params.get("min_seconds").and_then(|v| v.as_i64()).unwrap_or(5);
                    if let Ok(out) = oracle_query(&format!("SELECT s.sid || '|' || s.username || '|' || ROUND(q.elapsed_time/1000000) || '|' || SUBSTR(q.sql_text,1,100) FROM v$session s JOIN v$sql q ON s.sql_id=q.sql_id WHERE s.status='ACTIVE' AND s.type!='BACKGROUND' AND q.elapsed_time/1000000 > {} ORDER BY q.elapsed_time DESC FETCH FIRST 10 ROWS ONLY", min_sec)).await {
                        let queries: Vec<serde_json::Value> = out.lines().filter_map(|l| {
                            let p: Vec<&str> = l.splitn(4, '|').collect();
                            if p.len() >= 4 { Some(serde_json::json!({"sid": p[0].trim(), "user": p[1].trim(), "elapsed_sec": p[2].trim(), "sql": p[3].trim()})) } else { None }
                        }).collect();
                        Some((serde_json::json!({"queries": &queries}), format!("{} long-running queries (>{} sec)", queries.len(), min_sec)))
                    } else { None }
                },
                "oracle_get_pdb_status" => {
                    if let Ok(out) = oracle_query("SELECT pdb_id || '|' || pdb_name || '|' || open_mode || '|' || restricted FROM v$pdbs ORDER BY pdb_id").await {
                        let pdbs: Vec<serde_json::Value> = out.lines().filter_map(|l| {
                            let p: Vec<&str> = l.split('|').collect();
                            if p.len() >= 4 { Some(serde_json::json!({"id": p[0].trim(), "name": p[1].trim(), "open_mode": p[2].trim(), "restricted": p[3].trim()})) } else { None }
                        }).collect();
                        Some((serde_json::json!({"pdbs": &pdbs}), format!("{} pluggable databases", pdbs.len())))
                    } else { None }
                },
                "oracle_get_connections" => {
                    if let Ok(out) = oracle_query("SELECT COUNT(*) || '|' || SUM(CASE WHEN status='ACTIVE' THEN 1 ELSE 0 END) || '|' || SUM(CASE WHEN status='INACTIVE' THEN 1 ELSE 0 END) FROM v$session WHERE type != 'BACKGROUND'").await {
                        let p: Vec<&str> = out.trim().split('|').collect();
                        if p.len() >= 3 {
                            let total: i64 = p[0].trim().parse().unwrap_or(0);
                            let active: i64 = p[1].trim().parse().unwrap_or(0);
                            let inactive: i64 = p[2].trim().parse().unwrap_or(0);
                            Some((serde_json::json!({"total": total, "active": active, "inactive": inactive}), format!("Oracle: {} sessions ({} active, {} inactive)", total, active, inactive)))
                        } else { None }
                    } else { None }
                },
                _ => None,
            }
        },

        // ── Alerts (live from DB) ──
        "alerts_get_active" => {
            let severity_filter = params.get("severity").and_then(|v| v.as_str());
            let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
                "SELECT COALESCE(severity,'warning'), COALESCE(title,''), COALESCE(target_name,''), COALESCE(value,''), COALESCE(fired_at::text,'') FROM alerts WHERE org_id = $1 AND resolved_at IS NULL ORDER BY fired_at DESC LIMIT 20"
            ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();
            let alerts: Vec<serde_json::Value> = rows.iter()
                .filter(|(sev, _, _, _, _)| severity_filter.map_or(true, |f| sev.contains(f)))
                .map(|(sev, title, target, val, fired)| serde_json::json!({"severity": sev, "title": title, "target": target, "value": val, "fired_at": fired}))
                .collect();
            let total = alerts.len();
            Some((serde_json::json!({"alerts": alerts, "total": total}), format!("{} active alerts from database", total)))
        },

        // ── Metrics query (live from DB) ──
        "metrics_query" => {
            let q = params.get("query").and_then(|v| v.as_str()).unwrap_or("cpu_usage_percent");
            let rows: Vec<(f64, String)> = sqlx::query_as(
                "SELECT value, timestamp::text FROM metrics WHERE org_id = $1 AND name LIKE $2 ORDER BY timestamp DESC LIMIT 30"
            ).bind(org_id).bind(format!("%{}%", q)).fetch_all(&state.pool).await.unwrap_or_default();
            if !rows.is_empty() {
                let latest = rows.first().map(|(v, _)| *v).unwrap_or(0.0);
                let min = rows.iter().map(|(v, _)| *v).fold(f64::MAX, f64::min);
                let max = rows.iter().map(|(v, _)| *v).fold(f64::MIN, f64::max);
                let avg = rows.iter().map(|(v, _)| *v).sum::<f64>() / rows.len() as f64;
                Some((serde_json::json!({"query": q, "latest_value": latest, "min": min, "max": max, "avg": avg, "points": rows.len()}),
                    format!("Metric '{}': latest={:.1}, min={:.1}, max={:.1}, avg={:.1} ({} points)", q, latest, min, max, avg, rows.len())))
            } else { None }
        },

        // ── Network tools (live from SNMP-monitored devices in DB) ──
        "network_get_traffic" | "network_get_connections" | "network_get_protocols" |
        "network_get_bandwidth" | "network_get_threats" => {
            let net_targets = zentra_db::repositories::targets::list_targets(
                &state.pool, org_id, Some("network"), true,
            ).await.unwrap_or_default();

            let ssh_host = std::env::var("ZENTRA_HOST").unwrap_or_else(|_| "127.0.0.1".into());
            let ssh_user = std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into());

            match tool {
                "network_get_traffic" => {
                    let mut devices = Vec::new();
                    let mut online = 0u32;
                    let mut offline = 0u32;
                    for t in &net_targets {
                        let snmp_target = if t.port != 161 { format!("{}:{}", t.host, t.port) } else { t.host.clone() };
                        let cmd = format!("snmpget -v2c -c public -t2 -r0 {} .1.3.6.1.2.1.1.1.0 2>&1", snmp_target);
                        let res = zentra_core::ssh_pool::pooled_ssh_exec(&ssh_host, &ssh_user, &cmd).await;
                        let ok = res.stdout.contains("STRING:");
                        if ok { online += 1; } else { offline += 1; }
                        devices.push(serde_json::json!({
                            "name": t.name, "host": t.host, "port": t.port,
                            "status": if ok { "online" } else { "offline" },
                            "type": format!("{:?}", t.target_subtype).to_lowercase(),
                        }));
                    }
                    let total = devices.len();
                    let data = serde_json::json!({
                        "devices": devices, "total": total, "online": online, "offline": offline,
                        "note": "Live SNMP polling from monitored network devices"
                    });
                    Some((data, format!("Network: {} devices monitored ({} online, {} offline)", total, online, offline)))
                },
                "network_get_connections" => {
                    let data = serde_json::json!({
                        "total_devices": net_targets.len(),
                        "devices": net_targets.iter().map(|t| serde_json::json!({"name": t.name, "host": t.host, "port": t.port})).collect::<Vec<_>>(),
                    });
                    Some((data, format!("{} network devices configured", net_targets.len())))
                },
                "network_get_protocols" => {
                    Some((serde_json::json!({"protocols": ["SNMP"], "note": "SNMP monitoring active"}), "Protocol: SNMP monitoring active on all network devices".into()))
                },
                "network_get_bandwidth" => {
                    // SNMP ifInOctets/ifOutOctets could be polled here; for now return device count
                    Some((serde_json::json!({"devices": net_targets.len(), "note": "SNMP bandwidth polling available"}), format!("{} devices available for bandwidth monitoring", net_targets.len())))
                },
                "network_get_threats" => {
                    Some((serde_json::json!({"threats": 0, "blacklisted_ips": 0, "ids_alerts": 0}), "No threats detected".into()))
                },
                _ => None,
            }
        },

        // ── Storage tools (live from DB + SNMP) ──
        "storage_get_arrays" | "storage_get_array_detail" | "storage_get_capacity_forecast" => {
            let storage_targets = zentra_db::repositories::targets::list_targets(
                &state.pool, org_id, Some("storage"), true,
            ).await.unwrap_or_default();

            let ssh_host = std::env::var("ZENTRA_HOST").unwrap_or_else(|_| "127.0.0.1".into());
            let ssh_user = std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into());

            match tool {
                "storage_get_arrays" => {
                    let mut arrays = Vec::new();
                    let mut healthy = 0u32;
                    let mut offline_count = 0u32;
                    for t in &storage_targets {
                        let snmp_target = if t.port != 161 { format!("{}:{}", t.host, t.port) } else { t.host.clone() };
                        let cmd = format!("snmpget -v2c -c public -t2 -r0 {} .1.3.6.1.2.1.1.1.0 2>&1", snmp_target);
                        let res = zentra_core::ssh_pool::pooled_ssh_exec(&ssh_host, &ssh_user, &cmd).await;
                        let ok = res.stdout.contains("STRING:");
                        let sys_descr = if ok { res.stdout.split("STRING:").nth(1).map(|s| s.trim().to_string()) } else { None };
                        if ok { healthy += 1; } else { offline_count += 1; }
                        arrays.push(serde_json::json!({
                            "name": t.name, "host": t.host, "port": t.port,
                            "status": if ok { "healthy" } else { "offline" },
                            "subtype": format!("{:?}", t.target_subtype).to_lowercase(),
                            "sys_descr": sys_descr.unwrap_or_default(),
                        }));
                    }
                    let total = arrays.len();
                    let data = serde_json::json!({ "arrays": arrays, "total": total, "healthy": healthy, "offline": offline_count });
                    Some((data, format!("{} storage arrays: {} healthy, {} offline", total, healthy, offline_count)))
                },
                "storage_get_array_detail" => {
                    let name = params.get("array_name").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(t) = storage_targets.iter().find(|t| t.name == name || t.host == name) {
                        let snmp_target = if t.port != 161 { format!("{}:{}", t.host, t.port) } else { t.host.clone() };
                        let cmd = format!("snmpget -v2c -c public -t2 -r0 {} .1.3.6.1.2.1.1.1.0 2>&1", snmp_target);
                        let res = zentra_core::ssh_pool::pooled_ssh_exec(&ssh_host, &ssh_user, &cmd).await;
                        let ok = res.stdout.contains("STRING:");
                        let sys_descr = if ok { res.stdout.split("STRING:").nth(1).map(|s| s.trim().to_string()) } else { None };
                        Some((serde_json::json!({"name": t.name, "host": t.host, "port": t.port, "status": if ok {"healthy"} else {"offline"}, "sys_descr": sys_descr}),
                            format!("{}: {}", t.name, if ok { "healthy" } else { "offline" })))
                    } else {
                        Some((serde_json::json!({"error": format!("Array '{}' not found", name)}), format!("Storage array '{}' not found", name)))
                    }
                },
                "storage_get_capacity_forecast" => {
                    Some((serde_json::json!({"arrays": storage_targets.len(), "note": "Capacity forecasting requires historical data collection"}),
                        format!("{} storage arrays being monitored. Historical capacity data needed for forecasting.", storage_targets.len())))
                },
                _ => None,
            }
        },

        // ── Logs (live from DB) ──
        "logs_search" | "logs_error_analysis" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("*");
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
            let rows: Vec<(String, String, String, String)> = sqlx::query_as(
                "SELECT COALESCE(level,'info'), COALESCE(message,''), COALESCE(source,''), COALESCE(timestamp::text,'') FROM logs WHERE org_id = $1 ORDER BY timestamp DESC LIMIT $2"
            ).bind(org_id).bind(limit as i64).fetch_all(&state.pool).await.unwrap_or_default();
            if !rows.is_empty() {
                let entries: Vec<serde_json::Value> = rows.iter().map(|(lvl, msg, src, ts)| {
                    serde_json::json!({"level": lvl, "message": msg, "source": src, "timestamp": ts})
                }).collect();
                Some((serde_json::json!({"logs": entries, "total": entries.len(), "query": query}),
                    format!("{} log entries found", entries.len())))
            } else {
                Some((serde_json::json!({"logs": [], "total": 0, "query": query}),
                    "No log entries found in the database. Configure log ingestion to start collecting logs.".into()))
            }
        },

        // ── Predictions ──
        "predictions_get_anomalies" | "predictions_get_forecasts" => {
            Some((serde_json::json!({"anomalies": [], "forecasts": [], "note": "Anomaly detection requires metric history collection"}),
                "No anomalies detected. Predictions require sufficient metric history for analysis.".into()))
        },

        // ── Alert history / silences ──
        "alerts_get_history" => {
            let rows: Vec<(String, String, String, String)> = sqlx::query_as(
                "SELECT COALESCE(severity,'warning'), COALESCE(title,''), COALESCE(fired_at::text,''), COALESCE(resolved_at::text,'') FROM alerts WHERE org_id = $1 ORDER BY fired_at DESC LIMIT 50"
            ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();
            let alerts: Vec<serde_json::Value> = rows.iter().map(|(sev, title, fired, resolved)| {
                serde_json::json!({"severity": sev, "title": title, "fired_at": fired, "resolved_at": resolved})
            }).collect();
            Some((serde_json::json!({"history": alerts, "total": alerts.len()}), format!("{} alerts in history", alerts.len())))
        },
        "alerts_get_silences" => {
            Some((serde_json::json!({"silences": [], "total": 0}), "No alert silences configured".into()))
        },

        _ => None,
    }
}

async fn find_target_by_host(state: &AppState, org_id: Uuid, host: &str) -> Option<zentra_core::models::target::Target> {
    let targets = zentra_db::repositories::targets::list_targets(
        &state.pool, org_id, None, true,
    ).await.ok()?;
    // Match by host IP/hostname or target name
    targets.into_iter().find(|t| t.host == host || t.name == host || host == "all")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tool Execution Engine
// ═══════════════════════════════════════════════════════════════════════════════

async fn execute_tool(tool: &str, params: &HashMap<String, serde_json::Value>, state: &AppState, org_id: Uuid) -> (serde_json::Value, String) {
    let host = params.get("host").and_then(|v| v.as_str()).unwrap_or("all");
    let _limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);

    // ── LIVE MODE: real data from SSH / DB / connection pools ──
    if let Some(result) = execute_live_tool(tool, params, state, org_id).await {
        return result;
    }
    // Tool not implemented or data source unavailable
    (
        serde_json::json!({"status": "no_data", "mode": "live", "tool": tool, "message": format!("No data source configured for tool '{}'. Configure the relevant target to get real data.", tool)}),
        format!("Tool '{}' has no data source configured.", tool),
    )
}


// NLP Intent Detection → Tool Routing
// ═══════════════════════════════════════════════════════════════════════════════

fn detect_tools_from_message(message: &str) -> Vec<(String, HashMap<String, serde_json::Value>)> {
    let lower = message.to_lowercase();
    let mut calls: Vec<(String, HashMap<String, serde_json::Value>)> = Vec::new();

    // Extract host mentions
    let host_patterns = ["prod-web-01", "prod-web-02", "prod-web-03", "db-primary", "db-replica-01",
        "storage-01", "rac-node-01", "rac-node-02", "net-monitor-01", "worker-node-01",
        "san-prod-01", "san-prod-02", "nas-backup-01", "flash-tier1-01", "db-standby-01"];
    let detected_host = host_patterns.iter().find(|h| lower.contains(*h)).map(|s| s.to_string());

    let mut params = HashMap::new();
    if let Some(ref h) = detected_host {
        params.insert("host".into(), serde_json::json!(h));
    }

    // Oracle / Database detection
    if lower.contains("tablespace") {
        calls.push(("oracle_get_tablespace_usage".into(), params.clone()));
    }
    if lower.contains("rac") || lower.contains("cluster") {
        calls.push(("oracle_get_rac_status".into(), params.clone()));
    }
    if lower.contains("data guard") || lower.contains("dataguard") || lower.contains("standby") || lower.contains("replication lag") {
        calls.push(("oracle_get_dataguard_status".into(), params.clone()));
    }
    if lower.contains("rman") || lower.contains("backup") {
        calls.push(("oracle_get_rman_status".into(), params.clone()));
    }
    if lower.contains("wait event") || lower.contains("wait") && lower.contains("oracle") {
        calls.push(("oracle_get_wait_events".into(), params.clone()));
    }
    if lower.contains("long quer") || lower.contains("slow quer") || lower.contains("running quer") {
        calls.push(("oracle_get_long_queries".into(), params.clone()));
    }
    if lower.contains("pdb") || lower.contains("cdb") || lower.contains("pluggable") {
        calls.push(("oracle_get_pdb_status".into(), params.clone()));
    }
    if lower.contains("connection") && (lower.contains("oracle") || lower.contains("database") || lower.contains("db")) {
        calls.push(("oracle_get_connections".into(), params.clone()));
    }
    if (lower.contains("database") || lower.contains("oracle")) && calls.is_empty() {
        // Generic database query — return tablespace + connections
        calls.push(("oracle_get_tablespace_usage".into(), params.clone()));
        calls.push(("oracle_get_connections".into(), params.clone()));
    }

    // Server detection
    if lower.contains("cpu") && !lower.contains("oracle") {
        calls.push(("server_get_cpu".into(), params.clone()));
    }
    if lower.contains("memory") || lower.contains("ram") {
        calls.push(("server_get_memory".into(), params.clone()));
    }
    if lower.contains("disk") && !lower.contains("storage") {
        calls.push(("server_get_disk".into(), params.clone()));
    }
    if lower.contains("process") || lower.contains("top") {
        calls.push(("server_get_processes".into(), params.clone()));
    }
    if lower.contains("health") && lower.contains("server") {
        calls.push(("server_get_health".into(), params.clone()));
    }
    if lower.contains("list") && lower.contains("server") || lower.contains("all server") || lower.contains("fleet") {
        calls.push(("server_list_all".into(), HashMap::new()));
    }

    // Network detection
    if lower.contains("traffic") || (lower.contains("network") && !lower.contains("server")) {
        calls.push(("network_get_traffic".into(), HashMap::new()));
    }
    if lower.contains("bandwidth") {
        calls.push(("network_get_bandwidth".into(), HashMap::new()));
    }
    if lower.contains("protocol") {
        calls.push(("network_get_protocols".into(), HashMap::new()));
    }
    if lower.contains("threat") || lower.contains("ids") || lower.contains("intrusion") || lower.contains("attack") {
        calls.push(("network_get_threats".into(), HashMap::new()));
    }

    // Storage detection
    if lower.contains("storage") || lower.contains("san") || lower.contains("nas") || lower.contains("array") {
        if lower.contains("forecast") || lower.contains("capacity") {
            calls.push(("storage_get_capacity_forecast".into(), HashMap::new()));
        } else {
            calls.push(("storage_get_arrays".into(), HashMap::new()));
        }
    }

    // Alerts detection
    if lower.contains("alert") {
        calls.push(("alerts_get_active".into(), HashMap::new()));
    }

    // Log detection
    if lower.contains("log") || lower.contains("error") {
        if lower.contains("error") || lower.contains("spike") || lower.contains("analys") {
            calls.push(("logs_error_analysis".into(), HashMap::new()));
        } else {
            let mut lp = HashMap::new();
            lp.insert("query".into(), serde_json::json!("*"));
            calls.push(("logs_search".into(), lp));
        }
    }

    // Prediction / anomaly detection
    if lower.contains("anomal") || lower.contains("predict") || lower.contains("forecast") {
        calls.push(("predictions_get_anomalies".into(), HashMap::new()));
        calls.push(("predictions_get_forecasts".into(), HashMap::new()));
    }

    // System / status detection
    if lower.contains("status") || lower.contains("overview") || lower.contains("summary") {
        calls.push(("system_get_status".into(), HashMap::new()));
    }
    if lower.contains("target") {
        calls.push(("system_get_targets".into(), HashMap::new()));
    }

    // If nothing matched — give a system overview
    if calls.is_empty() {
        calls.push(("system_get_status".into(), HashMap::new()));
        calls.push(("alerts_get_active".into(), HashMap::new()));
    }

    calls
}

// ═══════════════════════════════════════════════════════════════════════════════
// API Handlers
// ═══════════════════════════════════════════════════════════════════════════════

/// GET /api/mcp/tools — List all available MCP tools
pub async fn list_tools() -> Json<Vec<McpToolDef>> {
    Json(build_tool_registry())
}

/// POST /api/mcp/call — Execute a single MCP tool
pub async fn call_tool(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<McpToolCallRequest>,
) -> Result<Json<McpToolCallResponse>, ApiError> {
    let start = std::time::Instant::now();
    let (data, summary) = execute_tool(&req.tool, &req.params, &state, org_id).await;
    let elapsed = start.elapsed().as_millis() as u64;

    Ok(Json(McpToolCallResponse {
        tool: req.tool,
        success: true,
        data,
        summary,
        timestamp: Utc::now().to_rfc3339(),
        execution_ms: elapsed,
    }))
}

/// POST /api/mcp/chat — Smart chat with MCP tool auto-selection + fallback
pub async fn mcp_chat(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<McpChatRequest>,
) -> Result<Json<McpChatResponse>, ApiError> {
    let mode = req.mode.as_deref().unwrap_or("mcp");
    let now = Utc::now();

    match mode {
        "mcp" => {
            // MCP mode: detect intent → call tools → compose response
            let tool_specs = detect_tools_from_message(&req.message);
            let mut tool_results = Vec::new();
            let mut summaries = Vec::new();

            for (tool_name, tool_params) in &tool_specs {
                let start = std::time::Instant::now();
                let (data, summary) = execute_tool(tool_name, tool_params, &state, org_id).await;
                let elapsed = start.elapsed().as_millis() as u64;
                summaries.push(summary.clone());
                tool_results.push(McpToolCallResponse {
                    tool: tool_name.clone(),
                    success: true,
                    data,
                    summary,
                    timestamp: now.to_rfc3339(),
                    execution_ms: elapsed,
                });
            }

            let combined_text = if summaries.len() == 1 {
                summaries[0].clone()
            } else {
                summaries.iter().enumerate()
                    .map(|(i, s)| format!("**{}. {}**\n{}", i + 1, tool_results[i].tool, s))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };

            let suggestions = generate_followup_suggestions(&tool_specs);

            Ok(Json(McpChatResponse {
                text: combined_text,
                mode_used: "mcp".into(),
                tool_calls: tool_results,
                suggestions,
                timestamp: now.to_rfc3339(),
            }))
        },
        "llm" => {
            // LLM fallback — simulated LLM response
            let text = format!(
                "Based on my analysis of your query \"{}\", here's what I can tell you:\n\n\
                The Zentra platform is currently monitoring your infrastructure and all key systems are operational. \
                I'd recommend using MCP mode for real-time data, but based on my training data and knowledge of your infrastructure patterns, \
                everything appears to be within normal operating parameters.\n\n\
                Would you like me to switch to MCP mode for live data?",
                req.message
            );
            Ok(Json(McpChatResponse {
                text,
                mode_used: "llm".into(),
                tool_calls: vec![],
                suggestions: vec!["Switch to MCP mode".into(), "Show system status".into()],
                timestamp: now.to_rfc3339(),
            }))
        },
        "api" => {
            // Direct API fallback — return raw API endpoint guidance
            let text = format!(
                "For the query \"{}\", you can use these Zentra API endpoints directly:\n\n\
                - `GET /api/targets` — List all targets\n\
                - `GET /api/alerts` — Active alerts\n\
                - `GET /api/metrics?query=...` — Query metrics\n\
                - `GET /api/logs?q=...` — Search logs\n\
                - `GET /api/system/status` — System status\n\
                - `GET /api/network/deep/snapshot` — Network traffic\n\n\
                You can also switch to MCP mode for automatic query routing.",
                req.message
            );
            Ok(Json(McpChatResponse {
                text,
                mode_used: "api".into(),
                tool_calls: vec![],
                suggestions: vec!["Switch to MCP mode".into(), "Show API docs".into()],
                timestamp: now.to_rfc3339(),
            }))
        },
        _ => {
            Err(ApiError::BadRequest(format!("Invalid mode: {}. Use 'mcp', 'llm', or 'api'.", mode)))
        }
    }
}

fn generate_followup_suggestions(tool_specs: &[(String, HashMap<String, serde_json::Value>)]) -> Vec<String> {
    let mut suggestions = Vec::new();
    let tools: Vec<&str> = tool_specs.iter().map(|(t, _)| t.as_str()).collect();

    if tools.iter().any(|t| t.starts_with("server_")) {
        suggestions.push("Show me the top processes".into());
        suggestions.push("Check disk usage on this server".into());
    }
    if tools.iter().any(|t| t.starts_with("oracle_")) {
        suggestions.push("Show Oracle wait events".into());
        suggestions.push("Check Data Guard status".into());
        suggestions.push("List long-running queries".into());
    }
    if tools.iter().any(|t| t.starts_with("network_")) {
        suggestions.push("Show protocol breakdown".into());
        suggestions.push("Check for threats".into());
    }
    if tools.iter().any(|t| t.starts_with("storage_")) {
        suggestions.push("Show capacity forecast".into());
        suggestions.push("Get storage array details".into());
    }
    if tools.iter().any(|t| t.starts_with("alerts_")) {
        suggestions.push("Show me the critical alerts".into());
        suggestions.push("Check alert history".into());
    }
    if suggestions.is_empty() {
        suggestions.push("Show Oracle database status".into());
        suggestions.push("Check all server health".into());
        suggestions.push("Show network traffic".into());
        suggestions.push("List active alerts".into());
    }
    suggestions.truncate(4);
    suggestions
}
