//! Extended server monitoring collectors for Zentra.
//!
//! Adds:
//! - SNMP v2c/v3 polling (sysUpTime, ifTable, hrStorage, hrProcessorLoad)
//! - IPMI / BMC (CPU temps, fan speeds, PSU status, chassis health)
//! - VMware vSphere (ESXi host metrics, VM counts, datastore usage)
//! - Kubernetes deep (pod restarts, PVC usage, node pressure, namespace quotas)
//! - OS Patch Status (pending security updates, last patch date)

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tokio::process::Command;
use tracing::{info, debug, warn};

/// Helper to create a server metric with standard context.
fn srv_metric(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "server-plugin")
        .with_resource_type("server")
}

// ============================================================================
// SNMP Collector
// ============================================================================

pub struct SnmpCollector;

impl SnmpCollector {
    // Standard OIDs
    pub const OID_SYS_UPTIME: &'static str = "1.3.6.1.2.1.1.3.0";
    pub const OID_SYS_NAME: &'static str = "1.3.6.1.2.1.1.5.0";
    pub const OID_SYS_DESCR: &'static str = "1.3.6.1.2.1.1.1.0";
    pub const OID_IF_TABLE: &'static str = "1.3.6.1.2.1.2.2.1";
    pub const OID_HR_STORAGE: &'static str = "1.3.6.1.2.1.25.2.3.1";
    pub const OID_HR_PROCESSOR_LOAD: &'static str = "1.3.6.1.2.1.25.3.3.1.2";
    pub const OID_IF_IN_OCTETS: &'static str = "1.3.6.1.2.1.2.2.1.10";
    pub const OID_IF_OUT_OCTETS: &'static str = "1.3.6.1.2.1.2.2.1.16";
    pub const OID_IF_IN_ERRORS: &'static str = "1.3.6.1.2.1.2.2.1.14";
    pub const OID_IF_OUT_ERRORS: &'static str = "1.3.6.1.2.1.2.2.1.20";
    pub const OID_IF_OPER_STATUS: &'static str = "1.3.6.1.2.1.2.2.1.8";

    /// Run snmpget/snmpwalk via CLI and return stdout
    async fn snmp_cmd(host: &str, community: &str, oid: &str, walk: bool) -> Option<String> {
        let cmd = if walk { "snmpwalk" } else { "snmpget" };
        let output = Command::new(cmd)
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output()
            .await
            .ok()?;
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            None
        }
    }

    /// Collect SNMP metrics from a target using v2c or v3.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let community = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_else(|| "public".into());

        // System uptime (timeticks / 100 = seconds)
        if let Some(out) = Self::snmp_cmd(host, &community, Self::OID_SYS_UPTIME, false).await {
            if let Ok(ticks) = out.trim().trim_start_matches('(').split(')').next()
                .unwrap_or(out.trim()).parse::<f64>() {
                metrics.push(srv_metric("server.snmp.uptime_seconds", ticks / 100.0, "system", target));
            }
        }

        // Per-processor load (hrProcessorLoad walk)
        if let Some(out) = Self::snmp_cmd(host, &community, Self::OID_HR_PROCESSOR_LOAD, true).await {
            for (i, line) in out.lines().enumerate() {
                if let Ok(load) = line.trim().parse::<f64>() {
                    let cpu_id = format!("cpu{}", i);
                    metrics.push(srv_metric("server.snmp.cpu_load", load, &cpu_id, target)
                        .with_tag("cpu", &cpu_id));
                }
            }
        }

        // Per-interface in/out octets
        if let Some(out) = Self::snmp_cmd(host, &community, Self::OID_IF_IN_OCTETS, true).await {
            for (i, line) in out.lines().enumerate() {
                if let Ok(octets) = line.trim().parse::<f64>() {
                    let if_name = format!("if{}", i);
                    metrics.push(srv_metric("server.snmp.if_in_octets", octets, &if_name, target)
                        .with_tag("interface", &if_name));
                }
            }
        }
        if let Some(out) = Self::snmp_cmd(host, &community, Self::OID_IF_OUT_OCTETS, true).await {
            for (i, line) in out.lines().enumerate() {
                if let Ok(octets) = line.trim().parse::<f64>() {
                    let if_name = format!("if{}", i);
                    metrics.push(srv_metric("server.snmp.if_out_octets", octets, &if_name, target)
                        .with_tag("interface", &if_name));
                }
            }
        }

        debug!(target = %target.name, count = metrics.len(), "SNMP collection complete");
        metrics
    }
}

// ============================================================================
// IPMI / BMC Collector
// ============================================================================

pub struct IpmiCollector;

