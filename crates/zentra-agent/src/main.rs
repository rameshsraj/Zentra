use zentra_agent::leader_election;
use zentra_agent::orchestrator::Orchestrator;
use zentra_agent::plugin_manager::create_plugin_for_target;
use zentra_agent::remote_delivery::RemoteDeliveryService;
use zentra_agent::heartbeat::{HeartbeatLoop, new_agent_id_cell};
use zentra_agent::remote_targets::RemoteTargetFetcher;
use zentra_agent::network_scan::NetworkScanner;
use zentra_agent::log_capture::{CaptureLayer, LogBuffer};
use zentra_agent::component_discovery::ComponentDiscovery;
use zentra_core::models::metric::MetricBatch;
use zentra_core::models::log_entry::LogBatch;
use zentra_db::pool::create_pool;
use zentra_ingestion::IngestionService;
use zentra_stream::EventBus;
use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Shared log buffer — populated by CaptureLayer, drained by HeartbeatLoop.
    let log_buffer = LogBuffer::new();

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(true);
    let otel_layer = zentra_core::telemetry::init_telemetry("zentra-agent");
    let capture_layer = CaptureLayer::new(log_buffer.clone());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .with(capture_layer)
        .init();

    info!("Zentra Agent starting...");

    let config = zentra_core::AppConfig::from_env();

    if config.agent_mode == "remote" {
        run_remote_mode(config, log_buffer).await;
    } else {
        run_local_mode(config).await;
    }
}

// ============================================================================
// LOCAL MODE — unchanged from original behaviour
// ============================================================================

async fn run_local_mode(config: zentra_core::AppConfig) {
    info!("Agent running in LOCAL mode (direct DB connection)");

    let pool = create_pool(&config.database_url)
        .await
        .expect("Failed to create database pool");

    let event_bus = EventBus::new();
    let cancel = CancellationToken::new();

    let (metric_tx, metric_rx) = mpsc::channel::<MetricBatch>(1024);
    let (_log_tx, log_rx) = mpsc::channel::<LogBatch>(1024);

    let ingestion = IngestionService::new(pool.clone(), event_bus.clone());
    let ingestion_metrics = ingestion.clone();
    let ingestion_logs = ingestion.clone();

    tokio::spawn(async move {
        ingestion_metrics.run_metric_ingestion(metric_rx).await;
    }.instrument(tracing::info_span!("ingestion.metrics")));
    tokio::spawn(async move {
        ingestion_logs.run_log_ingestion(log_rx).await;
    }.instrument(tracing::info_span!("ingestion.logs")));

    let active_ids: Arc<Mutex<HashSet<Uuid>>> = Arc::new(Mutex::new(HashSet::new()));

    let initial_targets = zentra_db::repositories::targets::list_targets(
        &pool, config.org_id, None, true,
    )
    .await
    .expect("Failed to load targets");

    info!(count = initial_targets.len(), "Loaded active targets");

    {
        let mut ids = active_ids.lock().await;
        for target in initial_targets {
            if ids.contains(&target.target_id) { continue; }
            if let Some(plugin) = create_plugin_for_target(&target) {
                let Some(lock) = leader_election::try_acquire(&config.database_url, target.target_id).await else {
                    continue;
                };
                ids.insert(target.target_id);
                let orch = Orchestrator::new(
                    event_bus.clone(),
                    metric_tx.clone(),
                    pool.clone(),
                    config.org_id,
                    cancel.clone(),
                );
                tokio::spawn({
                    let span = tracing::info_span!("agent.collection_loop", target.id = %target.target_id, target.name = %target.name);
                    async move {
                        let _lock = lock;
                        orch.run_collection_loop(plugin, target).await;
                    }.instrument(span)
                });
            }
        }
    }

    // Periodic target reload
    let reload_pool = pool.clone();
    let reload_event_bus = event_bus.clone();
    let reload_metric_tx = metric_tx.clone();
    let reload_cancel = cancel.clone();
    let reload_active = active_ids.clone();
    let reload_org_id = config.org_id;
    let reload_db_url = config.database_url.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = reload_cancel.cancelled() => break,
                _ = ticker.tick() => {}
            }
            let targets = match zentra_db::repositories::targets::list_targets(
                &reload_pool, reload_org_id, None, true,
            ).await {
                Ok(t) => t,
                Err(e) => { tracing::warn!(error = %e, "Target reload failed"); continue; }
            };
            let mut ids = reload_active.lock().await;
            let mut new_count = 0;
            for target in targets {
                if ids.contains(&target.target_id) { continue; }
                if let Some(plugin) = create_plugin_for_target(&target) {
                    let Some(lock) = leader_election::try_acquire(&reload_db_url, target.target_id).await else {
                        continue;
                    };
                    ids.insert(target.target_id);
                    new_count += 1;
                    let orch = Orchestrator::new(
                        reload_event_bus.clone(),
                        reload_metric_tx.clone(),
                        reload_pool.clone(),
                        reload_org_id,
                        reload_cancel.clone(),
                    );
                    tokio::spawn({
                        let span = tracing::info_span!("agent.collection_loop", target.id = %target.target_id, target.name = %target.name);
                        async move {
                            let _lock = lock;
                            orch.run_collection_loop(plugin, target).await;
                        }.instrument(span)
                    });
                }
            }
            if new_count > 0 {
                info!(new_targets = new_count, total = ids.len(), "Reloaded targets — started new collection loops");
            }
        }
    });

    info!("Target reload loop running (30s interval)");

    // Hubble collector
    let hubble_relay = std::env::var("HUBBLE_RELAY_ADDRESS").unwrap_or_else(|_| "hubble-relay:4245".into());
    let hubble_collector = zentra_agent::collectors::hubble::HubbleCollector::new(
        pool.clone(),
        hubble_relay,
        config.org_id,
    );
    tokio::spawn(async move {
        hubble_collector.run().await;
    }.instrument(tracing::info_span!("agent.hubble_collector")));

    tokio::signal::ctrl_c().await.ok();
    info!("Shutdown signal received, stopping agent...");
    cancel.cancel();
    zentra_core::telemetry::shutdown_telemetry();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    info!("Zentra Agent stopped");
}

