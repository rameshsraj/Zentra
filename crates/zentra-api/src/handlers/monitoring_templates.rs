//! Monitoring Templates API — Zabbix-inspired template library with macros, assignment, import/export.

use axum::{extract::{State, Path, Query}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::state::AppState;

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMacro {
    pub key: String,
    pub default: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCheck {
    pub name: String,
    pub check_type: String,
    pub config: serde_json::Value,
    pub interval_secs: u64,
    pub warning_threshold: Option<String>,
    pub critical_threshold: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateAlert {
    pub name: String,
    pub expression: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringTemplate {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub version: String,
    pub parent_template: Option<String>,
    pub macros: Vec<TemplateMacro>,
    pub checks: Vec<TemplateCheck>,
    pub alerts: Vec<TemplateAlert>,
    pub metrics: Vec<String>,
    pub dashboards: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateAssignment {
    pub id: String,
    pub template_id: String,
    pub template_name: String,
    pub target_id: String,
    pub target_name: String,
    pub macro_overrides: serde_json::Value,
    pub assigned_at: String,
}

// ---------------------------------------------------------------------------
// In-memory store
// ---------------------------------------------------------------------------

fn templates() -> &'static RwLock<Vec<MonitoringTemplate>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<MonitoringTemplate>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(built_in_templates()))
}
fn assignments() -> &'static RwLock<Vec<TemplateAssignment>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<TemplateAssignment>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(Vec::new()))
}

// ---------------------------------------------------------------------------
// Request / query types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TemplateListQuery {
    pub category: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub parent_template: Option<String>,
    pub macros: Option<Vec<TemplateMacro>>,
    pub checks: Option<Vec<TemplateCheck>>,
    pub alerts: Option<Vec<TemplateAlert>>,
    pub metrics: Option<Vec<String>>,
    pub dashboards: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRequest {
    pub target_id: String,
    pub target_name: Option<String>,
    pub macro_overrides: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/monitoring-templates
pub async fn list_templates(
    State(_s): State<AppState>,
    Query(q): Query<TemplateListQuery>,
) -> Result<Json<Vec<MonitoringTemplate>>, ApiError> {
    let store = templates().read();
    let mut out: Vec<MonitoringTemplate> = store.clone();
    if let Some(cat) = &q.category {
        out.retain(|t| t.category.eq_ignore_ascii_case(cat));
    }
    if let Some(s) = &q.search {
        let s = s.to_lowercase();
        out.retain(|t| t.name.to_lowercase().contains(&s) || t.description.to_lowercase().contains(&s));
    }
    Ok(Json(out))
}

/// POST /api/monitoring-templates
pub async fn create_template(
    State(_s): State<AppState>,
    Json(req): Json<CreateTemplateRequest>,
) -> Result<Json<MonitoringTemplate>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let tpl = MonitoringTemplate {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        category: req.category,
        description: req.description.unwrap_or_default(),
        version: req.version.unwrap_or_else(|| "1.0".into()),
        parent_template: req.parent_template,
        macros: req.macros.unwrap_or_default(),
        checks: req.checks.unwrap_or_default(),
        alerts: req.alerts.unwrap_or_default(),
        metrics: req.metrics.unwrap_or_default(),
        dashboards: req.dashboards.unwrap_or_default(),
        created_at: now.clone(),
        updated_at: now,
    };
    templates().write().push(tpl.clone());
    Ok(Json(tpl))
}

/// GET /api/monitoring-templates/:id
pub async fn get_template(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MonitoringTemplate>, ApiError> {
    let store = templates().read();
    store.iter().find(|t| t.id == id).cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("Template not found".into()))
}

/// PUT /api/monitoring-templates/:id
pub async fn update_template(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateTemplateRequest>,
) -> Result<Json<MonitoringTemplate>, ApiError> {
    let mut store = templates().write();
    if let Some(tpl) = store.iter_mut().find(|t| t.id == id) {
        tpl.name = req.name;
        tpl.category = req.category;
        if let Some(d) = req.description { tpl.description = d; }
        if let Some(v) = req.version { tpl.version = v; }
        tpl.parent_template = req.parent_template;
        if let Some(m) = req.macros { tpl.macros = m; }
        if let Some(c) = req.checks { tpl.checks = c; }
        if let Some(a) = req.alerts { tpl.alerts = a; }
        if let Some(m) = req.metrics { tpl.metrics = m; }
        if let Some(d) = req.dashboards { tpl.dashboards = d; }
        tpl.updated_at = Utc::now().to_rfc3339();
        Ok(Json(tpl.clone()))
    } else {
        Err(ApiError::NotFound("Template not found".into()))
    }
}

/// DELETE /api/monitoring-templates/:id
pub async fn delete_template(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = templates().write();
    let before = store.len();
    store.retain(|t| t.id != id);
    if store.len() < before {
        // also remove assignments
        assignments().write().retain(|a| a.template_id != id);
        Ok(Json(serde_json::json!({ "deleted": id })))
    } else {
        Err(ApiError::NotFound("Template not found".into()))
    }
}

/// POST /api/monitoring-templates/:id/assign
pub async fn assign_template(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AssignRequest>,
) -> Result<Json<TemplateAssignment>, ApiError> {
    let store = templates().read();
    let tpl = store.iter().find(|t| t.id == id)
        .ok_or_else(|| ApiError::NotFound("Template not found".into()))?;
    let assignment = TemplateAssignment {
        id: Uuid::new_v4().to_string(),
        template_id: id,
        template_name: tpl.name.clone(),
        target_id: req.target_id,
        target_name: req.target_name.unwrap_or_else(|| "Unknown".into()),
        macro_overrides: req.macro_overrides.unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        assigned_at: Utc::now().to_rfc3339(),
    };
    drop(store);
    assignments().write().push(assignment.clone());
    Ok(Json(assignment))
}

/// DELETE /api/monitoring-templates/:id/unassign/:target_id
pub async fn unassign_template(
    State(_s): State<AppState>,
    Path((id, target_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = assignments().write();
    let before = store.len();
    store.retain(|a| !(a.template_id == id && a.target_id == target_id));
    if store.len() < before {
        Ok(Json(serde_json::json!({ "unassigned": true })))
    } else {
        Err(ApiError::NotFound("Assignment not found".into()))
    }
}

/// GET /api/monitoring-templates/assignments
pub async fn list_assignments(
    State(_s): State<AppState>,
) -> Result<Json<Vec<TemplateAssignment>>, ApiError> {
    Ok(Json(assignments().read().clone()))
}

/// GET /api/monitoring-templates/categories — V5 Prompt 11
pub async fn list_categories(
    State(_s): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let store = templates().read();
    let mut map = std::collections::HashMap::<String, usize>::new();
    for t in store.iter() {
        *map.entry(t.category.clone()).or_default() += 1;
    }
    let mut cats: Vec<serde_json::Value> = map.into_iter()
        .map(|(cat, count)| serde_json::json!({"category": cat, "count": count}))
        .collect();
    cats.sort_by(|a, b| a["category"].as_str().cmp(&b["category"].as_str()));
    Ok(Json(cats))
}

/// POST /api/monitoring-templates/import-bundle — V5 Prompt 11
pub async fn import_bundle(
    State(_s): State<AppState>,
    Json(tpls): Json<Vec<MonitoringTemplate>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = templates().write();
    let mut imported = 0;
    for mut tpl in tpls {
        tpl.id = Uuid::new_v4().to_string();
        tpl.updated_at = Utc::now().to_rfc3339();
        store.push(tpl);
        imported += 1;
    }
    Ok(Json(serde_json::json!({"imported": imported})))
}

/// POST /api/monitoring-templates/import
pub async fn import_template(
    State(_s): State<AppState>,
    Json(tpl): Json<MonitoringTemplate>,
) -> Result<Json<MonitoringTemplate>, ApiError> {
    let mut imported = tpl;
    imported.id = Uuid::new_v4().to_string();
    imported.updated_at = Utc::now().to_rfc3339();
    templates().write().push(imported.clone());
    Ok(Json(imported))
}

/// GET /api/monitoring-templates/:id/export
pub async fn export_template(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MonitoringTemplate>, ApiError> {
    let store = templates().read();
    store.iter().find(|t| t.id == id).cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("Template not found".into()))
}

// ---------------------------------------------------------------------------
// Built-in template library (20+ templates)
// ---------------------------------------------------------------------------

fn built_in_templates() -> Vec<MonitoringTemplate> {
    let now = Utc::now().to_rfc3339();
    vec![
        // ── Server templates ──────────────────────────────────────────
        make_template("linux-server", "Linux Server", "server",
            "Comprehensive Linux server monitoring: CPU, memory, disk, load, network, processes, services.",
            None,
            vec![
                m("{$CPU_WARN}", "80", "CPU warning threshold (%)"),
                m("{$CPU_CRIT}", "95", "CPU critical threshold (%)"),
                m("{$MEM_WARN}", "85", "Memory warning threshold (%)"),
                m("{$MEM_CRIT}", "95", "Memory critical threshold (%)"),
                m("{$DISK_WARN}", "80", "Disk warning threshold (%)"),
                m("{$DISK_CRIT}", "90", "Disk critical threshold (%)"),
                m("{$LOAD_WARN}", "4", "Load average warning threshold"),
                m("{$LOAD_CRIT}", "8", "Load average critical threshold"),
            ],
            vec![
                check("CPU Usage", "custom_script", serde_json::json!({"command": "cat /proc/stat | head -1"}), 60, Some("80"), Some("95")),
                check("Memory Usage", "custom_script", serde_json::json!({"command": "free -m | grep Mem"}), 60, Some("85"), Some("95")),
                check("Disk Usage", "disk", serde_json::json!({"mount": "/"}), 300, Some("80"), Some("90")),
                check("Load Average", "load", serde_json::json!({}), 60, Some("4"), Some("8")),
                check("Swap Usage", "custom_script", serde_json::json!({"command": "free -m | grep Swap"}), 300, Some("50"), Some("80")),
                check("SSH Connectivity", "ssh", serde_json::json!({}), 120, None, None),
                check("System Uptime", "custom_script", serde_json::json!({"command": "cat /proc/uptime"}), 300, None, None),
                check("NTP Sync", "ntp", serde_json::json!({}), 600, Some("100"), Some("500")),
            ],
            vec![
                alert("High CPU Usage", "cpu_usage > {$CPU_WARN}", "warning", "CPU usage exceeded {$CPU_WARN}%"),
                alert("Critical CPU Usage", "cpu_usage > {$CPU_CRIT}", "critical", "CPU usage exceeded {$CPU_CRIT}%"),
                alert("High Memory Usage", "memory_usage > {$MEM_WARN}", "warning", "Memory usage exceeded {$MEM_WARN}%"),
                alert("Disk Space Low", "disk_usage > {$DISK_WARN}", "warning", "Disk usage exceeded {$DISK_WARN}%"),
                alert("High Load", "load_avg_1 > {$LOAD_WARN}", "warning", "Load average exceeded {$LOAD_WARN}"),
                alert("Host Unreachable", "ping_loss == 100", "critical", "Host is unreachable"),
            ],
            vec!["cpu_usage", "memory_usage", "disk_usage", "load_avg", "swap_usage", "network_bytes", "uptime"],
            &now,
        ),
        make_template("windows-server", "Windows Server", "server",
            "Windows server monitoring: CPU, memory, disk, services, event log, IIS.",
            None,
            vec![
                m("{$CPU_WARN}", "80", "CPU warning threshold (%)"),
                m("{$CPU_CRIT}", "95", "CPU critical threshold (%)"),
                m("{$MEM_WARN}", "85", "Memory warning threshold (%)"),
                m("{$DISK_WARN}", "80", "Disk warning threshold (%)"),
            ],
            vec![
                check("CPU Usage", "custom_script", serde_json::json!({"command": "wmic cpu get loadpercentage"}), 60, Some("80"), Some("95")),
                check("Memory Usage", "custom_script", serde_json::json!({"command": "wmic os get freephysicalmemory"}), 60, Some("85"), Some("95")),
                check("Disk Usage", "disk", serde_json::json!({"mount": "C:"}), 300, Some("80"), Some("90")),
                check("Ping", "ping", serde_json::json!({}), 60, Some("100"), Some("500")),
            ],
            vec![
                alert("High CPU", "cpu_usage > {$CPU_WARN}", "warning", "CPU usage high"),
                alert("High Memory", "memory_usage > {$MEM_WARN}", "warning", "Memory usage high"),
                alert("Disk Space Low", "disk_usage > {$DISK_WARN}", "warning", "Disk space low"),
            ],
            vec!["cpu_usage", "memory_usage", "disk_usage", "network_bytes"],
            &now,
        ),

        // ── Middleware templates ───────────────────────────────────────
        make_template("nginx", "Nginx", "middleware",
            "Nginx web server monitoring via stub_status module. Tracks connections, requests, workers.",
            Some("linux-server"),
            vec![
                m("{$NGINX.HOST}", "127.0.0.1", "Nginx status host"),
                m("{$NGINX.PORT}", "80", "Nginx status port"),
                m("{$NGINX.STATUS_PATH}", "/nginx_status", "stub_status path"),
                m("{$NGINX.CONN_WARN}", "500", "Active connections warning"),
                m("{$NGINX.CONN_CRIT}", "1000", "Active connections critical"),
            ],
            vec![
                check("Nginx Status", "http", serde_json::json!({"url": "http://{$NGINX.HOST}:{$NGINX.PORT}{$NGINX.STATUS_PATH}"}), 30, None, None),
                check("Nginx Port", "tcp", serde_json::json!({"port": 80}), 60, None, None),
                check("Nginx Process", "process", serde_json::json!({"name": "nginx"}), 60, None, None),
            ],
            vec![
                alert("Nginx Down", "nginx_process_count == 0", "critical", "Nginx process is not running"),
                alert("High Connections", "nginx_active_connections > {$NGINX.CONN_WARN}", "warning", "Nginx active connections high"),
                alert("Nginx 5xx Rate", "nginx_5xx_rate > 5", "warning", "Nginx 5xx error rate exceeds 5/s"),
            ],
            vec!["nginx_active_connections", "nginx_requests_per_sec", "nginx_reading", "nginx_writing", "nginx_waiting", "nginx_accepts", "nginx_handled", "nginx_5xx_rate", "nginx_4xx_rate"],
            &now,
        ),
        make_template("apache-http", "Apache HTTP", "middleware",
            "Apache HTTP server monitoring via mod_status. Tracks workers, requests, throughput.",
            Some("linux-server"),
            vec![
                m("{$APACHE.HOST}", "127.0.0.1", "Apache status host"),
                m("{$APACHE.PORT}", "80", "Apache status port"),
                m("{$APACHE.STATUS_PATH}", "/server-status?auto", "mod_status path"),
                m("{$APACHE.WORKERS_WARN}", "80", "Busy workers warning (%)"),
            ],
            vec![
                check("Apache Status", "http", serde_json::json!({"url": "http://{$APACHE.HOST}:{$APACHE.PORT}{$APACHE.STATUS_PATH}"}), 30, None, None),
                check("Apache Port", "tcp", serde_json::json!({"port": 80}), 60, None, None),
                check("Apache Process", "process", serde_json::json!({"name": "apache2"}), 60, None, None),
            ],
            vec![
                alert("Apache Down", "apache_process_count == 0", "critical", "Apache process is not running"),
                alert("Workers Saturated", "apache_busy_workers_pct > {$APACHE.WORKERS_WARN}", "warning", "Apache busy workers exceed threshold"),
            ],
            vec!["apache_busy_workers", "apache_idle_workers", "apache_req_per_sec", "apache_bytes_per_sec", "apache_cpu_load", "apache_uptime"],
            &now,
        ),
        make_template("haproxy", "HAProxy", "middleware",
            "HAProxy load balancer monitoring via stats page. Tracks sessions, connections, backends.",
            Some("linux-server"),
            vec![
                m("{$HAPROXY.HOST}", "127.0.0.1", "HAProxy stats host"),
                m("{$HAPROXY.PORT}", "8404", "HAProxy stats port"),
                m("{$HAPROXY.STATS_PATH}", "/haproxy?stats;csv", "Stats CSV path"),
                m("{$HAPROXY.SESS_WARN}", "80", "Session usage warning (%)"),
            ],
            vec![
                check("HAProxy Stats", "http", serde_json::json!({"url": "http://{$HAPROXY.HOST}:{$HAPROXY.PORT}{$HAPROXY.STATS_PATH}"}), 30, None, None),
                check("HAProxy Process", "process", serde_json::json!({"name": "haproxy"}), 60, None, None),
            ],
            vec![
                alert("HAProxy Down", "haproxy_process_count == 0", "critical", "HAProxy is not running"),
                alert("Backend Down", "haproxy_backend_down > 0", "critical", "One or more HAProxy backends are down"),
                alert("High Error Rate", "haproxy_error_rate > 1", "warning", "HAProxy error rate exceeds 1%"),
            ],
            vec!["haproxy_frontend_sessions", "haproxy_backend_sessions", "haproxy_connection_rate", "haproxy_queue_depth", "haproxy_error_rate", "haproxy_2xx", "haproxy_4xx", "haproxy_5xx"],
            &now,
        ),
        make_template("tomcat", "Apache Tomcat", "middleware",
            "Apache Tomcat application server monitoring via JMX or manager status page.",
            Some("linux-server"),
            vec![
                m("{$TOMCAT.HOST}", "127.0.0.1", "Tomcat host"),
                m("{$TOMCAT.PORT}", "8080", "Tomcat HTTP port"),
                m("{$TOMCAT.USER}", "admin", "Tomcat manager user"),
                m("{$TOMCAT.PASSWORD}", "admin", "Tomcat manager password"),
                m("{$TOMCAT.THREADS_WARN}", "80", "Thread pool usage warning (%)"),
            ],
            vec![
                check("Tomcat HTTP", "http", serde_json::json!({"url": "http://{$TOMCAT.HOST}:{$TOMCAT.PORT}"}), 30, None, None),
                check("Tomcat Process", "process", serde_json::json!({"name": "java"}), 60, None, None),
            ],
            vec![
                alert("Tomcat Down", "tomcat_http_check == 0", "critical", "Tomcat is not responding"),
                alert("Thread Pool Saturated", "tomcat_thread_busy_pct > {$TOMCAT.THREADS_WARN}", "warning", "Tomcat thread pool utilization high"),
            ],
            vec!["tomcat_threads_busy", "tomcat_threads_max", "tomcat_request_count", "tomcat_error_count", "tomcat_processing_time", "tomcat_sessions_active"],
            &now,
        ),
        make_template("phpfpm", "PHP-FPM", "middleware",
            "PHP-FPM process manager monitoring via status page.",
            Some("linux-server"),
            vec![
                m("{$PHPFPM.HOST}", "127.0.0.1", "PHP-FPM status host"),
                m("{$PHPFPM.PORT}", "9000", "PHP-FPM port"),
                m("{$PHPFPM.STATUS_PATH}", "/fpm-status", "Status page path"),
                m("{$PHPFPM.QUEUE_WARN}", "10", "Listen queue warning threshold"),
            ],
            vec![
                check("PHP-FPM Status", "http", serde_json::json!({"url": "http://{$PHPFPM.HOST}:{$PHPFPM.PORT}{$PHPFPM.STATUS_PATH}?json"}), 30, None, None),
            ],
            vec![
                alert("PHP-FPM Queue Backlog", "phpfpm_listen_queue > {$PHPFPM.QUEUE_WARN}", "warning", "PHP-FPM listen queue backlog"),
                alert("Max Children Reached", "phpfpm_max_children_reached > 0", "warning", "PHP-FPM max children reached"),
            ],
            vec!["phpfpm_active_processes", "phpfpm_idle_processes", "phpfpm_listen_queue", "phpfpm_max_children_reached", "phpfpm_slow_requests", "phpfpm_accepted_conn"],
            &now,
        ),
        make_template("rabbitmq", "RabbitMQ", "middleware",
            "RabbitMQ message broker monitoring via management HTTP API.",
            Some("linux-server"),
            vec![
                m("{$RABBITMQ.HOST}", "127.0.0.1", "RabbitMQ host"),
                m("{$RABBITMQ.PORT}", "15672", "Management API port"),
                m("{$RABBITMQ.USER}", "guest", "API user"),
                m("{$RABBITMQ.PASSWORD}", "guest", "API password"),
                m("{$RABBITMQ.QUEUE_WARN}", "10000", "Queue messages warning"),
            ],
            vec![
                check("RabbitMQ API", "http", serde_json::json!({"url": "http://{$RABBITMQ.HOST}:{$RABBITMQ.PORT}/api/overview"}), 30, None, None),
                check("RabbitMQ Port", "tcp", serde_json::json!({"port": 5672}), 60, None, None),
            ],
            vec![
                alert("RabbitMQ Down", "rabbitmq_api_check == 0", "critical", "RabbitMQ management API not responding"),
                alert("Queue Messages High", "rabbitmq_messages_ready > {$RABBITMQ.QUEUE_WARN}", "warning", "Queue message count exceeds threshold"),
                alert("Memory Alarm", "rabbitmq_mem_alarm == 1", "critical", "RabbitMQ memory alarm triggered"),
            ],
            vec!["rabbitmq_queues", "rabbitmq_messages_ready", "rabbitmq_messages_unacked", "rabbitmq_publish_rate", "rabbitmq_deliver_rate", "rabbitmq_connections", "rabbitmq_channels"],
            &now,
        ),
        make_template("kafka", "Apache Kafka", "middleware",
            "Apache Kafka broker monitoring via JMX or Prometheus exporter.",
            Some("linux-server"),
            vec![
                m("{$KAFKA.HOST}", "127.0.0.1", "Kafka broker host"),
                m("{$KAFKA.PORT}", "9092", "Kafka broker port"),
                m("{$KAFKA.JMX_PORT}", "9999", "JMX port"),
                m("{$KAFKA.LAG_WARN}", "1000", "Consumer lag warning"),
            ],
            vec![
                check("Kafka Port", "tcp", serde_json::json!({"port": 9092}), 30, None, None),
                check("Kafka Process", "process", serde_json::json!({"name": "kafka"}), 60, None, None),
            ],
            vec![
                alert("Kafka Down", "kafka_port_check == 0", "critical", "Kafka broker not reachable"),
                alert("Under-Replicated Partitions", "kafka_under_replicated > 0", "critical", "Kafka has under-replicated partitions"),
                alert("Consumer Lag High", "kafka_consumer_lag > {$KAFKA.LAG_WARN}", "warning", "Kafka consumer lag exceeds threshold"),
            ],
            vec!["kafka_brokers", "kafka_topics", "kafka_partitions", "kafka_under_replicated", "kafka_messages_in_per_sec", "kafka_bytes_in", "kafka_bytes_out", "kafka_consumer_lag"],
            &now,
        ),
        make_template("elasticsearch", "Elasticsearch", "middleware",
            "Elasticsearch cluster monitoring via REST API.",
            Some("linux-server"),
            vec![
                m("{$ES.HOST}", "127.0.0.1", "Elasticsearch host"),
                m("{$ES.PORT}", "9200", "Elasticsearch HTTP port"),
                m("{$ES.HEAP_WARN}", "80", "JVM heap usage warning (%)"),
            ],
            vec![
                check("ES Cluster Health", "http", serde_json::json!({"url": "http://{$ES.HOST}:{$ES.PORT}/_cluster/health"}), 30, None, None),
                check("ES Port", "tcp", serde_json::json!({"port": 9200}), 60, None, None),
            ],
            vec![
                alert("ES Cluster Red", "es_cluster_status == red", "critical", "Elasticsearch cluster is RED"),
                alert("ES Cluster Yellow", "es_cluster_status == yellow", "warning", "Elasticsearch cluster is YELLOW"),
                alert("ES Heap High", "es_jvm_heap_pct > {$ES.HEAP_WARN}", "warning", "Elasticsearch JVM heap usage high"),
                alert("Unassigned Shards", "es_unassigned_shards > 0", "warning", "Elasticsearch has unassigned shards"),
            ],
            vec!["es_cluster_status", "es_nodes", "es_shards_active", "es_shards_unassigned", "es_docs_count", "es_store_size", "es_indexing_rate", "es_search_rate", "es_jvm_heap_pct"],
            &now,
        ),
        make_template("redis", "Redis", "middleware",
            "Redis in-memory data store monitoring via redis-cli INFO.",
            Some("linux-server"),
            vec![
                m("{$REDIS.HOST}", "127.0.0.1", "Redis host"),
                m("{$REDIS.PORT}", "6379", "Redis port"),
                m("{$REDIS.MEM_WARN}", "80", "Memory usage warning (%)"),
            ],
            vec![
                check("Redis Port", "tcp", serde_json::json!({"port": 6379}), 30, None, None),
                check("Redis Process", "process", serde_json::json!({"name": "redis-server"}), 60, None, None),
            ],
            vec![
                alert("Redis Down", "redis_port_check == 0", "critical", "Redis is not reachable"),
                alert("High Memory", "redis_memory_pct > {$REDIS.MEM_WARN}", "warning", "Redis memory usage exceeds threshold"),
                alert("Low Hit Rate", "redis_hit_rate < 80", "warning", "Redis cache hit rate below 80%"),
            ],
            vec!["redis_connected_clients", "redis_memory_used", "redis_memory_max", "redis_hit_rate", "redis_ops_per_sec", "redis_keyspace_size", "redis_replication_lag"],
            &now,
        ),
        make_template("memcached", "Memcached", "middleware",
            "Memcached monitoring via stats protocol.",
            Some("linux-server"),
            vec![
                m("{$MEMCACHED.HOST}", "127.0.0.1", "Memcached host"),
                m("{$MEMCACHED.PORT}", "11211", "Memcached port"),
            ],
            vec![
                check("Memcached Port", "tcp", serde_json::json!({"port": 11211}), 30, None, None),
            ],
            vec![
                alert("Memcached Down", "memcached_port_check == 0", "critical", "Memcached is not reachable"),
                alert("High Evictions", "memcached_evictions > 100", "warning", "Memcached eviction rate is high"),
            ],
            vec!["memcached_connections", "memcached_hit_ratio", "memcached_items", "memcached_evictions", "memcached_bytes_read", "memcached_bytes_written"],
            &now,
        ),

        // ── Database templates ────────────────────────────────────────
        make_template("oracle-db", "Oracle Database", "database",
            "Oracle Database monitoring: tablespaces, wait classes, SGA, top queries, Data Guard, RMAN.",
            Some("linux-server"),
            vec![
                m("{$ORACLE.HOST}", "127.0.0.1", "Oracle host"),
                m("{$ORACLE.PORT}", "1521", "Oracle listener port"),
                m("{$ORACLE.SID}", "ORCL", "Oracle SID"),
                m("{$ORACLE.TBS_WARN}", "85", "Tablespace usage warning (%)"),
                m("{$ORACLE.TBS_CRIT}", "95", "Tablespace usage critical (%)"),
            ],
            vec![
                check("Oracle Listener", "tcp", serde_json::json!({"port": 1521}), 30, None, None),
                check("Oracle Process", "process", serde_json::json!({"name": "ora_pmon"}), 60, None, None),
            ],
            vec![
                alert("Oracle Down", "oracle_listener_check == 0", "critical", "Oracle listener not responding"),
                alert("Tablespace Full", "oracle_tbs_pct > {$ORACLE.TBS_CRIT}", "critical", "Tablespace usage critical"),
                alert("Tablespace Warning", "oracle_tbs_pct > {$ORACLE.TBS_WARN}", "warning", "Tablespace usage high"),
            ],
            vec!["oracle_sessions", "oracle_tbs_usage", "oracle_wait_time", "oracle_buffer_hit_ratio", "oracle_redo_rate", "oracle_pga_usage"],
            &now,
        ),
        make_template("postgresql", "PostgreSQL", "database",
            "PostgreSQL monitoring: connections, slow queries, replication, locks, vacuum, bloat.",
            Some("linux-server"),
            vec![
                m("{$PG.HOST}", "127.0.0.1", "PostgreSQL host"),
                m("{$PG.PORT}", "5432", "PostgreSQL port"),
                m("{$PG.CONN_WARN}", "80", "Connection usage warning (%)"),
            ],
            vec![
                check("PostgreSQL Port", "tcp", serde_json::json!({"port": 5432}), 30, None, None),
                check("PostgreSQL Process", "process", serde_json::json!({"name": "postgres"}), 60, None, None),
            ],
            vec![
                alert("PostgreSQL Down", "pg_port_check == 0", "critical", "PostgreSQL not reachable"),
                alert("Connections High", "pg_conn_pct > {$PG.CONN_WARN}", "warning", "Connection usage exceeds threshold"),
                alert("Replication Lag", "pg_replication_lag > 60", "warning", "Replication lag exceeds 60 seconds"),
            ],
            vec!["pg_connections", "pg_active_queries", "pg_transactions", "pg_cache_hit_ratio", "pg_replication_lag", "pg_dead_tuples"],
            &now,
        ),
        make_template("mysql", "MySQL", "database",
            "MySQL monitoring: connections, InnoDB, replication, slow queries, table stats.",
            Some("linux-server"),
            vec![
                m("{$MYSQL.HOST}", "127.0.0.1", "MySQL host"),
                m("{$MYSQL.PORT}", "3306", "MySQL port"),
                m("{$MYSQL.CONN_WARN}", "80", "Connection usage warning (%)"),
            ],
            vec![
                check("MySQL Port", "tcp", serde_json::json!({"port": 3306}), 30, None, None),
                check("MySQL Process", "process", serde_json::json!({"name": "mysqld"}), 60, None, None),
            ],
            vec![
                alert("MySQL Down", "mysql_port_check == 0", "critical", "MySQL not reachable"),
                alert("Connections High", "mysql_conn_pct > {$MYSQL.CONN_WARN}", "warning", "Connection usage high"),
                alert("Replication Broken", "mysql_slave_io == 0", "critical", "MySQL replication IO thread stopped"),
            ],
            vec!["mysql_connections", "mysql_queries_per_sec", "mysql_innodb_buffer_pool_hit", "mysql_replication_lag", "mysql_slow_queries"],
            &now,
        ),
        make_template("mongodb", "MongoDB", "database",
            "MongoDB monitoring: connections, operations, replica set, collections, indexes.",
            Some("linux-server"),
            vec![
                m("{$MONGO.HOST}", "127.0.0.1", "MongoDB host"),
                m("{$MONGO.PORT}", "27017", "MongoDB port"),
            ],
            vec![
                check("MongoDB Port", "tcp", serde_json::json!({"port": 27017}), 30, None, None),
                check("MongoDB Process", "process", serde_json::json!({"name": "mongod"}), 60, None, None),
            ],
            vec![
                alert("MongoDB Down", "mongo_port_check == 0", "critical", "MongoDB not reachable"),
                alert("Replication Lag", "mongo_repl_lag > 60", "warning", "Replica set lag exceeds 60s"),
            ],
            vec!["mongo_connections", "mongo_operations", "mongo_memory", "mongo_repl_lag", "mongo_docs_count"],
            &now,
        ),
        make_template("sqlserver", "SQL Server", "database",
            "Microsoft SQL Server monitoring: connections, queries, databases, jobs, wait stats.",
            None,
            vec![
                m("{$MSSQL.HOST}", "127.0.0.1", "SQL Server host"),
                m("{$MSSQL.PORT}", "1433", "SQL Server port"),
            ],
            vec![
                check("SQL Server Port", "tcp", serde_json::json!({"port": 1433}), 30, None, None),
            ],
            vec![
                alert("SQL Server Down", "mssql_port_check == 0", "critical", "SQL Server not reachable"),
            ],
            vec!["mssql_connections", "mssql_batch_requests", "mssql_buffer_cache_hit", "mssql_page_life_expectancy"],
            &now,
        ),

        // ── Network/endpoint templates ────────────────────────────────
        make_template("icmp-ping", "ICMP Ping", "network",
            "Basic ICMP ping check for host reachability, round-trip time, and packet loss.",
            None,
            vec![
                m("{$PING.WARN_RTT}", "100", "RTT warning threshold (ms)"),
                m("{$PING.CRIT_RTT}", "500", "RTT critical threshold (ms)"),
                m("{$PING.CRIT_LOSS}", "20", "Packet loss critical (%)"),
            ],
            vec![
                check("ICMP Ping", "ping", serde_json::json!({"count": 4, "timeout": 5}), 60, Some("100"), Some("500")),
            ],
            vec![
                alert("Host Unreachable", "ping_loss == 100", "critical", "Host is unreachable"),
                alert("High Packet Loss", "ping_loss > {$PING.CRIT_LOSS}", "critical", "Packet loss exceeds threshold"),
                alert("High RTT", "ping_rtt > {$PING.WARN_RTT}", "warning", "Round-trip time exceeds threshold"),
            ],
            vec!["ping_rtt", "ping_loss"],
            &now,
        ),
        make_template("tcp-port", "TCP Port Check", "network",
            "TCP port connectivity check with connection time measurement.",
            None,
            vec![
                m("{$TCP.PORT}", "443", "Port to check"),
                m("{$TCP.TIMEOUT}", "5", "Connection timeout (s)"),
            ],
            vec![
                check("TCP Port", "tcp", serde_json::json!({"port": "{$TCP.PORT}", "timeout": "{$TCP.TIMEOUT}"}), 60, None, None),
            ],
            vec![
                alert("Port Closed", "tcp_check == 0", "critical", "TCP port is not reachable"),
            ],
            vec!["tcp_connection_time", "tcp_status"],
            &now,
        ),
        make_template("http-endpoint", "HTTP Endpoint", "network",
            "HTTP/HTTPS endpoint monitoring with response code, time, content match, SSL.",
            None,
            vec![
                m("{$HTTP.URL}", "https://example.com", "URL to check"),
                m("{$HTTP.EXPECTED_CODE}", "200", "Expected HTTP status code"),
                m("{$HTTP.WARN_TIME}", "1000", "Response time warning (ms)"),
                m("{$HTTP.CRIT_TIME}", "5000", "Response time critical (ms)"),
                m("{$HTTP.CONTENT_MATCH}", "", "Expected content in response body"),
            ],
            vec![
                check("HTTP Check", "http", serde_json::json!({"url": "{$HTTP.URL}", "expected_code": "{$HTTP.EXPECTED_CODE}", "content_match": "{$HTTP.CONTENT_MATCH}"}), 60, Some("1000"), Some("5000")),
            ],
            vec![
                alert("HTTP Down", "http_check == 0", "critical", "HTTP endpoint not responding"),
                alert("Slow Response", "http_response_time > {$HTTP.WARN_TIME}", "warning", "HTTP response time exceeds threshold"),
                alert("Wrong Status Code", "http_status_code != {$HTTP.EXPECTED_CODE}", "critical", "Unexpected HTTP status code"),
            ],
            vec!["http_response_time", "http_status_code", "http_ssl_days_remaining"],
            &now,
        ),
        make_template("ssl-certificate", "SSL Certificate", "network",
            "SSL/TLS certificate monitoring: expiry, chain validity, protocol version.",
            None,
            vec![
                m("{$SSL.HOST}", "example.com", "Hostname to check"),
                m("{$SSL.PORT}", "443", "SSL port"),
                m("{$SSL.WARN_DAYS}", "30", "Days before expiry warning"),
                m("{$SSL.CRIT_DAYS}", "7", "Days before expiry critical"),
            ],
            vec![
                check("SSL Certificate", "ssl_cert", serde_json::json!({"host": "{$SSL.HOST}", "port": "{$SSL.PORT}"}), 3600, Some("30"), Some("7")),
            ],
            vec![
                alert("SSL Expiring Soon", "ssl_days_remaining < {$SSL.WARN_DAYS}", "warning", "SSL certificate expires in less than {$SSL.WARN_DAYS} days"),
                alert("SSL Expiring Critical", "ssl_days_remaining < {$SSL.CRIT_DAYS}", "critical", "SSL certificate expires in less than {$SSL.CRIT_DAYS} days"),
                alert("SSL Invalid", "ssl_valid == 0", "critical", "SSL certificate is invalid"),
            ],
            vec!["ssl_days_remaining", "ssl_valid", "ssl_protocol"],
            &now,
        ),

        // ── Container templates ───────────────────────────────────────
        make_template("docker-host", "Docker Host", "container",
            "Docker host monitoring: container count, CPU/memory per container, images, volumes.",
            Some("linux-server"),
            vec![
                m("{$DOCKER.SOCKET}", "/var/run/docker.sock", "Docker socket path"),
                m("{$DOCKER.CPU_WARN}", "80", "Container CPU warning (%)"),
                m("{$DOCKER.MEM_WARN}", "80", "Container memory warning (%)"),
            ],
            vec![
                check("Docker Daemon", "custom_script", serde_json::json!({"command": "docker info --format '{{.Containers}}'"}), 60, None, None),
                check("Docker Process", "process", serde_json::json!({"name": "dockerd"}), 60, None, None),
            ],
            vec![
                alert("Docker Down", "docker_daemon_check == 0", "critical", "Docker daemon is not running"),
                alert("Container High CPU", "container_cpu > {$DOCKER.CPU_WARN}", "warning", "Container CPU usage high"),
                alert("Container Restart", "container_restart_count > 3", "warning", "Container restarting frequently"),
            ],
            vec!["docker_containers_running", "docker_containers_stopped", "docker_images", "docker_volumes", "container_cpu", "container_memory"],
            &now,
        ),
        make_template("kubernetes-cluster", "Kubernetes Cluster", "container",
            "Kubernetes cluster monitoring: nodes, pods, deployments, resource utilization.",
            None,
            vec![
                m("{$K8S.API_SERVER}", "https://kubernetes.default.svc", "Kubernetes API server"),
                m("{$K8S.NAMESPACE}", "", "Namespace filter (empty = all)"),
                m("{$K8S.POD_RESTART_WARN}", "5", "Pod restart warning threshold"),
            ],
            vec![
                check("K8s API Server", "http", serde_json::json!({"url": "{$K8S.API_SERVER}/healthz"}), 30, None, None),
            ],
            vec![
                alert("K8s API Down", "k8s_api_check == 0", "critical", "Kubernetes API server not reachable"),
                alert("Node NotReady", "k8s_node_not_ready > 0", "critical", "Kubernetes node is NotReady"),
                alert("Pod CrashLoop", "k8s_pod_restart > {$K8S.POD_RESTART_WARN}", "warning", "Pod restarting frequently"),
                alert("Pending Pods", "k8s_pods_pending > 0", "warning", "Kubernetes has pending pods"),
            ],
            vec!["k8s_nodes", "k8s_pods_running", "k8s_pods_pending", "k8s_pods_failed", "k8s_deployments", "k8s_cpu_requests", "k8s_cpu_limits", "k8s_memory_requests", "k8s_memory_limits"],
            &now,
        ),

        // ═══════════════════════════════════════════════════════════════
        // V5 Prompt 11: 30+ additional vendor templates
        // ═══════════════════════════════════════════════════════════════

        // ── Network (10) ──────────────────────────────────────────────
        make_template("cisco-ios", "Cisco IOS", "network", "Cisco IOS switch/router monitoring via SNMP: interfaces, CPU, memory, BGP, OSPF.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community string"), m("{$CPU_WARN}", "80", "CPU warning %")],
            vec![check("SNMP Uptime", "snmp", serde_json::json!({"oid": "1.3.6.1.2.1.1.3.0"}), 60, None, None), check("CPU Usage", "snmp", serde_json::json!({"oid": "1.3.6.1.4.1.9.9.109.1.1.1.1.6"}), 60, Some("80"), Some("95"))],
            vec![alert("High CPU", "cpu > {$CPU_WARN}", "warning", "Cisco CPU high"), alert("Interface Down", "if_oper_status == 2", "critical", "Interface down")],
            vec!["cpu", "memory_used", "if_bandwidth", "bgp_peers"], &now),
        make_template("cisco-nexus", "Cisco Nexus", "network", "Cisco Nexus data center switch monitoring via SNMP/NX-API.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community")],
            vec![check("NX-API Health", "http", serde_json::json!({"url": "https://{$HOST}/ins"}), 60, None, None)],
            vec![alert("VPC Inconsistency", "vpc_consistency == 0", "critical", "vPC peer mismatch")],
            vec!["cpu", "memory", "vpc_status", "fabric_links"], &now),
        make_template("fortinet-fortigate", "Fortinet FortiGate", "network", "FortiGate firewall monitoring: sessions, policies, VPN tunnels, HA status.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community"), m("{$SESSION_WARN}", "80", "Session table warning %")],
            vec![check("Session Count", "snmp", serde_json::json!({"oid": "1.3.6.1.4.1.12356.101.4.1.8.0"}), 60, Some("80"), Some("95"))],
            vec![alert("Session Table Full", "session_pct > {$SESSION_WARN}", "warning", "Session table filling up"), alert("HA Failover", "ha_status_change == 1", "critical", "HA failover detected")],
            vec!["sessions", "cpu", "memory", "vpn_tunnels"], &now),
        make_template("palo-alto", "Palo Alto NGFW", "network", "Palo Alto next-gen firewall monitoring: sessions, threats, GlobalProtect.", None,
            vec![m("{$API_KEY}", "", "PAN-OS API key")],
            vec![check("System Info", "http", serde_json::json!({"url": "https://{$HOST}/api/?type=op&cmd=<show><system><info></info></system></show>"}), 120, None, None)],
            vec![alert("Threat Detected", "threat_count > 0", "warning", "Threats detected")],
            vec!["sessions", "threats_blocked", "gp_users"], &now),
        make_template("juniper-junos", "Juniper JunOS", "network", "Juniper switch/router monitoring via SNMP: routing, interfaces, chassis alarms.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community")],
            vec![check("Chassis Alarm", "snmp", serde_json::json!({"oid": "1.3.6.1.4.1.2636.3.4.2.2.1"}), 120, None, None)],
            vec![alert("Chassis Alarm", "alarm_count > 0", "critical", "Chassis alarm active")],
            vec!["cpu", "memory", "temperature", "re_status"], &now),
        make_template("mikrotik-routeros", "MikroTik RouterOS", "network", "MikroTik router monitoring via SNMP/API: bandwidth, firewall, queues.", None,
            vec![m("{$API_PORT}", "8728", "RouterOS API port")],
            vec![check("RouterOS API", "tcp", serde_json::json!({"port": 8728}), 60, None, None)],
            vec![alert("Board Temp High", "temperature > 70", "warning", "Board temperature high")],
            vec!["cpu", "memory", "disk", "if_bandwidth", "firewall_connections"], &now),
        make_template("aruba-wireless", "Aruba Wireless", "network", "Aruba wireless controller monitoring: APs, clients, channel utilization.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community")],
            vec![check("AP Count", "snmp", serde_json::json!({"oid": "1.3.6.1.4.1.14823.2.2.1.1.3.1.0"}), 120, None, None)],
            vec![alert("AP Down", "ap_down_count > 0", "warning", "Access point down")],
            vec!["aps_up", "aps_down", "clients_connected", "channel_utilization"], &now),
        make_template("dell-networking", "Dell Networking", "network", "Dell PowerSwitch monitoring via SNMP.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community")],
            vec![check("SNMP Reachable", "snmp", serde_json::json!({"oid": "1.3.6.1.2.1.1.1.0"}), 120, None, None)],
            vec![alert("Stack Unit Fail", "stack_unit_status != 1", "critical", "Stack unit failure")],
            vec!["cpu", "memory", "if_bandwidth", "psu_status"], &now),
        make_template("hpe-aruba-switch", "HPE Aruba Switch", "network", "HPE ProCurve/Aruba switch monitoring via SNMP.", None,
            vec![m("{$SNMP_COMMUNITY}", "public", "SNMP community")],
            vec![check("SNMP Reachable", "snmp", serde_json::json!({"oid": "1.3.6.1.2.1.1.1.0"}), 120, None, None)],
            vec![alert("Fan Failure", "fan_status != normal", "critical", "Fan failure detected")],
            vec!["cpu", "memory", "if_bandwidth", "poe_usage"], &now),
        make_template("snmpv3-generic", "SNMP v3 Generic", "network", "Generic SNMP v3 device monitoring with auth/priv support.", None,
            vec![m("{$SNMPv3_USER}", "admin", "SNMPv3 username"), m("{$SNMPv3_AUTH}", "SHA", "Auth protocol"), m("{$SNMPv3_PRIV}", "AES", "Privacy protocol")],
            vec![check("SNMP v3 Uptime", "snmpv3", serde_json::json!({"oid": "1.3.6.1.2.1.1.3.0"}), 120, None, None)],
            vec![alert("Device Unreachable", "snmp_reachable == 0", "critical", "Device not responding to SNMP")],
            vec!["uptime", "cpu", "memory", "if_bandwidth"], &now),

        // ── Server (6 additional) ─────────────────────────────────────
        make_template("dell-idrac", "Dell iDRAC", "server", "Dell iDRAC server monitoring: hardware health, temperatures, PSU, fans, storage.", None,
            vec![m("{$IDRAC.HOST}", "", "iDRAC IP"), m("{$IDRAC.USER}", "root", "iDRAC user")],
            vec![check("iDRAC Health", "http", serde_json::json!({"url": "https://{$IDRAC.HOST}/redfish/v1/Systems/System.Embedded.1"}), 300, None, None)],
            vec![alert("Hardware Error", "hardware_status != ok", "critical", "Hardware error detected"), alert("Temp High", "inlet_temp > 35", "warning", "Inlet temperature high")],
            vec!["inlet_temp", "cpu_temp", "fan_speed", "psu_watts", "storage_health"], &now),
        make_template("hpe-ilo", "HPE iLO", "server", "HPE iLO server monitoring: hardware health, temperatures, power, firmware.", None,
            vec![m("{$ILO.HOST}", "", "iLO IP"), m("{$ILO.USER}", "Administrator", "iLO user")],
            vec![check("iLO Health", "http", serde_json::json!({"url": "https://{$ILO.HOST}/redfish/v1/Systems/1"}), 300, None, None)],
            vec![alert("Health Degraded", "system_health != OK", "warning", "System health degraded")],
            vec!["system_health", "cpu_temp", "inlet_temp", "power_watts"], &now),
        make_template("macos-server", "macOS", "server", "macOS monitoring: CPU, memory, disk, load, Time Machine, spotlight.", None,
            vec![m("{$CPU_WARN}", "80", "CPU warning %")],
            vec![check("CPU", "custom_script", serde_json::json!({"command": "sysctl -n hw.ncpu"}), 60, Some("80"), Some("95")), check("Memory", "custom_script", serde_json::json!({"command": "vm_stat"}), 60, None, None)],
            vec![alert("High CPU", "cpu > {$CPU_WARN}", "warning", "macOS CPU high")],
            vec!["cpu", "memory", "disk", "load_avg"], &now),
        make_template("freebsd-server", "FreeBSD", "server", "FreeBSD server monitoring: CPU, memory, ZFS, jails.", None,
            vec![m("{$CPU_WARN}", "80", "CPU warning %")],
            vec![check("CPU", "custom_script", serde_json::json!({"command": "sysctl -n kern.cp_time"}), 60, Some("80"), Some("95"))],
            vec![alert("High CPU", "cpu > {$CPU_WARN}", "warning", "FreeBSD CPU high")],
            vec!["cpu", "memory", "zfs_arc", "jail_count"], &now),
        make_template("lenovo-xclarity", "Lenovo XClarity", "server", "Lenovo server hardware monitoring via XClarity Controller.", None,
            vec![m("{$XCC.HOST}", "", "XClarity IP")],
            vec![check("XCC Health", "http", serde_json::json!({"url": "https://{$XCC.HOST}/redfish/v1/Systems"}), 300, None, None)],
            vec![alert("Health Degraded", "health != OK", "warning", "Hardware health degraded")],
            vec!["cpu_temp", "inlet_temp", "fan_speed", "power_watts"], &now),
        make_template("supermicro-ipmi", "Supermicro IPMI", "server", "Supermicro server hardware monitoring via IPMI.", None,
            vec![m("{$IPMI.HOST}", "", "IPMI IP")],
            vec![check("IPMI Sensors", "ipmi", serde_json::json!({"host": "{$IPMI.HOST}"}), 300, None, None)],
            vec![alert("Sensor Critical", "sensor_status == critical", "critical", "IPMI sensor critical")],
            vec!["cpu_temp", "fan_rpm", "psu_voltage", "power_watts"], &now),

        // ── Database (6 additional) ───────────────────────────────────
        make_template("cassandra", "Apache Cassandra", "database", "Cassandra cluster monitoring via JMX: nodes, latency, compaction, gossip.", Some("linux-server"),
            vec![m("{$JMX_PORT}", "7199", "JMX port")],
            vec![check("Cassandra JMX", "tcp", serde_json::json!({"port": 7199}), 60, None, None)],
            vec![alert("Node Down", "node_status == DN", "critical", "Cassandra node down"), alert("Compaction Pending", "compaction_pending > 100", "warning", "High pending compactions")],
            vec!["nodes_up", "read_latency", "write_latency", "compaction_pending", "heap_usage"], &now),
        make_template("cockroachdb", "CockroachDB", "database", "CockroachDB cluster monitoring via HTTP admin UI.", Some("linux-server"),
            vec![m("{$CRDB_PORT}", "8080", "Admin UI port")],
            vec![check("CRDB Health", "http", serde_json::json!({"url": "http://{$HOST}:{$CRDB_PORT}/_status/vars"}), 60, None, None)],
            vec![alert("Unavailable Ranges", "unavailable_ranges > 0", "critical", "Unavailable ranges detected")],
            vec!["live_nodes", "ranges", "queries_per_sec", "storage_used"], &now),
        make_template("tidb", "TiDB", "database", "TiDB distributed database monitoring via Prometheus metrics.", Some("linux-server"),
            vec![m("{$STATUS_PORT}", "10080", "TiDB status port")],
            vec![check("TiDB Status", "http", serde_json::json!({"url": "http://{$HOST}:{$STATUS_PORT}/status"}), 60, None, None)],
            vec![alert("TiDB Down", "tidb_alive == 0", "critical", "TiDB server down")],
            vec!["qps", "statement_count", "connection_count", "tikv_leader_count"], &now),
        make_template("influxdb", "InfluxDB", "database", "InfluxDB time-series database monitoring.", Some("linux-server"),
            vec![m("{$INFLUX_PORT}", "8086", "InfluxDB port")],
            vec![check("InfluxDB Health", "http", serde_json::json!({"url": "http://{$HOST}:{$INFLUX_PORT}/health"}), 60, None, None)],
            vec![alert("InfluxDB Down", "influx_alive == 0", "critical", "InfluxDB not responding")],
            vec!["writes_per_sec", "queries_per_sec", "series_count", "memory_usage"], &now),
        make_template("clickhouse", "ClickHouse", "database", "ClickHouse analytics database monitoring.", Some("linux-server"),
            vec![m("{$CH_PORT}", "8123", "ClickHouse HTTP port")],
            vec![check("ClickHouse Ping", "http", serde_json::json!({"url": "http://{$HOST}:{$CH_PORT}/ping"}), 60, None, None)],
            vec![alert("ClickHouse Down", "ch_alive == 0", "critical", "ClickHouse not responding")],
            vec!["queries_per_sec", "inserts_per_sec", "merge_count", "replicated_parts_to_check"], &now),
        make_template("elasticsearch-v2", "Elasticsearch Cluster (v2)", "database", "Enhanced Elasticsearch monitoring with shard detail.", Some("linux-server"),
            vec![m("{$ES_PORT}", "9200", "Elasticsearch port")],
            vec![check("ES Cluster Health", "http", serde_json::json!({"url": "http://{$HOST}:{$ES_PORT}/_cluster/health"}), 30, None, None)],
            vec![alert("Cluster Red", "es_status == red", "critical", "Elasticsearch cluster red"), alert("Unassigned Shards", "es_unassigned > 0", "warning", "Unassigned shards")],
            vec!["cluster_status", "nodes", "data_nodes", "active_shards", "unassigned_shards", "pending_tasks"], &now),

        // ── Middleware (8 additional) ──────────────────────────────────
        make_template("jenkins", "Jenkins CI", "middleware", "Jenkins CI/CD server monitoring: jobs, queue, executors, plugins.", Some("linux-server"),
            vec![m("{$JENKINS_PORT}", "8080", "Jenkins port"), m("{$JENKINS_USER}", "admin", "Jenkins user")],
            vec![check("Jenkins Health", "http", serde_json::json!({"url": "http://{$HOST}:{$JENKINS_PORT}/api/json"}), 60, None, None)],
            vec![alert("Jenkins Down", "jenkins_alive == 0", "critical", "Jenkins not reachable"), alert("Queue Stuck", "queue_size > 20", "warning", "Build queue backing up")],
            vec!["jobs_total", "jobs_running", "queue_size", "executors_busy", "executors_idle"], &now),
        make_template("gitlab", "GitLab", "middleware", "GitLab DevOps platform monitoring.", Some("linux-server"),
            vec![m("{$GITLAB_PORT}", "443", "GitLab port")],
            vec![check("GitLab Health", "http", serde_json::json!({"url": "https://{$HOST}/-/health"}), 60, None, None)],
            vec![alert("GitLab Down", "gitlab_alive == 0", "critical", "GitLab not reachable")],
            vec!["sidekiq_jobs", "gitaly_requests", "puma_workers", "db_connections"], &now),
        make_template("hashicorp-vault", "HashiCorp Vault", "middleware", "HashiCorp Vault secrets management monitoring.", Some("linux-server"),
            vec![m("{$VAULT_PORT}", "8200", "Vault port")],
            vec![check("Vault Health", "http", serde_json::json!({"url": "http://{$HOST}:{$VAULT_PORT}/v1/sys/health"}), 60, None, None)],
            vec![alert("Vault Sealed", "vault_sealed == true", "critical", "Vault is sealed"), alert("Vault Standby", "vault_standby == true", "warning", "Vault is in standby")],
            vec!["vault_sealed", "vault_standby", "token_count", "lease_count"], &now),
        make_template("consul", "HashiCorp Consul", "middleware", "Consul service mesh monitoring.", Some("linux-server"),
            vec![m("{$CONSUL_PORT}", "8500", "Consul port")],
            vec![check("Consul Health", "http", serde_json::json!({"url": "http://{$HOST}:{$CONSUL_PORT}/v1/status/leader"}), 60, None, None)],
            vec![alert("No Leader", "consul_leader == ''", "critical", "Consul has no leader")],
            vec!["services", "nodes", "checks_passing", "checks_critical"], &now),
        make_template("envoy-proxy", "Envoy Proxy", "middleware", "Envoy proxy sidecar monitoring via admin interface.", Some("linux-server"),
            vec![m("{$ENVOY_ADMIN_PORT}", "9901", "Envoy admin port")],
            vec![check("Envoy Stats", "http", serde_json::json!({"url": "http://{$HOST}:{$ENVOY_ADMIN_PORT}/stats"}), 30, None, None)],
            vec![alert("High 5xx", "envoy_5xx_rate > 5", "warning", "Envoy 5xx rate high")],
            vec!["envoy_rq_total", "envoy_cx_active", "envoy_5xx", "envoy_4xx"], &now),
        make_template("traefik", "Traefik", "middleware", "Traefik reverse proxy/load balancer monitoring.", Some("linux-server"),
            vec![m("{$TRAEFIK_API_PORT}", "8080", "Traefik API port")],
            vec![check("Traefik Health", "http", serde_json::json!({"url": "http://{$HOST}:{$TRAEFIK_API_PORT}/ping"}), 30, None, None)],
            vec![alert("Traefik Down", "traefik_alive == 0", "critical", "Traefik not reachable")],
            vec!["entrypoint_requests", "service_requests", "open_connections"], &now),
        make_template("minio", "MinIO", "middleware", "MinIO S3-compatible object storage monitoring.", Some("linux-server"),
            vec![m("{$MINIO_PORT}", "9000", "MinIO port")],
            vec![check("MinIO Health", "http", serde_json::json!({"url": "http://{$HOST}:{$MINIO_PORT}/minio/health/live"}), 60, None, None)],
            vec![alert("MinIO Down", "minio_alive == 0", "critical", "MinIO not reachable")],
            vec!["objects_count", "storage_used", "storage_free", "requests_per_sec"], &now),
        make_template("ceph-rgw", "Ceph RGW", "middleware", "Ceph RADOS Gateway S3/Swift monitoring.", Some("linux-server"),
            vec![m("{$RGW_PORT}", "7480", "RGW port")],
            vec![check("RGW Health", "http", serde_json::json!({"url": "http://{$HOST}:{$RGW_PORT}/swift/healthcheck"}), 60, None, None)],
            vec![alert("RGW Down", "rgw_alive == 0", "critical", "Ceph RGW not reachable")],
            vec!["requests_per_sec", "successful_ops", "failed_ops", "bandwidth"], &now),

        // ── Cloud (6) ─────────────────────────────────────────────────
        make_template("aws-cloudwatch", "AWS CloudWatch", "cloud", "AWS CloudWatch integration: EC2, RDS, ELB, Lambda metrics.", None,
            vec![m("{$AWS_REGION}", "us-east-1", "AWS region"), m("{$AWS_ACCESS_KEY}", "", "AWS access key")],
            vec![check("CloudWatch API", "http", serde_json::json!({"url": "https://monitoring.{$AWS_REGION}.amazonaws.com"}), 300, None, None)],
            vec![alert("EC2 High CPU", "ec2_cpu > 80", "warning", "EC2 CPU high")],
            vec!["ec2_cpu", "ec2_network_in", "rds_connections", "elb_request_count", "lambda_invocations"], &now),
        make_template("azure-monitor", "Azure Monitor", "cloud", "Azure Monitor integration: VMs, SQL, App Services, AKS.", None,
            vec![m("{$AZURE_TENANT_ID}", "", "Azure tenant ID"), m("{$AZURE_CLIENT_ID}", "", "Azure client ID")],
            vec![check("Azure Health", "http", serde_json::json!({"url": "https://management.azure.com/providers/Microsoft.ResourceHealth"}), 300, None, None)],
            vec![alert("VM High CPU", "vm_cpu > 80", "warning", "Azure VM CPU high")],
            vec!["vm_cpu", "vm_memory", "sql_dtu", "app_service_requests"], &now),
        make_template("gcp-stackdriver", "GCP Cloud Monitoring", "cloud", "Google Cloud Monitoring integration: Compute, GKE, Cloud SQL.", None,
            vec![m("{$GCP_PROJECT}", "", "GCP project ID")],
            vec![check("GCP Health", "http", serde_json::json!({"url": "https://monitoring.googleapis.com/v3/projects/{$GCP_PROJECT}"}), 300, None, None)],
            vec![alert("Instance High CPU", "instance_cpu > 80", "warning", "GCP instance CPU high")],
            vec!["instance_cpu", "instance_memory", "gke_pod_count", "cloudsql_connections"], &now),
        make_template("digitalocean", "DigitalOcean", "cloud", "DigitalOcean monitoring: Droplets, Kubernetes, databases.", None,
            vec![m("{$DO_TOKEN}", "", "DigitalOcean API token")],
            vec![check("DO API", "http", serde_json::json!({"url": "https://api.digitalocean.com/v2/account"}), 300, None, None)],
            vec![alert("Droplet Down", "droplet_status != active", "critical", "Droplet not active")],
            vec!["droplets", "cpu_usage", "memory_usage", "bandwidth"], &now),
        make_template("linode", "Linode", "cloud", "Linode monitoring: Linodes, NodeBalancers, LKE.", None,
            vec![m("{$LINODE_TOKEN}", "", "Linode API token")],
            vec![check("Linode API", "http", serde_json::json!({"url": "https://api.linode.com/v4/profile"}), 300, None, None)],
            vec![alert("Linode Offline", "linode_status != running", "critical", "Linode offline")],
            vec!["linodes", "cpu", "memory", "network_in", "network_out"], &now),
        make_template("hetzner", "Hetzner Cloud", "cloud", "Hetzner Cloud monitoring: servers, load balancers, volumes.", None,
            vec![m("{$HETZNER_TOKEN}", "", "Hetzner API token")],
            vec![check("Hetzner API", "http", serde_json::json!({"url": "https://api.hetzner.cloud/v1/servers"}), 300, None, None)],
            vec![alert("Server Offline", "server_status != running", "critical", "Hetzner server offline")],
            vec!["servers", "cpu", "memory", "disk", "traffic"], &now),

        // ── SAN (4) ───────────────────────────────────────────────────
        make_template("hpe-msa", "HPE MSA SAN", "san", "HPE MSA storage array monitoring: volumes, disks, controllers, pools.", None,
            vec![m("{$MSA_HOST}", "", "MSA API host"), m("{$MSA_USER}", "manage", "MSA user")],
            vec![check("MSA Health", "http", serde_json::json!({"url": "https://{$MSA_HOST}/api/show/system"}), 300, None, None)],
            vec![alert("Disk Failure", "disk_health != OK", "critical", "Disk failure in array"), alert("Pool Full", "pool_usage > 85", "warning", "Storage pool filling up")],
            vec!["pool_usage", "iops", "throughput", "disk_count", "controller_status"], &now),
        make_template("huawei-oceanstor", "Huawei OceanStor", "san", "Huawei OceanStor storage monitoring.", None,
            vec![m("{$OCEANSTOR_HOST}", "", "OceanStor host")],
            vec![check("OceanStor API", "http", serde_json::json!({"url": "https://{$OCEANSTOR_HOST}:8088/deviceManager/rest"}), 300, None, None)],
            vec![alert("Array Degraded", "health_status != normal", "warning", "Array health degraded")],
            vec!["luns", "storage_pools", "iops", "bandwidth", "latency"], &now),
        make_template("ibm-flashsystem", "IBM FlashSystem", "san", "IBM FlashSystem storage monitoring.", None,
            vec![m("{$FS_HOST}", "", "FlashSystem host")],
            vec![check("FlashSystem Health", "http", serde_json::json!({"url": "https://{$FS_HOST}/rest"}), 300, None, None)],
            vec![alert("Pool Degraded", "pool_status != online", "critical", "Pool status degraded")],
            vec!["volumes", "mdisks", "iops", "bandwidth", "compression_ratio"], &now),
        make_template("dell-powerstore", "Dell PowerStore", "san", "Dell PowerStore storage monitoring.", None,
            vec![m("{$PS_HOST}", "", "PowerStore host")],
            vec![check("PowerStore API", "http", serde_json::json!({"url": "https://{$PS_HOST}/api/version"}), 300, None, None)],
            vec![alert("Alert Active", "active_alerts > 0", "warning", "Active alerts on PowerStore")],
            vec!["volumes", "capacity_used", "iops", "bandwidth", "latency"], &now),

        // ── Container (2 additional) ──────────────────────────────────
        make_template("docker-swarm", "Docker Swarm", "container", "Docker Swarm cluster monitoring: nodes, services, tasks.", Some("docker-host"),
            vec![m("{$SWARM_MANAGER}", "", "Swarm manager URL")],
            vec![check("Swarm Status", "custom_script", serde_json::json!({"command": "docker info --format '{{.Swarm.LocalNodeState}}'"}), 60, None, None)],
            vec![alert("Node Down", "swarm_node_down > 0", "critical", "Swarm node is down")],
            vec!["swarm_nodes", "swarm_services", "swarm_tasks_running", "swarm_tasks_failed"], &now),
        make_template("k8s-pod-detail", "Kubernetes Pod Detail", "container", "Detailed per-pod monitoring: resource usage, restarts, OOMKills.", None,
            vec![m("{$K8S.NAMESPACE}", "default", "Namespace"), m("{$K8S.POD_NAME}", "", "Pod name pattern")],
            vec![check("Pod Status", "http", serde_json::json!({"url": "{$K8S.API_SERVER}/api/v1/namespaces/{$K8S.NAMESPACE}/pods"}), 30, None, None)],
            vec![alert("OOMKilled", "oom_killed > 0", "critical", "Pod was OOMKilled")],
            vec!["cpu_usage", "memory_usage", "restarts", "oom_kills"], &now),

        // ── IoT/OT (4) ───────────────────────────────────────────────
        make_template("modbus-tcp", "Modbus TCP", "iot", "Modbus TCP device monitoring: registers, coils, discrete inputs.", None,
            vec![m("{$MODBUS_PORT}", "502", "Modbus port"), m("{$UNIT_ID}", "1", "Modbus unit ID")],
            vec![check("Modbus Connect", "tcp", serde_json::json!({"port": 502}), 60, None, None)],
            vec![alert("Device Offline", "modbus_reachable == 0", "critical", "Modbus device not reachable")],
            vec!["registers", "coils", "connection_time"], &now),
        make_template("opcua", "OPC-UA", "iot", "OPC-UA server monitoring: nodes, subscriptions, server status.", None,
            vec![m("{$OPCUA_PORT}", "4840", "OPC-UA port"), m("{$OPCUA_ENDPOINT}", "opc.tcp://{$HOST}:{$OPCUA_PORT}", "OPC-UA endpoint")],
            vec![check("OPC-UA Connect", "tcp", serde_json::json!({"port": 4840}), 60, None, None)],
            vec![alert("Server Down", "opcua_reachable == 0", "critical", "OPC-UA server not reachable")],
            vec!["server_state", "session_count", "subscription_count", "monitored_items"], &now),
        make_template("mqtt-broker", "MQTT Broker", "iot", "MQTT broker monitoring: clients, messages, subscriptions (Mosquitto/EMQX).", None,
            vec![m("{$MQTT_PORT}", "1883", "MQTT port"), m("{$MQTT_WS_PORT}", "8083", "MQTT WebSocket port")],
            vec![check("MQTT Connect", "tcp", serde_json::json!({"port": 1883}), 30, None, None)],
            vec![alert("Broker Down", "mqtt_reachable == 0", "critical", "MQTT broker not reachable"), alert("High Clients", "mqtt_clients > 10000", "warning", "High client count")],
            vec!["connected_clients", "messages_received", "messages_sent", "subscriptions", "retained_messages"], &now),
        make_template("snmp-trap", "SNMP Trap Receiver", "iot", "SNMP trap receiver monitoring: trap count, sources, community errors.", None,
            vec![m("{$TRAP_PORT}", "162", "SNMP trap port")],
            vec![check("Trap Listener", "tcp", serde_json::json!({"port": 162}), 60, None, None)],
            vec![alert("Auth Failure Trap", "auth_failure_traps > 10", "warning", "Frequent SNMP auth failure traps")],
            vec!["traps_received", "trap_sources", "auth_failures", "coldstart_traps"], &now),
    ]
}