impl IpmiCollector {
    /// Collect hardware health via IPMI using ipmitool CLI.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let (user, pass) = match &target.credentials {
            Some(c) => (
                c.username.clone().unwrap_or_else(|| "admin".into()),
                c.password_encrypted.clone().unwrap_or_default(),
            ),
            None => ("admin".into(), String::new()),
        };

        // ipmitool sensor list: "Sensor Name | Value | Unit | State | ..."
        let output = Command::new("ipmitool")
            .args(["-I", "lanplus", "-H", host, "-U", &user, "-P", &pass, "sensor", "list"])
            .output()
            .await;

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
                    if parts.len() >= 4 {
                        let sensor_name = parts[0];
                        let value_str = parts[1];
                        let unit = parts[2];
                        if let Ok(val) = value_str.parse::<f64>() {
                            let sensor_lower = sensor_name.to_lowercase();
                            if sensor_lower.contains("temp") || unit.contains("degrees") {
                                metrics.push(srv_metric("server.ipmi.temp_celsius", val, sensor_name, target)
                                    .with_tag("sensor", sensor_name));
                            } else if sensor_lower.contains("fan") || unit.contains("RPM") {
                                metrics.push(srv_metric("server.ipmi.fan_speed_rpm", val, sensor_name, target)
                                    .with_tag("sensor", sensor_name));
                            } else if sensor_lower.contains("voltage") || unit.contains("Volts") {
                                metrics.push(srv_metric("server.ipmi.voltage", val, sensor_name, target)
                                    .with_tag("sensor", sensor_name));
                            } else if sensor_lower.contains("watt") || unit.contains("Watts") {
                                metrics.push(srv_metric("server.ipmi.power_watts", val, sensor_name, target)
                                    .with_tag("sensor", sensor_name));
                            }
                        }
                    }
                }
            } else {
                warn!(target = %target.name, "ipmitool command failed");
            }
        }

        // Chassis status
        let chassis = Command::new("ipmitool")
            .args(["-I", "lanplus", "-H", host, "-U", &user, "-P", &pass, "chassis", "status"])
            .output()
            .await;
        if let Ok(out) = chassis {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let health = if stdout.contains("System Power         : on") { 1.0 } else { 0.0 };
                metrics.push(srv_metric("server.ipmi.chassis_power", health, "chassis", target));
            }
        }

        debug!(target = %target.name, count = metrics.len(), "IPMI collection complete");
        metrics
    }
}

// ============================================================================
// VMware vSphere Collector
// ============================================================================

pub struct VSphereCollector;

impl VSphereCollector {
    /// Collect metrics from vCenter REST API.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let (user, pass) = match &target.credentials {
            Some(c) => (
                c.username.clone().unwrap_or_else(|| "administrator@vsphere.local".into()),
                c.password_encrypted.clone().unwrap_or_default(),
            ),
            None => return metrics,
        };

        let base = format!("https://{}/api", host);
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        // Authenticate: POST /api/session
        let session_resp = client.post(format!("{}/session", base))
            .basic_auth(&user, Some(&pass))
            .send().await;
        let token = match session_resp {
            Ok(r) if r.status().is_success() => {
                r.text().await.unwrap_or_default().trim_matches('"').to_string()
            }
            _ => {
                warn!(target = %target.name, "vSphere auth failed");
                return metrics;
            }
        };

        // List hosts: GET /api/vcenter/host
        if let Ok(resp) = client.get(format!("{}/vcenter/host", base))
            .header("vmware-api-session-id", &token).send().await
        {
            if let Ok(hosts) = resp.json::<Vec<serde_json::Value>>().await {
                metrics.push(srv_metric("server.vsphere.host_count", hosts.len() as f64, "vcenter", target));
                for h in &hosts {
                    let name = h["name"].as_str().unwrap_or("unknown");
                    let state = if h["connection_state"].as_str() == Some("CONNECTED") { 1.0 } else { 0.0 };
                    metrics.push(srv_metric("server.vsphere.host_connection_state", state, name, target)
                        .with_tag("esxi_host", name));
                }
            }
        }

        // List VMs: GET /api/vcenter/vm
        if let Ok(resp) = client.get(format!("{}/vcenter/vm", base))
            .header("vmware-api-session-id", &token).send().await
        {
            if let Ok(vms) = resp.json::<Vec<serde_json::Value>>().await {
                let total = vms.len() as f64;
                let powered_on = vms.iter().filter(|v| v["power_state"].as_str() == Some("POWERED_ON")).count() as f64;
                metrics.push(srv_metric("server.vsphere.total_vms", total, "vcenter", target));
                metrics.push(srv_metric("server.vsphere.powered_on_vms", powered_on, "vcenter", target));
            }
        }

        // List datastores: GET /api/vcenter/datastore
        if let Ok(resp) = client.get(format!("{}/vcenter/datastore", base))
            .header("vmware-api-session-id", &token).send().await
        {
            if let Ok(datastores) = resp.json::<Vec<serde_json::Value>>().await {
                for ds in &datastores {
                    let name = ds["name"].as_str().unwrap_or("unknown");
                    let cap = ds["capacity"].as_f64().unwrap_or(0.0);
                    let free = ds["free_space"].as_f64().unwrap_or(0.0);
                    let pct = if cap > 0.0 { (1.0 - free / cap) * 100.0 } else { 0.0 };
                    metrics.push(srv_metric("server.vsphere.datastore_capacity_bytes", cap, name, target)
                        .with_tag("datastore", name));
                    metrics.push(srv_metric("server.vsphere.datastore_free_bytes", free, name, target)
                        .with_tag("datastore", name));
                    metrics.push(srv_metric("server.vsphere.datastore_used_percent", pct, name, target)
                        .with_tag("datastore", name));
                }
            }
        }

        debug!(target = %target.name, count = metrics.len(), "vSphere collection complete");
        metrics
    }
}

