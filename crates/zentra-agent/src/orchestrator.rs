use crate::plugin_manager::PluginInstance;
use crate::collectors::ebpf_profiler::EbpfProfiler;
use zentra_core::models::metric::MetricBatch;
use zentra_core::models::target::{Target, TargetType};
use zentra_stream::EventBus;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, error, warn, debug};
use sqlx::PgPool;
use uuid::Uuid;

const MAX_CONSECUTIVE_FAILURES: u32 = 5;
const BACKOFF_BASE_MS: u64 = 1000;
const MAX_BACKOFF_MS: u64 = 60_000;

/// Orchestrates plugin execution — runs collection loops for all active targets
pub struct Orchestrator {
    event_bus: EventBus,
    metric_tx: mpsc::Sender<MetricBatch>,
    profiler: Option<EbpfProfiler>,
    cancel: CancellationToken,
}

impl Orchestrator {
    pub fn new(
        event_bus: EventBus,
        metric_tx: mpsc::Sender<MetricBatch>,
        pool: sqlx::PgPool,
        org_id: uuid::Uuid,
        cancel: CancellationToken,
    ) -> Self {
        let profiler = Some(EbpfProfiler::new(pool, org_id));
        Self { event_bus, metric_tx, profiler, cancel }
    }

    /// Create an Orchestrator for remote mode — no DB pool, no eBPF profiling.
    pub fn new_remote(
        event_bus: EventBus,
        metric_tx: mpsc::Sender<MetricBatch>,
        _org_id: uuid::Uuid,
        cancel: CancellationToken,
    ) -> Self {
        Self { event_bus, metric_tx, profiler: None, cancel }
    }

    /// Start a collection loop for a single target + plugin pair.
    /// Respects the cancellation token for graceful shutdown.
    /// Item 8: Top-level span for trace correlation across agent collection.
    #[tracing::instrument(skip(self, plugin, target), fields(
        target.name = %target.name,
        target.id = %target.target_id,
        otel.kind = "server",
    ))]
    pub async fn run_collection_loop(
        &self,
        mut plugin: PluginInstance,
        target: Target,
    ) {
        let plugin_meta = plugin.metadata();
        let plugin_id = plugin_meta.id.to_string();
        let target_name = target.name.clone(); let target_type = target.target_type.clone();

        info!(plugin = %plugin_id, target = %target_name, "Starting collection loop");

        // Initialize plugin with retry
        let mut init_attempts = 0u32;
        loop {
            match plugin.initialize(&target).await {
                Ok(()) => break,
                Err(e) => {
                    init_attempts += 1;
                    if init_attempts >= MAX_CONSECUTIVE_FAILURES {
                        error!(
                            plugin = %plugin_id,
                            target = %target_name,
                            error = %e,
                            attempts = init_attempts,
                            "Plugin initialization failed permanently"
                        );
                        self.event_bus.publish(zentra_core::Event::PluginStopped {
                            plugin_id: plugin_id.clone(),
                            reason: format!("Init failed after {} attempts: {}", init_attempts, e),
                            timestamp: chrono::Utc::now(),
                        });
                        return;
                    }

                    let backoff = std::cmp::min(
                        BACKOFF_BASE_MS * 2u64.pow(init_attempts - 1),
                        MAX_BACKOFF_MS,
                    );
                    warn!(
                        plugin = %plugin_id,
                        error = %e,
                        attempt = init_attempts,
                        backoff_ms = backoff,
                        "Plugin init failed, retrying"
                    );

                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(backoff)) => {},
                        _ = self.cancel.cancelled() => {
                            info!(plugin = %plugin_id, "Shutdown during init retry");
                            return;
                        }
                    }
                }
            }
        }

        self.event_bus.publish(zentra_core::Event::PluginStarted {
            plugin_id: plugin_id.clone(),
            timestamp: chrono::Utc::now(),
        });

        let interval = plugin.polling_interval();
        let mut consecutive_failures: u32 = 0;
        let mut last_profile_at = tokio::time::Instant::now() - tokio::time::Duration::from_secs(3600);

        loop {
            // Check for shutdown
            if self.cancel.is_cancelled() {
                info!(plugin = %plugin_id, target = %target_name, "Collection loop stopping (shutdown)");
                break;
            }

            // Periodic profiling for server targets (every 1 hour) — local mode only
            if target_type == TargetType::Server
                && self.profiler.is_some()
                && last_profile_at.elapsed() > tokio::time::Duration::from_secs(3600)
            {
                let target_clone = target.clone();
                let profiler = self.profiler.as_ref().unwrap().clone_with_pool();
                tokio::spawn(async move {
                    if let Err(e) = profiler.profile_target(&target_clone, 30, None).await {
                        error!(target = %target_clone.name, error = %e, "Background profiling failed");
                    }
                });
                last_profile_at = tokio::time::Instant::now();
            }

            // Collect metrics
            match plugin.collect_metrics(&target).await {
                Ok(metrics) if !metrics.is_empty() => {
                    consecutive_failures = 0;
                    let count = metrics.len();
                    let batch = MetricBatch::new(plugin_meta.id, metrics);
                    if let Err(e) = self.metric_tx.send(batch).await {
                        error!(error = %e, "Failed to send metric batch to pipeline");
                    }
                    debug!(plugin = %plugin_id, count, "Metrics collected");
                }
                Ok(_) => {
                    consecutive_failures = 0;
                    debug!(plugin = %plugin_id, "No metrics collected");
                }
                Err(e) => {
                    consecutive_failures += 1;
                    error!(
                        plugin = %plugin_id,
                        target = %target_name,
                        error = %e,
                        consecutive = consecutive_failures,
                        "Metric collection failed"
                    );

                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        error!(
                            plugin = %plugin_id,
                            target = %target_name,
                            "Too many consecutive failures, stopping plugin"
                        );
                        self.event_bus.publish(zentra_core::Event::PluginStopped {
                            plugin_id: plugin_id.clone(),
                            reason: format!("{} consecutive collection failures", consecutive_failures),
                            timestamp: chrono::Utc::now(),
                        });
                        break;
                    }
                }
            }

            // Health check
            match plugin.health_check(&target).await {
                Ok(healthy) => {
                    self.event_bus.publish(zentra_core::Event::HealthCheck {
                        component: format!("{}:{}", plugin_id, target_name),
                        healthy,
                        timestamp: chrono::Utc::now(),
                    });
                }
                Err(e) => {
                    error!(plugin = %plugin_id, error = %e, "Health check failed");
                }
            }

            // Sleep with cancellation support
            tokio::select! {
                _ = tokio::time::sleep(interval) => {},
                _ = self.cancel.cancelled() => {
                    info!(plugin = %plugin_id, "Shutdown signal received");
                    break;
                }
            }
        }

        self.event_bus.publish(zentra_core::Event::PluginStopped {
            plugin_id: plugin_id.clone(),
            reason: "graceful_shutdown".into(),
            timestamp: chrono::Utc::now(),
        });

        info!(plugin = %plugin_id, target = %target_name, "Collection loop stopped");
    }
}
