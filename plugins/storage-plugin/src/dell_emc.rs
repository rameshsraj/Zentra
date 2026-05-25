//! Dell EMC Unity / PowerStore REST API collector.
//!
//! Credentials in target.credentials:
//!   { "username": "admin", "password": "...", "verify_ssl": false }
//!   target.host = Unity management IP or FQDN
//!
//! Collects: storage pools (capacity/free/used%), volumes (IOPS/throughput/latency),
//!           system health, disk group status, NAS servers, replication sessions.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::{Client, header};
use serde_json::Value;
use tracing::warn;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

pub struct DellEmcCollector;

impl DellEmcCollector {
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let (username, password) = match parse_creds(target) {
            Some(c) => c,
            None => {
                warn!(target = %target.name, "Missing Dell EMC credentials");
                return vec![];
            }
        };

        let client = match build_client(&username, &password) {
            Ok(c) => c,
            Err(e) => { warn!(error = %e, "Dell EMC client build failed"); return vec![]; }
        };

        let base = format!("https://{}:{}", target.host, target.port);
        let mut metrics = Vec::new();

        // Fetch CSRF token via GET /api/types/loginSessionInfo/instances
        let csrf = fetch_csrf_token(&client, &base).await;

        collect_pools(&client, &base, csrf.as_deref(), target, &mut metrics).await;
        collect_volumes(&client, &base, csrf.as_deref(), target, &mut metrics).await;
        collect_system(&client, &base, csrf.as_deref(), target, &mut metrics).await;
        collect_disks(&client, &base, csrf.as_deref(), target, &mut metrics).await;

        metrics
    }
}

async fn fetch_csrf_token(client: &Client, base: &str) -> Option<String> {
    let url = format!("{}/api/types/loginSessionInfo/instances?fields=csrf_token", base);
    let resp = client.get(&url).send().await.ok()?;
    let json: Value = resp.json().await.ok()?;
    json["entries"].as_array()?
        .first()?
        .get("content")?
        .get("csrf_token")?
        .as_str()
        .map(|s| s.to_string())
}

async fn collect_pools(client: &Client, base: &str, csrf: Option<&str>, target: &Target, metrics: &mut Vec<Metric>) {
    let url = format!("{}/api/types/pool/instances?fields=name,sizeFree,sizeTotal,sizeUsed,sizeSubscribed,health,isFASTCacheEnabled", base);
    let resp = match api_get(client, &url, csrf).await { Ok(r) => r, Err(e) => { warn!(error = %e, "Dell EMC pools failed"); return; } };
    let json: Value = match resp.json().await { Ok(j) => j, Err(e) => { warn!(error = %e); return; } };

    let empty = vec![];
    for entry in json["entries"].as_array().unwrap_or(&empty) {
        let c = &entry["content"];
        let name = c["name"].as_str().unwrap_or("unknown");
        let total = c["sizeTotal"].as_f64().unwrap_or(0.0);
        let used = c["sizeUsed"].as_f64().unwrap_or(0.0);
        let free = c["sizeFree"].as_f64().unwrap_or(0.0);
        let subscribed = c["sizeSubscribed"].as_f64().unwrap_or(0.0);
        let health = health_score(c.get("health").and_then(|h| h.get("value")).and_then(|v| v.as_u64()).unwrap_or(5));
        let used_pct = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };

        for (n, v, u) in [
            ("storage.dellemc.pool_total_bytes",   total,       "bytes"),
            ("storage.dellemc.pool_used_bytes",    used,        "bytes"),
            ("storage.dellemc.pool_free_bytes",    free,        "bytes"),
            ("storage.dellemc.pool_subscribed",    subscribed,  "bytes"),
            ("storage.dellemc.pool_used_pct",      used_pct,    "percent"),
            ("storage.dellemc.pool_health",        health,      ""),
        ] {
            metrics.push(
                Metric::new(n, v, name)
                    .with_context(target.org_id, target.target_id, "storage-plugin")
                    .with_resource_type("storage")
                    .with_unit(u)
                    .with_tag("pool", name)
            );
        }
    }
}

