use axum::{extract::{State, Path, Query}, Json};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_core::ssh_utils;
use crate::validation;
use zentra_core::models::target::{CreateTargetRequest, Target, TargetCredentials, TargetSubtype};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tokio::process::Command;
use validator::Validate;
use super::{middleware_nginx, middleware_apache, middleware_elasticsearch,
    middleware_haproxy, middleware_tomcat, middleware_phpfpm,
    middleware_rabbitmq, middleware_kafka, middleware_memcached,
    middleware_redis};

#[derive(Deserialize)]
pub struct ListTargetsQuery {
    pub target_type: Option<String>,
    pub active_only: Option<bool>,
}

pub async fn list_targets(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ListTargetsQuery>,
) -> Result<Json<Vec<Target>>, ApiError> {
    let targets = zentra_db::repositories::targets::list_targets(
        &state.pool,
        org_id,
        params.target_type.as_deref(),
        params.active_only.unwrap_or(true),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(targets))
}

pub async fn get_target(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<Target>, ApiError> {
    let target = zentra_db::repositories::targets::get_target_scoped(&state.pool, id, org_id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Target not found".into()))?;

    Ok(Json(target))
}

pub async fn create_target(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateTargetRequest>,
) -> Result<Json<Target>, ApiError> {
    if !validation::is_safe_ssh_host(&req.host) {
        return Err(ApiError::BadRequest("Invalid host: this address is not allowed for security reasons".into()));
    }
    let mut target = Target::new(
        org_id,
        &req.name,
        req.target_subtype.clone(),
        &req.host,
        req.port,
    );

    target.description = req.description.clone();
    target.environment = req.environment.clone();
    target.tags = req.tags.clone();

    if let Some(interval) = req.check_interval_seconds {
        target.check_interval_seconds = interval;
    }

    if req.username.is_some() || req.password.is_some() || req.database_name.is_some() {
        let encrypted_pass = match &req.password {
            Some(p) if !p.is_empty() => Some(
                zentra_core::crypto::encrypt(p)
                    .map_err(|e| ApiError::Internal(format!("encrypt: {e}")))?
            ),
            _ => None,
        };
        target.credentials = Some(TargetCredentials {
            username: req.username.clone(),
            password_encrypted: encrypted_pass,
            database_name: req.database_name.clone(),
            ssl_enabled: req.ssl_enabled,
        });
    }

    zentra_db::repositories::targets::insert_target(&state.pool, &target)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Auto-register middleware instances into in-memory stores
    register_middleware_instance(&target);

    Ok(Json(target))
}

/// Load all middleware targets from the database and register them into in-memory
/// stores. Called once at server startup so middleware pages are populated
/// immediately without waiting for a crawl or manual target creation.
pub async fn load_middleware_from_db(pool: &sqlx::PgPool, org_id: Uuid) {
    let targets = zentra_db::repositories::targets::list_targets(pool, org_id, Some("middleware"), true)
        .await
        .unwrap_or_default();
    let mut count = 0usize;
    for t in &targets {
        register_middleware_instance(t);
        count += 1;
    }
    if count > 0 {
        tracing::info!(count, "Loaded middleware instances from DB into in-memory stores");
    }
}

/// Push a newly created target into the corresponding in-memory middleware store
/// so it appears on the Middleware pages and dashboard immediately.
fn register_middleware_instance(target: &Target) {
    let now = chrono::Utc::now().to_rfc3339();
    let id = target.target_id.to_string();
    let name = target.name.clone();
    let host = target.host.clone();
    let port = target.port;
    let status = "unknown".to_string();

    match target.target_subtype {
        TargetSubtype::Nginx => {
            middleware_nginx::live_instances().write().push(
                middleware_nginx::NginxInstance {
                    id, name, host, port, status, created_at: now,
                    stub_status_path: "/nginx_status".into(),
                }
            );
        }
        TargetSubtype::Apache => {
            middleware_apache::live_instances().write().push(
                middleware_apache::ApacheInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::Elasticsearch => {
            middleware_elasticsearch::live_instances().write().push(
                middleware_elasticsearch::ElasticsearchInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::HAProxy => {
            middleware_haproxy::live_instances().write().push(
                middleware_haproxy::HAProxyInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::Tomcat => {
            middleware_tomcat::live_instances().write().push(
                middleware_tomcat::TomcatInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::PhpFpm => {
            middleware_phpfpm::live_instances().write().push(
                middleware_phpfpm::PhpFpmInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::RabbitMQ => {
            middleware_rabbitmq::live_instances().write().push(
                middleware_rabbitmq::RabbitMQInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::Kafka => {
            middleware_kafka::live_instances().write().push(
                middleware_kafka::KafkaInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::Memcached => {
            middleware_memcached::live_instances().write().push(
                middleware_memcached::MemcachedInstance { id, name, host, port, status, created_at: now }
            );
        }
        TargetSubtype::Redis => {
            // Redis can be both a database target and a middleware instance
            middleware_redis::live_instances().write().push(
                middleware_redis::RedisInstance { id, name, host, port, status, created_at: now }
            );
        }
        _ => {} // Non-middleware subtypes — no action needed
    }
}

pub async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    zentra_db::repositories::targets::delete_target(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let target = zentra_db::repositories::targets::get_target(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Target not found".into()))?;
    let result = tcp_test(&target.host, target.port).await;
    Ok(Json(serde_json::json!({
        "target_id": id,
        "reachable": result.reachable,
        "latency_ms": result.latency_ms,
        "error": result.error,
    })))
}

/// Pre-create connection test — does NOT save target.
/// Performs TCP reachability + (where possible) protocol-level handshake.
#[derive(Deserialize)]
pub struct TestDbConnectionRequest {
    pub host: String,
    pub port: u16,
    pub target_subtype: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub database_name: Option<String>,
}

#[derive(Serialize)]
struct TestResult {
    reachable: bool,
    latency_ms: u64,
    error: Option<String>,
}

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

async fn tcp_test(host: &str, port: u16) -> TestResult {
    let addr = format!("{}:{}", host, port);
    let start = std::time::Instant::now();
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr),
    ).await {
        Ok(Ok(_)) => TestResult {
            reachable: true,
            latency_ms: start.elapsed().as_millis() as u64,
            error: None,
        },
        Ok(Err(e)) => TestResult {
            reachable: false,
            latency_ms: 0,
            error: Some(format!("TCP connect failed: {}", e)),
        },
        Err(_) => TestResult {
            reachable: false,
            latency_ms: 0,
            error: Some(format!("Timeout after 5s connecting to {}", addr)),
        },
    }
}

pub async fn test_db_connection(
    Json(req): Json<TestDbConnectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.host.trim().is_empty() {
        return Ok(Json(serde_json::json!({
            "reachable": false, "latency_ms": 0,
            "error": "Host is required",
        })));
    }
    let tcp = tcp_test(&req.host, req.port).await;
    let mut detail = String::new();

    // Protocol-level test for PostgreSQL: try a real sqlx connection
    if tcp.reachable && req.target_subtype.as_deref() == Some("postgresql") {
        let user = req.username.clone().unwrap_or_else(|| "postgres".into());
        let pass = req.password.clone().unwrap_or_default();
        let db = req.database_name.clone().unwrap_or_else(|| "postgres".into());
        let url = format!(
            "postgres://{}:{}@{}:{}/{}?connect_timeout=5",
            url_encode(&user),
            url_encode(&pass),
            req.host,
            req.port,
            url_encode(&db),
        );
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&url)
            .await
        {
            Ok(pool) => {
                let v: Result<String, _> = sqlx::query_scalar("SELECT version()").fetch_one(&pool).await;
                match v {
                    Ok(ver) => detail = format!("Connected — {}", ver.chars().take(80).collect::<String>()),
                    Err(e) => return Ok(Json(serde_json::json!({
                        "reachable": false, "latency_ms": tcp.latency_ms,
                        "error": format!("Auth/protocol error: {}", e),
                    }))),
                }
                pool.close().await;
            }
            Err(e) => {
                return Ok(Json(serde_json::json!({
                    "reachable": false, "latency_ms": tcp.latency_ms,
                    "error": format!("PostgreSQL connect failed: {}", e),
                })));
            }
        }
    }

    Ok(Json(serde_json::json!({
        "reachable": tcp.reachable,
        "latency_ms": tcp.latency_ms,
        "error": tcp.error,
        "detail": detail,
    })))
}

// ============================================================================
// SSH Connectivity Verification (pre-create check, does NOT save target)
// ============================================================================

#[derive(Deserialize, Validate)]
pub struct VerifySshRequest {
    #[validate(length(min = 1, max = 255))]
    pub host: String,
    #[validate(length(max = 64))]
    pub username: Option<String>,
}

pub async fn verify_ssh(
    Json(req): Json<VerifySshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    req.validate().map_err(|e| ApiError::BadRequest(e.to_string()))?;
    if !validation::is_safe_ssh_host(&req.host) {
        return Err(ApiError::BadRequest("Invalid host: this address is not allowed for security reasons".into()));
    }
    let user = req.username.unwrap_or_else(|| {
        std::env::var("ZENTRA_SSH_USER").unwrap_or_else(|_| "root".into())
    });
    let result = zentra_core::ssh_pool::pooled_ssh_exec(&req.host, &user, "echo ZENTRA_VERIFY_OK && hostname").await;

    if result.success && result.stdout.contains("ZENTRA_VERIFY_OK") {
        let hostname = result.stdout.lines().nth(1).unwrap_or("").trim().to_string();
        Ok(Json(serde_json::json!({
            "reachable": true,
            "host": req.host,
            "hostname": hostname,
            "message": format!("SSH connection to {} succeeded", req.host),
        })))
    } else {
        Ok(Json(serde_json::json!({
            "reachable": false,
            "host": req.host,
            "hostname": "",
            "message": ssh_utils::friendly_error(&req.host, &result.stderr),
        })))
    }
}

// ============================================================================
// SSH Key Setup — automatically push the Zentra server's public key
// ============================================================================

#[derive(Deserialize)]
pub struct SetupSshRequest {
    pub host: String,
    pub username: String,
    pub password: String,
}

pub async fn setup_ssh(
    Json(req): Json<SetupSshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !validation::is_safe_ssh_host(&req.host) {
        return Err(ApiError::BadRequest("Invalid host".into()));
    }
    if !validation::is_safe_username(&req.username) {
        return Err(ApiError::BadRequest("Invalid username".into()));
    }
    if req.password.is_empty() || req.password.len() > 256 {
        return Err(ApiError::BadRequest("Invalid password".into()));
    }
    let result = ssh_utils::setup_ssh_key_auth(&req.host, &req.username, &req.password).await;

    if result.success {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "SSH key setup successful",
            "host": req.host,
            "username": req.username,
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": false,
            "message": ssh_utils::friendly_error(&req.host, &result.stderr),
            "host": req.host,
            "username": req.username,
        })))
    }
}

// ============================================================================
// Real Server Stats via SSH
// ============================================================================

#[derive(Serialize, Clone)]
pub struct DiskInfo {
    pub filesystem: String,
    pub mount: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub usage_percent: f64,
    pub fs_type: String,
}

#[derive(Serialize, Clone)]
pub struct NfsMount {
    pub remote: String,
    pub mount: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub usage_percent: f64,
}

#[derive(Serialize)]
pub struct ServerStats {
    pub target_id: String,
    pub host: String,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub cpu_cores: u32,
    pub cpu_usage_percent: f64,
    pub cpu_model: String,
    pub load_avg_1: f64,
    pub load_avg_5: f64,
    pub load_avg_15: f64,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
    pub memory_usage_percent: f64,
    pub swap_total_mb: u64,
    pub swap_used_mb: u64,
    pub disk_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_usage_percent: f64,
    pub uptime_seconds: u64,
    pub uptime_display: String,
    pub process_count: u32,
    pub logged_in_users: u32,
    pub reachable: bool,
    pub error: Option<String>,
    pub disks: Vec<DiskInfo>,
    pub nfs_mounts: Vec<NfsMount>,
}

/// SSH into a target server and collect real system stats
#[tracing::instrument(skip(state), fields(otel.kind = "client", db.system = "ssh"))]
pub async fn get_target_stats(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ServerStats>, ApiError> {
    let target = zentra_db::repositories::targets::get_target(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Target not found".into()))?;

    let host = &target.host;
    let user = target.credentials.as_ref()
        .and_then(|c| c.username.clone())
        .unwrap_or_else(|| "root".to_string());

    // Single SSH command that gathers all stats in one shot
    let stats_script = r#"
echo "===HOSTNAME==="
hostname 2>/dev/null || echo "unknown"
echo "===OS==="
(grep '^PRETTY_NAME=' /etc/os-release 2>/dev/null || grep '^ID=' /etc/os-release 2>/dev/null || echo "NAME=$(uname -s)") | head -1 | cut -d= -f2 | tr -d '"'
echo "===KERNEL==="
uname -r 2>/dev/null || echo "unknown"
echo "===CPU_CORES==="
nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo 2>/dev/null || echo "0"
echo "===CPU_MODEL==="
grep 'model name' /proc/cpuinfo 2>/dev/null | head -1 | cut -d: -f2 | sed 's/^ //' || echo "unknown"
echo "===CPU_USAGE==="
top -bn1 2>/dev/null | grep 'Cpu(s)' | awk '{print 100 - $8}' || mpstat 1 1 2>/dev/null | tail -1 | awk '{print 100 - $NF}' || echo "0"
echo "===LOAD_AVG==="
cat /proc/loadavg 2>/dev/null | awk '{print $1, $2, $3}' || echo "0 0 0"
echo "===MEMORY==="
free -m 2>/dev/null | grep Mem | awk '{print $2, $3}'  || echo "0 0"
echo "===SWAP==="
free -m 2>/dev/null | grep Swap | awk '{print $2, $3}' || echo "0 0"
echo "===DISK==="
df -BG / 2>/dev/null | tail -1 | awk '{gsub("G",""); print $2, $3, $5}' || echo "0 0 0%"
echo "===DISKS==="
df -BG -x tmpfs -x devtmpfs -x squashfs -x overlay -x nsfs 2>/dev/null | tail -n +2 | grep -v '/docker/' | grep -v '/containers/' | awk '{gsub("G",""); print $1"|"$6"|"$2"|"$3"|"$4"|"$5}'
echo "===DISK_TYPES==="
df -T -BG -x tmpfs -x devtmpfs -x squashfs -x overlay -x nsfs 2>/dev/null | tail -n +2 | grep -v '/docker/' | grep -v '/containers/' | awk '{print $7"|"$2}'
echo "===NFS_MOUNTS==="
df -t nfs -t nfs4 -BG 2>/dev/null | tail -n +2 | awk '{gsub("G",""); print $1"|"$6"|"$2"|"$3"|"$4"|"$5}' || echo ""
echo "===UPTIME==="
cat /proc/uptime 2>/dev/null | awk '{print int($1)}' || echo "0"
echo "===PROCS==="
ps aux 2>/dev/null | wc -l || echo "0"
echo "===USERS==="
who 2>/dev/null | wc -l || echo "0"
echo "===END==="
"#;

    let identity = zentra_core::ssh_utils::resolve_ssh_identity();
    let mut ssh_args = vec![
        "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(), "ConnectTimeout=5".to_string(),
        "-o".to_string(), "BatchMode=yes".to_string(),
    ];
    if let Some(ref id_file) = identity {
        ssh_args.push("-i".to_string());
        ssh_args.push(id_file.clone());
    }
    ssh_args.push(format!("{}@{}", user, host));
    ssh_args.push(stats_script.to_string());

    let output = Command::new("ssh")
        .args(&ssh_args)
        .output()
        .await
        .map_err(|e| ApiError::Internal(ssh_utils::friendly_error(host, &e.to_string())))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(Json(ServerStats {
            target_id: id.to_string(),
            host: host.clone(),
            hostname: target.name.clone(),
            os: String::new(),
            kernel: String::new(),
            cpu_cores: 0,
            cpu_usage_percent: 0.0,
            cpu_model: String::new(),
            load_avg_1: 0.0,
            load_avg_5: 0.0,
            load_avg_15: 0.0,
            memory_total_mb: 0,
            memory_used_mb: 0,
            memory_usage_percent: 0.0,
            swap_total_mb: 0,
            swap_used_mb: 0,
            disk_total_gb: 0.0,
            disk_used_gb: 0.0,
            disk_usage_percent: 0.0,
            uptime_seconds: 0,
            uptime_display: String::new(),
            process_count: 0,
            logged_in_users: 0,
            reachable: false,
            error: Some(ssh_utils::friendly_error(host, &stderr)),
            disks: vec![],
            nfs_mounts: vec![],
        }));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sections = parse_sections(&stdout);

    let hostname = sections.get("HOSTNAME").cloned().unwrap_or_default().trim().to_string();
    let os = sections.get("OS").cloned().unwrap_or_default().trim().to_string();
    let kernel = sections.get("KERNEL").cloned().unwrap_or_default().trim().to_string();
    let cpu_cores: u32 = sections.get("CPU_CORES").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let cpu_model = sections.get("CPU_MODEL").cloned().unwrap_or_default().trim().to_string();
    let cpu_usage: f64 = sections.get("CPU_USAGE").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);

    let load_parts: Vec<f64> = sections.get("LOAD_AVG")
        .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_default();

    let mem_parts: Vec<u64> = sections.get("MEMORY")
        .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_default();
    let mem_total = mem_parts.first().copied().unwrap_or(0);
    let mem_used = mem_parts.get(1).copied().unwrap_or(0);

    let swap_parts: Vec<u64> = sections.get("SWAP")
        .map(|s| s.trim().split_whitespace().filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_default();

    let disk_raw = sections.get("DISK").cloned().unwrap_or_default();
    let disk_parts: Vec<&str> = disk_raw.trim().split_whitespace().collect();
    let root_disk_total: f64 = disk_parts.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let root_disk_used: f64 = disk_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);

    // Parse per-filesystem disk type mapping (mount -> fstype)
    let mut fs_type_map = std::collections::HashMap::<String, String>::new();
    if let Some(types_raw) = sections.get("DISK_TYPES") {
        for line in types_raw.lines() {
            let p: Vec<&str> = line.trim().split('|').collect();
            if p.len() == 2 {
                fs_type_map.insert(p[0].to_string(), p[1].to_string());
            }
        }
    }

    // Parse all disk partitions
    let mut disks: Vec<DiskInfo> = Vec::new();
    if let Some(disks_raw) = sections.get("DISKS") {
        for line in disks_raw.lines() {
            let p: Vec<&str> = line.trim().split('|').collect();
            if p.len() >= 5 {
                let mount = p[1].to_string();
                let total: f64 = p[2].parse().unwrap_or(0.0);
                let used: f64 = p[3].parse().unwrap_or(0.0);
                let free: f64 = p[4].parse().unwrap_or(0.0);
                let pct: f64 = p.get(5).and_then(|s| s.trim_end_matches('%').parse().ok())
                    .unwrap_or(if total > 0.0 { (used / total) * 100.0 } else { 0.0 });
                let fs_type = fs_type_map.get(&mount).cloned().unwrap_or_default();
                // Skip NFS mounts and container overlays
                if fs_type == "nfs" || fs_type == "nfs4" || fs_type == "overlay" || fs_type == "overlay2" { continue; }
                if mount.contains("/docker/") || mount.contains("/containers/") { continue; }
                disks.push(DiskInfo {
                    filesystem: p[0].to_string(),
                    mount,
                    total_gb: total,
                    used_gb: used,
                    free_gb: free,
                    usage_percent: (pct * 10.0).round() / 10.0,
                    fs_type,
                });
            }
        }
    }

    // Parse NFS mounts
    let mut nfs_mounts: Vec<NfsMount> = Vec::new();
    if let Some(nfs_raw) = sections.get("NFS_MOUNTS") {
        for line in nfs_raw.lines() {
            let p: Vec<&str> = line.trim().split('|').collect();
            if p.len() >= 5 {
                let total: f64 = p[2].parse().unwrap_or(0.0);
                let used: f64 = p[3].parse().unwrap_or(0.0);
                let free: f64 = p[4].parse().unwrap_or(0.0);
                let pct: f64 = p.get(5).and_then(|s| s.trim_end_matches('%').parse().ok())
                    .unwrap_or(if total > 0.0 { (used / total) * 100.0 } else { 0.0 });
                nfs_mounts.push(NfsMount {
                    remote: p[0].to_string(),
                    mount: p[1].to_string(),
                    total_gb: total,
                    used_gb: used,
                    free_gb: free,
                    usage_percent: (pct * 10.0).round() / 10.0,
                });
            }
        }
    }

    // Aggregate disk totals from all local partitions (or fall back to root-only)
    let (disk_total, disk_used) = if !disks.is_empty() {
        (disks.iter().map(|d| d.total_gb).sum::<f64>(), disks.iter().map(|d| d.used_gb).sum::<f64>())
    } else {
        (root_disk_total, root_disk_used)
    };
    let disk_pct = if disk_total > 0.0 { (disk_used / disk_total) * 100.0 } else { 0.0 };

    let uptime_secs: u64 = sections.get("UPTIME").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let days = uptime_secs / 86400;
    let hours = (uptime_secs % 86400) / 3600;
    let uptime_display = format!("{}d {}h", days, hours);

    let procs: u32 = sections.get("PROCS").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let users: u32 = sections.get("USERS").and_then(|s| s.trim().parse().ok()).unwrap_or(0);

    Ok(Json(ServerStats {
        target_id: id.to_string(),
        host: host.clone(),
        hostname,
        os,
        kernel,
        cpu_cores,
        cpu_usage_percent: (cpu_usage * 10.0).round() / 10.0,
        cpu_model,
        load_avg_1: load_parts.first().copied().unwrap_or(0.0),
        load_avg_5: load_parts.get(1).copied().unwrap_or(0.0),
        load_avg_15: load_parts.get(2).copied().unwrap_or(0.0),
        memory_total_mb: mem_total,
        memory_used_mb: mem_used,
        memory_usage_percent: if mem_total > 0 { ((mem_used as f64 / mem_total as f64) * 1000.0).round() / 10.0 } else { 0.0 },
        swap_total_mb: swap_parts.first().copied().unwrap_or(0),
        swap_used_mb: swap_parts.get(1).copied().unwrap_or(0),
        disk_total_gb: (disk_total * 10.0).round() / 10.0,
        disk_used_gb: (disk_used * 10.0).round() / 10.0,
        disk_usage_percent: (disk_pct * 10.0).round() / 10.0,
        uptime_seconds: uptime_secs,
        uptime_display,
        process_count: procs,
        logged_in_users: users,
        reachable: true,
        error: None,
        disks,
        nfs_mounts,
    }))
}

fn parse_sections(output: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut current_key = String::new();
    let mut current_val = String::new();

    for line in output.lines() {
        if line.starts_with("===") && line.ends_with("===") {
            if !current_key.is_empty() {
                map.insert(current_key.clone(), current_val.trim().to_string());
            }
            current_key = line.trim_matches('=').to_string();
            current_val = String::new();
        } else if !current_key.is_empty() {
            if !current_val.is_empty() {
                current_val.push('\n');
            }
            current_val.push_str(line);
        }
    }
    if !current_key.is_empty() {
        map.insert(current_key, current_val.trim().to_string());
    }
    map
}

// ============================================================================
// Server Crawl — discover components running on a server via SSH
// ============================================================================

#[derive(Serialize)]
pub struct CrawledComponent {
    pub component_type: String,
    pub subtype: String,
    pub name: String,
    pub port: u16,
    pub version: String,
    pub status: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct CrawlResult {
    pub target_id: String,
    pub host: String,
    pub hostname: String,
    pub components: Vec<CrawledComponent>,
    pub crawl_duration_ms: u64,
}

pub async fn crawl_server(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CrawlResult>, ApiError> {
    let target = zentra_db::repositories::targets::get_target(&state.pool, id)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Target not found".into()))?;

    let host = &target.host;
    let user = target.credentials.as_ref()
        .and_then(|c| c.username.clone())
        .unwrap_or_else(|| "root".to_string());

    let start = std::time::Instant::now();

    // Crawl only for components that Zentra actually monitors:
    //   Database:   Oracle
    //   Middleware:  Nginx, Apache, Tomcat
    //   Storage:    NFS
    //   Network:    SNMP, Firewall (iptables/firewalld/nftables)
    let crawl_script = r#"
echo "===HOSTNAME==="
hostname 2>/dev/null || echo "unknown"

echo "===ORACLE==="
if command -v sqlplus >/dev/null 2>&1 || ss -tlnp 2>/dev/null | grep -q ':1521 '; then
  VER=$(sqlplus -V 2>/dev/null | head -1 || echo "detected")
  PORT=$(ss -tlnp 2>/dev/null | grep ':1521 ' | head -1 | awk '{print $4}' | rev | cut -d: -f1 | rev)
  echo "found|${PORT:-1521}|${VER}"
else
  echo "not_found"
fi

echo "===NGINX==="
if command -v nginx >/dev/null 2>&1 || ss -tlnp 2>/dev/null | grep -q 'nginx'; then
  VER=$(nginx -v 2>&1 | head -1 || echo "detected")
  PORT=$(ss -tlnp 2>/dev/null | grep 'nginx' | head -1 | awk '{print $4}' | rev | cut -d: -f1 | rev)
  echo "found|${PORT:-80}|${VER}"
else
  echo "not_found"
fi

echo "===APACHE==="
if command -v httpd >/dev/null 2>&1 || command -v apache2 >/dev/null 2>&1 || ss -tlnp 2>/dev/null | grep -qE 'httpd|apache2'; then
  VER=$(httpd -v 2>/dev/null | head -1 || apache2 -v 2>/dev/null | head -1 || echo "detected")
  PORT=$(ss -tlnp 2>/dev/null | grep -E 'httpd|apache2' | head -1 | awk '{print $4}' | rev | cut -d: -f1 | rev)
  echo "found|${PORT:-80}|${VER}"
else
  echo "not_found"
fi

echo "===TOMCAT==="
if ss -tlnp 2>/dev/null | grep -q ':8080 '; then
  VER="Tomcat/App Server"
  echo "found|8080|${VER}"
else
  echo "not_found"
fi

echo "===NFS==="
if command -v showmount >/dev/null 2>&1 || ss -tlnp 2>/dev/null | grep -q ':2049 '; then
  EXPORTS=$(showmount -e 127.0.0.1 2>/dev/null | tail -n +2 | wc -l || echo "0")
  echo "found|2049|NFS|exports=${EXPORTS}"
else
  echo "not_found"
fi

echo "===SNMP==="
if ss -ulnp 2>/dev/null | grep -q ':161 '; then
  echo "found|161|SNMP agent"
else
  echo "not_found"
fi

echo "===FIREWALL==="
FW="not_found"
if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state 2>/dev/null | grep -q running; then
  VER=$(firewall-cmd --version 2>/dev/null || echo "firewalld")
  ZONES=$(firewall-cmd --get-active-zones 2>/dev/null | head -1 || echo "default")
  FW="found|0|firewalld ${VER}|zone=${ZONES}"
elif command -v nft >/dev/null 2>&1 && nft list tables 2>/dev/null | grep -q .; then
  RULES=$(nft list ruleset 2>/dev/null | grep -c 'rule' || echo "0")
  FW="found|0|nftables|rules=${RULES}"
elif command -v iptables >/dev/null 2>&1 && iptables -L -n 2>/dev/null | grep -qv '^$'; then
  RULES=$(iptables -L -n 2>/dev/null | grep -c '^' || echo "0")
  FW="found|0|iptables|rules=${RULES}"
fi
echo "${FW}"

echo "===END==="
"#;

    let identity = zentra_core::ssh_utils::resolve_ssh_identity();
    let mut ssh_args = vec![
        "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
        "-o".to_string(), "ConnectTimeout=10".to_string(),
        "-o".to_string(), "BatchMode=yes".to_string(),
    ];
    if let Some(ref id_file) = identity {
        ssh_args.push("-i".to_string());
        ssh_args.push(id_file.clone());
    }
    ssh_args.push(format!("{}@{}", user, host));
    ssh_args.push(crawl_script.to_string());

    let output = Command::new("ssh")
        .args(&ssh_args)
        .output()
        .await
        .map_err(|e| ApiError::Internal(ssh_utils::friendly_error(host, &e.to_string())))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ApiError::Internal(ssh_utils::friendly_error(host, &stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sections = parse_sections(&stdout);
    let hostname = sections.get("HOSTNAME").cloned().unwrap_or_else(|| target.name.clone()).trim().to_string();

    let mut components = Vec::new();

    // Only components Zentra monitors — nothing else
    let component_defs: Vec<(&str, &str, &str, u16)> = vec![
        ("ORACLE",   "database",   "oracle",   1521),
        ("NGINX",    "middleware",  "nginx",    80),
        ("APACHE",   "middleware",  "apache",   80),
        ("TOMCAT",   "middleware",  "tomcat",   8080),
        ("NFS",      "storage",    "nfs",      2049),
        ("SNMP",     "network",    "snmp",     161),
        ("FIREWALL", "network",    "firewall", 0),
    ];

    for (section, comp_type, subtype, default_port) in &component_defs {
        if let Some(raw) = sections.get(*section) {
            let trimmed = raw.trim();
            if trimmed != "not_found" && trimmed.starts_with("found|") {
                let parts: Vec<&str> = trimmed.splitn(4, '|').collect();
                let port: u16 = parts.get(1).and_then(|p| p.trim().parse().ok()).unwrap_or(*default_port);
                let version = parts.get(2).unwrap_or(&"detected").trim().to_string();
                let detail = parts.get(3).unwrap_or(&"").trim().to_string();

                let name = format!("{}-{}", hostname, subtype);

                components.push(CrawledComponent {
                    component_type: comp_type.to_string(),
                    subtype: subtype.to_string(),
                    name,
                    port,
                    version,
                    status: "discovered".to_string(),
                    detail,
                });
            }
        }
    }

    let duration = start.elapsed().as_millis() as u64;

    Ok(Json(CrawlResult {
        target_id: id.to_string(),
        host: host.clone(),
        hostname,
        components,
        crawl_duration_ms: duration,
    }))
}

// ============================================================================
// SW P05: Target Hierarchy — parent/child relationships
// ============================================================================

#[derive(Debug, Serialize)]
pub struct TargetHierarchyNode {
    pub target_id: Uuid,
    pub name: String,
    pub target_type: String,
    pub hierarchy_depth: i32,
}

/// GET /api/targets/:id/hierarchy — recursive CTE walking parent chain
pub async fn get_target_hierarchy(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TargetHierarchyNode>>, ApiError> {
    let rows = sqlx::query_as::<_, HierarchyRow>(
        r#"WITH RECURSIVE ancestors AS (
            SELECT target_id, name, target_type, hierarchy_depth, parent_target_id
            FROM targets WHERE target_id = $1 AND org_id = $2
            UNION ALL
            SELECT t.target_id, t.name, t.target_type, t.hierarchy_depth, t.parent_target_id
            FROM targets t
            INNER JOIN ancestors a ON t.target_id = a.parent_target_id
            WHERE t.org_id = $2
        )
        SELECT target_id, name, target_type, hierarchy_depth FROM ancestors ORDER BY hierarchy_depth"#
    )
    .bind(id)
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| TargetHierarchyNode {
        target_id: r.target_id,
        name: r.name,
        target_type: r.target_type,
        hierarchy_depth: r.hierarchy_depth,
    }).collect()))
}

/// GET /api/targets/:id/children — direct children
pub async fn get_target_children(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TargetHierarchyNode>>, ApiError> {
    let rows = sqlx::query_as::<_, HierarchyRow>(
        "SELECT target_id, name, target_type, hierarchy_depth FROM targets WHERE parent_target_id = $1 AND org_id = $2 ORDER BY name"
    )
    .bind(id)
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| TargetHierarchyNode {
        target_id: r.target_id,
        name: r.name,
        target_type: r.target_type,
        hierarchy_depth: r.hierarchy_depth,
    }).collect()))
}

#[derive(Debug, Deserialize)]
pub struct SetParentRequest {
    pub parent_target_id: Option<Uuid>,
}

/// PATCH /api/targets/:id/parent — set/clear parent; recompute hierarchy
pub async fn set_target_parent(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
    Json(req): Json<SetParentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Set the parent
    sqlx::query("UPDATE targets SET parent_target_id = $1 WHERE target_id = $2 AND org_id = $3")
        .bind(req.parent_target_id)
        .bind(id)
        .bind(org_id)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Recompute hierarchy_depth and hierarchy_path for this target
    if let Some(parent_id) = req.parent_target_id {
        let parent = sqlx::query_as::<_, HierarchyPathRow>(
            "SELECT hierarchy_depth, hierarchy_path FROM targets WHERE target_id = $1 AND org_id = $2"
        )
        .bind(parent_id)
        .bind(org_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

        if let Some(p) = parent {
            let new_depth = p.hierarchy_depth + 1;
            let new_path = format!("{}/{}", p.hierarchy_path.unwrap_or_else(|| parent_id.to_string()), id);
            sqlx::query("UPDATE targets SET hierarchy_depth = $1, hierarchy_path = $2 WHERE target_id = $3 AND org_id = $4")
                .bind(new_depth)
                .bind(&new_path)
                .bind(id)
                .bind(org_id)
                .execute(&state.pool)
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;
        }
    } else {
        sqlx::query("UPDATE targets SET hierarchy_depth = 0, hierarchy_path = NULL WHERE target_id = $1 AND org_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    }

    Ok(Json(serde_json::json!({
        "target_id": id,
        "parent_target_id": req.parent_target_id,
        "status": "updated",
    })))
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct HierarchyRow {
    target_id: Uuid,
    name: String,
    target_type: String,
    hierarchy_depth: i32,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct HierarchyPathRow {
    hierarchy_depth: i32,
    hierarchy_path: Option<String>,
}
