//! AWS CloudWatch metrics collector — SigV4-signed HTTP requests, no AWS SDK dependency.
//!
//! Credentials read from target.credentials JSON:
//!   { "access_key_id": "AKIA...", "secret_access_key": "...", "region": "us-east-1",
//!     "assume_role_arn": "arn:aws:iam::..." (optional) }
//!
//! Collects: EC2 (CPU, NetworkIn/Out, DiskReadOps/WriteOps, StatusCheck),
//!           RDS (CPUUtilization, DatabaseConnections, FreeStorageSpace, ReadIOPS, WriteIOPS),
//!           ELB/ALB (RequestCount, TargetResponseTime, HTTPCode_ELB_5XX, HealthyHostCount),
//!           Lambda (Invocations, Errors, Duration, Throttles, ConcurrentExecutions),
//!           ECS/EKS (CPUUtilization, MemoryUtilization)

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use serde_json::Value;
use chrono::{Utc, Duration as CDuration};
use tracing::{debug, warn};
use hmac::{Hmac, Mac};
use sha2::{Sha256, Digest};
use std::collections::BTreeMap;

type HmacSha256 = Hmac<Sha256>;

pub struct AwsCloudWatchCollector;

struct AwsCredentials {
    access_key_id: String,
    secret_access_key: String,
    region: String,
    session_token: Option<String>,
}

impl AwsCloudWatchCollector {
    pub async fn test(target: &Target) -> bool {
        let creds = match parse_credentials(target) {
            Some(c) => c,
            None => return false,
        };
        let client = build_client();
        // Call GetMetricStatistics on a trivial metric; success = creds are valid
        let end = Utc::now();
        let start = end - CDuration::minutes(5);
        let params = cloudwatch_params("AWS/EC2", "CPUUtilization", &start.to_rfc3339(), &end.to_rfc3339(), 300, "Average", &[]);
        call_cloudwatch(&client, &creds, &params).await.is_ok()
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let creds = match parse_credentials(target) {
            Some(c) => c,
            None => {
                warn!(target = %target.name, "Missing AWS credentials");
                return vec![];
            }
        };

        let subtype = target.target_subtype.as_str().to_lowercase();
        let client = build_client();
        let mut metrics = Vec::new();

        match subtype.as_str() {
            "aws_ec2" | "aws" => collect_ec2(&client, &creds, target, &mut metrics).await,
            "aws_rds" => collect_rds(&client, &creds, target, &mut metrics).await,
            "aws_elb" => collect_elb(&client, &creds, target, &mut metrics).await,
            "aws_lambda" => collect_lambda(&client, &creds, target, &mut metrics).await,
            "aws_ecs" => collect_ecs(&client, &creds, target, &mut metrics).await,
            _ => collect_ec2(&client, &creds, target, &mut metrics).await,
        }

        metrics
    }
}

async fn collect_ec2(client: &Client, creds: &AwsCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    // Instance ID from target.host (or credentials JSON field "instance_id")
    let instance_id = instance_id_from_target(target);
    let dimensions = [("InstanceId", instance_id.as_str())];

    let metric_defs = [
        ("CPUUtilization",     "cloud.aws.ec2.cpu_percent",    "percent"),
        ("NetworkIn",          "cloud.aws.ec2.network_in",     "bytes"),
        ("NetworkOut",         "cloud.aws.ec2.network_out",    "bytes"),
        ("DiskReadOps",        "cloud.aws.ec2.disk_read_ops",  "count"),
        ("DiskWriteOps",       "cloud.aws.ec2.disk_write_ops", "count"),
        ("StatusCheckFailed",  "cloud.aws.ec2.status_failed",  "count"),
    ];

    fetch_cloudwatch_metrics(client, creds, target, "AWS/EC2", &metric_defs, &dimensions, metrics).await;
}

async fn collect_rds(client: &Client, creds: &AwsCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let db_id = db_instance_id_from_target(target);
    let dimensions = [("DBInstanceIdentifier", db_id.as_str())];

    let metric_defs = [
        ("CPUUtilization",          "cloud.aws.rds.cpu_percent",     "percent"),
        ("DatabaseConnections",     "cloud.aws.rds.connections",      "count"),
        ("FreeStorageSpace",        "cloud.aws.rds.free_storage",     "bytes"),
        ("ReadIOPS",                "cloud.aws.rds.read_iops",        "count/s"),
        ("WriteIOPS",               "cloud.aws.rds.write_iops",       "count/s"),
        ("ReadLatency",             "cloud.aws.rds.read_latency",     "seconds"),
        ("WriteLatency",            "cloud.aws.rds.write_latency",    "seconds"),
        ("FreeableMemory",          "cloud.aws.rds.freeable_memory",  "bytes"),
        ("ReplicaLag",              "cloud.aws.rds.replica_lag",      "seconds"),
    ];

    fetch_cloudwatch_metrics(client, creds, target, "AWS/RDS", &metric_defs, &dimensions, metrics).await;
}

