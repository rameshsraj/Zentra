use axum::{Router, routing::{get, post, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Network Deep Analysis (Sniffnet-inspired)
        .route("/api/network/deep/snapshot", get(handlers::network::traffic_snapshot))
        .route("/api/network/deep/services", get(handlers::network::service_breakdown))
        .route("/api/network/deep/icmp", get(handlers::network::icmp_breakdown))
        .route("/api/network/deep/threat-status", get(handlers::network::threat_list_status))
        .route("/api/network/deep/favorites", post(handlers::network::add_favorite))
        .route("/api/network/deep/favorites/:ip", delete(handlers::network::remove_favorite))

        // Network Capture & Monitoring
        .route("/api/network/capture", get(handlers::network::get_capture_status))
        .route("/api/network/capture", post(handlers::network::control_capture))
        .route("/api/network/connections", get(handlers::network::list_connections))
        .route("/api/network/hosts", get(handlers::network::list_hosts))
        .route("/api/network/protocols", get(handlers::network::list_protocols))
        .route("/api/network/bandwidth", get(handlers::network::list_bandwidth))
        .route("/api/network/adapters", get(handlers::network::list_adapters))
        .route("/api/network/programs", get(handlers::network::list_programs))
        .route("/api/network/service-db/count", get(handlers::network::service_db_count))
        .route("/api/network/stats", get(handlers::network::get_network_stats))
        .route("/api/network/traffic-chart", get(handlers::network::get_traffic_chart))
        .route("/api/network/protocol-breakdown", get(handlers::network::get_protocol_breakdown))
        .route("/api/network/service-breakdown", get(handlers::network::get_service_breakdown))
        .route("/api/network/country-breakdown", get(handlers::network::get_country_breakdown))
        .route("/api/network/connections/paginated", get(handlers::network::list_connections_paginated))

        // S5: PCAP Import & Export
        .route("/api/network/capture/export", post(handlers::network::start_pcap_export))
        .route("/api/network/capture/export/stop", post(handlers::network::stop_pcap_export))
        .route("/api/network/capture/export/download", get(handlers::network::download_pcap))
        .route("/api/network/capture/import", post(handlers::network::import_pcap))

        // S7: Favorites System
        .route("/api/network/favorites/list", get(handlers::network::list_favorites))
        .route("/api/network/favorites/add", post(handlers::network::add_favorite_item))
        .route("/api/network/favorites/remove/:id", delete(handlers::network::delete_favorite_item))

        // S8: IP Blacklist Management
        .route("/api/network/blacklist", get(handlers::network::list_blacklist))
        .route("/api/network/blacklist/add", post(handlers::network::add_blacklist_entry))
        .route("/api/network/blacklist/remove/:id", delete(handlers::network::remove_blacklist_entry))
        .route("/api/network/blacklist/import", post(handlers::network::import_blacklist))

        // S9: Network Notifications
        .route("/api/network/notification-rules", get(handlers::network::list_notification_rules))
        .route("/api/network/notification-rules", post(handlers::network::create_notification_rule))
        .route("/api/network/notification-rules/:id", delete(handlers::network::delete_notification_rule))
        .route("/api/network/notification-log", get(handlers::network::get_notification_log))

        // Network Devices — live view of registered targets
        .route("/api/network/devices", get(handlers::network::list_network_devices))

        // Network Device Discovery & Subnet Scan
        .route("/api/network/scan", post(handlers::network::scan_subnet))
        .route("/api/network/test-device", post(handlers::network::test_device))
        .route("/api/network/discovery/scan", get(handlers::network::discovery_scan))
        .route("/api/network/discovery/test", get(handlers::network::discovery_test))

        // P3: Network Interfaces, Protocol Stats, Socket Stats
        .route("/api/network/interfaces", get(handlers::network::list_interfaces))
        .route("/api/network/protocol-stats", get(handlers::network::protocol_stats))
        .route("/api/network/socket-stats", get(handlers::network::socket_stats))

        // P17: Container Awareness
        .route("/api/network/containers", get(handlers::network::list_containers))
        .route("/api/network/connections/filtered", get(handlers::network::list_connections_filtered))

        // V5 Prompt 20: Topology Enhancement
        .route("/api/network/topology", get(handlers::network::get_topology))
        .route("/api/network/topology/auto-discover", get(handlers::network::topology_auto_discover))
        .route("/api/network/topology/:node_id/neighbors", get(handlers::network::topology_neighbors))
        .route("/api/network/topology/paths/:src/:dest", get(handlers::network::topology_shortest_path))

        // Frontend-compatible aliases
        .route("/api/network/traffic", get(handlers::network::get_network_stats))
        .route("/api/network/traffic/chart", get(handlers::network::get_traffic_chart))
        .route("/api/network/traffic/protocols", get(handlers::network::get_protocol_breakdown))
        .route("/api/network/deep/countries", get(handlers::network::get_country_breakdown))
        .route("/api/network/interfaces/list", get(handlers::network::list_interfaces))
        .route("/api/network/capture/control", post(handlers::network::control_capture))
}
