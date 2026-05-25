use crate::DrilldownLevel;

/// Static hierarchy definition
/// Global → Network → Host → Server → DB → Query → Process → Metric
pub fn default_hierarchy() -> Vec<(DrilldownLevel, Vec<DrilldownLevel>)> {
    vec![
        (DrilldownLevel::Global, vec![
            DrilldownLevel::Network,
            DrilldownLevel::Host,
            DrilldownLevel::Server,
            DrilldownLevel::Database,
            DrilldownLevel::Storage,
        ]),
        (DrilldownLevel::Network, vec![DrilldownLevel::Host]),
        (DrilldownLevel::Host, vec![
            DrilldownLevel::Server,
            DrilldownLevel::Database,
            DrilldownLevel::Storage,
            DrilldownLevel::Service,
        ]),
        (DrilldownLevel::Server, vec![
            DrilldownLevel::Process,
            DrilldownLevel::Metric,
        ]),
        (DrilldownLevel::Database, vec![
            DrilldownLevel::Query,
            DrilldownLevel::Metric,
        ]),
        (DrilldownLevel::Storage, vec![DrilldownLevel::Metric]),
        (DrilldownLevel::Service, vec![
            DrilldownLevel::Process,
            DrilldownLevel::Metric,
        ]),
        (DrilldownLevel::Process, vec![DrilldownLevel::Metric]),
        (DrilldownLevel::Query, vec![DrilldownLevel::Metric]),
    ]
}