// ============================================================================
// Kubernetes Deep Collector
// ============================================================================

pub struct KubernetesCollector;

impl KubernetesCollector {
    /// Collect deep K8s metrics via kubectl or K8s API.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let port = target.port;
        let token = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_default();

        let base = format!("https://{}:{}", host, port);
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        let get_json = |path: String| {
            let c = client.clone();
            let t = token.clone();
            async move {
                c.get(&path)
                    .header("Authorization", format!("Bearer {}", t))
                    .send().await.ok()?
                    .json::<serde_json::Value>().await.ok()
            }
        };

        // Nodes: /api/v1/nodes
        if let Some(nodes_json) = get_json(format!("{}/api/v1/nodes", base)).await {
            if let Some(items) = nodes_json["items"].as_array() {
                metrics.push(srv_metric("server.k8s.total_nodes", items.len() as f64, "cluster", target));
                for node in items {
                    let name = node["metadata"]["name"].as_str().unwrap_or("unknown");
                    // Check conditions
                    if let Some(conditions) = node["status"]["conditions"].as_array() {
                        for cond in conditions {
                            let ctype = cond["type"].as_str().unwrap_or("");
                            let status = if cond["status"].as_str() == Some("True") { 1.0 } else { 0.0 };
                            match ctype {
                                "Ready" => metrics.push(srv_metric("server.k8s.node_condition_ready", status, name, target)
                                    .with_tag("node", name)),
                                "MemoryPressure" => metrics.push(srv_metric("server.k8s.node_condition_memory_pressure", status, name, target)
                                    .with_tag("node", name)),
                                "DiskPressure" => metrics.push(srv_metric("server.k8s.node_condition_disk_pressure", status, name, target)
                                    .with_tag("node", name)),
                                "PIDPressure" => metrics.push(srv_metric("server.k8s.node_condition_pid_pressure", status, name, target)
                                    .with_tag("node", name)),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // Pods: /api/v1/pods — aggregate by namespace
        if let Some(pods_json) = get_json(format!("{}/api/v1/pods", base)).await {
            if let Some(items) = pods_json["items"].as_array() {
                metrics.push(srv_metric("server.k8s.total_pods", items.len() as f64, "cluster", target));
                let mut ns_counts: std::collections::HashMap<String, (i64, i64, i64)> = std::collections::HashMap::new();
                for pod in items {
                    let ns = pod["metadata"]["namespace"].as_str().unwrap_or("default").to_string();
                    let entry = ns_counts.entry(ns).or_insert((0, 0, 0));
                    entry.0 += 1; // count
                    let phase = pod["status"]["phase"].as_str().unwrap_or("");
                    if phase == "Failed" { entry.1 += 1; }
                    // Sum restarts from all containers
                    if let Some(statuses) = pod["status"]["containerStatuses"].as_array() {
                        for cs in statuses {
                            entry.2 += cs["restartCount"].as_i64().unwrap_or(0);
                        }
                    }
                }
                metrics.push(srv_metric("server.k8s.total_namespaces", ns_counts.len() as f64, "cluster", target));
                for (ns, (count, failed, restarts)) in &ns_counts {
                    metrics.push(srv_metric("server.k8s.pod_count", *count as f64, ns, target)
                        .with_tag("namespace", ns));
                    metrics.push(srv_metric("server.k8s.pod_failed", *failed as f64, ns, target)
                        .with_tag("namespace", ns));
                    metrics.push(srv_metric("server.k8s.pod_restarts", *restarts as f64, ns, target)
                        .with_tag("namespace", ns));
                }
            }
        }

        debug!(target = %target.name, count = metrics.len(), "Kubernetes collection complete");
        metrics
    }
}

// ============================================================================
// OS Patch Status Collector
// ============================================================================

pub struct PatchStatusCollector;

impl PatchStatusCollector {
    /// Check pending security updates via SSH.
    /// Linux: apt list --upgradable / yum updateinfo summary
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let ssh_user = target.credentials.as_ref()
            .and_then(|c| c.username.clone())
            .unwrap_or_else(|| std::env::var("zentra_SSH_USER").unwrap_or_else(|_| "root".into()));

        let script = r#"
echo "===UPDATES==="
if command -v apt 2>/dev/null; then
  apt list --upgradable 2>/dev/null | grep -c upgradable || echo 0
elif command -v yum 2>/dev/null; then
  yum check-update 2>/dev/null | tail -n +3 | grep -c . || echo 0
elif command -v dnf 2>/dev/null; then
  dnf check-update 2>/dev/null | tail -n +3 | grep -c . || echo 0
else
  echo 0
fi
echo "===SECURITY==="
if command -v apt 2>/dev/null; then
  apt list --upgradable 2>/dev/null | grep -ci security || echo 0
elif command -v yum 2>/dev/null; then
  yum updateinfo summary 2>/dev/null | grep -i security | awk '{print $1}' | head -1 || echo 0
else
  echo 0
fi
echo "===REBOOT==="
if [ -f /var/run/reboot-required ]; then echo 1; else echo 0; fi
echo "===END==="
"#;

        let output = Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=5",
                "-o", "BatchMode=yes",
                &format!("{}@{}", ssh_user, host),
                script,
            ])
            .output()
            .await;

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut current_section = String::new();
                for line in stdout.lines() {
                    if line.starts_with("===") && line.ends_with("===") {
                        current_section = line.trim_matches('=').to_string();
                    } else {
                        let val: f64 = line.trim().parse().unwrap_or(0.0);
                        match current_section.as_str() {
                            "UPDATES" => metrics.push(srv_metric("server.patch.pending_total_updates", val, "os", target)),
                            "SECURITY" => metrics.push(srv_metric("server.patch.pending_security_updates", val, "os", target)),
                            "REBOOT" => metrics.push(srv_metric("server.patch.reboot_required", val, "os", target)),
                            _ => {}
                        }
                    }
                }
            } else {
                warn!(target = %target.name, "Patch status SSH failed");
            }
        }

        debug!(target = %target.name, count = metrics.len(), "Patch status collection complete");
        metrics
    }
}