// ---------------------------------------------------------------------------
// Helper builders
// ---------------------------------------------------------------------------

fn make_template(
    id: &str, name: &str, category: &str, description: &str,
    parent: Option<&str>,
    macros: Vec<TemplateMacro>,
    checks: Vec<TemplateCheck>,
    alerts: Vec<TemplateAlert>,
    metrics: Vec<&str>,
    now: &str,
) -> MonitoringTemplate {
    MonitoringTemplate {
        id: id.to_string(),
        name: name.to_string(),
        category: category.to_string(),
        description: description.to_string(),
        version: "1.0".to_string(),
        parent_template: parent.map(|s| s.to_string()),
        macros,
        checks,
        alerts,
        metrics: metrics.into_iter().map(|s| s.to_string()).collect(),
        dashboards: vec![],
        created_at: now.to_string(),
        updated_at: now.to_string(),
    }
}

fn m(key: &str, default: &str, desc: &str) -> TemplateMacro {
    TemplateMacro { key: key.to_string(), default: default.to_string(), description: desc.to_string() }
}

fn check(name: &str, check_type: &str, config: serde_json::Value, interval: u64, warn: Option<&str>, crit: Option<&str>) -> TemplateCheck {
    TemplateCheck {
        name: name.to_string(),
        check_type: check_type.to_string(),
        config,
        interval_secs: interval,
        warning_threshold: warn.map(|s| s.to_string()),
        critical_threshold: crit.map(|s| s.to_string()),
    }
}

fn alert(name: &str, expr: &str, severity: &str, msg: &str) -> TemplateAlert {
    TemplateAlert { name: name.to_string(), expression: expr.to_string(), severity: severity.to_string(), message: msg.to_string() }
}