async fn collect_elb(client: &Client, creds: &AwsCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let lb_name = lb_name_from_target(target);
    let dimensions = [("LoadBalancer", lb_name.as_str())];

    let metric_defs = [
        ("RequestCount",              "cloud.aws.elb.request_count",     "count"),
        ("TargetResponseTime",        "cloud.aws.elb.response_time",      "seconds"),
        ("HTTPCode_ELB_5XX_Count",    "cloud.aws.elb.5xx_count",         "count"),
        ("HTTPCode_ELB_4XX_Count",    "cloud.aws.elb.4xx_count",         "count"),
        ("HealthyHostCount",          "cloud.aws.elb.healthy_hosts",      "count"),
        ("UnHealthyHostCount",        "cloud.aws.elb.unhealthy_hosts",    "count"),
        ("ActiveConnectionCount",     "cloud.aws.elb.active_connections", "count"),
    ];

    fetch_cloudwatch_metrics(client, creds, target, "AWS/ApplicationELB", &metric_defs, &dimensions, metrics).await;
}

async fn collect_lambda(client: &Client, creds: &AwsCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let fn_name = fn_name_from_target(target);
    let dimensions = [("FunctionName", fn_name.as_str())];

    let metric_defs = [
        ("Invocations",              "cloud.aws.lambda.invocations",           "count"),
        ("Errors",                   "cloud.aws.lambda.errors",                 "count"),
        ("Duration",                 "cloud.aws.lambda.duration_ms",            "ms"),
        ("Throttles",                "cloud.aws.lambda.throttles",              "count"),
        ("ConcurrentExecutions",     "cloud.aws.lambda.concurrent_executions",  "count"),
        ("UnreservedConcurrentExecutions", "cloud.aws.lambda.unreserved_concurrent", "count"),
    ];

    fetch_cloudwatch_metrics(client, creds, target, "AWS/Lambda", &metric_defs, &dimensions, metrics).await;
}

async fn collect_ecs(client: &Client, creds: &AwsCredentials, target: &Target, metrics: &mut Vec<Metric>) {
    let cluster = cluster_name_from_target(target);
    let dimensions = [("ClusterName", cluster.as_str())];

    let metric_defs = [
        ("CPUUtilization",    "cloud.aws.ecs.cpu_percent",    "percent"),
        ("MemoryUtilization", "cloud.aws.ecs.mem_percent",    "percent"),
        ("CPUReservation",    "cloud.aws.ecs.cpu_reserved",   "percent"),
        ("MemoryReservation", "cloud.aws.ecs.mem_reserved",   "percent"),
    ];

    fetch_cloudwatch_metrics(client, creds, target, "AWS/ECS", &metric_defs, &dimensions, metrics).await;
}

async fn fetch_cloudwatch_metrics(
    client: &Client,
    creds: &AwsCredentials,
    target: &Target,
    namespace: &str,
    metric_defs: &[(&str, &str, &str)],
    dimensions: &[(&str, &str)],
    out: &mut Vec<Metric>,
) {
    let end = Utc::now();
    let start = end - CDuration::minutes(10);

    for (cw_metric, infra_metric, unit) in metric_defs {
        let params = cloudwatch_params(
            namespace, cw_metric,
            &start.to_rfc3339(), &end.to_rfc3339(),
            300, "Average", dimensions,
        );

        match call_cloudwatch(client, creds, &params).await {
            Ok(val) => {
                out.push(
                    Metric::new(*infra_metric, val, target.name.clone())
                        .with_context(target.org_id, target.target_id, "cloud-plugin")
                        .with_resource_type("cloud")
                        .with_unit(*unit)
                        .with_tag("provider", "aws")
                        .with_tag("region", &creds.region)
                );
                debug!(metric = infra_metric, value = val);
            }
            Err(e) => {
                warn!(metric = cw_metric, error = %e, "CloudWatch fetch failed");
            }
        }
    }
}

fn cloudwatch_params(
    namespace: &str,
    metric: &str,
    start: &str,
    end: &str,
    period: u32,
    stat: &str,
    dimensions: &[(&str, &str)],
) -> BTreeMap<String, String> {
    let mut p = BTreeMap::new();
    p.insert("Action".into(), "GetMetricStatistics".into());
    p.insert("Version".into(), "2010-08-01".into());
    p.insert("Namespace".into(), namespace.to_string());
    p.insert("MetricName".into(), metric.to_string());
    p.insert("StartTime".into(), start.to_string());
    p.insert("EndTime".into(), end.to_string());
    p.insert("Period".into(), period.to_string());
    p.insert("Statistics.member.1".into(), stat.to_string());

    for (i, (name, val)) in dimensions.iter().enumerate() {
        let idx = i + 1;
        p.insert(format!("Dimensions.member.{}.Name", idx), name.to_string());
        p.insert(format!("Dimensions.member.{}.Value", idx), val.to_string());
    }
    p
}