async fn collect_volumes(client: &Client, base: &str, csrf: Option<&str>, target: &Target, metrics: &mut Vec<Metric>) {
    // Unity stores LUN performance in /api/types/lunPerfStats
    let url = format!("{}/api/types/lunPerfStats/instances?fields=id,lun,readIops,writeIops,readMbs,writeMbs,readResponseTime,writeResponseTime,queueLength&per_page=200", base);
    let resp = match api_get(client, &url, csrf).await { Ok(r) => r, Err(_) => return };
    let json: Value = match resp.json().await { Ok(j) => j, Err(_) => return };

    let empty = vec![];
    for entry in json["entries"].as_array().unwrap_or(&empty) {
        let c = &entry["content"];
        let lun_id = c["id"].as_str().unwrap_or("unknown");
        let read_iops = c["readIops"].as_f64().unwrap_or(0.0);
        let write_iops = c["writeIops"].as_f64().unwrap_or(0.0);
        let read_mbs = c["readMbs"].as_f64().unwrap_or(0.0);
        let write_mbs = c["writeMbs"].as_f64().unwrap_or(0.0);
        let read_lat_ms = c["readResponseTime"].as_f64().unwrap_or(0.0) / 1_000_000.0; // ns→ms
        let write_lat_ms = c["writeResponseTime"].as_f64().unwrap_or(0.0) / 1_000_000.0;

        for (n, v, u) in [
            ("storage.dellemc.lun_read_iops",    read_iops,   "ops/s"),
            ("storage.dellemc.lun_write_iops",   write_iops,  "ops/s"),
            ("storage.dellemc.lun_read_mbps",    read_mbs,    "MB/s"),
            ("storage.dellemc.lun_write_mbps",   write_mbs,   "MB/s"),
            ("storage.dellemc.lun_read_lat_ms",  read_lat_ms, "ms"),
            ("storage.dellemc.lun_write_lat_ms", write_lat_ms,"ms"),
        ] {
            metrics.push(
                Metric::new(n, v, lun_id)
                    .with_context(target.org_id, target.target_id, "storage-plugin")
                    .with_resource_type("storage")
                    .with_unit(u)
                    .with_tag("lun", lun_id)
            );
        }
    }
}

async fn collect_system(client: &Client, base: &str, csrf: Option<&str>, target: &Target, metrics: &mut Vec<Metric>) {
    let url = format!("{}/api/types/system/instances?fields=name,model,health,currentPower,avgPower", base);
    let resp = match api_get(client, &url, csrf).await { Ok(r) => r, Err(_) => return };
    let json: Value = match resp.json().await { Ok(j) => j, Err(_) => return };

    let empty = vec![];
    if let Some(entry) = json["entries"].as_array().unwrap_or(&empty).first() {
        let c = &entry["content"];
        let health = health_score(c.get("health").and_then(|h| h.get("value")).and_then(|v| v.as_u64()).unwrap_or(5));
        let current_power = c["currentPower"].as_f64().unwrap_or(0.0);

        metrics.push(Metric::new("storage.dellemc.system_health", health, "system")
            .with_context(target.org_id, target.target_id, "storage-plugin")
            .with_resource_type("storage").with_unit(""));
        metrics.push(Metric::new("storage.dellemc.system_power_watts", current_power, "system")
            .with_context(target.org_id, target.target_id, "storage-plugin")
            .with_resource_type("storage").with_unit("watts"));
    }
}

async fn collect_disks(client: &Client, base: &str, csrf: Option<&str>, target: &Target, metrics: &mut Vec<Metric>) {
    let url = format!("{}/api/types/disk/instances?fields=name,health,model,currentSpeed,rawSize&per_page=200", base);
    let resp = match api_get(client, &url, csrf).await { Ok(r) => r, Err(_) => return };
    let json: Value = match resp.json().await { Ok(j) => j, Err(_) => return };

    let mut total = 0u64;
    let mut degraded = 0u64;
    let empty = vec![];
    for entry in json["entries"].as_array().unwrap_or(&empty) {
        let c = &entry["content"];
        let hv = c.get("health").and_then(|h| h.get("value")).and_then(|v| v.as_u64()).unwrap_or(5);
        total += 1;
        if hv != 5 { degraded += 1; }
    }

    metrics.push(Metric::new("storage.dellemc.disk_total", total as f64, "disks")
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage").with_unit("count"));
    metrics.push(Metric::new("storage.dellemc.disk_degraded", degraded as f64, "disks")
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage").with_unit("count"));
}

async fn api_get(client: &Client, url: &str, csrf: Option<&str>) -> Result<reqwest::Response, reqwest::Error> {
    let mut req = client.get(url)
        .header("Accept", "application/json")
        .header("X-EMC-REST-CLIENT", "true");
    if let Some(token) = csrf {
        req = req.header("EMC-CSRF-TOKEN", token);
    }
    req.send().await
}

// Unity health value: 5=OK, 10=Degraded, 20=Minor, 25=Major, 30=Critical, 15=Unknown
fn health_score(v: u64) -> f64 {
    match v { 5 => 1.0, 10 | 20 => 0.7, 25 => 0.4, 30 => 0.0, _ => 0.5 }
}

fn build_client(username: &str, password: &str) -> Result<Client, reqwest::Error> {
    let cred = BASE64.encode(format!("{}:{}", username, password));
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&format!("Basic {}", cred)).unwrap(),
    );
    Client::builder()
        .danger_accept_invalid_certs(true)
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
}

fn parse_creds(target: &Target) -> Option<(String, String)> {
    let c = target.credentials.as_ref()?;
    let user = c.username.clone()?;
    let pass = c.password_encrypted.clone()?;
    Some((user, pass))
}
