//! Unified Background Metric Collector
//!
//! Periodically polls ALL monitored targets (servers, network devices, databases,
//! storage arrays) and persists metrics into the `metrics` TimescaleDB hypertable
//! for historical analysis. Data persists until the target is removed from monitoring.

use zentra_core::ssh_utils;
use crate::state::AppState;
use crate::handlers::{
    middleware_nginx, middleware_apache, middleware_haproxy, middleware_tomcat,
    middleware_phpfpm, middleware_rabbitmq, middleware_kafka, middleware_elasticsearch,
    middleware_redis, middleware_memcached, middleware_docker, middleware_kubernetes,
    healthchecks,
};
use chrono::Utc;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Start the unified background poller. Spawns a tokio task that runs every 5 minutes.
pub fn start_metric_collector(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.tick().await; // skip immediate first tick
        info!("Unified metric collector started (every 5 min)");
        loop {
            interval.tick().await;
            let ts = Utc::now();
            debug!("Metric collector: polling all targets");

            let org_id = state.org_id;
            let pool = &state.pool;

            // Collect from all target types in parallel
            let (server_count, network_count, db_count, oracle_count,
                 mw_count, hc_count) = tokio::join!(
                collect_server_metrics(pool, org_id),
                collect_network_metrics(pool, org_id),
                collect_database_metrics(pool, org_id),
                collect_oracle_metrics(pool, org_id, &state),
                collect_middleware_metrics(pool, org_id, &state.http_client),
                collect_healthcheck_metrics(pool, org_id),
            );

            let elapsed = Utc::now().signed_duration_since(ts).num_milliseconds();
            info!(
                servers = server_count,
                network = network_count,
                databases = db_count,
                oracle = oracle_count,
                middleware = mw_count,
                healthchecks = hc_count,
                elapsed_ms = elapsed,
                "Metric collector cycle complete"
            );
        }
    });
}

/// Insert a batch of metrics into the existing `metrics` hypertable.
async fn insert_metrics(
    pool: &PgPool,
    org_id: Uuid,
    target_id: Uuid,
    plugin_id: &str,
    resource_type: &str,
    resource_id: &str,
    metrics: &[(&str, f64, &str)], // (metric_name, value, unit)
) {
    let now = Utc::now();
    for (metric_name, value, unit) in metrics {
        let _ = sqlx::query(
            "INSERT INTO metrics (timestamp, org_id, target_id, plugin_id, resource_type, \
             resource_id, metric_name, value, unit, tags, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, '{}'::jsonb, NULL)"
        )
        .bind(now)
        .bind(org_id)
        .bind(target_id)
        .bind(plugin_id)
        .bind(resource_type)
        .bind(resource_id)
        .bind(metric_name)
        .bind(*value)
        .bind(*unit)
        .execute(pool)
        .await
        .map_err(|e| error!(target_id = %target_id, metric = metric_name, error = %e, "Failed to insert metric"));
    }
}

// ============================================================================
// Server Metrics (SSH)
// ============================================================================

async fn collect_server_metrics(pool: &PgPool, org_id: Uuid) -> usize {
    let rows = sqlx::query_as::<_, (Uuid, String, String, i32, Option<String>)>(
        "SELECT target_id, name, host, port, \
         credentials->>'username' \
         FROM targets WHERE org_id = $1 AND target_type = 'server' AND is_active = true"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut count = 0usize;
    for (target_id, name, host, _port, username) in &rows {
        let user = username.as_deref().unwrap_or("root");
        let stats = ssh_utils::collect_server_stats_pooled(&host, user).await;

        if stats.contains_key("ERROR") {
            debug!(target = %name, "Server unreachable, skipping metrics");
            continue;
        }

        let cpu: f64 = stats.get("CPU_USAGE").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        let load_parts: Vec<f64> = stats.get("LOAD_AVG")
            .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_default();
        let mem_parts: Vec<f64> = stats.get("MEMORY")
            .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_default();
        let mem_total = mem_parts.first().copied().unwrap_or(0.0);
        let mem_used = mem_parts.get(1).copied().unwrap_or(0.0);
        let mem_pct = if mem_total > 0.0 { (mem_used / mem_total) * 100.0 } else { 0.0 };

        let swap_parts: Vec<f64> = stats.get("SWAP")
            .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
            .unwrap_or_default();
        let swap_total = swap_parts.first().copied().unwrap_or(0.0);
        let swap_used = swap_parts.get(1).copied().unwrap_or(0.0);

        let disk_raw = stats.get("DISK").cloned().unwrap_or_default();
        let disk_parts: Vec<&str> = disk_raw.trim().split_whitespace().collect();
        let disk_total: f64 = disk_parts.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let disk_used: f64 = disk_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let disk_pct: f64 = disk_parts.get(2)
            .and_then(|s| s.trim_end_matches('%').parse().ok())
            .unwrap_or(0.0);

        let uptime: f64 = stats.get("UPTIME").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
        let procs: f64 = stats.get("PROCS").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);

        let metrics: Vec<(&str, f64, &str)> = vec![
            ("server.cpu.usage_percent", cpu, "percent"),
            ("server.load.1m", load_parts.first().copied().unwrap_or(0.0), ""),
            ("server.load.5m", load_parts.get(1).copied().unwrap_or(0.0), ""),
            ("server.load.15m", load_parts.get(2).copied().unwrap_or(0.0), ""),
            ("server.memory.total_mb", mem_total, "MB"),
            ("server.memory.used_mb", mem_used, "MB"),
            ("server.memory.usage_percent", mem_pct, "percent"),
            ("server.swap.total_mb", swap_total, "MB"),
            ("server.swap.used_mb", swap_used, "MB"),
            ("server.disk.total_gb", disk_total, "GB"),
            ("server.disk.used_gb", disk_used, "GB"),
            ("server.disk.usage_percent", disk_pct, "percent"),
            ("server.uptime_seconds", uptime, "seconds"),
            ("server.process_count", procs, "count"),
        ];

        insert_metrics(pool, org_id, *target_id, "server-collector", "server", &host, &metrics).await;
        count += 1;
    }
    count
}

