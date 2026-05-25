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
        if config.database_url.is_empty() {
            eprintln!("FATAL: DATABASE_URL environment variable is not set.");
            std::process::exit(2);
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
