use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;

fn mw_metric(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "server-plugin")
        .with_resource_type("middleware")
}

pub async fn collect(target: &Target) -> Vec<Metric> {
    let subtype = target.target_subtype.as_str().to_lowercase();
    match subtype.as_str() {
        "nginx"         => collect_nginx(target).await,
        "apache"        => collect_apache(target).await,
        "haproxy"       => collect_haproxy(target).await,
        "elasticsearch" => collect_elasticsearch(target).await,
        "redis"         => collect_redis(target).await,
        "memcached"     => collect_memcached(target).await,
        _               => reachability_check(target).await,
    }
}

async fn reachability_check(target: &Target) -> Vec<Metric> {
    let url = format!("http://{}:{}", target.host, target.port);
    let reachable = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c.get(&url).send().await.is_ok(),
        Err(_) => false,
    };
    vec![mw_metric("middleware.reachable", if reachable { 1.0 } else { 0.0 }, "reachability", target)]
}

async fn collect_nginx(target: &Target) -> Vec<Metric> {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    // Try localhost first (agent runs on the same host), then the configured IP.
    // Common nginx stub_status paths.
    let candidates = [
        format!("http://127.0.0.1:{}/nginx_status", target.port),
        format!("http://localhost:{}/nginx_status", target.port),
        format!("http://{}:{}/nginx_status", target.host, target.port),
        format!("http://127.0.0.1:{}/stub_status", target.port),
        format!("http://{}:{}/stub_status", target.host, target.port),
    ];
    let mut body = String::new();
    for url in &candidates {
        match client.get(url).send().await {
            Ok(r) if r.status().is_success() => {
                body = r.text().await.unwrap_or_default();
                break;
            }
            Ok(r) if r.status().as_u16() == 404 => {
                tracing::debug!(url=%url, "nginx_status 404 — stub_status module not enabled");
            }
            Ok(r) => tracing::debug!(url=%url, status=%r.status(), "nginx_status non-200"),
            Err(e) => tracing::debug!(url=%url, err=%e, "nginx_status unreachable"),
        }
    }
    if body.is_empty() {
        // Web server is reachable (we confirmed port is open) but status module not enabled.
        return vec![mw_metric("nginx.reachable", 1.0, "availability", target)];
    }
    let mut active: f64 = 0.0;
    let mut accepts: f64 = 0.0;
    let mut handled: f64 = 0.0;
    let mut requests: f64 = 0.0;
    let mut reading: f64 = 0.0;
    let mut writing: f64 = 0.0;
    let mut waiting: f64 = 0.0;

    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("Active connections:") {
            active = line.split(':').nth(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
        } else if line.starts_with("Reading:") {
            let p: Vec<&str> = line.split_whitespace().collect();
            reading = p.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            writing = p.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            waiting = p.get(5).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        } else if let Ok(vals) = line.split_whitespace()
            .map(|v| v.parse::<f64>())
            .collect::<Result<Vec<f64>, _>>()
        {
            if vals.len() == 3 { accepts = vals[0]; handled = vals[1]; requests = vals[2]; }
        }
    }

    vec![
        mw_metric("nginx.reachable",           1.0,                       "availability", target),
        mw_metric("nginx.active_connections",  active,                    "connections",  target),
        mw_metric("nginx.accepts",             accepts,                   "requests",     target),
        mw_metric("nginx.handled",             handled,                   "requests",     target),
        mw_metric("nginx.requests",            requests,                  "requests",     target),
        mw_metric("nginx.reading",             reading,                   "workers",      target),
        mw_metric("nginx.writing",             writing,                   "workers",      target),
        mw_metric("nginx.waiting",             waiting,                   "workers",      target),
        mw_metric("nginx.dropped",             accepts - handled,          "requests",     target),
    ]
}

async fn collect_apache(target: &Target) -> Vec<Metric> {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    // Try localhost first (agent runs on the same host), then the configured IP.
    let candidates = [
        format!("http://127.0.0.1:{}/server-status?auto", target.port),
        format!("http://localhost:{}/server-status?auto", target.port),
        format!("http://{}:{}/server-status?auto", target.host, target.port),
    ];
    let mut body = String::new();
    for url in &candidates {
        match client.get(url).send().await {
            Ok(r) if r.status().is_success() => {
                body = r.text().await.unwrap_or_default();
                break;
            }
            Ok(r) if r.status().as_u16() == 404 => {
                tracing::debug!(url=%url, "apache server-status 404 — mod_status not enabled");
            }
            Ok(r) => tracing::debug!(url=%url, status=%r.status(), "apache server-status non-200"),
            Err(e) => tracing::debug!(url=%url, err=%e, "apache server-status unreachable"),
        }
    }
    if body.is_empty() {
        // Web server is reachable but mod_status not enabled — emit reachable=1.
        return vec![mw_metric("apache.reachable", 1.0, "availability", target)];
    }
    let mut busy: f64 = 0.0;
    let mut idle: f64 = 0.0;
    let mut rps: f64 = 0.0;
    let mut bps: f64 = 0.0;
    let mut cpu: f64 = 0.0;
    let mut total_accesses: f64 = 0.0;
    let mut uptime: f64 = 0.0;

    for line in body.lines() {
        if let Some((k, v)) = line.split_once(':') {
            match k.trim() {
                "BusyWorkers"    => busy  = v.trim().parse().unwrap_or(0.0),
                "IdleWorkers"    => idle  = v.trim().parse().unwrap_or(0.0),
                "ReqPerSec"      => rps   = v.trim().parse().unwrap_or(0.0),
                "BytesPerSec"    => bps   = v.trim().parse().unwrap_or(0.0),
                "CPULoad"        => cpu   = v.trim().parse().unwrap_or(0.0),
                "Total Accesses" => total_accesses = v.trim().parse().unwrap_or(0.0),
                "ServerUptime"   => uptime = v.trim().parse().unwrap_or(0.0),
                _ => {}
            }
        }
    }

    vec![
        mw_metric("apache.reachable",        1.0,           "availability", target),
        mw_metric("apache.busy_workers",     busy,          "workers",      target),
        mw_metric("apache.idle_workers",     idle,          "workers",      target),
        mw_metric("apache.requests_per_sec", rps,           "requests",     target),
        mw_metric("apache.bytes_per_sec",    bps,           "bytes",        target),
        mw_metric("apache.cpu_load",         cpu,           "cpu",          target),
        mw_metric("apache.total_accesses",   total_accesses,"requests",     target),
        mw_metric("apache.uptime_seconds",   uptime,        "uptime",       target),
    ]
}