// ============================================================================
// Network Device Metrics (SNMP)
// ============================================================================

async fn collect_network_metrics(pool: &PgPool, org_id: Uuid) -> usize {
    let rows = sqlx::query_as::<_, (Uuid, String, String, i32, String)>(
        "SELECT target_id, name, host, port, target_subtype \
         FROM targets WHERE org_id = $1 AND target_type = 'network' AND is_active = true"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        return 0;
    }

    let ssh_host = std::env::var("ZENTRA_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let ssh_user = std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into());

    // Build parallel SNMP poll script for all devices
    let mut script_parts = vec!["TMPDIR=$(mktemp -d)".to_string()];
    for (i, (_, _, host, port, _)) in rows.iter().enumerate() {
        let snmp_target = if *port != 161 { format!("{}:{}", host, port) } else { host.clone() };
        // Collect: sysDescr, sysUpTime, ifNumber, ifInOctets.1, ifOutOctets.1
        script_parts.push(format!(
            "{{ \
               echo '@@DEV_{}@@'; \
               snmpget -v2c -c public -t2 -r0 {} \
                 .1.3.6.1.2.1.1.1.0 \
                 .1.3.6.1.2.1.1.3.0 \
                 .1.3.6.1.2.1.2.1.0 \
                 .1.3.6.1.2.1.2.2.1.10.1 \
                 .1.3.6.1.2.1.2.2.1.16.1 \
                 2>&1; \
               echo '@@END_{}@@'; \
             }} > $TMPDIR/dev_{} 2>&1 &",
            i, snmp_target, i, i
        ));
    }
    script_parts.push("wait".to_string());
    for (i, _) in rows.iter().enumerate() {
        script_parts.push(format!("cat $TMPDIR/dev_{}", i));
    }
    script_parts.push("rm -rf $TMPDIR".to_string());

    let bulk_res = zentra_core::ssh_pool::pooled_ssh_exec(&ssh_host, &ssh_user, &script_parts.join("\n")).await;
    let bulk_out = bulk_res.stdout;

    let mut count = 0usize;
    for (i, (target_id, name, host, _port, _subtype)) in rows.iter().enumerate() {
        let dev_marker = format!("@@DEV_{}@@", i);
        let end_marker = format!("@@END_{}@@", i);

        let section = bulk_out.split(&dev_marker).nth(1)
            .and_then(|s| s.split(&end_marker).next())
            .unwrap_or("");

        let snmp_ok = section.contains("STRING:") || section.contains("Timeticks:");
        let reachable: f64 = if snmp_ok { 1.0 } else { 0.0 };

        // Parse uptime timeticks
        let uptime_secs: f64 = section.lines()
            .find(|l| l.contains("Timeticks:"))
            .and_then(|l| {
                // Format: (12345678) 1 day, 2:03:04.56
                l.split('(').nth(1)
                    .and_then(|s| s.split(')').next())
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .map(|t| t / 100.0) // centiseconds to seconds
            })
            .unwrap_or(0.0);

        // Parse interface count
        let if_count: f64 = section.lines()
            .find(|l| l.contains(".2.1.0") && l.contains("INTEGER:"))
            .and_then(|l| l.split("INTEGER:").nth(1))
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);

        // Parse ifInOctets.1 and ifOutOctets.1
        let in_octets: f64 = section.lines()
            .find(|l| l.contains(".2.2.1.10.1"))
            .and_then(|l| {
                l.split("Counter32:").nth(1)
                    .or_else(|| l.split("Counter64:").nth(1))
                    .or_else(|| l.split("INTEGER:").nth(1))
            })
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);

        let out_octets: f64 = section.lines()
            .find(|l| l.contains(".2.2.1.16.1"))
            .and_then(|l| {
                l.split("Counter32:").nth(1)
                    .or_else(|| l.split("Counter64:").nth(1))
                    .or_else(|| l.split("INTEGER:").nth(1))
            })
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);

        let metrics: Vec<(&str, f64, &str)> = vec![
            ("network.device.reachable", reachable, "bool"),
            ("network.device.uptime_seconds", uptime_secs, "seconds"),
            ("network.device.interface_count", if_count, "count"),
            ("network.device.if_in_octets", in_octets, "bytes"),
            ("network.device.if_out_octets", out_octets, "bytes"),
        ];

        insert_metrics(pool, org_id, *target_id, "network-collector", "network", host, &metrics).await;

        // Update target status
        let status = if snmp_ok { "healthy" } else { "unreachable" };
        let _ = sqlx::query("UPDATE targets SET status = $1, last_check_at = NOW() WHERE target_id = $2")
            .bind(status).bind(target_id).execute(pool).await;

        count += 1;
    }
    count
}

// ============================================================================
// Database Metrics (PostgreSQL via SQL)
// ============================================================================

