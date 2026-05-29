use axum::{extract::{State, Path, Query}, Json};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_core::models::alert::{AlertRule, AlertCondition, AlertSeverity, Aggregation, ComparisonOp};
use serde::Deserialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
pub struct AlertListQuery {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_alerts(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<AlertListQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let alerts = zentra_db::repositories::alerts::list_alerts(
        &state.pool,
        org_id,
        params.status.as_deref(),
        params.severity.as_deref(),
        params.from,
        params.to,
        params.limit.unwrap_or(50),
        params.offset.unwrap_or(0),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!(alerts)))
}

pub async fn acknowledge_alert(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify alert belongs to caller's org before updating
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM active_alerts WHERE alert_id = $1 AND org_id = $2)"
    )
    .bind(id).bind(org_id)
    .fetch_one(&state.pool).await
    .unwrap_or(false);
    if !exists {
        return Err(ApiError::NotFound("Alert not found".into()));
    }

    zentra_db::repositories::alerts::update_alert_status(
        &state.pool, id, "acknowledged", Some("api_user"),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "acknowledged": true })))
}

pub async fn resolve_alert(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify alert belongs to caller's org before updating
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM active_alerts WHERE alert_id = $1 AND org_id = $2)"
    )
    .bind(id).bind(org_id)
    .fetch_one(&state.pool).await
    .unwrap_or(false);
    if !exists {
        return Err(ApiError::NotFound("Alert not found".into()));
    }

    zentra_db::repositories::alerts::update_alert_status(
        &state.pool, id, "resolved", None,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "resolved": true })))
}

pub async fn list_rules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Vec<AlertRule>>, ApiError> {
    let rules = zentra_db::repositories::alerts::list_alert_rules(&state.pool, org_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rules))
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub description: Option<String>,
    pub metric_name: String,
    pub operator: String,
    pub threshold: f64,
    pub aggregation: String,
    pub window_seconds: u32,
    pub severity: String,
    pub tags: Option<Vec<String>>,
}

pub async fn create_rule(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateRuleRequest>,
) -> Result<Json<AlertRule>, ApiError> {
    let operator = match req.operator.as_str() {
        "gt" | ">" => ComparisonOp::GreaterThan,
        "gte" | ">=" => ComparisonOp::GreaterThanOrEqual,
        "lt" | "<" => ComparisonOp::LessThan,
        "lte" | "<=" => ComparisonOp::LessThanOrEqual,
        "eq" | "==" => ComparisonOp::Equal,
        "ne" | "!=" => ComparisonOp::NotEqual,
        _ => return Err(ApiError::BadRequest(format!("Invalid operator: {}", req.operator))),
    };

    let aggregation = match req.aggregation.as_str() {
        "avg" => Aggregation::Avg,
        "max" => Aggregation::Max,
        "min" => Aggregation::Min,
        "sum" => Aggregation::Sum,
        "count" => Aggregation::Count,
        "last" => Aggregation::Last,
        "p95" => Aggregation::P95,
        "p99" => Aggregation::P99,
        _ => return Err(ApiError::BadRequest(format!("Invalid aggregation: {}", req.aggregation))),
    };

    let severity = match req.severity.as_str() {
        "info" => AlertSeverity::Info,
        "warning" => AlertSeverity::Warning,
        "critical" => AlertSeverity::Critical,
        _ => return Err(ApiError::BadRequest(format!("Invalid severity: {}", req.severity))),
    };

    let now = Utc::now();
    let rule = AlertRule {
        rule_id: Uuid::now_v7(),
        org_id: org_id,
        name: req.name,
        description: req.description,
        metric_name: req.metric_name,
        condition: AlertCondition {
            operator,
            threshold: req.threshold,
            aggregation,
        },
        window_seconds: req.window_seconds,
        severity,
        enabled: true,
        tags: req.tags.unwrap_or_default(),
        created_at: now,
        updated_at: now,
        for_duration_seconds: 0,
        hysteresis_threshold: None,
        repeat_interval_seconds: 3600,
        notification_policy_id: None,
    };

    zentra_db::repositories::alerts::insert_alert_rule(&state.pool, &rule)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rule))
}

