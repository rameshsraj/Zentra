use zentra_agent::leader_election;
use zentra_agent::orchestrator::Orchestrator;
use zentra_agent::plugin_manager::create_plugin_for_target;
use zentra_core::models::metric::MetricBatch;
use zentra_core::models::log_entry::LogBatch;
use zentra_db::pool::create_pool;
use zentra_ingestion::IngestionService;
use zentra_stream::EventBus;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .with_target(true)
        .init();

    info!("Zentra Agent starting...");

    let config = zentra_core::AppConfig::from_env();

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
    });
    tokio::spawn(async move {
        ingestion_logs.run_log_ingestion(log_rx).await;
    });

    // Tracks target IDs that already have a collection loop running.
    let active_ids: Arc<Mutex<HashSet<Uuid>>> = Arc::new(Mutex::new(HashSet::new()));

    // Initial load of targets
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
                // Acquire advisory lock — skip if another agent instance already owns this target.
                let Some(lock) = leader_election::try_acquire(&config.database_url, target.target_id).await else {
                    continue;
                };
                ids.insert(target.target_id);
                let orch = Orchestrator::new(
                    event_bus.clone(),
                    metric_tx.clone(),
                    cancel.clone(),
                );
                tokio::spawn(async move {
                    let _lock = lock; // hold advisory lock for duration of collection loop
                    orch.run_collection_loop(plugin, target).await;
                });
            }
        }
    }

    // Periodic target reload — picks up newly added targets without restart.
    let reload_pool = pool.clone();
    let reload_event_bus = event_bus.clone();
    let reload_metric_tx = metric_tx.clone();
    let reload_cancel = cancel.clone();
    let reload_active = active_ids.clone();
    let reload_org_id = config.org_id;
    let reload_db_url = config.database_url.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        ticker.tick().await; // skip immediate
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
                    // Acquire advisory lock — skip if another agent already owns this target.
                    let Some(lock) = leader_election::try_acquire(&reload_db_url, target.target_id).await else {
                        continue;
                    };
                    ids.insert(target.target_id);
                    new_count += 1;
                    let orch = Orchestrator::new(
                        reload_event_bus.clone(),
                        reload_metric_tx.clone(),
                        reload_cancel.clone(),
                    );
                    tokio::spawn(async move {
                        let _lock = lock; // hold advisory lock for duration of collection loop
                        orch.run_collection_loop(plugin, target).await;
                    });
                }
            }
            if new_count > 0 {
                info!(new_targets = new_count, total = ids.len(), "Reloaded targets — started new collection loops");
            }
        }
    });

    info!("Target reload loop running (30s interval)");

    tokio::signal::ctrl_c().await.ok();
    info!("Shutdown signal received, stopping agent...");
    cancel.cancel();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    info!("Zentra Agent stopped");
}