async fn collect_database_metrics(pool: &PgPool, org_id: Uuid) -> usize {
    let targets = zentra_db::repositories::targets::list_targets(pool, org_id, Some("database"), true)
        .await
        .unwrap_or_default();

    fn url_encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                _ => out.push_str(&format!("%{:02X}", b)),
            }
        }
        out
    }

    let mut count = 0usize;
    for t in &targets {
        let subtype = t.target_subtype.as_str();
        if subtype != "postgresql" {
            continue;
        }

        let creds = t.credentials.as_ref();
        let user = creds.and_then(|c| c.username.clone()).unwrap_or_else(|| "postgres".into());
        let pass = creds.map(|c| c.decrypt_password()).unwrap_or_default();
        let db = creds.and_then(|c| c.database_name.clone()).unwrap_or_else(|| "postgres".into());

        let url = format!(
            "postgres://{}:{}@{}:{}/{}?connect_timeout=5",
            url_encode(&user), url_encode(&pass), t.host, t.port, url_encode(&db),
        );

        let target_pool = match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                debug!(target = %t.name, error = %e, "Database unreachable, skipping metrics");
                let _ = sqlx::query("UPDATE targets SET status = 'unreachable', last_check_at = NOW() WHERE target_id = $1")
                    .bind(t.target_id).execute(pool).await;
                continue;
            }
        };

        let target_id = t.target_id;
        let host = &t.host;
        let name = &t.name;

        // Collect key PostgreSQL metrics
        let mut metrics: Vec<(&str, f64, &str)> = vec![];

        // Connection counts
        if let Ok(row) = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT \
             (SELECT count(*) FROM pg_stat_activity)::bigint, \
             (SELECT count(*) FROM pg_stat_activity WHERE state = 'active')::bigint, \
             (SELECT setting::bigint FROM pg_settings WHERE name = 'max_connections')"
        ).fetch_one(&target_pool).await {
            metrics.push(("db.connections.total", row.0 as f64, "count"));
            metrics.push(("db.connections.active", row.1 as f64, "count"));
            metrics.push(("db.connections.max", row.2 as f64, "count"));
            metrics.push(("db.connections.usage_percent",
                if row.2 > 0 { (row.0 as f64 / row.2 as f64) * 100.0 } else { 0.0 }, "percent"));
        }

        // Database size, cache hit ratio, transactions, deadlocks
        if let Ok(row) = sqlx::query_as::<_, (i64, f64, i64, i64, i64, i64)>(
            "SELECT \
             pg_database_size(current_database())::bigint, \
             COALESCE(ROUND(100.0 * sum(blks_hit) / NULLIF(sum(blks_hit) + sum(blks_read), 0), 2), 0) AS cache_hit, \
             COALESCE(sum(xact_commit), 0)::bigint, \
             COALESCE(sum(xact_rollback), 0)::bigint, \
             COALESCE(sum(deadlocks), 0)::bigint, \
             COALESCE(sum(temp_files), 0)::bigint \
             FROM pg_stat_database WHERE datname = current_database()"
        ).fetch_one(&target_pool).await {
            metrics.push(("db.size_bytes", row.0 as f64, "bytes"));
            metrics.push(("db.cache_hit_ratio", row.1, "percent"));
            metrics.push(("db.transactions.committed", row.2 as f64, "count"));
            metrics.push(("db.transactions.rolled_back", row.3 as f64, "count"));
            metrics.push(("db.deadlocks", row.4 as f64, "count"));
            metrics.push(("db.temp_files", row.5 as f64, "count"));
        }

        // Replication lag (if applicable)
        if let Ok(Some(lag_opt)) = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT EXTRACT(EPOCH FROM replay_lag)::bigint FROM pg_stat_replication LIMIT 1"
        ).fetch_optional(&target_pool).await {
            if let Some(lag_secs) = lag_opt {
                metrics.push(("db.replication.lag_seconds", lag_secs as f64, "seconds"));
            }
        }

        // Table and index bloat indicator: total live tuples and dead tuples
        if let Ok(row) = sqlx::query_as::<_, (i64, i64)>(
            "SELECT COALESCE(sum(n_live_tup), 0)::bigint, COALESCE(sum(n_dead_tup), 0)::bigint \
             FROM pg_stat_user_tables"
        ).fetch_one(&target_pool).await {
            metrics.push(("db.tuples.live", row.0 as f64, "count"));
            metrics.push(("db.tuples.dead", row.1 as f64, "count"));
        }

        insert_metrics(pool, org_id, target_id, "database-collector", "database", host, &metrics).await;

        let _ = sqlx::query("UPDATE targets SET status = 'healthy', last_check_at = NOW() WHERE target_id = $1")
            .bind(target_id).execute(pool).await;

        target_pool.close().await;
        count += 1;
    }
    count
}

// ============================================================================
// Oracle Metrics (sqlplus via SSH or local)
// ============================================================================

