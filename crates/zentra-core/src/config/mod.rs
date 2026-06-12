use serde::{Deserialize, Serialize};

/// Application-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub org_id: uuid::Uuid,
    pub api_port: u16,
    pub agent_port: u16,
    pub database_url: String,
    pub collection_interval_secs: u64,
    pub log_level: String,
    pub max_plugins: usize,
    /// Agent mode: "local" (default) or "remote"
    pub agent_mode: String,
    /// Zentra server API endpoint (required in remote mode)
    pub api_endpoint: String,
    /// Agent API key for remote mode (must start with "isok_")
    pub agent_api_key: String,
    /// Datacenter name for heartbeat and OTLP resource attributes
    pub dc_name: String,
    /// Offline buffer directory for remote mode
    pub buffer_dir: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            org_id: uuid::Uuid::nil(),
            api_port: 5006,
            agent_port: 5001,
            // No default credentials — must be supplied via DATABASE_URL env var.
            database_url: String::new(),
            collection_interval_secs: 60,
            log_level: "info".into(),
            max_plugins: 20,
            agent_mode: "local".into(),
            api_endpoint: String::new(),
            agent_api_key: String::new(),
            dc_name: String::new(),
            buffer_dir: String::new(),
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(v) = std::env::var("ZENTRA_ORG_ID") {
            if let Ok(id) = v.parse() {
                config.org_id = id;
            }
        }
        if let Ok(v) = std::env::var("ZENTRA_API_PORT") {
            if let Ok(p) = v.parse() {
                config.api_port = p;
            }
        }
        if let Ok(v) = std::env::var("ZENTRA_AGENT_PORT") {
            if let Ok(p) = v.parse() {
                config.agent_port = p;
            }
        }
        if let Ok(v) = std::env::var("DATABASE_URL") {
            config.database_url = v;
        }

        // Agent mode configuration
        if let Ok(v) = std::env::var("ZENTRA_AGENT_MODE") {
            let mode = v.to_lowercase();
            if mode != "local" && mode != "remote" {
                eprintln!("FATAL: ZENTRA_AGENT_MODE must be 'local' or 'remote', got '{}'", v);
                std::process::exit(2);
            }
            config.agent_mode = mode;
        }

        if config.agent_mode == "remote" {
            if let Ok(v) = std::env::var("ZENTRA_API_ENDPOINT") {
                config.api_endpoint = v;
            }
            if config.api_endpoint.is_empty() {
                eprintln!("FATAL: ZENTRA_API_ENDPOINT is required when ZENTRA_AGENT_MODE=remote");
                std::process::exit(2);
            }

            if let Ok(v) = std::env::var("ZENTRA_AGENT_API_KEY") {
                config.agent_api_key = v;
            }
            if config.agent_api_key.is_empty() || !config.agent_api_key.starts_with("isok_") {
                eprintln!("FATAL: ZENTRA_AGENT_API_KEY is required and must start with 'isok_' when ZENTRA_AGENT_MODE=remote");
                std::process::exit(2);
            }
        } else if config.database_url.is_empty() {
            eprintln!("FATAL: DATABASE_URL environment variable is not set.");
            std::process::exit(2);
        }

        // Datacenter name — defaults to hostname
        if let Ok(v) = std::env::var("ZENTRA_DC_NAME") {
            config.dc_name = v;
        }
        if config.dc_name.is_empty() {
            config.dc_name = hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".into());
        }

        // Buffer directory for offline batches in remote mode
        if let Ok(v) = std::env::var("ZENTRA_BUFFER_DIR") {
            config.buffer_dir = v;
        }
        if config.buffer_dir.is_empty() {
            let default_path = "/var/lib/zentra-agent/buffer";
            if std::path::Path::new("/var/lib").exists() {
                config.buffer_dir = default_path.into();
            } else {
                config.buffer_dir = "./buffer".into();
            }
        }
        if let Ok(v) = std::env::var("ZENTRA_COLLECTION_INTERVAL") {
            if let Ok(i) = v.parse() {
                config.collection_interval_secs = i;
            }
        }
        if let Ok(v) = std::env::var("ZENTRA_LOG_LEVEL") {
            config.log_level = v;
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let c = AppConfig::default();
        assert_eq!(c.api_port, 5006);
        assert_eq!(c.collection_interval_secs, 60);
    }
}