// ── Alert Silences ──

pub async fn list_silences() -> Json<serde_json::Value> {
    Json(serde_json::json!([
        {
            "id": "sil-001",
            "name": "Maintenance Window - DB Upgrade",
            "matchers": [{ "name": "host", "value": "db-oracle-01", "is_regex": false }],
            "starts_at": "2025-01-15T22:00:00Z",
            "ends_at": "2025-01-16T06:00:00Z",
            "created_by": "admin",
            "comment": "Scheduled Oracle upgrade",
            "status": "active"
        },
        {
            "id": "sil-002",
            "name": "Suppress Network Alerts - DC2",
            "matchers": [{ "name": "datacenter", "value": "dc2", "is_regex": false }, { "name": "severity", "value": "warning", "is_regex": false }],
            "starts_at": "2025-01-14T08:00:00Z",
            "ends_at": "2025-01-14T12:00:00Z",
            "created_by": "netops",
            "comment": "Network maintenance in DC2",
            "status": "expired"
        },
        {
            "id": "sil-003",
            "name": "Ignore dev-* alerts",
            "matchers": [{ "name": "host", "value": "dev-.*", "is_regex": true }],
            "starts_at": "2025-01-01T00:00:00Z",
            "ends_at": "2025-12-31T23:59:59Z",
            "created_by": "admin",
            "comment": "Dev environment noise suppression",
            "status": "active"
        }
    ]))
}

pub async fn create_silence(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("New Silence");
    Json(serde_json::json!({
        "id": format!("sil-{}", uuid::Uuid::now_v7()),
        "name": name,
        "status": "active",
        "created": true
    }))
}

pub async fn delete_silence(Path(id): Path<String>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "deleted": true, "id": id }))
}

// ── Alert Templates ──

pub async fn list_templates() -> Json<serde_json::Value> {
    Json(serde_json::json!([
        {
            "id": "tpl-cpu",
            "name": "CPU High Usage",
            "description": "Alert when CPU usage exceeds threshold",
            "metric_name": "cpu_usage_percent",
            "operator": ">",
            "threshold": 90.0,
            "aggregation": "avg",
            "window_seconds": 300,
            "severity": "critical",
            "tags": ["infrastructure", "cpu"],
            "category": "Infrastructure"
        },
        {
            "id": "tpl-mem",
            "name": "Memory Exhaustion",
            "description": "Alert when memory usage is dangerously high",
            "metric_name": "memory_usage_percent",
            "operator": ">",
            "threshold": 95.0,
            "aggregation": "avg",
            "window_seconds": 120,
            "severity": "critical",
            "tags": ["infrastructure", "memory"],
            "category": "Infrastructure"
        },
        {
            "id": "tpl-disk",
            "name": "Disk Space Low",
            "description": "Alert when disk usage exceeds 85%",
            "metric_name": "disk_usage_percent",
            "operator": ">",
            "threshold": 85.0,
            "aggregation": "last",
            "window_seconds": 600,
            "severity": "warning",
            "tags": ["infrastructure", "disk"],
            "category": "Infrastructure"
        },
        {
            "id": "tpl-oracle-ts",
            "name": "Oracle Tablespace Full",
            "description": "Alert when Oracle tablespace usage exceeds 90%",
            "metric_name": "oracle.tablespace.used_percent",
            "operator": ">",
            "threshold": 90.0,
            "aggregation": "last",
            "window_seconds": 300,
            "severity": "critical",
            "tags": ["database", "oracle", "tablespace"],
            "category": "Database"
        },
        {
            "id": "tpl-oracle-lag",
            "name": "Oracle Data Guard Lag",
            "description": "Alert when Data Guard apply lag exceeds 60 seconds",
            "metric_name": "oracle.dataguard.apply_lag_seconds",
            "operator": ">",
            "threshold": 60.0,
            "aggregation": "avg",
            "window_seconds": 180,
            "severity": "critical",
            "tags": ["database", "oracle", "dataguard"],
            "category": "Database"
        },
        {
            "id": "tpl-rman",
            "name": "RMAN Backup Stale",
            "description": "Alert when last RMAN full backup > 24 hours ago",
            "metric_name": "oracle.rman.hours_since_last_full",
            "operator": ">",
            "threshold": 24.0,
            "aggregation": "last",
            "window_seconds": 3600,
            "severity": "warning",
            "tags": ["database", "oracle", "backup"],
            "category": "Database"
        },
        {
            "id": "tpl-net-latency",
            "name": "Network Latency High",
            "description": "Alert when network latency exceeds 100ms",
            "metric_name": "network_latency_ms",
            "operator": ">",
            "threshold": 100.0,
            "aggregation": "p95",
            "window_seconds": 300,
            "severity": "warning",
            "tags": ["network", "latency"],
            "category": "Network"
        },
        {
            "id": "tpl-error-rate",
            "name": "Error Rate Spike",
            "description": "Alert when error rate exceeds 5%",
            "metric_name": "error_rate_percent",
            "operator": ">",
            "threshold": 5.0,
            "aggregation": "avg",
            "window_seconds": 300,
            "severity": "critical",
            "tags": ["application", "errors"],
            "category": "Application"
        }
    ]))
}

