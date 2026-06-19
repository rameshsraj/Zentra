//! Remote target fetching — periodically calls GET /api/agents/targets
//! and reconciles running collection loops.

use crate::orchestrator::Orchestrator;
use crate::plugin_manager::create_plugin_for_target;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;
use zentra_core::models::metric::MetricBatch;
use zentra_core::models::target::Target;
use zentra_stream::EventBus;

pub struct RemoteTargetFetcher {
    client: Client,
    api_endpoint: String,
    api_key: String,
    event_bus: EventBus,
    metric_tx: mpsc::Sender<MetricBatch>,
    org_id: Uuid,
    cancel: CancellationToken,
    active_targets: Arc<AtomicUsize>,
}

impl RemoteTargetFetcher {
    pub fn new(
        api_endpoint: String,
        api_key: String,
        event_bus: EventBus,
        metric_tx: mpsc::Sender<MetricBatch>,
        org_id: Uuid,
        cancel: CancellationToken,
        active_targets: Arc<AtomicUsize>,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build target-fetch HTTP client");

        Self {
            client,
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_key,
            event_bus,
            metric_tx,
            org_id,
            cancel,
            active_targets,
        }
    }

    /// Fetch targets from the server, start/stop collection loops as needed.
    pub async fn run(self) {
        info!("Remote target fetcher started (60s interval)");

        // Track running collection loops: target_id -> CancellationToken
        let mut running: HashMap<Uuid, CancellationToken> = HashMap::new();

        // Initial fetch
        match self.fetch_targets().await {
            Ok(targets) => self.reconcile(&mut running, targets),
            Err(e) => error!(error = %e, "Initial target fetch failed"),
        }

        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        ticker.tick().await; // skip immediate

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Remote target fetcher stopping (shutdown)");
                    // Cancel all running collection loops
                    for (_, token) in &running {
                        token.cancel();
                    }
                    break;
                }
                _ = ticker.tick() => {}
            }

            match self.fetch_targets().await {
                Ok(targets) => self.reconcile(&mut running, targets),
                Err(e) => warn!(error = %e, "Target fetch failed, keeping existing loops"),
            }
        }
    }

    async fn fetch_targets(&self) -> Result<Vec<Target>, String> {
        let url = format!("{}/api/agents/targets", self.api_endpoint);
        let resp = self
            .client
            .get(&url)
            .header("X-Zentra-Key", &self.api_key)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;

        let status = resp.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            error!("Target fetch rejected ({}): API key revoked. Stopping.", status);
            std::process::exit(1);
        }
        if !status.is_success() {
            return Err(format!("Server returned {}", status));
        }

        let targets: Vec<Target> = resp
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        Ok(targets)
    }

    fn reconcile(&self, running: &mut HashMap<Uuid, CancellationToken>, targets: Vec<Target>) {
        let new_ids: std::collections::HashSet<Uuid> =
            targets.iter().map(|t| t.target_id).collect();

        // Stop loops for targets no longer in the list
        let to_remove: Vec<Uuid> = running
            .keys()
            .filter(|id| !new_ids.contains(id))
            .copied()
            .collect();
        for id in to_remove {
            if let Some(token) = running.remove(&id) {
                info!(target_id = %id, "Cancelling removed target");
                token.cancel();
            }
        }

        // Start loops for new targets
        for target in targets {
            if running.contains_key(&target.target_id) {
                continue;
            }
            if let Some(plugin) = create_plugin_for_target(&target) {
                let target_token = CancellationToken::new();
                running.insert(target.target_id, target_token.clone());

                // In remote mode, Orchestrator gets a no-op pool (never used for profiling)
                // We create a child token linked to the main cancel
                let linked = self.cancel.child_token();
                let combined_cancel = target_token;

                let orch = Orchestrator::new_remote(
                    self.event_bus.clone(),
                    self.metric_tx.clone(),
                    self.org_id,
                    combined_cancel,
                );

                let target_name = target.name.clone();
                tokio::spawn(async move {
                    orch.run_collection_loop(plugin, target).await;
                });

                info!(target = %target_name, "Started collection loop for new target");
            }
        }

        let count = running.len();
        self.active_targets.store(count, Ordering::Relaxed);
    }
}