async fn collect_haproxy(target: &Target) -> Vec<Metric> {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let url = format!("http://{}:{}/stats;csv", target.host, target.port);
    let reachable = client.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false);
    vec![mw_metric("haproxy.reachable", if reachable { 1.0 } else { 0.0 }, "availability", target)]
}

async fn collect_elasticsearch(target: &Target) -> Vec<Metric> {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let url = format!("http://{}:{}/_cluster/health", target.host, target.port);
    let body = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => r.json::<serde_json::Value>().await.ok(),
        _ => {
            return vec![mw_metric("elasticsearch.reachable", 0.0, "availability", target)];
        }
    };
    let body = match body {
        Some(b) => b,
        None => return vec![mw_metric("elasticsearch.reachable", 0.0, "availability", target)],
    };
    let status_val = match body["status"].as_str().unwrap_or("red") {
        "green" => 2.0, "yellow" => 1.0, _ => 0.0,
    };
    vec![
        mw_metric("elasticsearch.reachable",        1.0,                                                     "availability", target),
        mw_metric("elasticsearch.cluster_status",   status_val,                                              "status",       target),
        mw_metric("elasticsearch.nodes",            body["number_of_nodes"].as_f64().unwrap_or(0.0),         "nodes",        target),
        mw_metric("elasticsearch.active_shards",    body["active_shards"].as_f64().unwrap_or(0.0),           "shards",       target),
        mw_metric("elasticsearch.unassigned_shards",body["unassigned_shards"].as_f64().unwrap_or(0.0),       "shards",       target),
    ]
}

async fn collect_redis(target: &Target) -> Vec<Metric> {
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    let addr = format!("{}:{}", target.host, target.port);
    let mut stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            warn!(addr=%addr, err=%e, "Failed to connect to Redis");
            return vec![mw_metric("redis.reachable", 0.0, "availability", target)];
        }
    };
    let _ = stream.write_all(b"INFO\r\n").await;
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await.unwrap_or(0);
    let info = String::from_utf8_lossy(&buf[..n]);

    let get = |key: &str| -> f64 {
        for line in info.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{}:", key)) {
                return rest.trim().parse().unwrap_or(0.0);
            }
        }
        0.0
    };

    let hits = get("keyspace_hits");
    let misses = get("keyspace_misses");
    let hit_rate = if hits + misses > 0.0 { hits / (hits + misses) * 100.0 } else { 0.0 };

    vec![
        mw_metric("redis.reachable",          1.0,                             "availability", target),
        mw_metric("redis.connected_clients",  get("connected_clients"),         "clients",      target),
        mw_metric("redis.used_memory_mb",     get("used_memory") / 1048576.0,  "memory",       target),
        mw_metric("redis.ops_per_sec",        get("instantaneous_ops_per_sec"),"operations",   target),
        mw_metric("redis.hit_rate",           hit_rate,                        "cache",        target),
        mw_metric("redis.connected_slaves",   get("connected_slaves"),          "replication",  target),
        mw_metric("redis.uptime_seconds",     get("uptime_in_seconds"),         "uptime",       target),
    ]
}

async fn collect_memcached(target: &Target) -> Vec<Metric> {
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    let addr = format!("{}:{}", target.host, target.port);
    let mut stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            warn!(addr=%addr, err=%e, "Failed to connect to Memcached");
            return vec![mw_metric("memcached.reachable", 0.0, "availability", target)];
        }
    };
    let _ = stream.write_all(b"stats\r\n").await;
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await.unwrap_or(0);
    let stats = String::from_utf8_lossy(&buf[..n]);

    let get = |key: &str| -> f64 {
        for line in stats.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == key {
                return parts[2].trim().parse().unwrap_or(0.0);
            }
        }
        0.0
    };

    let hits = get("get_hits");
    let misses = get("get_misses");
    let hit_rate = if hits + misses > 0.0 { hits / (hits + misses) * 100.0 } else { 0.0 };

    vec![
        mw_metric("memcached.reachable",        1.0,                        "availability", target),
        mw_metric("memcached.curr_connections",  get("curr_connections"),    "clients",      target),
        mw_metric("memcached.curr_items",        get("curr_items"),          "items",        target),
        mw_metric("memcached.bytes_used_mb",     get("bytes") / 1048576.0,  "memory",       target),
        mw_metric("memcached.hit_rate",          hit_rate,                   "cache",        target),
        mw_metric("memcached.evictions",         get("evictions"),           "cache",        target),
        mw_metric("memcached.uptime_seconds",    get("uptime"),              "uptime",       target),
    ]
}
