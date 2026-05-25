pub mod engine;
pub mod hierarchy;
pub mod breadcrumb;

pub use engine::DrilldownEngine;
pub use breadcrumb::Breadcrumb;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// A drill-down level in the hierarchy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DrilldownLevel {
    Global,
    Network,
    Host,
    Server,
    Database,
    Storage,
    Service,
    Process,
    Query,
    Metric,
}

impl DrilldownLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Network => "network",
            Self::Host => "host",
            Self::Server => "server",
            Self::Database => "database",
            Self::Storage => "storage",
            Self::Service => "service",
            Self::Process => "process",
            Self::Query => "query",
            Self::Metric => "metric",
        }
    }

    /// Returns children levels for this drill-down level
    pub fn children(&self) -> Vec<DrilldownLevel> {
        match self {
            Self::Global => vec![Self::Network, Self::Host, Self::Server, Self::Database, Self::Storage],
            Self::Network => vec![Self::Host],
            Self::Host => vec![Self::Server, Self::Database, Self::Storage, Self::Service],
            Self::Server => vec![Self::Process, Self::Metric],
            Self::Database => vec![Self::Query, Self::Metric],
            Self::Storage => vec![Self::Metric],
            Self::Service => vec![Self::Process, Self::Metric],
            Self::Process => vec![Self::Metric],
            Self::Query => vec![Self::Metric],
            Self::Metric => vec![],
        }
    }
}

/// Time-based drill-down levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimeLevel {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
}

impl TimeLevel {
    pub fn bucket_seconds(&self) -> i64 {
        match self {
            Self::Year => 86400 * 365,
            Self::Month => 86400 * 30,
            Self::Day => 86400,
            Self::Hour => 3600,
            Self::Minute => 60,
            Self::Second => 1,
        }
    }

    pub fn child(&self) -> Option<TimeLevel> {
        match self {
            Self::Year => Some(Self::Month),
            Self::Month => Some(Self::Day),
            Self::Day => Some(Self::Hour),
            Self::Hour => Some(Self::Minute),
            Self::Minute => Some(Self::Second),
            Self::Second => None,
        }
    }
}

/// A drill-down context representing the current navigation state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrilldownContext {
    pub level: DrilldownLevel,
    pub entity_id: Option<String>,
    pub entity_label: Option<String>,
    pub time_level: TimeLevel,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub filters: std::collections::BTreeMap<String, String>,
}

/// Result of a drill-down query — items at the current level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrilldownResult {
    pub context: DrilldownContext,
    pub items: Vec<DrilldownItem>,
    pub breadcrumb: Vec<BreadcrumbEntry>,
    pub available_children: Vec<DrilldownLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrilldownItem {
    pub id: String,
    pub label: String,
    pub item_type: String,
    pub status: String,
    pub summary: serde_json::Value,
    pub has_children: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreadcrumbEntry {
    pub level: DrilldownLevel,
    pub entity_id: Option<String>,
    pub label: String,
    pub time_level: TimeLevel,
    pub filters: std::collections::BTreeMap<String, String>,
}
