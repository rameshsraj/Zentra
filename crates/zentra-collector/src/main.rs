//! zentra-collector — On-premises network/SNMP/storage collector.
//!
//! Runs inside a customer data center and monitors network devices, storage
//! appliances, and other agentless targets that cannot run zentra-agent.
//!
//! Communication model (identical to zentra-agent remote mode):
//!   - Registers with the Zentra server via POST /api/agents/heartbeat
//!   - Fetches assigned targets from GET /api/agents/targets
//!   - Ships metrics/logs outbound via HTTPS to the Zentra server (OTLP)
//!   - Zero inbound ports required — only outbound HTTPS 443 / 5006
//!
//! Environment variables:
//!   ZENTRA_API_ENDPOINT   Zentra server URL        (required)
//!   ZENTRA_COLLECTOR_KEY  API key (isok_...)        (required)
//!   ZENTRA_DC_NAME        Datacenter label          (default: "dc-collector")
//!   ZENTRA_COLLECTOR_ID   Persist identity UUID     (optional, set after first run)
//!   ZENTRA_ORG_ID         Organisation UUID         (optional)
//!   RUST_LOG              Log level                 (default: info)

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

use zentra_agent::heartbeat::{HeartbeatLoop, new_agent_id_cell};
use zentra_agent::log_capture::{CaptureLayer, LogBuffer};
use zentra_agent::remote_delivery::RemoteDeliveryService;
use zentra_agent::remote_targets::RemoteTargetFetcher;
use zentra_core::models::log_entry::LogBatch;
use zentra_core::models::metric::MetricBatch;
use zentra_stream::EventBus;

const BUFFER_DIR: &str = "/var/lib/zentra-collector/buffer";

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let log_buffer = LogBuffer::new();
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);
    let otel_layer = zentra_core::telemetry::init_telemetry("zentra-collector");
    let capture_layer = CaptureLayer::new(log_buffer.clone());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .with(capture_layer)
        .init();

    let api_endpoint = std::env::var("ZENTRA_API_ENDPOINT")
        .expect("ZENTRA_API_ENDPOINT is required (e.g. https://zentra.example.com:5006)");
    let api_key = std::env::var("ZENTRA_COLLECTOR_KEY")
        .expect("ZENTRA_COLLECTOR_KEY is required (isok_... from Fleet -> API Keys)");
    let dc_name = std::env::var("ZENTRA_DC_NAME")
        .unwrap_or_else(|_| "dc-collector".to_string());
    let org_id: Uuid = std::env::var("ZENTRA_ORG_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(Uuid::nil());
    let collector_id: Option<Uuid> = std::env::var("ZENTRA_COLLECTOR_ID")
        .ok()
        .and_then(|s| s.parse().ok());

    info!(api_endpoint = %api_endpoint, dc_name = %dc_name,
        "Zentra Collector starting (network/SNMP/agentless mode)");

    let _ = std::fs::create_dir_all(BUFFER_DIR);

    let cancel = CancellationToken::new();
    let event_bus = EventBus::new();
    let active_targets = Arc::new(AtomicUsize::new(0));

    let agent_id_cell = new_agent_id_cell();
    if let Some(id) = collector_id {
        let _ = agent_id_cell.set(id);
        info!(collector_id = %id, "Collector ID pre-loaded from ZENTRA_COLLECTOR_ID");
    }

    let (metric_tx, metric_rx) = mpsc::channel::<MetricBatch>(2048);
    let (_log_tx, log_rx)      = mpsc::channel::<LogBatch>(1024);

    let delivery_metrics = RemoteDeliveryService::new(
        api_endpoint.clone(), api_key.clone(), dc_name.clone(), BUFFER_DIR.to_string(),
    );
    let delivery_logs = RemoteDeliveryService::new(
        api_endpoint.clone(), api_key.clone(), dc_name.clone(), BUFFER_DIR.to_string(),
    );

    tokio::spawn(async move {
        delivery_metrics.run_metric_delivery(metric_rx).await;
    }.instrument(tracing::info_span!("collector.metric_delivery")));

    tokio::spawn(async move {
        delivery_logs.run_log_delivery(log_rx).await;
    }.instrument(tracing::info_span!("collector.log_delivery")));

    // Heartbeat — registers the collector on the Zentra server.
    // Appears in Fleet UI exactly like a zentra-agent.
    let heartbeat = HeartbeatLoop::new(
        api_endpoint.clone(), api_key.clone(), dc_name.clone(),
        active_targets.clone(), cancel.clone(),
        log_buffer.clone(), agent_id_cell.clone(),
    );
    tokio::spawn(async move {
        heartbeat.run().await;
    }.instrument(tracing::info_span!("collector.heartbeat")));

    // Target fetcher — polls /api/agents/targets for targets the Zentra admin
    // has assigned to this collector (switches, routers, storage, ESXi, etc.).
    let fetcher = RemoteTargetFetcher::new(
        api_endpoint.clone(), api_key.clone(), event_bus.clone(),
        metric_tx.clone(), org_id, cancel.clone(), active_targets.clone(),
    );
    tokio::spawn(async move {
        fetcher.run().await;
    }.instrument(tracing::info_span!("collector.target_fetcher")));

    info!("Zentra Collector running — assign targets via Fleet UI");
    info!("After first start, note the collector_id in logs and set ZENTRA_COLLECTOR_ID for persistence");

    tokio::signal::ctrl_c().await.ok();
    info!("Shutdown signal received, stopping collector...");
    cancel.cancel();
    zentra_core::telemetry::shutdown_telemetry();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    info!("Zentra Collector stopped");
}