async fn collect_oracle_metrics(pool: &PgPool, org_id: Uuid, state: &AppState) -> usize {
    let targets = zentra_db::repositories::targets::list_targets(pool, org_id, Some("database"), true)
        .await
        .unwrap_or_default();

    let ssh_user = std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into());
    let mut count = 0usize;

    for t in &targets {
        if t.target_subtype.as_str() != "oracle" { continue; }

        let creds = t.credentials.as_ref();
        let db_user = creds.and_then(|c| c.username.clone()).unwrap_or_else(|| "system".into());
        let db_pass = creds.map(|c| c.decrypt_password()).unwrap_or_default();
        let service = creds.and_then(|c| c.database_name.clone()).unwrap_or_else(|| "ORCL".into());

        // Batched SQL: each SELECT outputs M|<metric_name>|<value> for easy parsing.
        // Covers: instance, sessions, memory, I/O, redo, waits, cache, tablespace, sysmetric.
        let sql_script = r#"SET PAGESIZE 0
SET FEEDBACK OFF
SET HEADING OFF
SET LINESIZE 1000
SET TRIMOUT ON
SET TRIMSPOOL ON
-- Instance
SELECT 'M|uptime_seconds|'||ROUND((SYSDATE-startup_time)*86400) FROM v$instance;
-- Sessions
SELECT 'M|sessions_total|'||COUNT(*) FROM v$session WHERE type='USER';
SELECT 'M|sessions_active|'||COUNT(*) FROM v$session WHERE status='ACTIVE' AND type='USER';
SELECT 'M|sessions_inactive|'||COUNT(*) FROM v$session WHERE status='INACTIVE' AND type='USER';
SELECT 'M|sessions_blocked|'||COUNT(*) FROM v$session WHERE blocking_session IS NOT NULL;
-- Processes
SELECT 'M|processes_count|'||COUNT(*) FROM v$process;
SELECT 'M|processes_limit|'||value FROM v$parameter WHERE name='processes';
-- Memory
SELECT 'M|sga_size_mb|'||ROUND(SUM(value)/1048576) FROM v$sga;
SELECT 'M|pga_allocated_mb|'||ROUND(value/1048576) FROM v$pgastat WHERE name='total PGA allocated';
SELECT 'M|pga_target_mb|'||ROUND(value/1048576) FROM v$pgastat WHERE name='aggregate PGA target parameter';
SELECT 'M|shared_pool_free_mb|'||ROUND(SUM(bytes)/1048576) FROM v$sgastat WHERE pool='shared pool' AND name='free memory';
-- Cache hit ratios
SELECT 'M|buffer_cache_hit_pct|'||ROUND((1-(SELECT value FROM v$sysstat WHERE name='physical reads')/(NULLIF((SELECT value FROM v$sysstat WHERE name='db block gets')+(SELECT value FROM v$sysstat WHERE name='consistent gets'),0)))*100,2) FROM dual;
SELECT 'M|library_cache_hit_pct|'||ROUND((1-SUM(reloads)/NULLIF(SUM(pins),0))*100,2) FROM v$librarycache;
SELECT 'M|dict_cache_hit_pct|'||ROUND((1-SUM(getmisses)/NULLIF(SUM(gets),0))*100,2) FROM v$rowcache;
SELECT 'M|redo_buffer_alloc_retries|'||value FROM v$sysstat WHERE name='redo buffer allocation retries';
-- I/O
SELECT 'M|physical_reads|'||value FROM v$sysstat WHERE name='physical reads';
SELECT 'M|physical_writes|'||value FROM v$sysstat WHERE name='physical writes';
SELECT 'M|physical_read_bytes|'||value FROM v$sysstat WHERE name='physical read total bytes';
SELECT 'M|physical_write_bytes|'||value FROM v$sysstat WHERE name='physical write total bytes';
SELECT 'M|db_block_gets|'||value FROM v$sysstat WHERE name='db block gets';
SELECT 'M|consistent_gets|'||value FROM v$sysstat WHERE name='consistent gets';
-- Redo
SELECT 'M|redo_size|'||value FROM v$sysstat WHERE name='redo size';
SELECT 'M|redo_writes|'||value FROM v$sysstat WHERE name='redo writes';
SELECT 'M|redo_log_space_requests|'||value FROM v$sysstat WHERE name='redo log space requests';
-- Sorts
SELECT 'M|sorts_memory|'||value FROM v$sysstat WHERE name='sorts (memory)';
SELECT 'M|sorts_disk|'||value FROM v$sysstat WHERE name='sorts (disk)';
-- Parse
SELECT 'M|parse_count_total|'||value FROM v$sysstat WHERE name='parse count (total)';
SELECT 'M|parse_count_hard|'||value FROM v$sysstat WHERE name='parse count (hard)';
SELECT 'M|user_commits|'||value FROM v$sysstat WHERE name='user commits';
SELECT 'M|user_rollbacks|'||value FROM v$sysstat WHERE name='user rollbacks';
SELECT 'M|execute_count|'||value FROM v$sysstat WHERE name='execute count';
-- Wait time
SELECT 'M|wait_db_cpu_ms|'||ROUND(value/1000) FROM v$sys_time_model WHERE stat_name='DB CPU';
SELECT 'M|wait_db_time_ms|'||ROUND(value/1000) FROM v$sys_time_model WHERE stat_name='DB time';
-- Tablespace: total_mb, used_mb, max_used_pct
SELECT 'M|tablespace_total_mb|'||ROUND(SUM(bytes)/1048576) FROM dba_data_files;
SELECT 'M|tablespace_used_mb|'||ROUND(SUM(bytes)/1048576) FROM dba_segments;
SELECT 'M|tablespace_max_used_pct|'||ROUND(MAX(used_percent),1) FROM dba_tablespace_usage_metrics;
-- Temp usage
SELECT 'M|temp_used_mb|'||ROUND(NVL(SUM(blocks*block_size)/1048576,0)) FROM v$tempseg_usage;
-- Undo usage
SELECT 'M|undo_used_mb|'||ROUND(SUM(bytes)/1048576) FROM dba_undo_extents WHERE status='ACTIVE';
-- Key sysmetric values (per-second rates, last 60s window)
SELECT 'M|sysmetric_cpu_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='CPU Usage Per Sec' AND group_id=2;
SELECT 'M|sysmetric_db_time_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='DB Time Per Sec' AND group_id=2;
SELECT 'M|sysmetric_logons_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Logons Per Sec' AND group_id=2;
SELECT 'M|sysmetric_executions_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Executions Per Sec' AND group_id=2;
SELECT 'M|sysmetric_hard_parse_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Hard Parse Count Per Sec' AND group_id=2;
SELECT 'M|sysmetric_phys_reads_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Physical Reads Per Sec' AND group_id=2;
SELECT 'M|sysmetric_phys_writes_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Physical Writes Per Sec' AND group_id=2;
SELECT 'M|sysmetric_redo_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Redo Generated Per Sec' AND group_id=2;
SELECT 'M|sysmetric_user_commits_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='User Commits Per Sec' AND group_id=2;
SELECT 'M|sysmetric_user_calls_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='User Calls Per Sec' AND group_id=2;
SELECT 'M|sysmetric_io_mb_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='I/O Megabytes per Second' AND group_id=2;
SELECT 'M|sysmetric_io_requests_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='I/O Requests per Second' AND group_id=2;
SELECT 'M|sysmetric_db_block_changes_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='DB Block Changes Per Sec' AND group_id=2;
SELECT 'M|sysmetric_gc_cr_blocks_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='GC CR Block Received Per Second' AND group_id=2;
SELECT 'M|sysmetric_host_cpu_pct|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Host CPU Utilization (%)' AND group_id=2;
SELECT 'M|sysmetric_network_bytes_per_sec|'||ROUND(value,2) FROM v$sysmetric WHERE metric_name='Network Traffic Volume Per Sec' AND group_id=2;
-- DataGuard lag (0 if not standby)
SELECT 'M|dg_apply_lag_seconds|'||NVL((SELECT EXTRACT(DAY FROM TO_DSINTERVAL(value))*86400+EXTRACT(HOUR FROM TO_DSINTERVAL(value))*3600+EXTRACT(MINUTE FROM TO_DSINTERVAL(value))*60+EXTRACT(SECOND FROM TO_DSINTERVAL(value)) FROM v$dataguard_stats WHERE name='apply lag'),0) FROM dual;
EXIT;
"#;

        // Try local sqlplus first (via Oracle connection pool), fall back to SSH
        let local_cid: Option<String> = {
            let conns = crate::handlers::oracle::oracle_connections().read();
            // Match by host, or by service name, or just use any active Oracle connection
            conns.iter().find(|c| c.is_active && (c.host == t.host || c.service_name.to_lowercase() == service.to_lowercase()))
                .or_else(|| conns.iter().find(|c| c.is_active))
                .map(|c| c.id.clone())
        };
        let output = match local_cid {
            Some(ref cid) => crate::handlers::oracle::oracle_local_query(cid, sql_script).await.ok(),
            None => None,
        };

        let output = match output {
            Some(o) if !o.trim().is_empty() => o,
            _ => {
                // Fall back to SSH — use single-quoted heredoc so $ is literal (no shell expansion)
                let conn_str = format!("{}/{}@//{}:{}/{}", db_user, db_pass, t.host, t.port, service);
                let cmd = format!(
                    "sqlplus -S {} <<'EOSQL'\n{}\nEOSQL",
                    conn_str, sql_script
                );
                let result = zentra_core::ssh_pool::pooled_ssh_exec(&t.host, &ssh_user, &cmd).await;
                if !result.success || result.stdout.trim().is_empty() {
                    debug!(target = %t.name, stderr = %result.stderr, "Oracle unreachable, skipping metrics");
                    let _ = sqlx::query("UPDATE targets SET status = 'unreachable', last_check_at = NOW() WHERE target_id = $1")
                        .bind(t.target_id).execute(pool).await;
                    continue;
                }
                result.stdout
            }
        };

        // Parse M|<name>|<value> lines
        let mut kv: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for line in output.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("M|") {
                if let Some((name, val_str)) = rest.split_once('|') {
                    if let Ok(v) = val_str.trim().parse::<f64>() {
                        kv.insert(name.to_string(), v);
                    }
                }
            }
        }

        if kv.is_empty() {
            debug!(target = %t.name, "Oracle: no metrics parsed from output");
            continue;
        }

        let g = |key: &str| -> f64 { kv.get(key).copied().unwrap_or(0.0) };

        let metrics: Vec<(&str, f64, &str)> = vec![
            // Instance
            ("oracle.uptime_seconds", g("uptime_seconds"), "seconds"),
            // Sessions
            ("oracle.sessions_total", g("sessions_total"), "count"),
            ("oracle.sessions_active", g("sessions_active"), "count"),
            ("oracle.sessions_inactive", g("sessions_inactive"), "count"),
            ("oracle.sessions_blocked", g("sessions_blocked"), "count"),
            // Processes
            ("oracle.processes_count", g("processes_count"), "count"),
            ("oracle.processes_limit", g("processes_limit"), "count"),
            // Memory
            ("oracle.sga_size_mb", g("sga_size_mb"), "MB"),
            ("oracle.pga_allocated_mb", g("pga_allocated_mb"), "MB"),
            ("oracle.pga_target_mb", g("pga_target_mb"), "MB"),
            ("oracle.shared_pool_free_mb", g("shared_pool_free_mb"), "MB"),
            // Cache hit ratios
            ("oracle.buffer_cache_hit_pct", g("buffer_cache_hit_pct"), "percent"),
            ("oracle.library_cache_hit_pct", g("library_cache_hit_pct"), "percent"),
            ("oracle.dict_cache_hit_pct", g("dict_cache_hit_pct"), "percent"),
            ("oracle.redo_buffer_alloc_retries", g("redo_buffer_alloc_retries"), "count"),
            // I/O cumulative
            ("oracle.physical_reads", g("physical_reads"), "count"),
            ("oracle.physical_writes", g("physical_writes"), "count"),
            ("oracle.physical_read_bytes", g("physical_read_bytes"), "bytes"),
            ("oracle.physical_write_bytes", g("physical_write_bytes"), "bytes"),
            ("oracle.db_block_gets", g("db_block_gets"), "count"),
            ("oracle.consistent_gets", g("consistent_gets"), "count"),
            // Redo cumulative
            ("oracle.redo_size", g("redo_size"), "bytes"),
            ("oracle.redo_writes", g("redo_writes"), "count"),
            ("oracle.redo_log_space_requests", g("redo_log_space_requests"), "count"),
            // Sorts
            ("oracle.sorts_memory", g("sorts_memory"), "count"),
            ("oracle.sorts_disk", g("sorts_disk"), "count"),
            // Parse / execute
            ("oracle.parse_count_total", g("parse_count_total"), "count"),
            ("oracle.parse_count_hard", g("parse_count_hard"), "count"),
            ("oracle.user_commits", g("user_commits"), "count"),
            ("oracle.user_rollbacks", g("user_rollbacks"), "count"),
            ("oracle.execute_count", g("execute_count"), "count"),
            // Wait / time model
            ("oracle.wait_db_cpu_ms", g("wait_db_cpu_ms"), "ms"),
            ("oracle.wait_db_time_ms", g("wait_db_time_ms"), "ms"),
            // Tablespace
            ("oracle.tablespace_total_mb", g("tablespace_total_mb"), "MB"),
            ("oracle.tablespace_used_mb", g("tablespace_used_mb"), "MB"),
            ("oracle.tablespace_max_used_pct", g("tablespace_max_used_pct"), "percent"),
            // Temp & undo
            ("oracle.temp_used_mb", g("temp_used_mb"), "MB"),
            ("oracle.undo_used_mb", g("undo_used_mb"), "MB"),
            // Sysmetric rates (per second, last 60s)
            ("oracle.cpu_per_sec", g("sysmetric_cpu_per_sec"), "centiseconds"),
            ("oracle.db_time_per_sec", g("sysmetric_db_time_per_sec"), "centiseconds"),
            ("oracle.logons_per_sec", g("sysmetric_logons_per_sec"), "ops"),
            ("oracle.executions_per_sec", g("sysmetric_executions_per_sec"), "ops"),
            ("oracle.hard_parse_per_sec", g("sysmetric_hard_parse_per_sec"), "ops"),
            ("oracle.phys_reads_per_sec", g("sysmetric_phys_reads_per_sec"), "ops"),
            ("oracle.phys_writes_per_sec", g("sysmetric_phys_writes_per_sec"), "ops"),
            ("oracle.redo_per_sec", g("sysmetric_redo_per_sec"), "bytes"),
            ("oracle.user_commits_per_sec", g("sysmetric_user_commits_per_sec"), "ops"),
            ("oracle.user_calls_per_sec", g("sysmetric_user_calls_per_sec"), "ops"),
            ("oracle.io_mb_per_sec", g("sysmetric_io_mb_per_sec"), "MB"),
            ("oracle.io_requests_per_sec", g("sysmetric_io_requests_per_sec"), "ops"),
            ("oracle.db_block_changes_per_sec", g("sysmetric_db_block_changes_per_sec"), "ops"),
            ("oracle.gc_cr_blocks_per_sec", g("sysmetric_gc_cr_blocks_per_sec"), "ops"),
            ("oracle.host_cpu_pct", g("sysmetric_host_cpu_pct"), "percent"),
            ("oracle.network_bytes_per_sec", g("sysmetric_network_bytes_per_sec"), "bytes"),
            // DataGuard
            ("oracle.dg_apply_lag_seconds", g("dg_apply_lag_seconds"), "seconds"),
        ];

        insert_metrics(pool, org_id, t.target_id, "oracle-collector", "database", &t.host, &metrics).await;
        let _ = sqlx::query("UPDATE targets SET status = 'healthy', last_check_at = NOW() WHERE target_id = $1")
            .bind(t.target_id).execute(pool).await;
        info!(target = %t.name, metrics_count = metrics.len(), "Oracle metrics collected");
        count += 1;
    }
    count
}

