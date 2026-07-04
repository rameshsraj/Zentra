//! Local component discovery — detects services running on the agent's own host.
//!
//! Detection strategy (no root required):
//!   1. Parse /proc/net/tcp and /proc/net/tcp6 for listening ports → map to known services.
//!   2. Scan /proc/*/comm + /proc/*/cmdline for recognizable process names.
//!   3. Try systemctl is-active for known service units (best-effort).
//!   4. Report to /api/agents/by-dc/:dc_name/components every 5 minutes.

use crate::heartbeat::AgentIdCell;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const REPORT_INTERVAL_SECS: u64 = 300; // 5 min
const AGENT_ID_WAIT_SECS: u64 = 60;    // max wait for agent_id from first heartbeat

pub struct ComponentDiscovery {
    client: Client,
    api_endpoint: String,
    api_key: String,
    dc_name: String,
    cancel: CancellationToken,
    agent_id_cell: AgentIdCell,
}

impl ComponentDiscovery {
    pub fn new(
        api_endpoint: String,
        api_key: String,
        dc_name: String,
        cancel: CancellationToken,
        agent_id_cell: AgentIdCell,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to build component-discovery HTTP client");
        Self {
            client,
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_key,
            dc_name,
            cancel,
            agent_id_cell,
        }
    }

    pub async fn run(self) {
        info!("Component discovery started ({}s interval)", REPORT_INTERVAL_SECS);

        // Wait until the heartbeat loop has populated agent_id from the server.
        // This guarantees the server has registered this agent before we report.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(AGENT_ID_WAIT_SECS);
        loop {
            if self.agent_id_cell.get().is_some() { break; }
            if tokio::time::Instant::now() >= deadline {
                warn!("Timed out waiting for agent_id — component discovery will proceed anyway");
                break;
            }
            tokio::select! {
                _ = self.cancel.cancelled() => { return; }
                _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            }
        }

        let mut ticker = tokio::time::interval(Duration::from_secs(REPORT_INTERVAL_SECS));

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Component discovery stopping (shutdown)");
                    break;
                }
                _ = ticker.tick() => {}
            }

            let components = discover_local_components().await;
            if components.is_empty() {
                debug!("No local components discovered");
                continue;
            }

            info!(count = components.len(), "Local components discovered, reporting to server");
            if let Err(e) = self.report(&components).await {
                warn!(error = %e, "Failed to report local components");
            }
        }
    }

    async fn report(&self, components: &[DiscoveredComponent]) -> Result<(), String> {
        let entries: Vec<Value> = components
            .iter()
            .map(|c| json!({
                "component_type": c.component_type,
                "component_name": c.component_name,
                "address":        c.address,
                "port":           c.port,
                "version":        c.version,
                "extra":          c.extra,
            }))
            .collect();

        // Encode dc_name for URL safety (replace spaces; most names are alphanumeric)
        let dc_encoded: String = self.dc_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c.to_string() } else { format!("%{:02X}", c as u32) })
            .collect();
        let url = format!(
            "{}/api/agents/by-dc/{}/components",
            self.api_endpoint, dc_encoded
        );

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({ "components": entries }))
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Server returned {status}: {body}"));
        }

        debug!("Component report accepted by server");
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovery logic
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DiscoveredComponent {
    component_type: String,
    component_name: String,
    address: String,
    port: Option<i32>,
    version: Option<String>,
    extra: Value,
}

/// Top-level: combine port-based and process-based detection.
async fn discover_local_components() -> Vec<DiscoveredComponent> {
    let mut components: HashMap<String, DiscoveredComponent> = HashMap::new();

    // 1. Always add the host OS itself as a "linux" component
    let os_version = read_os_version();
    let host_key = "host:linux".to_string();
    components.insert(host_key, DiscoveredComponent {
        component_type: "linux".to_string(),
        component_name: "Linux Host".to_string(),
        address: "127.0.0.1".to_string(),
        port: None,
        version: os_version,
        extra: json!({}),
    });

    // 2. Parse listening ports from /proc/net/tcp[6]
    let listening = listening_ports();
    for port in &listening {
        if let Some(def) = port_to_component(*port) {
            let key = format!("{}:{}", def.component_type, port);
            components.entry(key).or_insert_with(|| DiscoveredComponent {
                component_type: def.component_type.clone(),
                component_name: def.component_name.clone(),
                address: "127.0.0.1".to_string(),
                port: Some(*port as i32),
                version: probe_version(&def.component_type),
                extra: json!({ "detected_via": "port_scan" }),
            });
        }
    }

    // 3. Scan /proc for known process names
    let procs = running_processes();
    for proc_name in &procs {
        if let Some(def) = proc_to_component(proc_name) {
            let key = format!("proc:{}", def.component_type);
            components.entry(key).or_insert_with(|| DiscoveredComponent {
                component_type: def.component_type.clone(),
                component_name: def.component_name.clone(),
                address: "127.0.0.1".to_string(),
                port: def.default_port,
                version: probe_version(&def.component_type),
                extra: json!({ "detected_via": "process", "process_name": proc_name }),
            });
        }
    }

    components.into_values().collect()
}

struct ComponentDef {
    component_type: String,
    component_name: String,
    default_port: Option<i32>,
}

