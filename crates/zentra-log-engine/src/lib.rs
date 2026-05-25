pub mod parser;
pub mod collector;
pub mod correlation;

use serde::{Deserialize, Serialize};

/// Log source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSourceConfig {
    pub source_type: LogSourceType,
    pub name: String,
    pub target_id: uuid::Uuid,
    pub path: Option<String>,         // file path for file-based logs
    pub endpoint: Option<String>,      // API endpoint for remote logs
    pub format: LogFormat,
    pub polling_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogSourceType {
    File,
    Journald,
    Syslog,
    PostgresLog,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Plain,
    Json,
    Syslog,
    PostgresCsv,
    Custom(String), // regex pattern
}
