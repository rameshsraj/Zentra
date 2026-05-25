use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tracing::{debug, warn};

fn st_metric(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage")
}

pub struct MinioDeepCollector;

impl MinioDeepCollector {
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let port = if target.port == 0 { 9000 } else { target.port as u16 };
        let base = format!("http://{}:{}", host, port);

        // MinIO Prometheus metrics endpoint (no auth needed for /minio/v2/metrics/cluster)
        // Auth can optionally be via Bearer token in credentials
        let token = target.credentials.as_ref()
            .and_then(|c| c.username.clone());

        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .danger_accept_invalid_certs(true)
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!(target = %target.name, "Failed to build HTTP client: {}", e);
                return metrics;
            }
        };

        // Health check endpoint
        let health_url = format!("{}/minio/health/live", base);
        let alive = client.get(&health_url).send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        metrics.push(st_metric("storage.minio.alive", if alive { 1.0 } else { 0.0 }, "minio", target));
        if !alive {
            return metrics;
        }

        // Prometheus metrics endpoint
        let metrics_url = format!("{}/minio/v2/metrics/cluster", base);
        let mut req = client.get(&metrics_url);
        if let Some(tok) = &token {
            req = req.bearer_auth(tok);
        }
        let prom_body = match req.send().await {
            Ok(r) => match r.text().await {
                Ok(b) => b,
                Err(e) => {
                    warn!(target = %target.name, "Failed to read MinIO metrics body: {}", e);
                    return metrics;
                }
            },
            Err(e) => {
                warn!(target = %target.name, "Failed to fetch MinIO metrics: {}", e);
                return metrics;
            }
        };

        // Parse Prometheus text format
        for line in prom_body.lines() {
            if line.starts_with('#') { continue; }
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() < 2 { continue; }
            let metric_expr = parts[0];
            let val: f64 = parts[1].split_whitespace().next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(f64::NAN);
            if val.is_nan() { continue; }

            // Extract base metric name (strip labels)
            let base_name = metric_expr.split('{').next().unwrap_or(metric_expr);

            match base_name {
                "minio_cluster_capacity_raw_total_bytes" =>
                    metrics.push(st_metric("storage.minio.capacity_raw_bytes", val, "cluster", target).with_unit("bytes")),
                "minio_cluster_capacity_raw_free_bytes" =>
                    metrics.push(st_metric("storage.minio.capacity_raw_free_bytes", val, "cluster", target).with_unit("bytes")),
                "minio_cluster_capacity_usable_total_bytes" =>
                    metrics.push(st_metric("storage.minio.capacity_usable_bytes", val, "cluster", target).with_unit("bytes")),
                "minio_cluster_capacity_usable_free_bytes" =>
                    metrics.push(st_metric("storage.minio.capacity_usable_free_bytes", val, "cluster", target).with_unit("bytes")),
                "minio_cluster_nodes_online_total" =>
                    metrics.push(st_metric("storage.minio.nodes_online", val, "cluster", target)),
                "minio_cluster_nodes_offline_total" =>
                    metrics.push(st_metric("storage.minio.nodes_offline", val, "cluster", target)),
                "minio_cluster_drives_online_total" =>
                    metrics.push(st_metric("storage.minio.drives_online", val, "cluster", target)),
                "minio_cluster_drives_offline_total" =>
                    metrics.push(st_metric("storage.minio.drives_offline", val, "cluster", target)),
                "minio_cluster_drives_healing_total" =>
                    metrics.push(st_metric("storage.minio.drives_healing", val, "cluster", target)),
                "minio_cluster_bucket_total" =>
                    metrics.push(st_metric("storage.minio.bucket_count", val, "cluster", target)),
                "minio_cluster_objects_total" =>
                    metrics.push(st_metric("storage.minio.object_count", val, "cluster", target)),
                "minio_s3_requests_total" =>
                    metrics.push(st_metric("storage.minio.s3_requests_total", val, "api", target)),
                "minio_s3_requests_errors_total" =>
                    metrics.push(st_metric("storage.minio.s3_errors_total", val, "api", target)),
                "minio_s3_traffic_received_bytes" =>
                    metrics.push(st_metric("storage.minio.traffic_received_bytes", val, "api", target).with_unit("bytes")),
                "minio_s3_traffic_sent_bytes" =>
                    metrics.push(st_metric("storage.minio.traffic_sent_bytes", val, "api", target).with_unit("bytes")),
                "minio_s3_requests_ttfb_seconds_distribution_count" =>
                    metrics.push(st_metric("storage.minio.ttfb_requests", val, "api", target)),
                "minio_node_disk_total_bytes" =>
                    metrics.push(st_metric("storage.minio.node_disk_total_bytes", val, "node", target).with_unit("bytes")),
                "minio_node_disk_free_bytes" =>
                    metrics.push(st_metric("storage.minio.node_disk_free_bytes", val, "node", target).with_unit("bytes")),
                "minio_node_disk_used_bytes" =>
                    metrics.push(st_metric("storage.minio.node_disk_used_bytes", val, "node", target).with_unit("bytes")),
                _ => {}
            }
        }

        // Compute utilization if we have the values
        let usable_total = metrics.iter().find(|m| m.metric_name == "storage.minio.capacity_usable_bytes").map(|m| m.value);
        let usable_free = metrics.iter().find(|m| m.metric_name == "storage.minio.capacity_usable_free_bytes").map(|m| m.value);
        if let (Some(total), Some(free)) = (usable_total, usable_free) {
            if total > 0.0 {
                let used = total - free;
                let pct = (used / total) * 100.0;
                metrics.push(st_metric("storage.minio.capacity_used_pct", pct, "cluster", target).with_unit("percent"));
            }
        }

        debug!(target = %target.name, count = metrics.len(), "MinIO collection complete");
        metrics
    }
}
