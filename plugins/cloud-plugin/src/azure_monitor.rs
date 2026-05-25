//! Azure Monitor metrics collector — REST API with OAuth2 client credentials.
//!
//! Credentials in target.credentials:
//!   { "tenant_id": "...", "client_id": "...", "client_secret": "...",
//!     "subscription_id": "...", "resource_id": "/subscriptions/.../resourceGroups/.../providers/..." }
//!
//! Collects: VM (CPU, Network, Disk), SQL Database, AKS, App Service metrics.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use serde_json::Value;
use chrono::{Utc, Duration as CDuration};
use tracing::{debug, warn};

pub struct AzureMonitorCollector;

struct AzureCredentials {
    tenant_id: String,
    client_id: String,
    client_secret: String,
    subscription_id: String,
    resource_id: String,
}

impl AzureMonitorCollector {
    pub async fn test(target: &Target) -> bool {
        let creds = match parse_credentials(target) { Some(c) => c, None => return false };
        get_token(&build_client(), &creds).await.is_ok()
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let creds = match parse_credentials(target) {
            Some(c) => c,
            None => {
                warn!(target = %target.name, "Missing Azure credentials");
                return vec![];
            }
        };

        let client = build_client();
        let token = match get_token(&client, &creds).await {
            Ok(t) => t,
            Err(e) => {
                warn!(target = %target.name, error = %e, "Azure token fetch failed");
                return vec![];
            }
        };

        let subtype = target.subtype.as_deref().unwrap_or("azure").to_lowercase();
        let mut metrics = Vec::new();

        match subtype.as_str() {
            "azure_vm" | "azure" => collect_vm(&client, &token, &creds, target, &mut metrics).await,
            "azure_sql" => collect_sql(&client, &token, &creds, target, &mut metrics).await,
            "azure_aks" => collect_aks(&client, &token, &creds, target, &mut metrics).await,
            "azure_appservice" => collect_appservice(&client, &token, &creds, target, &mut metrics).await,
            _ => collect_vm(&client, &token, &creds, target, &mut metrics).await,
        }

        metrics
    }
}

async fn collect_vm(client: &Client, token: &str, creds: &AzureCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let metric_defs = [
        ("Percentage CPU",          "cloud.azure.vm.cpu_percent",       "percent"),
        ("Network In Total",        "cloud.azure.vm.network_in",        "bytes"),
        ("Network Out Total",       "cloud.azure.vm.network_out",       "bytes"),
        ("Disk Read Bytes",         "cloud.azure.vm.disk_read_bytes",   "bytes"),
        ("Disk Write Bytes",        "cloud.azure.vm.disk_write_bytes",  "bytes"),
        ("Disk Read Operations/Sec","cloud.azure.vm.disk_read_ops",     "count/s"),
        ("Disk Write Operations/Sec","cloud.azure.vm.disk_write_ops",   "count/s"),
        ("Available Memory Bytes",  "cloud.azure.vm.mem_available",     "bytes"),
    ];

    fetch_azure_metrics(client, token, creds, target, &metric_defs, metrics).await;
}

async fn collect_sql(client: &Client, token: &str, creds: &AzureCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let metric_defs = [
        ("cpu_percent",          "cloud.azure.sql.cpu_percent",       "percent"),
        ("dtu_consumption_percent", "cloud.azure.sql.dtu_percent",    "percent"),
        ("storage_percent",      "cloud.azure.sql.storage_percent",   "percent"),
        ("connection_successful","cloud.azure.sql.connections_success","count"),
        ("connection_failed",    "cloud.azure.sql.connections_failed","count"),
        ("deadlock",             "cloud.azure.sql.deadlocks",         "count"),
        ("workers_percent",      "cloud.azure.sql.workers_percent",   "percent"),
    ];

    fetch_azure_metrics(client, token, creds, target, &metric_defs, metrics).await;
}

async fn collect_aks(client: &Client, token: &str, creds: &AzureCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let metric_defs = [
        ("node_cpu_usage_percentage",    "cloud.azure.aks.node_cpu_percent",    "percent"),
        ("node_memory_working_set_percentage","cloud.azure.aks.node_mem_percent","percent"),
        ("pod_count",                    "cloud.azure.aks.pod_count",           "count"),
        ("node_count",                   "cloud.azure.aks.node_count",          "count"),
        ("kube_node_status_condition",   "cloud.azure.aks.node_ready",          "count"),
    ];

    fetch_azure_metrics(client, token, creds, target, &metric_defs, metrics).await;
}

