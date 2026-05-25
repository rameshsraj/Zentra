//! Redfish REST API collector for server hardware inventory and health.
//! Connects to BMC (iDRAC, iLO, XCC, IPMI) via Redfish v1 API — agentless.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, warn};

fn srv(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "server-plugin")
        .with_resource_type("hardware")
}

#[derive(Debug, Deserialize, Default)]
struct RedfishHealth {
    #[serde(rename = "State")]
    state: Option<String>,
    #[serde(rename = "Health")]
    health: Option<String>,
}

pub struct RedfishCollector;

impl RedfishCollector {
    async fn get(client: &Client, url: &str, user: &str, pass: &str) -> Option<serde_json::Value> {
        client
            .get(url)
            .basic_auth(user, Some(pass))
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()
    }

    fn health_score(health: Option<&str>) -> f64 {
        match health {
            Some("OK") => 1.0,
            Some("Warning") => 0.5,
            _ => 0.0,
        }
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let port = if target.port > 0 { target.port } else { 443 };
        let (user, pass) = target.credentials.as_ref()
            .map(|c| (
                c.username.clone().unwrap_or_else(|| "admin".into()),
                c.password_encrypted.clone().unwrap_or_else(|| "admin".into()),
            ))
            .unwrap_or_else(|| ("admin".into(), "admin".into()));

        let base = format!("https://{}:{}/redfish/v1", host, port);

        let client = match Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!(target = %target.name, err = %e, "Redfish client build error");
                return metrics;
            }
        };

        // ── System summary ─────────────────────────────────────────────────
        if let Some(sys) = Self::get(&client, &format!("{}/Systems/System.Embedded.1", base), &user, &pass).await
            .or_else(|| None)
        {
            // Try common paths
            let system_urls = [
                format!("{}/Systems/System.Embedded.1", base),
                format!("{}/Systems/1", base),
                format!("{}/Systems/Self", base),
            ];

            for sys_url in &system_urls {
                if let Some(sys) = Self::get(&client, sys_url, &user, &pass).await {
                    let health = sys["Status"]["Health"].as_str();
                    metrics.push(srv("server.hardware.system_health", Self::health_score(health), "system", target)
                        .with_tag("health", health.unwrap_or("Unknown")));

                    if let Some(cores) = sys["ProcessorSummary"]["Count"].as_f64() {
                        metrics.push(srv("server.hardware.processor_count", cores, "system", target));
                    }
                    if let Some(mem_gb) = sys["MemoryGiB"].as_f64()
                        .or_else(|| sys["MemorySummary"]["TotalSystemMemoryGiB"].as_f64())
                    {
                        metrics.push(srv("server.hardware.memory_total_gib", mem_gb, "system", target).with_unit("gib"));
                    }
                    if let Some(state) = sys["PowerState"].as_str() {
                        let power_val = if state == "On" { 1.0 } else { 0.0 };
                        metrics.push(srv("server.hardware.power_state", power_val, "system", target)
                            .with_tag("state", state));
                    }
                    break;
                }
            }
            let _ = sys; // avoid unused warning
        }

        // ── Thermal (fans + temperatures) ──────────────────────────────────
        let thermal_urls = [
            format!("{}/Chassis/System.Embedded.1/Thermal", base),
            format!("{}/Chassis/1/Thermal", base),
            format!("{}/Chassis/Self/Thermal", base),
        ];

        for thermal_url in &thermal_urls {
            if let Some(thermal) = Self::get(&client, thermal_url, &user, &pass).await {
                // Temperatures
                if let Some(temps) = thermal["Temperatures"].as_array() {
                    for temp in temps {
                        let name = temp["Name"].as_str().unwrap_or("unknown");
                        if let Some(reading) = temp["ReadingCelsius"].as_f64() {
                            let resource = format!("temp_{}", name.to_lowercase().replace(' ', "_"));
                            metrics.push(srv("server.hardware.temperature_celsius", reading, &resource, target)
                                .with_tag("sensor", name).with_unit("celsius"));
                        }
                    }
                }
                // Fans
                if let Some(fans) = thermal["Fans"].as_array() {
                    for fan in fans {
                        let name = fan["FanName"].as_str()
                            .or_else(|| fan["Name"].as_str())
                            .unwrap_or("unknown");
                        if let Some(rpm) = fan["Reading"].as_f64() {
                            let resource = format!("fan_{}", name.to_lowercase().replace(' ', "_"));
                            metrics.push(srv("server.hardware.fan_rpm", rpm, &resource, target)
                                .with_tag("fan", name).with_unit("rpm"));
                        }
                        let fan_health = fan["Status"]["Health"].as_str();
                        metrics.push(srv("server.hardware.fan_health",
                            Self::health_score(fan_health),
                            &format!("fan_{}", name.to_lowercase().replace(' ', "_")),
                            target).with_tag("fan", name));
                    }
                }
                break;
            }
        }

        // ── Power (PSU wattage) ────────────────────────────────────────────
        let power_urls = [
            format!("{}/Chassis/System.Embedded.1/Power", base),
            format!("{}/Chassis/1/Power", base),
            format!("{}/Chassis/Self/Power", base),
        ];

        for power_url in &power_urls {
            if let Some(power) = Self::get(&client, power_url, &user, &pass).await {
                // PowerControl: actual consumed wattage
                if let Some(controls) = power["PowerControl"].as_array() {
                    for ctrl in controls {
                        if let Some(watts) = ctrl["PowerConsumedWatts"].as_f64() {
                            metrics.push(srv("server.hardware.power_consumed_watts", watts, "psu", target).with_unit("watts"));
                        }
                        if let Some(watts) = ctrl["PowerCapacityWatts"].as_f64() {
                            metrics.push(srv("server.hardware.power_capacity_watts", watts, "psu", target).with_unit("watts"));
                        }
                    }
                }
                // PSU health
                if let Some(supplies) = power["PowerSupplies"].as_array() {
                    for (i, psu) in supplies.iter().enumerate() {
                        let psu_id = format!("psu{}", i);
                        let health = psu["Status"]["Health"].as_str();
                        metrics.push(srv("server.hardware.psu_health", Self::health_score(health), &psu_id, target)
                            .with_tag("psu", &psu_id).with_tag("health", health.unwrap_or("Unknown")));
                        if let Some(watts) = psu["LastPowerOutputWatts"].as_f64() {
                            metrics.push(srv("server.hardware.psu_output_watts", watts, &psu_id, target)
                                .with_tag("psu", &psu_id).with_unit("watts"));
                        }
                    }
                }
                break;
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "Redfish collection complete");
        metrics
    }
}