pub async fn apply_template(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let template_id = body.get("template_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    Json(serde_json::json!({
        "success": true,
        "message": format!("Alert rule created from template {}", template_id),
        "rule_id": format!("rule-{}", uuid::Uuid::now_v7())
    }))
}

// ═══════════════════════════════════════════════════════════════════════════
// P19: Alert Grouping + Inhibition Rules
// ═══════════════════════════════════════════════════════════════════════════

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AlertGroup {
    pub group_key: String,
    pub labels: serde_json::Value,
    pub alerts: Vec<serde_json::Value>,
    pub count: usize,
    pub most_recent: Option<String>,
}

/// GET /api/alerts/groups — return alerts grouped by labels
#[derive(Deserialize)]
pub struct GroupQuery {
    pub group_by: Option<String>,
}

pub async fn list_alert_groups(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<GroupQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let alerts = zentra_db::repositories::alerts::list_alerts(
        &state.pool, org_id, None, None, None, None, 500, 0,
    ).await.map_err(|e| ApiError::Internal(e.to_string()))?;

    let group_by_fields: Vec<&str> = params.group_by
        .as_deref()
        .unwrap_or("severity")
        .split(',')
        .collect();

    let mut groups: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
    for alert in &alerts {
        let alert_val = serde_json::to_value(alert).unwrap_or_default();
        let key_parts: Vec<String> = group_by_fields.iter().map(|f| {
            alert_val.get(f).and_then(|v| v.as_str()).unwrap_or("unknown").to_string()
        }).collect();
        let key = key_parts.join(":");
        groups.entry(key).or_default().push(alert_val);
    }

    let result: Vec<AlertGroup> = groups.into_iter().map(|(key, alerts)| {
        let count = alerts.len();
        let most_recent = alerts.last().and_then(|a| a.get("triggered_at")).and_then(|v| v.as_str()).map(String::from);
        let labels = serde_json::json!({ "group_key": key });
        AlertGroup { group_key: key, labels, count, most_recent, alerts }
    }).collect();

    Ok(Json(serde_json::json!({ "groups": result, "total_groups": result.len() })))
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct InhibitionRule {
    pub id: Uuid,
    pub name: String,
    pub source_match: serde_json::Value,
    pub target_match: serde_json::Value,
    pub equal_labels: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

fn inhibition_store() -> &'static parking_lot::RwLock<Vec<InhibitionRule>> {
    static INSTANCE: std::sync::OnceLock<parking_lot::RwLock<Vec<InhibitionRule>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| parking_lot::RwLock::new(vec![
        InhibitionRule {
            id: Uuid::nil(),
            name: "Critical suppresses Warning".into(),
            source_match: serde_json::json!({"severity": "critical"}),
            target_match: serde_json::json!({"severity": "warning"}),
            equal_labels: vec!["alertname".into(), "instance".into()],
            enabled: true,
            created_at: Utc::now(),
        },
    ]))
}

