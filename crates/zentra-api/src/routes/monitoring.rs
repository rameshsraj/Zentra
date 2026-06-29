use axum::{Router, routing::{get, post, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Windows Monitoring
        .route("/api/windows/:id/metrics/cpu", get(handlers::windows_monitoring::windows_cpu_metrics))
        .route("/api/windows/:id/metrics/memory", get(handlers::windows_monitoring::windows_memory_metrics))
        .route("/api/windows/:id/metrics/disks", get(handlers::windows_monitoring::windows_disk_metrics))
        .route("/api/windows/:id/metrics/network", get(handlers::windows_monitoring::windows_network_metrics))
        .route("/api/windows/:id/services", get(handlers::windows_monitoring::windows_services))
        .route("/api/windows/:id/hardware", get(handlers::windows_monitoring::windows_hardware))

        // SNMP Trap Management
        .route("/api/snmp/traps", get(handlers::snmp_traps::recent_traps))
        .route("/api/snmp/traps/stats", get(handlers::snmp_traps::trap_stats))
        .route("/api/snmp/traps/:id/acknowledge", axum::routing::put(handlers::snmp_traps::acknowledge_trap))
        .route("/api/snmp/trap-rules", get(handlers::snmp_traps::list_trap_rules))
        .route("/api/snmp/trap-rules", post(handlers::snmp_traps::create_trap_rule))
        .route("/api/snmp/trap-rules/:id", delete(handlers::snmp_traps::delete_trap_rule))

        // MIB Management
        .route("/api/snmp/mibs", get(handlers::mib_management::list_mibs))
        .route("/api/snmp/mibs", post(handlers::mib_management::upload_mib))
        .route("/api/snmp/mibs/:id", delete(handlers::mib_management::delete_mib))
        .route("/api/snmp/oid-resolve", get(handlers::mib_management::resolve_oid))

        // Cloud Monitoring (AWS / Azure / GCP)
        .route("/api/cloud/accounts", get(handlers::cloud_monitoring::list_accounts))
        .route("/api/cloud/accounts", post(handlers::cloud_monitoring::create_account))
        .route("/api/cloud/accounts/:id", delete(handlers::cloud_monitoring::delete_account))
        .route("/api/cloud/accounts/:id/validate", post(handlers::cloud_monitoring::validate_account))
        .route("/api/cloud/accounts/:id/metrics", get(handlers::cloud_monitoring::get_account_metrics))
        .route("/api/cloud/summary", get(handlers::cloud_monitoring::cloud_summary))

        // NetFlow / sFlow / IPFIX analytics
        .route("/api/netflow/top-talkers", get(handlers::netflow::top_talkers))
        .route("/api/netflow/stats", get(handlers::netflow::flow_stats))
        .route("/api/netflow/exporters", get(handlers::netflow::list_exporters))
        .route("/api/netflow/protocol-breakdown", get(handlers::netflow::protocol_breakdown))
        .route("/api/netflow/timeseries", get(handlers::netflow::flow_timeseries))

        // ML Anomaly Detection
        .route("/api/anomalies", get(handlers::anomaly::list_anomalies))
        .route("/api/anomalies/stats", get(handlers::anomaly::anomaly_stats))
        .route("/api/anomalies/top-metrics", get(handlers::anomaly::top_anomalous_metrics))
        .route("/api/anomalies/:id/acknowledge", axum::routing::put(handlers::anomaly::acknowledge_anomaly))
        .route("/api/anomalies/:id/feedback", post(handlers::anomaly::submit_feedback))

        // Automated Remediation Runbooks
        .route("/api/runbooks", get(handlers::runbooks::list_runbooks))
        .route("/api/runbooks", post(handlers::runbooks::create_runbook))
        .route("/api/runbooks/stats", get(handlers::runbooks::runbook_stats))
        .route("/api/runbooks/:id", get(handlers::runbooks::get_runbook))
        .route("/api/runbooks/:id", axum::routing::put(handlers::runbooks::update_runbook))
        .route("/api/runbooks/:id", delete(handlers::runbooks::delete_runbook))
        .route("/api/runbooks/:id/toggle", post(handlers::runbooks::toggle_runbook))
        .route("/api/runbooks/:id/execute", post(handlers::runbooks::execute_runbook))
        .route("/api/runbooks/executions", get(handlers::runbooks::list_executions))
        .route("/api/runbooks/executions/:id", get(handlers::runbooks::get_execution))
        .route("/api/runbooks/executions/:id/approve", post(handlers::runbooks::approve_execution))

        // SW P04: Apdex Scoring
        .route("/api/apdex", get(handlers::apdex::list_apdex_scores))
        .route("/api/apdex/config", axum::routing::put(handlers::apdex::upsert_apdex_config))
        .route("/api/apdex/:service", get(handlers::apdex::get_service_apdex))

        // SW P02: Cilium / eBPF Network Monitoring
        .route("/api/cilium/status", get(handlers::cilium::cilium_status))
        .route("/api/cilium/metrics/summary", get(handlers::cilium::cilium_metrics_summary))
        .route("/api/cilium/register-scrape-targets", post(handlers::cilium::register_cilium_targets))

        // SW P09: Continuous eBPF Profiling
        .route("/api/profiles/trigger", post(handlers::profiling::trigger_profile))
        .route("/api/profiles", get(handlers::profiling::list_profiles))
        .route("/api/profiles/:id", get(handlers::profiling::get_profile))
        .route("/api/profiles/jobs/:job_id/status", get(handlers::profiling::get_job_status))

        // P04 (PROF-UI): Flamegraph Aggregation
        .route("/api/profiles/flamegraph", get(handlers::profiling::flamegraph))

        // SW P07: Trace ↔ DB Cross-Correlation
        .route("/api/drilldown/trace/:trace_id/db-context", get(handlers::trace_db_correlator::trace_db_context))
        .route("/api/drilldown/db-slowness", get(handlers::trace_db_correlator::db_slowness))
        .route("/api/correlation/service-to-db", get(handlers::trace_db_correlator::service_to_db_map))

        // SW P06: Hubble gRPC Flow Collector
        .route("/api/hubble/status", get(handlers::cilium::hubble_status))
        .route("/api/hubble/flows", get(handlers::cilium::hubble_flows))

        // Enterprise Storage Arrays
        .route("/api/storage-arrays", get(handlers::storage_arrays::list_arrays))
        .route("/api/storage-arrays/summary", get(handlers::storage_arrays::storage_summary))
        .route("/api/storage-arrays/replications", get(handlers::storage_arrays::list_replications))
        .route("/api/storage-arrays/:id", get(handlers::storage_arrays::get_array))
        .route("/api/storage-arrays/:id/volumes", get(handlers::storage_arrays::list_volumes))
        .route("/api/storage-arrays/:id/metrics/history", get(handlers::storage_arrays::get_array_metrics_history))
        .route("/api/storage-arrays/:id/metrics/latest", get(handlers::storage_arrays::get_array_metrics_latest))

        // SLO / SLI (G-11)
        .route("/api/slo/services", get(handlers::slo::slo_services))
        .route("/api/slo/targets", get(handlers::slo::list_slo_targets))
        .route("/api/slo/targets", post(handlers::slo::create_slo_target))
        .route("/api/slo/targets/:id", delete(handlers::slo::delete_slo_target))
        .route("/api/slo/budget/:service_name", get(handlers::slo::slo_budget))
}