// ============================================================================
// REMOTE MODE — no DB, communicates with Zentra server over HTTPS
// ============================================================================

async fn run_remote_mode(config: zentra_core::AppConfig, log_buffer: LogBuffer) {
    info!(
        api_endpoint = %config.api_endpoint,
        dc_name = %config.dc_name,
        "Agent running in REMOTE mode (no database connection)"
    );

    let cancel = CancellationToken::new();
    let event_bus = EventBus::new();
    let active_targets = Arc::new(AtomicUsize::new(0));

    // Shared cell — populated by HeartbeatLoop on first successful response.
    // Seed it from env var if already known (e.g. systemd service after first run).
    let agent_id_cell = new_agent_id_cell();
    if let Some(id) = config.agent_id {
        let _ = agent_id_cell.set(id);
        info!(agent_id = %id, "Agent ID pre-loaded from ZENTRA_AGENT_ID env var");
    }

    let (metric_tx, metric_rx) = mpsc::channel::<MetricBatch>(1024);
    let (_log_tx, log_rx) = mpsc::channel::<LogBatch>(1024);

    // Remote delivery service — replaces IngestionService
    let delivery_metrics = RemoteDeliveryService::new(
        config.api_endpoint.clone(),
        config.agent_api_key.clone(),
        config.dc_name.clone(),
        config.buffer_dir.clone(),
    );
    let delivery_logs = RemoteDeliveryService::new(
        config.api_endpoint.clone(),
        config.agent_api_key.clone(),
        config.dc_name.clone(),
        config.buffer_dir.clone(),
    );

    tokio::spawn(async move {
        delivery_metrics.run_metric_delivery(metric_rx).await;
    }.instrument(tracing::info_span!("remote.metric_delivery")));
    tokio::spawn(async move {
        delivery_logs.run_log_delivery(log_rx).await;
    }.instrument(tracing::info_span!("remote.log_delivery")));

    // Heartbeat loop (30s) — fires immediately on startup to get agent_id fast
    let heartbeat = HeartbeatLoop::new(
        config.api_endpoint.clone(),
        config.agent_api_key.clone(),
        config.dc_name.clone(),
        active_targets.clone(),
        cancel.clone(),
        log_buffer.clone(),
        agent_id_cell.clone(),
    );
    tokio::spawn(async move {
        heartbeat.run().await;
    }.instrument(tracing::info_span!("agent.heartbeat")));

    // Remote target fetcher (60s reconciliation)
    let fetcher = RemoteTargetFetcher::new(
        config.api_endpoint.clone(),
        config.agent_api_key.clone(),
        event_bus.clone(),
        metric_tx.clone(),
        config.org_id,
        cancel.clone(),
        active_targets.clone(),
    );
    tokio::spawn(async move {
        fetcher.run().await;
    }.instrument(tracing::info_span!("agent.target_fetcher")));

    // Network scanner — waits for agent_id then discovers hosts in local subnets
    {
        let scanner_cell = agent_id_cell.clone();
        let scanner_endpoint = config.api_endpoint.clone();
        let scanner_key = config.agent_api_key.clone();
        let scanner_cancel = cancel.clone();
        tokio::spawn(async move {
            // Wait up to 60s for agent_id from the first heartbeat response
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
            loop {
                if let Some(id) = scanner_cell.get() {
                    let scanner = NetworkScanner::new(
                        scanner_endpoint,
                        scanner_key,
                        *id,
                        scanner_cancel,
                    );
                    info!(agent_id = %id, "Network scanner spawned");
                    scanner.run().await;
                    return;
                }
                if tokio::time::Instant::now() >= deadline {
                    tracing::warn!("Timed out waiting for agent_id — network scanner disabled");
                    return;
                }
                tokio::select! {
                    _ = scanner_cancel.cancelled() => { return; }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
                }
            }
        }.instrument(tracing::info_span!("agent.network_scanner")));
    }

    // Component discovery — waits for agent_id then detects and reports local services
    let comp_disc = ComponentDiscovery::new(
        config.api_endpoint.clone(),
        config.agent_api_key.clone(),
        config.dc_name.clone(),
        cancel.clone(),
        agent_id_cell.clone(),
    );
    tokio::spawn(async move {
        comp_disc.run().await;
    }.instrument(tracing::info_span!("agent.component_discovery")));

    // Wait for shutdown
    tokio::signal::ctrl_c().await.ok();
    info!("Shutdown signal received, stopping remote agent...");
    cancel.cancel();

    // Wait up to 5 seconds for in-flight plugin calls
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Flush offline buffer
    let mut flusher = RemoteDeliveryService::new(
        config.api_endpoint.clone(),
        config.agent_api_key.clone(),
        config.dc_name.clone(),
        config.buffer_dir.clone(),
    );
    flusher.flush_all().await;

    zentra_core::telemetry::shutdown_telemetry();
    info!("Zentra Agent (remote) stopped");
}
