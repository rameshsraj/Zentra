use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentKeyMaterial {
    pub agent_key: String,
    pub agent_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct AgentKeyMeta {
    agent_id: Option<Uuid>,
    expires_at: Option<DateTime<Utc>>,
}

fn key_meta_path(key_path: &str) -> String {
    format!("{}.meta", key_path)
}

fn load_meta(key_path: &str) -> AgentKeyMeta {
    std::fs::read_to_string(key_meta_path(key_path))
        .ok()
        .and_then(|s| serde_json::from_str::<AgentKeyMeta>(&s).ok())
        .unwrap_or_default()
}

fn save_meta(key_path: &str, meta: &AgentKeyMeta) {
    if let Some(parent) = Path::new(key_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(txt) = serde_json::to_string(meta) {
        let _ = std::fs::write(key_meta_path(key_path), txt);
    }
}

fn load_agent_key_file(key_path: &str) -> Option<String> {
    std::fs::read_to_string(key_path)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn save_agent_key_file(key_path: &str, value: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(key_path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(key_path, value.as_bytes()).map_err(|e| e.to_string())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(key_path, perms).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn local_outbound_ip(api_endpoint: &str) -> String {
    use std::net::UdpSocket;
    let host = api_endpoint
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");

    if host.is_empty() {
        return "127.0.0.1".to_string();
    }

    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect(format!("{}:80", host))?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

pub async fn ensure_agent_key(
    api_endpoint: &str,
    dc_name: &str,
    org_key: &mut String,
    existing_agent_key: &str,
    key_path: &str,
) -> Result<AgentKeyMaterial, String> {
    if !existing_agent_key.trim().is_empty() {
        let mut meta = load_meta(key_path);
        save_agent_key_file(key_path, existing_agent_key)?;
        if meta.expires_at.is_none() {
            meta.expires_at = Some(Utc::now() + Duration::days(365));
            save_meta(key_path, &meta);
        }
        return Ok(AgentKeyMaterial {
            agent_key: existing_agent_key.trim().to_string(),
            agent_id: meta.agent_id,
            expires_at: meta.expires_at,
        });
    }

    if let Some(file_key) = load_agent_key_file(key_path) {
        let meta = load_meta(key_path);
        return Ok(AgentKeyMaterial {
            agent_key: file_key,
            agent_id: meta.agent_id,
            expires_at: meta.expires_at,
        });
    }

    if !org_key.starts_with("isok_") {
        return Err("No agent key available and org key is missing/invalid".into());
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| dc_name.to_string());

    let ip = local_outbound_ip(api_endpoint);
    let url = format!("{}/api/agents/register", api_endpoint.trim_end_matches('/'));
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", org_key))
        .json(&json!({
            "datacenter_name": dc_name,
            "hostname": hostname,
            "ip_address": ip,
            "os": std::env::consts::OS,
            "agent_version": env!("CARGO_PKG_VERSION"),
        }))
        .send()
        .await
        .map_err(|e| format!("agent register request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("agent registration failed with {}: {}", status, body));
    }

    let payload: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("invalid register response JSON: {}", e))?;

    if payload
        .get("status")
        .and_then(|v| v.as_str())
        == Some("pending_approval")
    {
        return Err("agent registration is pending admin approval".into());
    }

    let agent_key = payload
        .get("agent_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "register response missing agent_key".to_string())?
        .to_string();

    let agent_id = payload
        .get("agent_id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Uuid>().ok());

    let expires_at = payload
        .get("expires_at")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|ts| ts.with_timezone(&Utc));

    save_agent_key_file(key_path, &agent_key)?;
    save_meta(key_path, &AgentKeyMeta { agent_id, expires_at });

    org_key.clear();
    std::env::remove_var("ZENTRA_ORG_KEY");
    std::env::remove_var("ZENTRA_AGENT_API_KEY");

    info!("Agent successfully registered and operational key stored on disk");

    Ok(AgentKeyMaterial {
        agent_key,
        agent_id,
        expires_at,
    })
}

pub async fn auto_rotate_if_needed(
    api_endpoint: &str,
    key_path: &str,
    material: &mut AgentKeyMaterial,
) -> Result<(), String> {
    let Some(expiry) = material.expires_at else {
        return Ok(());
    };
    let Some(agent_id) = material.agent_id else {
        return Ok(());
    };

    if expiry > Utc::now() + Duration::days(30) {
        return Ok(());
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "{}/api/agents/{}/rotate-key",
        api_endpoint.trim_end_matches('/'),
        agent_id
    );

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", material.agent_key))
        .send()
        .await
        .map_err(|e| format!("rotate-key request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!(status = %status, "Auto-rotation skipped: {}", body);
        return Ok(());
    }

    let payload: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("invalid rotate response JSON: {}", e))?;

    let new_key = payload
        .get("agent_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "rotate response missing agent_key".to_string())?
        .to_string();

    let new_expiry = payload
        .get("expires_at")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|ts| ts.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() + Duration::days(365));

    save_agent_key_file(key_path, &new_key)?;
    save_meta(
        key_path,
        &AgentKeyMeta {
            agent_id: Some(agent_id),
            expires_at: Some(new_expiry),
        },
    );

    material.agent_key = new_key;
    material.expires_at = Some(new_expiry);

    info!(agent_id = %agent_id, "Agent key rotated successfully");
    Ok(())
}
