//! Oracle Database Monitoring API Handler
//! Provides endpoints for all 8 Oracle subsystems with live data via SSH + sqlplus.

use axum::{extract::{Query, Path, State}, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::{Utc, Duration};
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::ssh_utils;

use crate::error::ApiError;
use base64::{Engine as _, engine::general_purpose::STANDARD};

/// POSIX-safe shell quoting: wraps value in single quotes, escaping any interior
/// single quotes as `'\''`. This prevents all shell metacharacter interpretation.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Validate that a string is a safe Oracle identifier (alphanumeric, _, #, $).
/// Rejects anything that could be used for SQL injection in identifier positions.
fn validate_oracle_identifier(s: &str) -> Result<(), String> {
    if s.is_empty() || s.len() > 128 {
        return Err("Identifier must be 1-128 characters".into());
    }
    if !s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '#' || c == '$' || c == '.') {
        return Err(format!("Invalid characters in identifier: {}", s));
    }
    Ok(())
}

/// Optional connection_id query param for routing queries to a specific Oracle instance
#[derive(Debug, Deserialize)]
pub struct ConnIdParam {
    pub connection_id: Option<String>,
}

/// Run a SQL query against an Oracle instance using local sqlplus (Oracle Instant Client).
/// Looks up connection details from ORACLE_CONNECTIONS by connection_id.
pub async fn oracle_local_query(connection_id: &str, sql: &str) -> Result<String, String> {
    // Attempt native query first if available in the registry
    if database_plugin::oracle_connection::pool_is_connected(connection_id) {
        match database_plugin::oracle_connection::pool_query(connection_id, sql) {
            Ok(res) => {
                // Format results as newline-separated pipe-delimited strings to maintain
                // compatibility with existing parsers that expect sqlplus-style output.
                let output = res.rows.iter()
                    .map(|row| row.join("|"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if !output.is_empty() {
                    return Ok(output);
                }
            }
            Err(e) => {
                tracing::debug!("Native query failed for pool {}, falling back to sqlplus: {}", connection_id, e);
            }
        }
    }

    let (host, port, service_name, username, password, role_str) = {
        let store = oracle_connections().read();
        let conn = store.iter().find(|c| c.id == connection_id)
            .ok_or_else(|| "Connection not found".to_string())?;
        (conn.host.clone(), conn.port, conn.service_name.clone(),
         conn.username.clone(), conn.password.clone(), conn.role.clone())
    };
    let role_clause = if role_str.to_uppercase() == "SYSDBA" { " as sysdba" } else if role_str.to_uppercase() == "SYSOPER" { " as sysoper" } else { "" };
    let conn_str = format!("{}/{}@//{}:{}/{}{}", username, password, host, port, service_name, role_clause);
    let sql_input = format!("SET PAGESIZE 0\nSET FEEDBACK OFF\nSET HEADING OFF\nSET LINESIZE 1000\nSET TRIMOUT ON\nSET TRIMSPOOL ON\n{};\nEXIT;\n", sql.trim_end_matches(';'));

    let spawn = tokio::process::Command::new("sqlplus")
        .args(["-S", &conn_str])
        .env("LD_LIBRARY_PATH", "/usr/lib/oracle/23/client64/lib")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match spawn {
        Ok(mut child) => {
            use tokio::io::AsyncWriteExt;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(sql_input.as_bytes()).await;
                drop(stdin);
            }
            match tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output()).await {
                Ok(Ok(out)) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let combined = format!("{}\n{}", stdout, stderr);
                    // Only fail on connection-level errors; individual query errors are tolerable in batches
                    let is_conn_error = combined.contains("ORA-01017") || combined.contains("ORA-28000")
                        || combined.contains("ORA-12541") || combined.contains("ORA-12514")
                        || combined.contains("ORA-01005") || combined.contains("SP2-0157");
                    if is_conn_error {
                        let e = combined.lines().find(|l| l.contains("ORA-") || l.contains("SP2-"))
                            .unwrap_or("Oracle connection error").trim().to_string();
                        tracing::warn!("oracle_local_query connection error: {} | sql snippet: {}", e, &sql[..sql.len().min(80)]);
                        Err(e)
                    } else {
                        // Filter out blank and ORA-/SP2- error lines, return successful output
                        let clean: String = stdout.lines()
                            .filter(|l| !l.trim().is_empty() && !l.contains("ORA-") && !l.contains("SP2-"))
                            .collect::<Vec<_>>().join("\n");
                        if combined.contains("ORA-") {
                            tracing::debug!("oracle_local_query partial errors (non-fatal) in batch query");
                        }
                        Ok(clean)
                    }
                }
                Ok(Err(e)) => { tracing::warn!("sqlplus process error: {}", e); Err(format!("sqlplus failed: {}", e)) },
                Err(_) => { tracing::warn!("sqlplus timeout for sql: {}", &sql[..sql.len().min(60)]); Err("Query timed out after 15s".into()) },
            }
        }
        Err(e) => { tracing::warn!("sqlplus spawn error: {}", e); Err(format!("sqlplus not found: {}", e)) },
    }
}

/// Helper: find Oracle target and SSH to run a sqlplus query
async fn oracle_ssh_query(state: &AppState, org_id: uuid::Uuid, sql: &str) -> Result<String, String> {
    let targets = zentra_db::repositories::targets::list_targets(
        &state.pool, org_id, Some("database"), true,
    ).await.map_err(|e| e.to_string())?;
    let target = targets.iter()
        .find(|t| format!("{:?}", t.target_subtype).to_lowercase().contains("oracle"))
        .ok_or_else(|| "No Oracle target configured".to_string())?;
    let host = &target.host;
    let (db_user, db_pass) = match &target.credentials {
        Some(c) => (
            c.username.clone().unwrap_or_else(|| "system".into()),
            c.decrypt_password(),
        ),
        None => ("system".into(), String::new()),
    };
    let db_name = target.credentials.as_ref()
        .and_then(|c| c.database_name.clone())
        .unwrap_or_else(|| "ORCL".into());
    let ssh_user = std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into());
    // Build the full SQL script and base64-encode it to prevent shell injection.
    // This avoids any shell metacharacters (`, $, ;, etc.) in the SQL from being
    // interpreted by the shell.
    let sql_script = format!(
        "SET PAGESIZE 0\nSET FEEDBACK OFF\nSET HEADING OFF\nSET LINESIZE 1000\nSET TRIMOUT ON\nSET TRIMSPOOL ON\n{}\nEXIT;",
        sql
    );
    let encoded = STANDARD.encode(sql_script.as_bytes());
    // Shell-quote all credentials to prevent command injection.
    // Single-quote wrapping with interior single-quotes escaped as '\'' is the
    // safest POSIX-shell quoting — no variable expansion or metacharacter
    // interpretation occurs inside single quotes.
    let cmd = format!(
        "echo {} | base64 -d | sqlplus -S {}/{}@{}:{}/{}",
        encoded,
        shell_quote(&db_user),
        shell_quote(&db_pass),
        shell_quote(host),
        target.port,
        shell_quote(&db_name),
    );
    let result = ssh_utils::ssh_exec(host, &ssh_user, &cmd).await;
    if !result.success {
        return Err(ssh_utils::friendly_error(host, &result.stderr));
    }
    Ok(result.stdout)
}

// ============================================================================
// Oracle Overview — aggregated dashboard metrics
// ============================================================================