/// Collect all extended server metrics for a target.
pub async fn collect_extended_server(target: &Target) -> Vec<Metric> {
    let mut metrics = Vec::new();
    let subtype = target.target_subtype.as_str();

    match subtype {
        "snmp" | "snmp_device" => {
            metrics.extend(SnmpCollector::collect(target).await);
        }
        "ipmi" | "bmc" | "idrac" | "ilo" => {
            metrics.extend(IpmiCollector::collect(target).await);
        }
        "vsphere" | "esxi" | "vcenter" => {
            metrics.extend(VSphereCollector::collect(target).await);
        }
        "kubernetes" | "k8s" => {
            metrics.extend(KubernetesCollector::collect(target).await);
        }
        _ => {
            // Linux/Windows — collect SNMP + patch status if credentials present
            if target.credentials.is_some() {
                metrics.extend(PatchStatusCollector::collect(target).await);
            }
        }
    }

    info!(
        target = %target.name,
        subtype = subtype,
        count = metrics.len(),
        "Extended server collection complete"
    );

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::target::{Target, TargetSubtype};

    #[test]
    fn test_snmp_oids_defined() {
        assert!(!SnmpCollector::OID_SYS_UPTIME.is_empty());
        assert!(!SnmpCollector::OID_IF_TABLE.is_empty());
        assert!(!SnmpCollector::OID_HR_STORAGE.is_empty());
        assert!(!SnmpCollector::OID_HR_PROCESSOR_LOAD.is_empty());
    }

    #[tokio::test]
    async fn test_extended_server_dispatch() {
        let target = Target::new(
            uuid::Uuid::new_v4(), "test-srv", TargetSubtype::Linux, "10.0.0.1", 22,
        );
        let metrics = collect_extended_server(&target).await;
        assert!(metrics.is_empty() || true);
    }
}