/// Maps a listening TCP port to a known component type.
fn port_to_component(port: u16) -> Option<ComponentDef> {
    match port {
        5432 => Some(ComponentDef { component_type: "postgres".into(), component_name: "PostgreSQL".into(), default_port: Some(5432) }),
        3306 => Some(ComponentDef { component_type: "mysql".into(), component_name: "MySQL".into(), default_port: Some(3306) }),
        1521 | 1522 => Some(ComponentDef { component_type: "oracle".into(), component_name: "Oracle DB".into(), default_port: Some(port as i32) }),
        6379 => Some(ComponentDef { component_type: "redis".into(), component_name: "Redis".into(), default_port: Some(6379) }),
        27017 => Some(ComponentDef { component_type: "mongodb".into(), component_name: "MongoDB".into(), default_port: Some(27017) }),
        80 | 8080 => Some(ComponentDef { component_type: "nginx".into(), component_name: "Web Server (HTTP)".into(), default_port: Some(port as i32) }),
        443 | 8443 => Some(ComponentDef { component_type: "nginx".into(), component_name: "Web Server (HTTPS)".into(), default_port: Some(port as i32) }),
        9200 | 9300 => Some(ComponentDef { component_type: "elasticsearch".into(), component_name: "Elasticsearch".into(), default_port: Some(port as i32) }),
        5672 | 15672 => Some(ComponentDef { component_type: "rabbitmq".into(), component_name: "RabbitMQ".into(), default_port: Some(port as i32) }),
        9092 => Some(ComponentDef { component_type: "kafka".into(), component_name: "Kafka".into(), default_port: Some(9092) }),
        11211 => Some(ComponentDef { component_type: "memcached".into(), component_name: "Memcached".into(), default_port: Some(11211) }),
        _ => None,
    }
}

/// Maps a process name to a known component type.
fn proc_to_component(name: &str) -> Option<ComponentDef> {
    let n = name.to_lowercase();
    if n.contains("postgres") {
        return Some(ComponentDef { component_type: "postgres".into(), component_name: "PostgreSQL".into(), default_port: Some(5432) });
    }
    if n.contains("mysqld") || n == "mysql" {
        return Some(ComponentDef { component_type: "mysql".into(), component_name: "MySQL".into(), default_port: Some(3306) });
    }
    if n.contains("oracle") || n == "ora_pmon" || n.starts_with("ora_") {
        return Some(ComponentDef { component_type: "oracle".into(), component_name: "Oracle DB".into(), default_port: Some(1521) });
    }
    if n.contains("redis") {
        return Some(ComponentDef { component_type: "redis".into(), component_name: "Redis".into(), default_port: Some(6379) });
    }
    if n.contains("mongod") {
        return Some(ComponentDef { component_type: "mongodb".into(), component_name: "MongoDB".into(), default_port: Some(27017) });
    }
    if n == "nginx" || n.starts_with("nginx:") {
        return Some(ComponentDef { component_type: "nginx".into(), component_name: "nginx".into(), default_port: Some(80) });
    }
    if n.contains("apache2") || n == "httpd" {
        return Some(ComponentDef { component_type: "apache".into(), component_name: "Apache HTTP".into(), default_port: Some(80) });
    }
    if n.contains("haproxy") {
        return Some(ComponentDef { component_type: "haproxy".into(), component_name: "HAProxy".into(), default_port: Some(80) });
    }
    if n.contains("php-fpm") || n.contains("php_fpm") {
        return Some(ComponentDef { component_type: "phpfpm".into(), component_name: "PHP-FPM".into(), default_port: None });
    }
    if n.contains("java") || n.contains("tomcat") || n.contains("catalina") {
        return Some(ComponentDef { component_type: "tomcat".into(), component_name: "Tomcat / Java App".into(), default_port: Some(8080) });
    }
    if n.contains("elasticsearch") {
        return Some(ComponentDef { component_type: "elasticsearch".into(), component_name: "Elasticsearch".into(), default_port: Some(9200) });
    }
    if n.contains("kafka") {
        return Some(ComponentDef { component_type: "kafka".into(), component_name: "Kafka".into(), default_port: Some(9092) });
    }
    if n.contains("rabbitmq") || n.contains("beam.smp") {
        return Some(ComponentDef { component_type: "rabbitmq".into(), component_name: "RabbitMQ".into(), default_port: Some(5672) });
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// /proc parsing helpers (Linux-only; gracefully no-ops on other platforms)
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the set of TCP ports currently in LISTEN state.
fn listening_ports() -> Vec<u16> {
    let mut ports = Vec::new();
    for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines().skip(1) {
                let cols: Vec<&str> = line.split_whitespace().collect();
                // state is cols[3]; 0A = LISTEN
                if cols.len() >= 4 && cols[3] == "0A" {
                    // local_address is cols[1]: hex_ip:hex_port
                    if let Some(hex_port) = cols[1].split(':').nth(1) {
                        if let Ok(port) = u16::from_str_radix(hex_port, 16) {
                            if !ports.contains(&port) {
                                ports.push(port);
                            }
                        }
                    }
                }
            }
        }
    }
    ports
}

/// Returns process names from /proc/*/comm.
fn running_processes() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let comm_path = entry.path().join("comm");
            if let Ok(name) = std::fs::read_to_string(&comm_path) {
                let name = name.trim().to_string();
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

/// Reads /etc/os-release for PRETTY_NAME.
fn read_os_version() -> Option<String> {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                return Some(val.trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Best-effort version probe by running the CLI tool.
fn probe_version(component_type: &str) -> Option<String> {
    let cmd = match component_type {
        "postgres"  => vec!["psql", "--version"],
        "mysql"     => vec!["mysql", "--version"],
        "redis"     => vec!["redis-server", "--version"],
        "nginx"     => vec!["nginx", "-v"],
        "apache"    => vec!["apache2", "-v"],
        _           => return None,
    };

    if let Ok(out) = std::process::Command::new(cmd[0])
        .args(&cmd[1..])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        let first_line = text.lines().next().unwrap_or("").trim().to_string();
        if !first_line.is_empty() {
            return Some(first_line);
        }
    }
    None
}