async fn collect_appservice(client: &Client, token: &str, creds: &AzureCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let metric_defs = [
        ("CpuTime",          "cloud.azure.appservice.cpu_seconds",   "seconds"),
        ("Requests",         "cloud.azure.appservice.requests",       "count"),
        ("Http5xx",          "cloud.azure.appservice.http_5xx",       "count"),
        ("Http4xx",          "cloud.azure.appservice.http_4xx",       "count"),
        ("AverageResponseTime","cloud.azure.appservice.response_time","seconds"),
        ("MemoryWorkingSet",  "cloud.azure.appservice.memory",        "bytes"),
        ("BytesReceived",    "cloud.azure.appservice.bytes_in",       "bytes"),
        ("BytesSent",        "cloud.azure.appservice.bytes_out",      "bytes"),
    ];

    fetch_azure_metrics(client, token, creds, target, &metric_defs, metrics).await;
}

async fn fetch_azure_metrics(
    client: &Client,
    token: &str,
    creds: &AzureCredentials,
    target: &Target,
    metric_defs: &[(&str, &str, &str)],
    out: &mut Vec<Metric>,
) {
    let end = Utc::now();
    let start = end - CDuration::minutes(10);
    let timespan = format!("{}/{}", start.format("%Y-%m-%dT%H:%M:%SZ"), end.format("%Y-%m-%dT%H:%M:%SZ"));

    let metric_names: Vec<&str> = metric_defs.iter().map(|(n, _, _)| *n).collect();
    let metrics_param = metric_names.join(",");

    let url = format!(
        "https://management.azure.com{}{}?api-version=2021-05-01&timespan={}&interval=PT5M&metricnames={}&aggregation=Average",
        creds.resource_id,
        if creds.resource_id.contains("/providers/") { "/metrics" } else { "/providers/microsoft.insights/metrics" },
        timespan,
        url_encode(&metrics_param),
    );

    let resp = match client.get(&url)
        .bearer_auth(token)
        .send().await {
        Ok(r) => r,
        Err(e) => { warn!(error = %e, "Azure Monitor request failed"); return; }
    };

    let json: Value = match resp.json().await {
        Ok(j) => j,
        Err(e) => { warn!(error = %e, "Azure Monitor JSON parse failed"); return; }
    };

    let empty_arr = vec![];
    let az_metrics = json["value"].as_array().unwrap_or(&empty_arr);

    for az_metric in az_metrics {
        let name = az_metric["name"]["value"].as_str().unwrap_or("");
        let timeseries = az_metric["timeseries"].as_array().unwrap_or(&empty_arr);

        if let Some(series) = timeseries.first() {
            let data_pts = series["data"].as_array().unwrap_or(&empty_arr);
            // Take the latest non-null average
            let val = data_pts.iter().rev()
                .find_map(|pt| pt["average"].as_f64());

            if let Some(val) = val {
                // Find matching infra metric name
                if let Some((_, infra_name, unit)) = metric_defs.iter().find(|(n, _, _)| *n == name) {
                    out.push(
                        Metric::new(infra_name, val, &target.name)
                            .with_context(target.org_id, target.target_id, "cloud-plugin")
                            .with_resource_type("cloud")
                            .with_unit(unit)
                            .with_tag("provider", "azure")
                            .with_tag("subscription", &creds.subscription_id)
                    );
                    debug!(metric = infra_name, value = val);
                }
            }
        }
    }
}

async fn get_token(client: &Client, creds: &AzureCredentials) -> Result<String, String> {
    let url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", creds.tenant_id);
    let body = [
        ("grant_type", "client_credentials"),
        ("client_id", &creds.client_id),
        ("client_secret", &creds.client_secret),
        ("scope", "https://management.azure.com/.default"),
    ];

    let resp = client.post(&url)
        .form(&body)
        .send().await
        .map_err(|e| e.to_string())?;

    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    json["access_token"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| json["error_description"].as_str().unwrap_or("token missing").to_string())
}

fn parse_credentials(target: &Target) -> Option<AzureCredentials> {
    let c = target.credentials.as_ref()?;
    Some(AzureCredentials {
        tenant_id: c.get("tenant_id")?.as_str()?.to_string(),
        client_id: c.get("client_id")?.as_str()?.to_string(),
        client_secret: c.get("client_secret")?.as_str()?.to_string(),
        subscription_id: c.get("subscription_id")?.as_str()?.to_string(),
        resource_id: c.get("resource_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

fn build_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

fn url_encode(s: &str) -> String {
    s.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' || c == ',' {
            c.to_string()
        } else {
            format!("%{:02X}", c as u8)
        }
    }).collect()
}