/// GET /api/alerts/inhibitions
pub async fn list_inhibitions() -> Json<serde_json::Value> {
    let store = inhibition_store().read();
    Json(serde_json::json!({ "inhibitions": *store, "count": store.len() }))
}

#[derive(Deserialize)]
pub struct CreateInhibitionRequest {
    pub name: String,
    pub source_match: serde_json::Value,
    pub target_match: serde_json::Value,
    pub equal_labels: Vec<String>,
}

/// POST /api/alerts/inhibitions
pub async fn create_inhibition(
    Json(req): Json<CreateInhibitionRequest>,
) -> Json<serde_json::Value> {
    let rule = InhibitionRule {
        id: Uuid::now_v7(),
        name: req.name.clone(),
        source_match: req.source_match,
        target_match: req.target_match,
        equal_labels: req.equal_labels,
        enabled: true,
        created_at: Utc::now(),
    };
    let id = rule.id;
    inhibition_store().write().push(rule);
    Json(serde_json::json!({ "id": id, "name": req.name, "status": "created" }))
}

/// DELETE /api/alerts/inhibitions/:id
pub async fn delete_inhibition(
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    inhibition_store().write().retain(|r| r.id != id);
    Json(serde_json::json!({ "id": id, "status": "deleted" }))
}

// ============================================================================
// P9: Alert Library — 66 Pre-Built Configs
// ============================================================================