// ============================================================================
// Middleware Metrics (Nginx, Apache, HAProxy, Tomcat, PHP-FPM, RabbitMQ,
//                     Kafka, Elasticsearch, Redis, Memcached, Docker, K8s)
// ============================================================================

/// Generate a stable target_id from middleware instance id (deterministic UUID v5).
/// Public so middleware handlers can compute the UUID to query their own persisted history.
pub fn middleware_uuid(ns: &str, id: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("{}:{}", ns, id).as_bytes())
}

async fn collect_middleware_metrics(pool: &PgPool, org_id: Uuid, http_client: &reqwest::Client) -> usize {
    let mut count = 0usize;

    // ---- Nginx ----
    {
        let insts = middleware_nginx::live_instances().read().clone();
        let client = http_client;
        for inst in &insts {
            let target_id = middleware_uuid("nginx", &inst.id);
            let url = format!("http://{}:{}{}", inst.host, inst.port, inst.stub_status_path);
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    let mut active: f64 = 0.0; let mut requests: f64 = 0.0;
                    let mut reading: f64 = 0.0; let mut writing: f64 = 0.0; let mut waiting: f64 = 0.0;
                    let mut accepts: f64 = 0.0; let mut handled: f64 = 0.0;
                    for line in body.lines() {
                        let line = line.trim();
                        if line.starts_with("Active connections:") {
                            active = line.split(':').nth(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
                        } else if line.starts_with("Reading:") {
                            let p: Vec<&str> = line.split_whitespace().collect();
                            reading = p.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                            writing = p.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                            waiting = p.get(5).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                        } else if let Ok(vals) = line.split_whitespace().map(|v| v.parse::<f64>()).collect::<Result<Vec<f64>,_>>() {
                            if vals.len() == 3 { accepts = vals[0]; handled = vals[1]; requests = vals[2]; }
                        }
                    }
                    let m: Vec<(&str, f64, &str)> = vec![
                        ("nginx.active_connections", active, "count"),
                        ("nginx.accepts", accepts, "count"),
                        ("nginx.handled", handled, "count"),
                        ("nginx.requests", requests, "count"),
                        ("nginx.reading", reading, "count"),
                        ("nginx.writing", writing, "count"),
                        ("nginx.waiting", waiting, "count"),
                    ];
                    insert_metrics(pool, org_id, target_id, "nginx-collector", "middleware", &inst.host, &m).await;
                    count += 1;
                }
            }
        }
    }

    // ---- Apache ----
    {
        let insts = middleware_apache::live_instances().read().clone();
        let client = http_client;
        for inst in &insts {
            let target_id = middleware_uuid("apache", &inst.id);
            let url = format!("http://{}:{}/server-status?auto", inst.host, inst.port);
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    let mut busy: f64 = 0.0; let mut idle: f64 = 0.0;
                    let mut rps: f64 = 0.0; let mut bps: f64 = 0.0; let mut cpu: f64 = 0.0;
                    for line in body.lines() {
                        if let Some((k, v)) = line.split_once(':') {
                            match k.trim() {
                                "BusyWorkers" => busy = v.trim().parse().unwrap_or(0.0),
                                "IdleWorkers" => idle = v.trim().parse().unwrap_or(0.0),
                                "ReqPerSec" => rps = v.trim().parse().unwrap_or(0.0),
                                "BytesPerSec" => bps = v.trim().parse().unwrap_or(0.0),
                                "CPULoad" => cpu = v.trim().parse().unwrap_or(0.0),
                                _ => {}
                            }
                        }
                    }
                    let m: Vec<(&str, f64, &str)> = vec![
                        ("apache.busy_workers", busy, "count"),
                        ("apache.idle_workers", idle, "count"),
                        ("apache.requests_per_sec", rps, "ops"),
                        ("apache.bytes_per_sec", bps, "bytes"),
                        ("apache.cpu_load", cpu, "percent"),
                    ];
                    insert_metrics(pool, org_id, target_id, "apache-collector", "middleware", &inst.host, &m).await;
                    count += 1;
                }
            }
        }
    }

    // ---- HAProxy ----
    {
        let insts = middleware_haproxy::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("haproxy", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("haproxy.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "haproxy-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    // ---- Tomcat ----
    {
        let insts = middleware_tomcat::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("tomcat", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("tomcat.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "tomcat-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    // ---- PHP-FPM ----
    {
        let insts = middleware_phpfpm::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("phpfpm", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("phpfpm.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "phpfpm-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    // ---- RabbitMQ ----
    {
        let insts = middleware_rabbitmq::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("rabbitmq", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("rabbitmq.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "rabbitmq-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    // ---- Kafka ----
    {
        let insts = middleware_kafka::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("kafka", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("kafka.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "kafka-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    // ---- Elasticsearch ----
    {
        let insts = middleware_elasticsearch::live_instances().read().clone();
        let client = http_client;
        for inst in &insts {
            let target_id = middleware_uuid("elasticsearch", &inst.id);
            let url = format!("http://{}:{}/_cluster/health", inst.host, inst.port);
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    let status_val = match body["status"].as_str().unwrap_or("unknown") {
                        "green" => 2.0, "yellow" => 1.0, _ => 0.0,
                    };
                    let m: Vec<(&str, f64, &str)> = vec![
                        ("elasticsearch.cluster_status", status_val, "enum"),
                        ("elasticsearch.nodes", body["number_of_nodes"].as_f64().unwrap_or(0.0), "count"),
                        ("elasticsearch.active_shards", body["active_shards"].as_f64().unwrap_or(0.0), "count"),
                        ("elasticsearch.unassigned_shards", body["unassigned_shards"].as_f64().unwrap_or(0.0), "count"),
                    ];
                    insert_metrics(pool, org_id, target_id, "es-collector", "middleware", &inst.host, &m).await;
                    count += 1;
                }
            }
        }
    }

    // ---- Redis ----
    {
        let insts = middleware_redis::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("redis", &inst.id);
            // Try connecting to Redis via TCP and sending INFO
            if let Ok(mut stream) = tokio::net::TcpStream::connect(format!("{}:{}", inst.host, inst.port)).await {
                use tokio::io::{AsyncWriteExt, AsyncReadExt};
                let _ = stream.write_all(b"INFO\r\n").await;
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let info = String::from_utf8_lossy(&buf[..n]);

                let get_info = |key: &str| -> f64 {
                    for line in info.lines() {
                        if let Some(rest) = line.strip_prefix(&format!("{}:", key)) {
                            return rest.trim().parse().unwrap_or(0.0);
                        }
                    }
                    0.0
                };

                let hits = get_info("keyspace_hits");
                let misses = get_info("keyspace_misses");
                let hit_rate = if hits + misses > 0.0 { hits / (hits + misses) * 100.0 } else { 0.0 };

                let m: Vec<(&str, f64, &str)> = vec![
                    ("redis.connected_clients", get_info("connected_clients"), "count"),
                    ("redis.used_memory_mb", get_info("used_memory") / 1048576.0, "MB"),
                    ("redis.ops_per_sec", get_info("instantaneous_ops_per_sec"), "ops"),
                    ("redis.hit_rate", hit_rate, "percent"),
                    ("redis.connected_slaves", get_info("connected_slaves"), "count"),
                    ("redis.uptime_seconds", get_info("uptime_in_seconds"), "seconds"),
                ];
                insert_metrics(pool, org_id, target_id, "redis-collector", "middleware", &inst.host, &m).await;
                count += 1;
            }
        }
    }

    // ---- Memcached ----
    {
        let insts = middleware_memcached::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("memcached", &inst.id);
            if let Ok(mut stream) = tokio::net::TcpStream::connect(format!("{}:{}", inst.host, inst.port)).await {
                use tokio::io::{AsyncWriteExt, AsyncReadExt};
                let _ = stream.write_all(b"stats\r\n").await;
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let stats = String::from_utf8_lossy(&buf[..n]);

                let get_stat = |key: &str| -> f64 {
                    for line in stats.lines() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 && parts[1] == key {
                            return parts[2].trim().parse().unwrap_or(0.0);
                        }
                    }
                    0.0
                };

                let hits = get_stat("get_hits");
                let misses = get_stat("get_misses");
                let hit_rate = if hits + misses > 0.0 { hits / (hits + misses) * 100.0 } else { 0.0 };

                let m: Vec<(&str, f64, &str)> = vec![
                    ("memcached.curr_connections", get_stat("curr_connections"), "count"),
                    ("memcached.curr_items", get_stat("curr_items"), "count"),
                    ("memcached.bytes_used_mb", get_stat("bytes") / 1048576.0, "MB"),
                    ("memcached.hit_rate", hit_rate, "percent"),
                    ("memcached.evictions", get_stat("evictions"), "count"),
                    ("memcached.uptime_seconds", get_stat("uptime"), "seconds"),
                ];
                insert_metrics(pool, org_id, target_id, "memcached-collector", "middleware", &inst.host, &m).await;
                count += 1;
            }
        }
    }

    // ---- Docker ----
    {
        let insts = middleware_docker::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("docker", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("docker.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "docker-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    // ---- Kubernetes ----
    {
        let insts = middleware_kubernetes::live_instances().read().clone();
        for inst in &insts {
            let target_id = middleware_uuid("k8s", &inst.id);
            let m: Vec<(&str, f64, &str)> = vec![
                ("k8s.reachable", 1.0, "bool"),
            ];
            insert_metrics(pool, org_id, target_id, "k8s-collector", "middleware", &inst.host, &m).await;
            count += 1;
        }
    }

    count
}

// ============================================================================
// Healthcheck Metrics
// ============================================================================

async fn collect_healthcheck_metrics(pool: &PgPool, org_id: Uuid) -> usize {
    let checks = healthchecks::snapshot_checks();
    let mut count = 0usize;
    for (check, result) in &checks {
        let target_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("healthcheck:{}", check.id).as_bytes());
        let (status_val, response_ms) = match result {
            Some(r) => {
                let sv = match r.status.as_str() { "ok" => 1.0, "warning" => 0.5, "critical" => 0.0, _ => -1.0 };
                (sv, r.response_time_ms)
            }
            None => (-1.0, 0.0),
        };
        let m: Vec<(&str, f64, &str)> = vec![
            ("healthcheck.status", status_val, "enum"),
            ("healthcheck.response_time_ms", response_ms, "ms"),
        ];
        insert_metrics(pool, org_id, target_id, "healthcheck-collector", "healthcheck", &check.target_host, &m).await;
        count += 1;
    }
    count
}

// ============================================================================
// Unified History API — query persisted metrics for any target
// ============================================================================

use axum::{
    extract::{Path, Query as AxumQuery, State, Json},
};
use serde::{Deserialize, Serialize};
use crate::tenant::OrgId;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub metric: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct HistoryPoint {
    pub timestamp: String,
    pub metric_name: String,
    pub value: f64,
    pub unit: String,
}

/// GET /api/targets/:id/metrics/history — query persisted metrics for any target type
pub async fn get_target_metrics_history(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
    AxumQuery(q): AxumQuery<HistoryQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let now = Utc::now();
    let from = q.from.as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| now - chrono::Duration::hours(24));
    let to = q.to.as_ref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(now);
    let limit = q.limit.unwrap_or(2000).min(10000);

    let rows = if let Some(metric_filter) = &q.metric {
        sqlx::query_as::<_, (chrono::DateTime<Utc>, String, f64, String)>(
            "SELECT timestamp, metric_name, value, COALESCE(unit, '') \
             FROM metrics \
             WHERE org_id = $1 AND target_id = $2 AND timestamp >= $3 AND timestamp < $4 \
               AND metric_name LIKE $5 \
             ORDER BY timestamp DESC LIMIT $6"
        )
        .bind(org_id).bind(id).bind(from).bind(to)
        .bind(format!("{}%", metric_filter))
        .bind(limit)
        .fetch_all(&state.pool).await.unwrap_or_default()
    } else {
        sqlx::query_as::<_, (chrono::DateTime<Utc>, String, f64, String)>(
            "SELECT timestamp, metric_name, value, COALESCE(unit, '') \
             FROM metrics \
             WHERE org_id = $1 AND target_id = $2 AND timestamp >= $3 AND timestamp < $4 \
             ORDER BY timestamp DESC LIMIT $5"
        )
        .bind(org_id).bind(id).bind(from).bind(to).bind(limit)
        .fetch_all(&state.pool).await.unwrap_or_default()
    };

    let data: Vec<HistoryPoint> = rows.into_iter().map(|r| HistoryPoint {
        timestamp: r.0.to_rfc3339(),
        metric_name: r.1,
        value: r.2,
        unit: r.3,
    }).collect();

    Ok(Json(serde_json::json!({
        "target_id": id.to_string(),
        "from": from.to_rfc3339(),
        "to": to.to_rfc3339(),
        "count": data.len(),
        "data": data,
    })))
}

/// GET /api/targets/:id/metrics/latest — get the most recent value of each metric for a target
pub async fn get_target_metrics_latest(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, f64, String, chrono::DateTime<Utc>)>(
        "SELECT DISTINCT ON (metric_name) metric_name, value, COALESCE(unit, ''), timestamp \
         FROM metrics \
         WHERE org_id = $1 AND target_id = $2 \
         ORDER BY metric_name, timestamp DESC"
    )
    .bind(org_id).bind(id)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let mut latest: HashMap<String, serde_json::Value> = HashMap::new();
    for (metric_name, value, unit, ts) in &rows {
        latest.insert(metric_name.clone(), serde_json::json!({
            "value": value,
            "unit": unit,
            "timestamp": ts.to_rfc3339(),
        }));
    }

    Ok(Json(serde_json::json!({
        "target_id": id.to_string(),
        "metric_count": latest.len(),
        "metrics": latest,
    })))
}

/// GET /api/metrics/catalog — list all distinct metric names by target type
pub async fn get_metrics_catalog(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT resource_type, metric_name, COALESCE(unit, ''), COUNT(*) \
         FROM metrics WHERE org_id = $1 \
         GROUP BY resource_type, metric_name, unit \
         ORDER BY resource_type, metric_name"
    )
    .bind(org_id)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let mut catalog: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for (rtype, mname, unit, count) in &rows {
        catalog.entry(rtype.clone()).or_default().push(serde_json::json!({
            "metric_name": mname,
            "unit": unit,
            "sample_count": count,
        }));
    }

    Ok(Json(serde_json::json!({
        "resource_types": catalog.keys().collect::<Vec<_>>(),
        "catalog": catalog,
    })))
}