async fn call_cloudwatch(
    client: &Client,
    creds: &AwsCredentials,
    params: &BTreeMap<String, String>,
) -> Result<f64, String> {
    let endpoint = format!("https://monitoring.{}.amazonaws.com/", creds.region);
    let now = Utc::now();
    let date_time = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = now.format("%Y%m%d").to_string();

    // Build query string (already sorted via BTreeMap)
    let query_string: String = params.iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    // Canonical request
    let canonical_headers = format!("host:monitoring.{}.amazonaws.com\nx-amz-date:{}\n", creds.region, date_time);
    let signed_headers = "host;x-amz-date";
    let payload_hash = hex_sha256(b"");
    let canonical_request = format!(
        "GET\n/\n{}\n{}\n{}\n{}",
        query_string, canonical_headers, signed_headers, payload_hash
    );

    // String to sign
    let credential_scope = format!("{}/{}/monitoring/aws4_request", date, creds.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        date_time, credential_scope, hex_sha256(canonical_request.as_bytes())
    );

    // Signing key derivation
    let signing_key = derive_signing_key(&creds.secret_access_key, &date, &creds.region, "monitoring");
    let signature = hmac_hex(&signing_key, string_to_sign.as_bytes());

    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={}/{},SignedHeaders={},Signature={}",
        creds.access_key_id, credential_scope, signed_headers, signature
    );

    let url = format!("{}?{}", endpoint, query_string);
    let mut req = client.get(&url)
        .header("x-amz-date", &date_time)
        .header("Authorization", &auth_header);

    if let Some(token) = &creds.session_token {
        req = req.header("x-amz-security-token", token);
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    let body = resp.text().await.map_err(|e| e.to_string())?;

    extract_cloudwatch_average(&body)
}

fn extract_cloudwatch_average(xml: &str) -> Result<f64, String> {
    // Extract <Average>...</Average> from CloudWatch XML response
    let start_tag = "<Average>";
    let end_tag = "</Average>";
    if let Some(s) = xml.find(start_tag) {
        let after = &xml[s + start_tag.len()..];
        if let Some(e) = after.find(end_tag) {
            return after[..e].trim().parse::<f64>().map_err(|e| e.to_string());
        }
    }
    Err("No datapoints in response".into())
}

// ──────────────────────────────────────────────────────────────────────────────
// SigV4 helpers
// ──────────────────────────────────────────────────────────────────────────────

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn hmac_hex(key: &[u8], data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    format!("{:x}", mac.finalize().into_bytes())
}

fn hmac_bytes(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn derive_signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_secret = format!("AWS4{}", secret);
    let k_date = hmac_bytes(k_secret.as_bytes(), date.as_bytes());
    let k_region = hmac_bytes(&k_date, region.as_bytes());
    let k_service = hmac_bytes(&k_region, service.as_bytes());
    hmac_bytes(&k_service, b"aws4_request")
}

fn url_encode(s: &str) -> String {
    s.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
            c.to_string()
        } else {
            format!("%{:02X}", c as u8)
        }
    }).collect()
}

fn build_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

fn parse_credentials(target: &Target) -> Option<AwsCredentials> {
    let creds = target.credentials.as_ref()?;
    let access_key_id = creds.username.clone()?;
    let secret_access_key = creds.decrypt_password();
    if access_key_id.is_empty() || secret_access_key.is_empty() {
        return None;
    }
    
    let region = target.connection_options.get("region")
        .and_then(|v| v.as_str())
        .unwrap_or("us-east-1")
        .to_string();
        
    let session_token = target.connection_options.get("session_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(AwsCredentials {
        access_key_id,
        secret_access_key,
        region,
        session_token,
    })
}

fn instance_id_from_target(target: &Target) -> String {
    target.connection_options.get("instance_id")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.host)
        .to_string()
}

fn db_instance_id_from_target(target: &Target) -> String {
    target.connection_options.get("db_instance_identifier")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.host)
        .to_string()
}

fn lb_name_from_target(target: &Target) -> String {
    target.connection_options.get("load_balancer_arn")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.host)
        .to_string()
}

fn fn_name_from_target(target: &Target) -> String {
    target.connection_options.get("function_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.host)
        .to_string()
}

fn cluster_name_from_target(target: &Target) -> String {
    target.connection_options.get("cluster_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.host)
        .to_string()
}