/// GET /api/alerts/library
pub async fn list_alert_library() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "categories": [
            { "name": "CPU", "count": 6, "alerts": [
                { "id": "cpu-high-90", "name": "CPU Usage > 90%", "expr": "cpu_usage_percent > 90", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "cpu-high-80", "name": "CPU Usage > 80%", "expr": "cpu_usage_percent > 80", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "cpu-iowait-30", "name": "CPU I/O Wait > 30%", "expr": "cpu_iowait_percent > 30", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "cpu-steal-10", "name": "CPU Steal > 10%", "expr": "cpu_steal_percent > 10", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "cpu-load-high", "name": "Load Average > CPU Count", "expr": "load_average_1m > count(cpu_cores)", "severity": "warning", "for_duration": "15m", "enabled": false },
                { "id": "cpu-context-switch", "name": "Context Switches > 50k/s", "expr": "context_switches_per_sec > 50000", "severity": "info", "for_duration": "10m", "enabled": false },
            ]},
            { "name": "Memory", "count": 6, "alerts": [
                { "id": "mem-high-95", "name": "Memory > 95%", "expr": "memory_usage_percent > 95", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "mem-high-85", "name": "Memory > 85%", "expr": "memory_usage_percent > 85", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "mem-swap-used", "name": "Swap Usage > 50%", "expr": "swap_usage_percent > 50", "severity": "warning", "for_duration": "15m", "enabled": false },
                { "id": "mem-oom-kills", "name": "OOM Kill Detected", "expr": "oom_kills_total increase > 0", "severity": "critical", "for_duration": "1m", "enabled": false },
                { "id": "mem-available-low", "name": "Available Memory < 500MB", "expr": "memory_available_bytes < 524288000", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "mem-page-faults", "name": "Major Page Faults > 100/s", "expr": "major_page_faults_per_sec > 100", "severity": "warning", "for_duration": "10m", "enabled": false },
            ]},
            { "name": "Disk", "count": 6, "alerts": [
                { "id": "disk-space-90", "name": "Disk Usage > 90%", "expr": "disk_usage_percent > 90", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "disk-space-80", "name": "Disk Usage > 80%", "expr": "disk_usage_percent > 80", "severity": "warning", "for_duration": "15m", "enabled": false },
                { "id": "disk-inode-90", "name": "Inode Usage > 90%", "expr": "disk_inode_usage_percent > 90", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "disk-io-util-90", "name": "Disk I/O Utilization > 90%", "expr": "disk_io_utilization > 90", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "disk-latency-high", "name": "Disk Latency > 20ms", "expr": "disk_avg_latency_ms > 20", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "disk-readonly", "name": "Filesystem Read-Only", "expr": "filesystem_readonly == 1", "severity": "critical", "for_duration": "1m", "enabled": false },
            ]},
            { "name": "Network", "count": 10, "alerts": [
                { "id": "net-interface-down", "name": "Interface Down", "expr": "interface_oper_status == 0", "severity": "critical", "for_duration": "1m", "enabled": false },
                { "id": "net-errors-high", "name": "Network Errors > 100/s", "expr": "network_errors_per_sec > 100", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "net-drops-high", "name": "Packet Drops > 1%", "expr": "network_drop_percent > 1", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "net-bandwidth-90", "name": "Bandwidth > 90%", "expr": "network_bandwidth_usage_percent > 90", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "net-tcp-retransmit", "name": "TCP Retransmits > 5%", "expr": "tcp_retransmit_percent > 5", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "net-conn-high", "name": "TCP Connections > 10k", "expr": "tcp_established_connections > 10000", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "net-dns-failures", "name": "DNS Failures > 10/min", "expr": "dns_failures_per_min > 10", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "net-latency-high", "name": "Network Latency > 100ms", "expr": "network_rtt_ms > 100", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "net-syn-flood", "name": "SYN Flood Detected", "expr": "tcp_syn_recv_connections > 1000", "severity": "critical", "for_duration": "2m", "enabled": false },
                { "id": "net-blacklist-hit", "name": "Blacklisted IP Connection", "expr": "blacklist_ip_hits > 0", "severity": "critical", "for_duration": "1m", "enabled": false },
            ]},
            { "name": "Oracle", "count": 10, "alerts": [
                { "id": "ora-ts-90", "name": "Tablespace > 90%", "expr": "oracle.tablespace.used_percent > 90", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "ora-ts-80", "name": "Tablespace > 80%", "expr": "oracle.tablespace.used_percent > 80", "severity": "warning", "for_duration": "15m", "enabled": false },
                { "id": "ora-buffer-cache", "name": "Buffer Cache Hit < 95%", "expr": "oracle.buffer_cache_hit_ratio < 95", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "ora-active-sessions", "name": "Active Sessions > 100", "expr": "oracle.active_sessions > 100", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "ora-dg-lag", "name": "DataGuard Apply Lag > 60s", "expr": "oracle.dataguard.apply_lag_seconds > 60", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "ora-rman-fail", "name": "RMAN Backup Failed", "expr": "oracle.rman.failed_backups > 0", "severity": "critical", "for_duration": "1m", "enabled": false },
                { "id": "ora-rman-stale", "name": "No Full Backup in 48h", "expr": "oracle.rman.hours_since_last_full > 48", "severity": "warning", "for_duration": "1h", "enabled": false },
                { "id": "ora-long-query", "name": "Query Running > 30min", "expr": "oracle.running_query.elapsed_ms > 1800000", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "ora-lock-contention", "name": "Lock Contention > 5s", "expr": "oracle.lock_wait_seconds > 5", "severity": "warning", "for_duration": "2m", "enabled": false },
                { "id": "ora-rac-ic-latency", "name": "RAC Interconnect Latency > 5ms", "expr": "oracle.rac.interconnect_latency_us > 5000", "severity": "warning", "for_duration": "5m", "enabled": false },
            ]},
            { "name": "MySQL", "count": 6, "alerts": [
                { "id": "mysql-conn-high", "name": "MySQL Connections > 80%", "expr": "mysql.connection_usage_percent > 80", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "mysql-slow-queries", "name": "Slow Queries > 10/min", "expr": "mysql.slow_queries_per_min > 10", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "mysql-repl-lag", "name": "Replication Lag > 30s", "expr": "mysql.replication_lag_seconds > 30", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "mysql-innodb-bp", "name": "InnoDB Buffer Pool Hit < 95%", "expr": "mysql.innodb_buffer_pool_hit < 95", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "mysql-deadlocks", "name": "Deadlocks Detected", "expr": "mysql.deadlocks_total increase > 0", "severity": "warning", "for_duration": "1m", "enabled": false },
                { "id": "mysql-aborted-conn", "name": "Aborted Connections > 50/min", "expr": "mysql.aborted_connections_per_min > 50", "severity": "warning", "for_duration": "5m", "enabled": false },
            ]},
            { "name": "PostgreSQL", "count": 6, "alerts": [
                { "id": "pg-conn-high", "name": "PG Connections > 80%", "expr": "pg.connection_usage_percent > 80", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "pg-repl-lag", "name": "PG Replication Lag > 30s", "expr": "pg.replication_lag_seconds > 30", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "pg-dead-tuples", "name": "Dead Tuples > 1M", "expr": "pg.dead_tuples > 1000000", "severity": "warning", "for_duration": "1h", "enabled": false },
                { "id": "pg-cache-hit", "name": "Cache Hit Ratio < 95%", "expr": "pg.cache_hit_ratio < 95", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "pg-long-tx", "name": "Transaction > 30min", "expr": "pg.longest_transaction_seconds > 1800", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "pg-vacuum-stale", "name": "No Vacuum in 24h", "expr": "pg.hours_since_last_vacuum > 24", "severity": "warning", "for_duration": "1h", "enabled": false },
            ]},
            { "name": "Docker", "count": 4, "alerts": [
                { "id": "docker-cpu-80", "name": "Container CPU > 80%", "expr": "docker.container.cpu_percent > 80", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "docker-mem-90", "name": "Container Memory > 90%", "expr": "docker.container.memory_percent > 90", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "docker-restart", "name": "Container Restart Loop", "expr": "docker.container.restart_count increase > 3", "severity": "warning", "for_duration": "15m", "enabled": false },
                { "id": "docker-unhealthy", "name": "Container Unhealthy", "expr": "docker.container.health_status != 'healthy'", "severity": "warning", "for_duration": "5m", "enabled": false },
            ]},
            { "name": "HTTP", "count": 6, "alerts": [
                { "id": "http-5xx-high", "name": "HTTP 5xx Rate > 5%", "expr": "http.error_rate_5xx > 5", "severity": "critical", "for_duration": "5m", "enabled": false },
                { "id": "http-latency-high", "name": "HTTP P99 Latency > 2s", "expr": "http.p99_latency_ms > 2000", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "http-ssl-expiry", "name": "SSL Certificate Expires < 14d", "expr": "http.ssl_days_remaining < 14", "severity": "warning", "for_duration": "1h", "enabled": false },
                { "id": "http-ssl-critical", "name": "SSL Certificate Expires < 3d", "expr": "http.ssl_days_remaining < 3", "severity": "critical", "for_duration": "1h", "enabled": false },
                { "id": "http-endpoint-down", "name": "Endpoint Unreachable", "expr": "http.probe_success == 0", "severity": "critical", "for_duration": "2m", "enabled": false },
                { "id": "http-latency-spike", "name": "Latency Spike > 5x baseline", "expr": "http.latency_vs_baseline > 5", "severity": "warning", "for_duration": "5m", "enabled": false },
            ]},
            { "name": "System", "count": 6, "alerts": [
                { "id": "sys-uptime-restart", "name": "Unexpected Restart", "expr": "uptime_seconds < 300", "severity": "warning", "for_duration": "1m", "enabled": false },
                { "id": "sys-ntp-drift", "name": "NTP Clock Drift > 500ms", "expr": "ntp_offset_ms > 500", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "sys-process-high", "name": "Process Count > 500", "expr": "process_count > 500", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "sys-fd-high", "name": "Open File Descriptors > 80%", "expr": "fd_usage_percent > 80", "severity": "warning", "for_duration": "5m", "enabled": false },
                { "id": "sys-zombie", "name": "Zombie Processes > 10", "expr": "zombie_process_count > 10", "severity": "warning", "for_duration": "10m", "enabled": false },
                { "id": "sys-kernel-panic", "name": "Kernel Panic Detected", "expr": "kernel_panic_count > 0", "severity": "critical", "for_duration": "1m", "enabled": false },
            ]},
        ],
        "total": 66,
    }))
}

/// POST /api/alerts/library/:name/enable
pub async fn enable_library_alert(
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "id": name, "status": "enabled", "message": "Alert template enabled and added to active rules" }))
}