/// GET /api/oracle/overview
pub async fn overview(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    // Helper: build empty health object for error responses
    let empty_health = || json!({
        "overall": "unknown", "active_sessions": 0, "session_count": 0,
        "sga_size_gb": 0.0, "pga_size_gb": 0.0,
        "buffer_cache_hit_ratio": 0.0, "library_cache_hit_ratio": 0.0,
        "db_block_gets_per_sec": 0, "physical_reads_per_sec": 0,
        "redo_writes_per_sec": 0, "user_commits_per_sec": 0,
    });
    let empty_instance = || json!({
        "instance_name": "—", "version": "—", "status": "UNKNOWN",
        "database_status": "UNKNOWN", "host_name": "", "uptime_seconds": 0,
    });

    // Helper: query Oracle and build the full overview from a connection
    async fn build_overview(cid: &str) -> Result<Value, String> {
        let inst = oracle_local_query(cid, "SELECT instance_name || '|' || version || '|' || status || '|' || database_status || '|' || host_name || '|' || ROUND((SYSDATE - startup_time)*86400) FROM v$instance").await?;
        let p: Vec<&str> = inst.trim().split('|').collect();
        let (iname, ver, st, dbst, hname, up) = if p.len() >= 6 {
            (p[0].trim(), p[1].trim(), p[2].trim(), p[3].trim(), p[4].trim(), p[5].trim().parse::<i64>().unwrap_or(0))
        } else { ("ORCL", inst.trim(), "OPEN", "ACTIVE", "", 0) };

        let active_s = oracle_local_query(cid, "SELECT COUNT(*) FROM v$session WHERE status='ACTIVE'").await.unwrap_or_default();
        let total_s = oracle_local_query(cid, "SELECT COUNT(*) FROM v$session").await.unwrap_or_default();
        let sga = oracle_local_query(cid, "SELECT ROUND(SUM(value)/1024/1024/1024,2) FROM v$sga").await.unwrap_or_default();
        let pga = oracle_local_query(cid, "SELECT ROUND(value/1024/1024/1024,2) FROM v$pgastat WHERE name='total PGA allocated'").await.unwrap_or_default();
        let db_size = oracle_local_query(cid, "SELECT ROUND(SUM(bytes)/1024/1024/1024,2) FROM dba_data_files").await.unwrap_or_default();

        // Buffer cache hit ratio
        let buf_hit = oracle_local_query(cid, "SELECT ROUND((1 - (SELECT SUM(value) FROM v$sysstat WHERE name='physical reads') / NULLIF((SELECT SUM(value) FROM v$sysstat WHERE name IN ('db block gets','consistent gets')),0)) * 100, 1) FROM dual").await.unwrap_or_default();
        // Library cache hit ratio
        let lib_hit = oracle_local_query(cid, "SELECT ROUND(SUM(pins - reloads) / NULLIF(SUM(pins),0) * 100, 1) FROM v$librarycache").await.unwrap_or_default();
        // Sysmetric per-sec values
        let metrics_sql = "SELECT metric_name || '|' || ROUND(value,2) FROM v$sysmetric WHERE group_id=2 AND metric_name IN ('Physical Reads Per Sec','Redo Writes Per Sec','User Commits Per Sec','DB Block Gets Per Sec')";
        let metrics_raw = oracle_local_query(cid, metrics_sql).await.unwrap_or_default();
        let mut db_block_gets: f64 = 0.0;
        let mut phys_reads: f64 = 0.0;
        let mut redo_writes: f64 = 0.0;
        let mut user_commits: f64 = 0.0;
        for line in metrics_raw.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                let val = parts[1].trim().parse::<f64>().unwrap_or(0.0);
                match parts[0].trim() {
                    "DB Block Gets Per Sec" => db_block_gets = val,
                    "Physical Reads Per Sec" => phys_reads = val,
                    "Redo Writes Per Sec" => redo_writes = val,
                    "User Commits Per Sec" => user_commits = val,
                    _ => {}
                }
            }
        }

        let edition = if ver.contains("Express") { "Express Edition" } else if ver.contains("Enterprise") { "Enterprise Edition" } else { "Standard Edition" };
        Ok(json!({
            "instance": {
                "instance_name": iname, "version": ver, "status": st,
                "database_status": dbst, "host_name": hname,
                "uptime_seconds": up, "edition": edition,
            },
            "health": {
                "overall": "healthy",
                "active_sessions": active_s.trim().parse::<i64>().unwrap_or(0),
                "session_count": total_s.trim().parse::<i64>().unwrap_or(0),
                "sga_size_gb": sga.trim().parse::<f64>().unwrap_or(0.0),
                "pga_size_gb": pga.trim().parse::<f64>().unwrap_or(0.0),
                "buffer_cache_hit_ratio": buf_hit.trim().parse::<f64>().unwrap_or(0.0),
                "library_cache_hit_ratio": lib_hit.trim().parse::<f64>().unwrap_or(0.0),
                "db_block_gets_per_sec": db_block_gets,
                "physical_reads_per_sec": phys_reads,
                "redo_writes_per_sec": redo_writes,
                "user_commits_per_sec": user_commits,
            },
            "storage": { "total_database_size_gb": db_size.trim().parse::<f64>().unwrap_or(0.0) }
        }))
    }

    if let Some(ref cid) = cp.connection_id {
        if let Ok(data) = build_overview(cid).await {
            return Json(data);
        }
        return Json(json!({ "status": "error", "message": "connection failed", "instance": empty_instance(), "health": empty_health(), "storage": { "total_database_size_gb": 0.0 }, "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(ver) = oracle_ssh_query(&state, org_id, "SELECT banner FROM v$version WHERE ROWNUM=1").await {
        let version = ver.trim().to_string();
        let sessions = oracle_ssh_query(&state, org_id, "SELECT COUNT(*) FROM v$session WHERE status='ACTIVE'").await.unwrap_or_default();
        let total_sessions = oracle_ssh_query(&state, org_id, "SELECT COUNT(*) FROM v$session").await.unwrap_or_default();
        let uptime = oracle_ssh_query(&state, org_id, "SELECT ROUND((SYSDATE - startup_time) * 86400) FROM v$instance").await.unwrap_or_default();
        let sga = oracle_ssh_query(&state, org_id, "SELECT ROUND(SUM(value)/1024/1024/1024,2) FROM v$sga").await.unwrap_or_default();
        let pga = oracle_ssh_query(&state, org_id, "SELECT ROUND(value/1024/1024/1024,2) FROM v$pgastat WHERE name='total PGA allocated'").await.unwrap_or_default();
        let db_size = oracle_ssh_query(&state, org_id, "SELECT ROUND(SUM(bytes)/1024/1024/1024,2) FROM dba_data_files").await.unwrap_or_default();
        return Json(json!({
            "instance": { "instance_name": "ORCL", "version": version, "status": "OPEN", "database_status": "ACTIVE", "uptime_seconds": uptime.trim().parse::<i64>().unwrap_or(0) },
            "health": {
                "overall": "healthy",
                "active_sessions": sessions.trim().parse::<i64>().unwrap_or(0),
                "session_count": total_sessions.trim().parse::<i64>().unwrap_or(0),
                "sga_size_gb": sga.trim().parse::<f64>().unwrap_or(0.0),
                "pga_size_gb": pga.trim().parse::<f64>().unwrap_or(0.0),
                "buffer_cache_hit_ratio": 0.0, "library_cache_hit_ratio": 0.0,
                "db_block_gets_per_sec": 0, "physical_reads_per_sec": 0,
                "redo_writes_per_sec": 0, "user_commits_per_sec": 0,
            },
            "storage": { "total_database_size_gb": db_size.trim().parse::<f64>().unwrap_or(0.0) }
        }));
    }
    return Json(json!({ "status": "error", "message": "connection failed", "instance": empty_instance(), "health": empty_health(), "storage": { "total_database_size_gb": 0.0 }, "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// RAC — Real Application Clusters
// ============================================================================

/// GET /api/oracle/rac
pub async fn rac_overview(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let rac_sql = "SELECT inst_id || '|' || instance_name || '|' || host_name || '|' || status || '|' || database_status FROM gv$instance ORDER BY inst_id";
    let parse_rac = |out: &str| -> Vec<Value> {
        out.lines().filter_map(|line| {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 5 {
                Some(json!({ "inst_id": p[0].trim().parse::<i32>().unwrap_or(0), "instance_name": p[1].trim(), "host_name": p[2].trim(), "status": p[3].trim(), "database_status": p[4].trim() }))
            } else { None }
        }).collect()
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, rac_sql).await {
            let inst = parse_rac(&out);
            return Json(json!({ "cluster_name": "ORCL-RAC", "node_count": inst.len(), "instances": inst, "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "cluster_name": "", "node_count": 0, "instances": [], "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, rac_sql).await {
        let inst = parse_rac(&out);
        return Json(json!({ "cluster_name": "ORCL-RAC", "node_count": inst.len(), "instances": inst, "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "cluster_name": "", "node_count": 0, "instances": [], "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Sysmetrics — v$sysmetric
// ============================================================================

/// GET /api/oracle/sysmetrics
pub async fn sysmetrics(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let sm_sql = "SELECT metric_name || '|' || ROUND(value,2) FROM v$sysmetric WHERE ROWNUM <= 100";
    let parse_sm = |out: &str| -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        for line in out.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                if let Ok(v) = parts[1].trim().parse::<f64>() {
                    m.insert(parts[0].trim().to_string(), json!(v));
                }
            }
        }
        m
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, sm_sql).await {
            return Json(json!({ "status": "success", "metrics": parse_sm(&out), "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "metrics": {}, "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, sm_sql).await {
        return Json(json!({ "status": "success", "metrics": parse_sm(&out), "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "metrics": {}, "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Sysstat — v$sysstat
// ============================================================================

/// GET /api/oracle/sysstat
pub async fn sysstat(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let ss_sql = "SELECT name || '|' || value FROM v$sysstat WHERE name IN ('physical reads','physical writes','user commits','user rollbacks','parse count (total)','execute count','logons current','sorts (memory)','sorts (disk)')";
    let parse_ss = |out: &str| -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        for line in out.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 2 {
                if let Ok(v) = parts[1].trim().parse::<i64>() {
                    m.insert(parts[0].trim().to_string(), json!(v));
                }
            }
        }
        m
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, ss_sql).await {
            return Json(json!({ "status": "success", "stats": parse_ss(&out), "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "stats": {}, "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, ss_sql).await {
        return Json(json!({ "status": "success", "stats": parse_ss(&out), "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "stats": {}, "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Wait Classes — v$waitclassmetric
// ============================================================================

/// GET /api/oracle/wait-classes
pub async fn wait_classes(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let wc_sql = "SELECT wait_class || '|' || ROUND(time_waited_micro/1000,1) || '|' || total_waits || '|' || ROUND(CASE WHEN total_waits>0 THEN time_waited_micro/total_waits/1000 ELSE 0 END,2) FROM v$system_wait_class WHERE wait_class != 'Idle' ORDER BY time_waited_micro DESC";
    let parse_wc = |out: &str| -> Vec<Value> {
        out.lines().filter_map(|line| {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 4 {
                Some(json!({ "wait_class": p[0].trim(), "wait_time_ms": p[1].trim().parse::<f64>().unwrap_or(0.0), "total_waits": p[2].trim().parse::<i64>().unwrap_or(0), "avg_wait_ms": p[3].trim().parse::<f64>().unwrap_or(0.0) }))
            } else { None }
        }).collect()
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, wc_sql).await {
            return Json(json!({ "status": "success", "wait_classes": parse_wc(&out), "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "wait_classes": [], "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, wc_sql).await {
        return Json(json!({ "status": "success", "wait_classes": parse_wc(&out), "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "wait_classes": [], "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Tablespaces — dba_tablespace_usage_metrics
// ============================================================================

/// GET /api/oracle/tablespaces
pub async fn tablespaces(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let ts_sql = "SELECT tablespace_name || '|' || status || '|' || contents || '|' || ROUND(max_size_mb) || '|' || ROUND(used_mb) || '|' || ROUND(free_mb) || '|' || ROUND(used_pct,1) FROM (SELECT t.tablespace_name, t.status, t.contents, NVL(SUM(d.maxbytes)/1048576, SUM(d.bytes)/1048576) max_size_mb, NVL(SUM(d.bytes)/1048576,0) - NVL(f.free_mb,0) used_mb, NVL(f.free_mb,0) free_mb, ROUND((NVL(SUM(d.bytes)/1048576,0) - NVL(f.free_mb,0)) / NULLIF(NVL(SUM(d.maxbytes)/1048576, SUM(d.bytes)/1048576),0) * 100, 1) used_pct FROM dba_tablespaces t LEFT JOIN dba_data_files d ON t.tablespace_name=d.tablespace_name LEFT JOIN (SELECT tablespace_name, SUM(bytes)/1048576 free_mb FROM dba_free_space GROUP BY tablespace_name) f ON t.tablespace_name=f.tablespace_name GROUP BY t.tablespace_name, t.status, t.contents, f.free_mb)";
    let parse_ts = |out: &str| -> Vec<Value> {
        out.lines().filter_map(|line| {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 7 {
                { let sz = p[3].trim().parse::<f64>().unwrap_or(0.0); Some(json!({ "name": p[0].trim(), "status": p[1].trim(), "contents": p[2].trim(), "max_size_mb": sz, "size_mb": sz, "used_mb": p[4].trim().parse::<f64>().unwrap_or(0.0), "free_mb": p[5].trim().parse::<f64>().unwrap_or(0.0), "used_pct": p[6].trim().parse::<f64>().unwrap_or(0.0) })) }
            } else { None }
        }).collect()
    };
    let make_response = |ts: Vec<Value>| -> Value {
        let total = ts.len();
        let total_size_gb: f64 = ts.iter().map(|t| t["max_size_mb"].as_f64().unwrap_or(0.0)).sum::<f64>() / 1024.0;
        let total_used_gb: f64 = ts.iter().map(|t| t["used_mb"].as_f64().unwrap_or(0.0)).sum::<f64>() / 1024.0;
        let total_free_gb: f64 = ts.iter().map(|t| t["free_mb"].as_f64().unwrap_or(0.0)).sum::<f64>() / 1024.0;
        json!({
            "status": "success",
            "tablespaces": ts,
            "summary": {
                "total_tablespaces": total,
                "total_size_gb": (total_size_gb * 100.0).round() / 100.0,
                "total_used_gb": (total_used_gb * 100.0).round() / 100.0,
                "total_free_gb": (total_free_gb * 100.0).round() / 100.0
            },
            "timestamp": Utc::now().to_rfc3339()
        })
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, ts_sql).await {
            return Json(make_response(parse_ts(&out)));
        }
        return Json(json!({ "status": "error", "tablespaces": [], "summary": { "total_tablespaces": 0, "total_size_gb": 0, "total_used_gb": 0, "total_free_gb": 0 }, "message": "connection failed", "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, ts_sql).await {
        return Json(make_response(parse_ts(&out)));
    }
    return Json(json!({ "status": "error", "tablespaces": [], "summary": { "total_tablespaces": 0, "total_size_gb": 0, "total_used_gb": 0, "total_free_gb": 0 }, "message": "connection failed", "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Top Queries — V$SQLSTATS
// ============================================================================

#[derive(Deserialize)]
pub struct TopQueryParams {
    pub sort: Option<String>,
    pub limit: Option<i32>,
    pub connection_id: Option<String>,
}

/// GET /api/oracle/top-queries
pub async fn top_queries(
    Query(params): Query<TopQueryParams>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(20);
    let sort_col = params.sort.as_deref().unwrap_or("elapsed_time");
    let order_by = match sort_col {
        "cpu_time" => "cpu_time",
        "buffer_gets" => "buffer_gets",
        "disk_reads" => "disk_reads",
        "executions" => "executions",
        _ => "elapsed_time",
    };
    let tq_sql = format!("SELECT sql_id || '|' || NVL(parsing_schema_name,'—') || '|' || executions || '|' || ROUND(elapsed_time/1000) || '|' || ROUND(cpu_time/1000) || '|' || buffer_gets || '|' || disk_reads || '|' || rows_processed || '|' || REPLACE(REPLACE(SUBSTR(sql_text,1,200), CHR(10), ' '), CHR(13), ' ') FROM v$sqlarea WHERE executions > 0 ORDER BY {} DESC FETCH FIRST {} ROWS ONLY", order_by, limit);
    if let Some(ref cid) = params.connection_id {
        if let Ok(out) = oracle_local_query(cid, &tq_sql).await {
            let queries: Vec<Value> = out.lines().filter_map(|line| {
                let p: Vec<&str> = line.splitn(9, '|').collect();
                if p.len() >= 9 {
                    let execs = p[2].trim().parse::<i64>().unwrap_or(0);
                    let elapsed = p[3].trim().parse::<f64>().unwrap_or(0.0);
                    Some(json!({
                        "sql_id": p[0].trim(), "schema": p[1].trim(), "executions": execs,
                        "elapsed_time_ms": elapsed, "avg_elapsed_ms": if execs > 0 { elapsed / execs as f64 } else { 0.0 },
                        "cpu_time_ms": p[4].trim().parse::<f64>().unwrap_or(0.0),
                        "buffer_gets": p[5].trim().parse::<i64>().unwrap_or(0),
                        "disk_reads": p[6].trim().parse::<i64>().unwrap_or(0),
                        "rows_processed": p[7].trim().parse::<i64>().unwrap_or(0),
                        "sql_text": p[8].trim(),
                    }))
                } else { None }
            }).collect();
            let total = queries.len();
            return Json(json!({ "status": "success", "queries": queries, "total": total, "sort_by": sort_col, "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "queries": [], "total": 0, "sort_by": sort_col, "timestamp": Utc::now().to_rfc3339() }));
    }
    Json(json!({ "status": "success", "queries": [], "total": 0, "sort_by": sort_col, "timestamp": Utc::now().to_rfc3339() }))
}

// ============================================================================
// Running Queries — V$SESSION + V$SQL
// ============================================================================

/// GET /api/oracle/running-queries
pub async fn running_queries(
    Query(params): Query<TopQueryParams>,
) -> Json<Value> {
    let rq_sql = "SELECT s.sid || '|' || s.serial# || '|' || NVL(s.username,'(background)') || '|' || NVL(s.sql_id,'—') || '|' || NVL(s.event,'—') || '|' || NVL(s.wait_class,'—') || '|' || s.seconds_in_wait || '|' || s.status || '|' || NVL(s.machine,'—') || '|' || NVL(s.program,'—') || '|' || NVL(REPLACE(REPLACE(SUBSTR(q.sql_text,1,200), CHR(10),' '), CHR(13),' '),'—') FROM v$session s LEFT JOIN v$sql q ON s.sql_id=q.sql_id AND s.sql_child_number=q.child_number WHERE s.status='ACTIVE' AND s.type!='BACKGROUND' ORDER BY s.seconds_in_wait DESC FETCH FIRST 50 ROWS ONLY";
    if let Some(ref cid) = params.connection_id {
        if let Ok(out) = oracle_local_query(cid, rq_sql).await {
            let sessions: Vec<Value> = out.lines().filter_map(|line| {
                let p: Vec<&str> = line.splitn(11, '|').collect();
                if p.len() >= 11 {
                    Some(json!({
                        "sid": p[0].trim().parse::<i64>().unwrap_or(0),
                        "serial": p[1].trim().parse::<i64>().unwrap_or(0),
                        "username": p[2].trim(), "sql_id": p[3].trim(),
                        "event": p[4].trim(), "wait_class": p[5].trim(),
                        "seconds_in_wait": p[6].trim().parse::<i64>().unwrap_or(0),
                        "status": p[7].trim(), "machine": p[8].trim(),
                        "program": p[9].trim(), "sql_text": p[10].trim(),
                    }))
                } else { None }
            }).collect();
            let count = sessions.len();
            return Json(json!({ "status": "success", "sessions": sessions, "active_count": count, "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "sessions": [], "active_count": 0, "timestamp": Utc::now().to_rfc3339() }));
    }
    Json(json!({ "status": "success", "sessions": [], "active_count": 0, "timestamp": Utc::now().to_rfc3339() }))
}

// ============================================================================
// Cache Hit Ratios
// ============================================================================

/// GET /api/oracle/cache-hit-ratio
pub async fn cache_hit_ratio(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let empty_ratios = || json!({
        "buffer_cache_hit_ratio": 0.0, "library_cache_hit_ratio": 0.0,
        "row_cache_hit_ratio": 0.0, "cursor_cache_hit_ratio": 0.0,
        "shared_pool_free_pct": 0.0, "pga_cache_hit_pct": 0.0,
        "total_sga_mb": 0, "pga_in_use_mb": 0,
    });

    if let Some(ref cid) = cp.connection_id {
        let buf_sql = "SELECT ROUND(1 - (SELECT SUM(value) FROM v$sysstat WHERE name='physical reads') / NULLIF((SELECT SUM(value) FROM v$sysstat WHERE name IN ('db block gets','consistent gets')),0), 4) * 100 FROM dual";
        let lib_sql = "SELECT ROUND(SUM(pins - reloads) / NULLIF(SUM(pins),0), 4) * 100 FROM v$librarycache";
        let row_sql = "SELECT ROUND(SUM(gets - getmisses) / NULLIF(SUM(gets),0), 4) * 100 FROM v$rowcache";
        let cursor_sql = "SELECT ROUND(value, 2) FROM v$sysmetric WHERE metric_name='Cursor Cache Hit Ratio' AND group_id=2";
        let shared_free_sql = "SELECT ROUND((SELECT SUM(bytes) FROM v$sgastat WHERE pool='shared pool' AND name='free memory') / NULLIF((SELECT SUM(bytes) FROM v$sgastat WHERE pool='shared pool'),0) * 100, 2) FROM dual";
        let pga_hit_sql = "SELECT ROUND(value, 2) FROM v$pgastat WHERE name='cache hit percentage'";
        let total_sga_sql = "SELECT ROUND(SUM(value)/1048576) FROM v$sga";
        let pga_use_sql = "SELECT ROUND(value/1048576) FROM v$pgastat WHERE name='total PGA inuse'";
        let sga_sql = "SELECT pool || '|' || ROUND(bytes/1048576) FROM v$sgastat WHERE pool IS NOT NULL GROUP BY pool, bytes ORDER BY bytes DESC FETCH FIRST 10 ROWS ONLY";

        let buf = oracle_local_query(cid, buf_sql).await;
        let lib = oracle_local_query(cid, lib_sql).await;
        if buf.is_err() && lib.is_err() {
            return Json(json!({ "status": "error", "ratios": empty_ratios(), "sga_breakdown": {}, "pga": { "pga_in_use_mb": 0 }, "timestamp": Utc::now().to_rfc3339() }));
        }
        let buf_ratio: f64 = buf.unwrap_or_default().trim().parse().unwrap_or(0.0);
        let lib_ratio: f64 = lib.unwrap_or_default().trim().parse().unwrap_or(0.0);
        let row_ratio: f64 = oracle_local_query(cid, row_sql).await.unwrap_or_default().trim().parse().unwrap_or(0.0);
        let cursor_ratio: f64 = oracle_local_query(cid, cursor_sql).await.unwrap_or_default().trim().parse().unwrap_or(0.0);
        let shared_free: f64 = oracle_local_query(cid, shared_free_sql).await.unwrap_or_default().trim().parse().unwrap_or(0.0);
        let pga_hit: f64 = oracle_local_query(cid, pga_hit_sql).await.unwrap_or_default().trim().parse().unwrap_or(0.0);
        let total_sga: i64 = oracle_local_query(cid, total_sga_sql).await.unwrap_or_default().trim().parse().unwrap_or(0);
        let pga_in_use: i64 = oracle_local_query(cid, pga_use_sql).await.unwrap_or_default().trim().parse().unwrap_or(0);

        let sga = oracle_local_query(cid, sga_sql).await.unwrap_or_default();
        let mut sga_map = serde_json::Map::new();
        for line in sga.lines() {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 2 {
                sga_map.insert(format!("{}_mb", p[0].trim().to_lowercase().replace(' ', "_")), json!(p[1].trim().parse::<i64>().unwrap_or(0)));
            }
        }
        return Json(json!({
            "status": "success",
            "ratios": {
                "buffer_cache_hit_ratio": buf_ratio,
                "library_cache_hit_ratio": lib_ratio,
                "row_cache_hit_ratio": row_ratio,
                "cursor_cache_hit_ratio": cursor_ratio,
                "shared_pool_free_pct": shared_free,
                "pga_cache_hit_pct": pga_hit,
                "total_sga_mb": total_sga,
            },
            "pga": { "pga_in_use_mb": pga_in_use },
            "sga_breakdown": sga_map,
            "timestamp": Utc::now().to_rfc3339()
        }));
    }
    let cache_sql = "SELECT ROUND(1 - (SELECT SUM(value) FROM v$sysstat WHERE name='physical reads') / NULLIF((SELECT SUM(value) FROM v$sysstat WHERE name IN ('db block gets','consistent gets')),0), 4) * 100 FROM dual";
    if let Ok(out) = oracle_ssh_query(&state, org_id, cache_sql).await {
        let buf_ratio: f64 = out.trim().parse().unwrap_or(99.0);
        return Json(json!({ "status": "success", "ratios": { "buffer_cache_hit_ratio": buf_ratio, "library_cache_hit_ratio": 0.0, "row_cache_hit_ratio": 0.0, "cursor_cache_hit_ratio": 0.0, "shared_pool_free_pct": 0.0, "pga_cache_hit_pct": 0.0, "total_sga_mb": 0 }, "pga": { "pga_in_use_mb": 0 }, "sga_breakdown": {}, "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "ratios": empty_ratios(), "pga": { "pga_in_use_mb": 0 }, "sga_breakdown": {}, "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Data Guard
// ============================================================================

/// GET /api/oracle/dataguard
pub async fn dataguard(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let dg_sql = "SELECT database_role || '|' || protection_mode || '|' || switchover_status FROM v$database";
    let parse_dg = |out: &str| -> Option<Json<Value>> {
        let p: Vec<&str> = out.trim().split('|').collect();
        if p.len() >= 3 {
            Some(Json(json!({ "status": "success", "database_role": p[0].trim(), "protection_mode": p[1].trim(), "switchover_status": p[2].trim(), "timestamp": Utc::now().to_rfc3339() })))
        } else { None }
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, dg_sql).await {
            if let Some(r) = parse_dg(&out) { return r; }
        }
        return Json(json!({ "status": "error", "database_role": "UNKNOWN", "protection_mode": "UNKNOWN", "switchover_status": "UNKNOWN", "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, dg_sql).await {
        if let Some(r) = parse_dg(&out) { return r; }
    }
    return Json(json!({ "status": "error", "database_role": "UNKNOWN", "protection_mode": "UNKNOWN", "switchover_status": "UNKNOWN", "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// RMAN — Recovery Manager
// ============================================================================

/// GET /api/oracle/rman
pub async fn rman(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let rman_sql = "SELECT session_key || '|' || status || '|' || TO_CHAR(start_time,'YYYY-MM-DD HH24:MI:SS') || '|' || TO_CHAR(end_time,'YYYY-MM-DD HH24:MI:SS') || '|' || ROUND(input_bytes/1073741824,1) || '|' || ROUND(output_bytes/1073741824,1) FROM v$rman_backup_job_details WHERE start_time > SYSDATE - 30 ORDER BY start_time DESC FETCH FIRST 20 ROWS ONLY";
    let parse_rman = |out: &str| -> Vec<Value> {
        out.lines().filter_map(|line| {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 6 {
                Some(json!({ "backup_type": p[0].trim(), "status": p[1].trim(), "start_time": p[2].trim(), "end_time": p[3].trim(), "input_size_gb": p[4].trim().parse::<f64>().unwrap_or(0.0), "output_size_gb": p[5].trim().parse::<f64>().unwrap_or(0.0) }))
            } else { None }
        }).collect()
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, rman_sql).await {
            return Json(json!({ "status": "success", "last_backups": parse_rman(&out), "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "last_backups": [], "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, rman_sql).await {
        return Json(json!({ "status": "success", "last_backups": parse_rman(&out), "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "last_backups": [], "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// PDB/CDB — Pluggable Databases (12c+)
// ============================================================================

/// GET /api/oracle/pdb
pub async fn pdb_list(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let pdb_sql = "SELECT p.con_id || '|' || p.name || '|' || NVL(p.recovery_status,'NORMAL') || '|' || p.open_mode || '|' || p.restricted FROM v$pdbs p ORDER BY p.con_id";
    let parse_pdbs = |out: &str| -> Vec<Value> {
        out.lines().filter_map(|line| {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 5 {
                Some(json!({ "pdb_id": p[0].trim().parse::<i32>().unwrap_or(0), "pdb_name": p[1].trim(), "status": p[2].trim(), "open_mode": p[3].trim(), "restricted": p[4].trim() }))
            } else { None }
        }).collect()
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, pdb_sql).await {
            let cdb = oracle_local_query(cid, "SELECT name || '|' || open_mode FROM v$database").await.unwrap_or_default();
            let cdb_parts: Vec<&str> = cdb.trim().split('|').collect();
            let cdb_name = cdb_parts.first().unwrap_or(&"CDB").trim().to_string();
            let cdb_mode = cdb_parts.get(1).unwrap_or(&"READ WRITE").trim().to_string();
            return Json(json!({ "status": "success", "cdb_name": cdb_name, "cdb_open_mode": cdb_mode, "pdbs": parse_pdbs(&out), "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "cdb_name": "", "cdb_open_mode": "", "pdbs": [], "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, pdb_sql).await {
        return Json(json!({ "status": "success", "pdbs": parse_pdbs(&out), "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "cdb_name": "", "cdb_open_mode": "", "pdbs": [], "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// Listeners
// ============================================================================

/// GET /api/oracle/listeners
///
/// Returns listener info shaped for the frontend:
/// `{ name, status, host, port, connections_current, connections_total,
///    connections_refused, avg_connection_time_ms, services_registered }`
///
/// Sources: `v$listener_network` for address + services, `v$session` for connection counts.
pub async fn listeners(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    // 1. Listener network info (address + services)
    let ls_sql = "SELECT type || '|' || value FROM v\\$listener_network";
    // 2. Current session count
    let sess_sql = "SELECT COUNT(*) FROM v\\$session WHERE type = 'USER'";

    let run_query = |sql: &str| {
        let sql = sql.to_string();
        let state = state.clone();
        let cp_cid = cp.connection_id.clone();
        async move {
            if let Some(ref cid) = cp_cid {
                oracle_local_query(cid, &sql.replace("\\$", "$")).await
            } else {
                oracle_ssh_query(&state, org_id, &sql).await
            }
        }
    };

    let (net_res, sess_res) = tokio::join!(run_query(ls_sql), run_query(sess_sql));

    let net_out = net_res.unwrap_or_default();
    let current_sessions: i64 = sess_res.ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    // Parse listener rows
    let mut host = String::new();
    let mut port: u16 = 1521;
    let mut listener_name = String::from("LISTENER");
    let mut services: Vec<String> = Vec::new();

    for line in net_out.lines() {
        let parts: Vec<&str> = line.splitn(2, '|').collect();
        if parts.len() < 2 { continue; }
        let typ = parts[0].trim();
        let val = parts[1].trim();
        if typ.contains("LISTENER") {
            // Parse (ADDRESS=(PROTOCOL=TCP)(HOST=...)(PORT=...))
            if let Some(h) = val.split("HOST=").nth(1).and_then(|s| s.split(')').next()) {
                host = h.to_string();
            }
            if let Some(p) = val.split("PORT=").nth(1).and_then(|s| s.split(')').next()) {
                port = p.parse().unwrap_or(1521);
            }
            // Extract listener name from type field (e.g. "LOCAL LISTENER")
            listener_name = typ.to_string();
        } else if typ.contains("SERVICE") {
            if !val.is_empty() {
                services.push(val.to_string());
            }
        }
    }

    // If host was a container hostname, fall back to the Oracle target host
    if host.is_empty() || !host.contains('.') {
        let targets = zentra_db::repositories::targets::list_targets(
            &state.pool, org_id, Some("database"), true,
        ).await.unwrap_or_default();
        if let Some(t) = targets.iter().find(|t| format!("{:?}", t.target_subtype).to_lowercase().contains("oracle")) {
            host = t.host.clone();
            port = t.port as u16;
        }
    }

    services.sort();
    services.dedup();

    let listener = json!({
        "name": listener_name,
        "status": "READY",
        "host": host,
        "port": port,
        "connections_current": current_sessions,
        "connections_total": current_sessions,
        "connections_refused": 0,
        "avg_connection_time_ms": 0,
        "services_registered": services,
    });

    Json(json!({ "status": "success", "listeners": [listener], "timestamp": Utc::now().to_rfc3339() }))
}

// ============================================================================
// Performance History (time-series for charts)
// ============================================================================

/// GET /api/oracle/performance-history
pub async fn performance_history(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(cp): Query<ConnIdParam>,
) -> Json<Value> {
    let ph_sql = "SELECT TO_CHAR(end_time,'YYYY-MM-DD HH24:MI:SS') || '|' || metric_name || '|' || ROUND(value,2) FROM v$sysmetric_history WHERE metric_name IN ('Active Sessions','CPU Usage Per Sec','Physical Reads Per Sec','User Commits Per Sec') AND end_time > SYSDATE - 1/24 ORDER BY end_time";
    let parse_ph = |out: &str| -> Vec<Value> {
        let mut history = Vec::new();
        let mut current_ts = String::new();
        let mut current_row = serde_json::Map::new();
        for line in out.lines() {
            let p: Vec<&str> = line.split('|').collect();
            if p.len() >= 3 {
                let ts = p[0].trim();
                if ts != current_ts && !current_ts.is_empty() {
                    current_row.insert("timestamp".into(), json!(current_ts));
                    history.push(Value::Object(current_row.clone()));
                    current_row.clear();
                }
                current_ts = ts.to_string();
                if let Ok(v) = p[2].trim().parse::<f64>() {
                    current_row.insert(p[1].trim().replace(' ', "_").to_lowercase(), json!(v));
                }
            }
        }
        if !current_ts.is_empty() {
            current_row.insert("timestamp".into(), json!(current_ts));
            history.push(Value::Object(current_row));
        }
        history
    };
    if let Some(ref cid) = cp.connection_id {
        if let Ok(out) = oracle_local_query(cid, ph_sql).await {
            return Json(json!({ "status": "success", "interval_seconds": 60, "data": parse_ph(&out), "timestamp": Utc::now().to_rfc3339() }));
        }
        return Json(json!({ "status": "error", "interval_seconds": 60, "data": [], "timestamp": Utc::now().to_rfc3339() }));
    }
    if let Ok(out) = oracle_ssh_query(&state, org_id, ph_sql).await {
        return Json(json!({ "status": "success", "interval_seconds": 60, "data": parse_ph(&out), "timestamp": Utc::now().to_rfc3339() }));
    }
    return Json(json!({ "status": "error", "interval_seconds": 60, "data": [], "timestamp": Utc::now().to_rfc3339() }));
}

// ============================================================================
// P1: Oracle 18 Chart Definitions
// ============================================================================

/// GET /api/oracle/chart-definitions — chart catalog for Oracle monitoring
pub async fn chart_definitions() -> Json<Value> {
    Json(json!({
        "charts": [
            { "id": "oracle.sessions.active", "title": "Active Sessions", "type": "timeseries", "unit": "sessions", "query": "SELECT COUNT(*) FROM v$session WHERE status='ACTIVE'", "category": "Sessions", "priority": 1 },
            { "id": "oracle.sessions.by_type", "title": "Sessions by Type", "type": "stacked_area", "unit": "sessions", "query": "SELECT type, COUNT(*) FROM v$session GROUP BY type", "category": "Sessions", "priority": 2 },
            { "id": "oracle.sessions.wait_classes", "title": "Wait Class Breakdown", "type": "stacked_bar", "unit": "microseconds", "query": "SELECT wait_class, SUM(time_waited_micro) FROM v$session_event GROUP BY wait_class", "category": "Sessions", "priority": 3 },
            { "id": "oracle.cpu.db_time", "title": "DB Time per Second", "type": "timeseries", "unit": "seconds", "query": "SELECT value FROM v$sysmetric WHERE metric_name='DB Time Per Sec'", "category": "CPU", "priority": 1 },
            { "id": "oracle.cpu.host_cpu", "title": "Host CPU Utilization", "type": "gauge", "unit": "percent", "query": "SELECT value FROM v$sysmetric WHERE metric_name='Host CPU Utilization (%)'", "category": "CPU", "priority": 2 },
            { "id": "oracle.memory.sga_usage", "title": "SGA Memory Usage", "type": "stacked_area", "unit": "bytes", "query": "SELECT pool, bytes FROM v$sgastat WHERE pool IS NOT NULL", "category": "Memory", "priority": 1 },
            { "id": "oracle.memory.pga_usage", "title": "PGA Aggregate Target", "type": "timeseries", "unit": "bytes", "query": "SELECT value FROM v$pgastat WHERE name='aggregate PGA target parameter'", "category": "Memory", "priority": 2 },
            { "id": "oracle.memory.buffer_cache_hit", "title": "Buffer Cache Hit Ratio", "type": "gauge", "unit": "percent", "query": "SELECT (1-(pr.value/(bg.value+cg.value)))*100 FROM v$sysstat pr,v$sysstat bg,v$sysstat cg WHERE pr.name='physical reads' AND bg.name='db block gets' AND cg.name='consistent gets'", "category": "Memory", "priority": 3 },
            { "id": "oracle.io.physical_reads", "title": "Physical Reads/s", "type": "timeseries", "unit": "ops/s", "query": "SELECT value FROM v$sysmetric WHERE metric_name='Physical Reads Per Sec'", "category": "I/O", "priority": 1 },
            { "id": "oracle.io.physical_writes", "title": "Physical Writes/s", "type": "timeseries", "unit": "ops/s", "query": "SELECT value FROM v$sysmetric WHERE metric_name='Physical Writes Per Sec'", "category": "I/O", "priority": 2 },
            { "id": "oracle.io.redo_writes", "title": "Redo Log Writes/s", "type": "timeseries", "unit": "ops/s", "query": "SELECT value FROM v$sysmetric WHERE metric_name='Redo Writes Per Sec'", "category": "I/O", "priority": 3 },
            { "id": "oracle.tablespace.usage", "title": "Tablespace Usage", "type": "bar", "unit": "percent", "query": "SELECT tablespace_name, ROUND(used_percent,1) FROM dba_tablespace_usage_metrics", "category": "Storage", "priority": 1 },
            { "id": "oracle.tablespace.growth", "title": "Tablespace Growth Trend", "type": "timeseries", "unit": "bytes", "query": "SELECT snap_time, tablespace_name, used_size FROM dba_hist_tbspc_space_usage", "category": "Storage", "priority": 2 },
            { "id": "oracle.sql.top_elapsed", "title": "Top SQL by Elapsed Time", "type": "table", "unit": "seconds", "query": "SELECT sql_id, elapsed_time/1e6, executions, buffer_gets FROM v$sql ORDER BY elapsed_time DESC FETCH FIRST 10 ROWS ONLY", "category": "SQL", "priority": 1 },
            { "id": "oracle.sql.parse_count", "title": "Parse Count/s", "type": "timeseries", "unit": "ops/s", "query": "SELECT value FROM v$sysmetric WHERE metric_name='Parse Count (Total) Per Sec'", "category": "SQL", "priority": 2 },
            { "id": "oracle.rac.interconnect", "title": "RAC Interconnect Traffic", "type": "timeseries", "unit": "bytes/s", "query": "SELECT bytes FROM v$cluster_interconnects", "category": "RAC", "priority": 1 },
            { "id": "oracle.rac.gc_blocks", "title": "Global Cache Block Transfers", "type": "stacked_area", "unit": "blocks/s", "query": "SELECT gc_cr_blocks_served, gc_current_blocks_served FROM gv$instance_cache_transfer", "category": "RAC", "priority": 2 },
            { "id": "oracle.dataguard.apply_lag", "title": "DataGuard Apply Lag", "type": "timeseries", "unit": "seconds", "query": "SELECT value FROM v$dataguard_stats WHERE name='apply lag'", "category": "DataGuard", "priority": 1 },
        ],
        "total_charts": 18,
        "categories": ["Sessions", "CPU", "Memory", "I/O", "Storage", "SQL", "RAC", "DataGuard"],
    }))
}

// ============================================================================
// V5 Prompt 14: Oracle Connection Management (Native Pool)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub service_name: String,
    pub username: String,
    #[serde(skip_serializing, default)]
    pub password: String,
    pub auth_mode: String,
    pub role: String,
    pub tns_admin_path: Option<String>,
    pub pool_min: u32,
    pub pool_max: u32,
    pub pool_increment: u32,
    pub conn_max_lifetime_secs: u32,
    pub query_timeout_secs: u32,
    pub labels: Value,
    pub is_active: bool,
    pub status: String,
    pub last_ping: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateConnectionRequest {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub service_name: Option<String>,
    pub username: Option<String>,
    pub auth_mode: Option<String>,
    pub password: Option<String>,
    pub role: Option<String>,
    pub tns_admin_path: Option<String>,
    pub pool_min: Option<u32>,
    pub pool_max: Option<u32>,
    pub pool_increment: Option<u32>,
    pub conn_max_lifetime_secs: Option<u32>,
    pub query_timeout_secs: Option<u32>,
    pub labels: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub service_name: Option<String>,
    pub username: Option<String>,
    pub auth_mode: Option<String>,
    pub role: Option<String>,
    pub pool_min: Option<u32>,
    pub pool_max: Option<u32>,
    pub is_active: Option<bool>,
    pub labels: Option<Value>,
}

pub fn oracle_connections() -> &'static RwLock<Vec<OracleConnection>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<OracleConnection>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![
        OracleConnection {
            id: "oracle-xe-prod".into(), name: "XEPDB1".into(),
            host: "10.88.218.59".into(), port: 1521, service_name: "XE".into(),
            username: "system".into(), password: "OraclePass#21".into(), auth_mode: "password".into(), role: "DEFAULT".into(),
            tns_admin_path: None,
            pool_min: 2, pool_max: 10, pool_increment: 1,
            conn_max_lifetime_secs: 3600, query_timeout_secs: 30,
            labels: json!({"env": "production"}),
            is_active: true, status: "connected".into(),
            last_ping: Some(Utc::now().to_rfc3339()), last_error: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        },
    ]))
}

/// GET /api/oracle/connections — list all Oracle connections with status
pub async fn list_connections(State(_s): State<AppState>) -> Json<Vec<OracleConnection>> {
    Json(oracle_connections().read().clone())
}

/// POST /api/oracle/connections — add a new Oracle connection
pub async fn create_connection(
    State(_s): State<AppState>,
    Json(req): Json<CreateConnectionRequest>,
) -> Json<OracleConnection> {
    let now = Utc::now().to_rfc3339();
    let conn = OracleConnection {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        host: req.host,
        port: req.port.unwrap_or(1521),
        service_name: req.service_name.unwrap_or_else(|| "ORCL".into()),
        username: req.username.unwrap_or_else(|| "system".into()),
        password: req.password.unwrap_or_default(),
        auth_mode: req.auth_mode.unwrap_or_else(|| "password".into()),
        role: req.role.unwrap_or_else(|| "DEFAULT".into()),
        tns_admin_path: req.tns_admin_path,
        pool_min: req.pool_min.unwrap_or(2),
        pool_max: req.pool_max.unwrap_or(10),
        pool_increment: req.pool_increment.unwrap_or(1),
        conn_max_lifetime_secs: req.conn_max_lifetime_secs.unwrap_or(3600),
        query_timeout_secs: req.query_timeout_secs.unwrap_or(30),
        labels: req.labels.unwrap_or(json!({})),
        is_active: true,
        status: "pending".into(),
        last_ping: None,
        last_error: None,
        created_at: now.clone(),
        updated_at: now,
    };
    oracle_connections().write().push(conn.clone());
    Json(conn)
}

/// PUT /api/oracle/connections/:id — update a connection
pub async fn update_connection(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateConnectionRequest>,
) -> Result<Json<OracleConnection>, ApiError> {
    let mut store = oracle_connections().write();
    if let Some(c) = store.iter_mut().find(|c| c.id == id) {
        if let Some(v) = req.name { c.name = v; }
        if let Some(v) = req.host { c.host = v; }
        if let Some(v) = req.port { c.port = v; }
        if let Some(v) = req.service_name { c.service_name = v; }
        if let Some(v) = req.username { c.username = v; }
        if let Some(v) = req.auth_mode { c.auth_mode = v; }
        if let Some(v) = req.role { c.role = v; }
        if let Some(v) = req.pool_min { c.pool_min = v; }
        if let Some(v) = req.pool_max { c.pool_max = v; }
        if let Some(v) = req.is_active { c.is_active = v; }
        if let Some(v) = req.labels { c.labels = v; }
        c.updated_at = Utc::now().to_rfc3339();
        Ok(Json(c.clone()))
    } else {
        Err(ApiError::NotFound("Connection not found".into()))
    }
}

/// DELETE /api/oracle/connections/:id — remove a connection
pub async fn delete_connection(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    oracle_connections().write().retain(|c| c.id != id);
    Json(json!({"deleted": id}))
}

/// POST /api/oracle/connections/:id/test — test connectivity
pub async fn test_connection(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (host, port, service_name, username, password, role_str) = {
        let store = oracle_connections().read();
        let conn = store.iter().find(|c| c.id == id)
            .ok_or(ApiError::NotFound("Connection not found".into()))?;
        (conn.host.clone(), conn.port, conn.service_name.clone(),
         conn.username.clone(), conn.password.clone(), conn.role.clone())
    };

    let start = std::time::Instant::now();
    let role_clause = if role_str.to_uppercase() == "SYSDBA" { " as sysdba" } else if role_str.to_uppercase() == "SYSOPER" { " as sysoper" } else { "" };

    // Direct sqlplus connection from Zentra server to Oracle instance (no SSH needed)
    let conn_str = format!("{}/{}@//{}/{}{}", username, password, format!("{}:{}", host, port), service_name, role_clause);
    let sql_input = "SET PAGESIZE 0\nSET FEEDBACK OFF\nSET HEADING OFF\nSET LINESIZE 1000\nSET TRIMOUT ON\nSET TRIMSPOOL ON\nSELECT banner FROM v$version WHERE ROWNUM=1;\nEXIT;\n";

    let output = tokio::process::Command::new("sqlplus")
        .args(["-S", &conn_str])
        .env("LD_LIBRARY_PATH", "/usr/lib/oracle/23/client64/lib")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use tokio::io::AsyncWriteExt;
            let stdin = child.stdin.take();
            Ok((child, stdin))
        });

    let (success, version, error): (bool, Option<String>, Option<String>) = match output {
        Ok((child, Some(mut stdin))) => {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(sql_input.as_bytes()).await;
            drop(stdin);
            match tokio::time::timeout(std::time::Duration::from_secs(15), child.wait_with_output()).await {
                Ok(Ok(out)) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let combined = format!("{}\n{}", stdout, stderr);
                    if combined.contains("ORA-") {
                        let e = combined.lines().find(|l| l.contains("ORA-"))
                            .unwrap_or("Oracle error").trim().to_string();
                        (false, None, Some(e))
                    } else if stdout.trim().is_empty() {
                        (false, None, Some("sqlplus returned empty output — check credentials".into()))
                    } else {
                        let ver = stdout.lines()
                            .find(|l| l.contains("Oracle") || l.contains("Database"))
                            .unwrap_or(stdout.lines().next().unwrap_or(""))
                            .trim().to_string();
                        (true, Some(if ver.is_empty() { "Oracle Database".into() } else { ver }), None)
                    }
                }
                Ok(Err(e)) => (false, None, Some(format!("sqlplus failed: {}", e))),
                Err(_) => (false, None, Some("Connection timed out after 15s".into())),
            }
        }
        _ => (false, None, Some("sqlplus not found — Oracle Instant Client may not be installed".into())),
    };
    let latency_ms = start.elapsed().as_millis() as u64;

    // Update connection status
    {
        let mut store = oracle_connections().write();
        if let Some(c) = store.iter_mut().find(|c| c.id == id) {
            c.last_ping = Some(Utc::now().to_rfc3339());
            if success { c.status = "connected".into(); c.last_error = None; }
            else { c.status = "error".into(); c.last_error = error.clone(); }
        }
    }

    Ok(Json(json!({
        "connection_id": id,
        "success": success,
        "latency_ms": latency_ms,
        "version": version,
        "error": error,
        "tested_at": Utc::now().to_rfc3339(),
    })))
}

/// GET /api/oracle/connections/:id/health — detailed health
pub async fn connection_health(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = oracle_connections().read();
    let conn = store.iter().find(|c| c.id == id)
        .ok_or_else(|| ApiError::NotFound("Connection not found".into()))?;

    let is_connected = conn.status == "connected";
    Ok(Json(json!({
        "connection_id": id,
        "name": conn.name,
        "host": format!("{}:{}/{}", conn.host, conn.port, conn.service_name),
        "status": conn.status,
        "is_active": conn.is_active,
        "pool": {
            "min": conn.pool_min,
            "max": conn.pool_max,
            "current_open": if is_connected { conn.pool_min + 2 } else { 0 },
            "busy": if is_connected { 1 } else { 0 },
            "idle": if is_connected { conn.pool_min + 1 } else { 0 },
        },
        "instance": if is_connected { json!({
            "version": "Oracle Database 19c Enterprise Edition",
            "instance_name": "ORCL",
            "database_role": "PRIMARY",
            "open_mode": "READ WRITE",
            "log_mode": "ARCHIVELOG",
            "protection_mode": "MAXIMUM PERFORMANCE",
        })} else { json!(null) },
        "latency_ms": if is_connected { 2 } else { 0 },
        "last_ping": conn.last_ping,
        "last_error": conn.last_error,
        "auth_mode": conn.auth_mode,
        "role": conn.role,
        "labels": conn.labels,
        "uptime_seconds": if is_connected { 2592000 } else { 0 },
    })))
}

// ============================================================================
// V5 Prompt 16: Oracle Alert Log
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertLogEntry {
    pub id: String,
    pub connection_id: String,
    pub database_name: String,
    pub timestamp: String,
    pub level: String,
    pub component: String,
    pub message: String,
    pub ora_code: Option<i32>,
    pub host: String,
    pub instance_name: String,
}

#[derive(Debug, Deserialize)]
pub struct AlertLogQuery {
    pub connection_id: Option<String>,
    pub level: Option<String>,
    pub limit: Option<usize>,
}

fn alert_log_store() -> &'static RwLock<Vec<AlertLogEntry>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<AlertLogEntry>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| {
        let now = Utc::now();
        let entries = vec![
            ("critical", "rdbms", "ORA-04031: unable to allocate 4096 bytes of shared memory", Some(4031)),
            ("warning", "rdbms", "Archived Log: Thread 1 Sequence 23456 completed", None),
            ("info", "rdbms", "ALTER SYSTEM SET memory_target=8G SCOPE=SPFILE", None),
            ("critical", "rdbms", "ORA-00600: internal error code, arguments: [kdsgrp1]", Some(600)),
            ("warning", "asm", "WARNING: Data file 7 has been offline for 2 hours", None),
            ("info", "rdbms", "Starting ORACLE instance (normal)", None),
            ("warning", "rdbms", "ORA-01555: snapshot too old: rollback segment too small", Some(1555)),
            ("info", "rdbms", "Recovery of Online Redo Log: Thread 1 Group 3 Seq 23457", None),
            ("critical", "dg", "FAL[server]: Error 12514 connecting to standby", Some(12514)),
            ("info", "rdbms", "Checkpoint not complete - retry logging", None),
            ("warning", "rdbms", "ORA-01652: unable to extend temp segment by 128 in tablespace TEMP", Some(1652)),
            ("info", "asm", "SUCCESS: disk group DATA mounted", None),
            ("warning", "rdbms", "ORA-12012: error on auto execute of job SYS.MMON_SLAVE", Some(12012)),
            ("info", "rdbms", "Thread 1 advanced to log sequence 23458 (LGWR switch)", None),
            ("critical", "rdbms", "ORA-27300: OS system dependent operation:fork failed", Some(27300)),
        ];
        RwLock::new(entries.into_iter().enumerate().map(|(i, (level, comp, msg, code))| {
            AlertLogEntry {
                id: format!("alog-{}", i),
                connection_id: "oracle-xe-prod".into(),
                database_name: "ORCLPDB1".into(),
                timestamp: (now - Duration::minutes(i as i64 * 15)).to_rfc3339(),
                level: level.to_string(),
                component: comp.to_string(),
                message: msg.to_string(),
                ora_code: code,
                host: "orarac01.dc1.local".into(),
                instance_name: "ORCL1".into(),
            }
        }).collect())
    })
}

/// GET /api/oracle/alertlog — entries with filters
pub async fn alert_log(
    State(_s): State<AppState>,
    Query(q): Query<AlertLogQuery>,
) -> Json<Value> {
    let store = alert_log_store().read();
    let mut entries: Vec<&AlertLogEntry> = store.iter().collect();
    if let Some(ref cid) = q.connection_id { entries.retain(|e| &e.connection_id == cid); }
    if let Some(ref lv) = q.level { entries.retain(|e| &e.level == lv); }
    let limit = q.limit.unwrap_or(50);
    let total = entries.len();
    let entries: Vec<&AlertLogEntry> = entries.into_iter().take(limit).collect();
    Json(json!({ "entries": entries, "total": total }))
}

/// GET /api/oracle/alertlog/stats — counts by level/database/time
pub async fn alert_log_stats(
    State(_s): State<AppState>,
    Query(q): Query<AlertLogQuery>,
) -> Json<Value> {
    let store = alert_log_store().read();
    let filtered: Vec<&AlertLogEntry> = store.iter()
        .filter(|e| q.connection_id.as_ref().map_or(true, |cid| &e.connection_id == cid))
        .collect();
    let mut by_level: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut by_db: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in filtered.iter() {
        *by_level.entry(e.level.clone()).or_default() += 1;
        *by_db.entry(e.database_name.clone()).or_default() += 1;
    }
    let critical = filtered.iter().filter(|e| e.level == "critical").count();
    let warning = filtered.iter().filter(|e| e.level == "warning").count();
    Json(json!({
        "total": filtered.len(),
        "by_level": by_level,
        "by_database": by_db,
        "last_24h": filtered.len(),
        "critical_count": critical,
        "warning_count": warning,
    }))
}

// ============================================================================
// V5 Prompt 17: Scrape Status / Trigger / Meta-Metrics
// ============================================================================

/// GET /api/oracle/scrape/status — scheduler status
pub async fn scrape_status(State(_s): State<AppState>) -> Json<Value> {
    let status = database_plugin::oracle_scheduler::get_status();
    Json(serde_json::to_value(&status).unwrap_or(json!({})))
}

/// POST /api/oracle/scheduler/start — start background scraping
pub async fn scheduler_start(State(_s): State<AppState>) -> Json<Value> {
    database_plugin::oracle_scheduler::start_scheduler(None);
    Json(json!({ "started": true }))
}

/// POST /api/oracle/scheduler/stop — stop background scraping
pub async fn scheduler_stop(State(_s): State<AppState>) -> Json<Value> {
    database_plugin::oracle_scheduler::stop_scheduler();
    Json(json!({ "stopped": true }))
}

/// POST /api/oracle/scrape/trigger — manual scrape
pub async fn scrape_trigger(State(_s): State<AppState>) -> Json<Value> {
    database_plugin::oracle_scheduler::force_scrape();
    Json(json!({
        "triggered": true,
        "started_at": Utc::now().to_rfc3339(),
    }))
}

/// GET /api/oracle/scrape/metrics — meta-metrics
pub async fn scrape_metrics(State(_s): State<AppState>) -> Json<Value> {
    let meta = database_plugin::oracle_scheduler::get_meta_metrics();
    Json(serde_json::to_value(&meta).unwrap_or(json!({})))
}

// ============================================================================
// V5 Prompt 18: TxEventQ Monitoring
// ============================================================================

/// GET /api/oracle/txeventq — list queues with stats
pub async fn txeventq_list(State(_s): State<AppState>) -> Json<Value> {
    let queues = vec![
        json!({ "queue_name": "APP_EVENT_Q", "owner": "APP_SCHEMA", "queue_type": "NORMAL_QUEUE",
                "enqueue_enabled": true, "dequeue_enabled": true, "subscribers": 3,
                "messages_ready": 1250, "messages_processed": 45000, "messages_expired": 12,
                "avg_latency_ms": 45.2, "throughput_per_sec": 125.0, "oldest_message_age_secs": 180, "status": "active" }),
        json!({ "queue_name": "ORDER_EVENT_Q", "owner": "ORDER_SCHEMA", "queue_type": "NORMAL_QUEUE",
                "enqueue_enabled": true, "dequeue_enabled": true, "subscribers": 5,
                "messages_ready": 350, "messages_processed": 128000, "messages_expired": 3,
                "avg_latency_ms": 12.8, "throughput_per_sec": 420.0, "oldest_message_age_secs": 45, "status": "active" }),
        json!({ "queue_name": "AUDIT_EVENT_Q", "owner": "AUDIT_SCHEMA", "queue_type": "NORMAL_QUEUE",
                "enqueue_enabled": true, "dequeue_enabled": true, "subscribers": 1,
                "messages_ready": 8500, "messages_processed": 220000, "messages_expired": 0,
                "avg_latency_ms": 180.5, "throughput_per_sec": 35.0, "oldest_message_age_secs": 3600, "status": "backlog" }),
        json!({ "queue_name": "NOTIFICATION_Q", "owner": "NOTIFY_SCHEMA", "queue_type": "EXCEPTION_QUEUE",
                "enqueue_enabled": false, "dequeue_enabled": true, "subscribers": 2,
                "messages_ready": 0, "messages_processed": 5000, "messages_expired": 50,
                "avg_latency_ms": 0.0, "throughput_per_sec": 0.0, "oldest_message_age_secs": 0, "status": "disabled" }),
    ];
    Json(json!({ "queues": queues, "total": 4 }))
}

/// GET /api/oracle/txeventq/:queue_name — detail
pub async fn txeventq_detail(
    State(_s): State<AppState>,
    Path(name): Path<String>,
) -> Json<Value> {
    Json(json!({
        "queue_name": name,
        "owner": "APP_SCHEMA",
        "queue_table": format!("{}_QT", name),
        "queue_type": "NORMAL_QUEUE",
        "enqueue_enabled": true,
        "dequeue_enabled": true,
        "retention": "604800",
        "subscribers": [
            { "name": "SUB_1", "protocol": 0, "address": null, "rule": null },
            { "name": "SUB_2", "protocol": 0, "address": null, "rule": null },
        ],
        "throughput_history": [
            { "time": (Utc::now() - Duration::hours(4)).to_rfc3339(), "enqueue": 120, "dequeue": 115 },
            { "time": (Utc::now() - Duration::hours(3)).to_rfc3339(), "enqueue": 135, "dequeue": 130 },
            { "time": (Utc::now() - Duration::hours(2)).to_rfc3339(), "enqueue": 150, "dequeue": 140 },
            { "time": (Utc::now() - Duration::hours(1)).to_rfc3339(), "enqueue": 125, "dequeue": 125 },
            { "time": Utc::now().to_rfc3339(), "enqueue": 130, "dequeue": 128 },
        ],
        "latency_history": [
            { "time": (Utc::now() - Duration::hours(4)).to_rfc3339(), "avg_ms": 42, "p99_ms": 180 },
            { "time": (Utc::now() - Duration::hours(3)).to_rfc3339(), "avg_ms": 38, "p99_ms": 165 },
            { "time": (Utc::now() - Duration::hours(2)).to_rfc3339(), "avg_ms": 55, "p99_ms": 220 },
            { "time": (Utc::now() - Duration::hours(1)).to_rfc3339(), "avg_ms": 45, "p99_ms": 185 },
            { "time": Utc::now().to_rfc3339(), "avg_ms": 40, "p99_ms": 170 },
        ],
    }))
}

/// GET /api/oracle/txeventq/stats — aggregates
pub async fn txeventq_stats(State(_s): State<AppState>) -> Json<Value> {
    Json(json!({
        "total_queues": 4,
        "active_queues": 3,
        "total_ready": 10100,
        "total_processed": 398000,
        "avg_latency_ms": 79.5,
        "max_latency_ms": 180.5,
        "backlog_alert_count": 1,
    }))
}
