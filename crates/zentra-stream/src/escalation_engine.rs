//! Alert escalation engine — auto-escalates unacknowledged alerts through notification tiers.
//!
//! Each alert rule can have an ordered escalation chain. If an alert is not
//! acknowledged within the delay of the current tier, the next tier fires.

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use parking_lot::RwLock;
use tokio::time;
use tracing::{info, debug, warn};
use uuid::Uuid;

/// One level in an escalation chain.
#[derive(Debug, Clone)]
pub struct EscalationLevel {
    pub level_order: u32,
    /// Minutes to wait before escalating from this level.
    pub delay_minutes: i64,
    /// Notification channel IDs to notify at this level.
    pub notification_channel_ids: Vec<Uuid>,
    /// If true, look up current on-call user at evaluation time (future: integrate oncall schedule).
    pub use_oncall: bool,
}

/// Escalation chain definition for a single alert rule.
#[derive(Debug, Clone)]
pub struct EscalationChain {
    pub alert_rule_id: Uuid,
    pub levels: Vec<EscalationLevel>,
}

/// Tracks the escalation state for a single active (firing, unacknowledged) alert.
#[derive(Debug, Clone)]
struct EscalationState {
    pub alert_rule_id: Uuid,
    pub alert_id: Uuid,
    pub fired_at: DateTime<Utc>,
    pub current_level: u32,
    pub last_notified_at: DateTime<Utc>,
    pub acknowledged: bool,
}

/// Notification callback type — called by the engine when a tier fires.
pub type NotifyFn = Arc<dyn Fn(Uuid, Vec<Uuid>) + Send + Sync>;

pub struct EscalationEngine {
    chains: RwLock<HashMap<Uuid, EscalationChain>>,
    states: RwLock<HashMap<Uuid, EscalationState>>, // keyed by alert_id
    notify_fn: Option<NotifyFn>,
}

impl EscalationEngine {
    pub fn new(notify_fn: Option<NotifyFn>) -> Self {
        Self {
            chains: RwLock::new(HashMap::new()),
            states: RwLock::new(HashMap::new()),
            notify_fn,
        }
    }

    /// Load or replace an escalation chain for a rule.
    pub fn upsert_chain(&self, chain: EscalationChain) {
        let mut chains = self.chains.write();
        let rule_id = chain.alert_rule_id;
        chains.insert(rule_id, chain);
        debug!(rule_id = %rule_id, "Escalation chain upserted");
    }

    pub fn remove_chain(&self, rule_id: Uuid) {
        self.chains.write().remove(&rule_id);
    }

    /// Called when a new alert fires. Starts tracking for escalation.
    pub fn on_alert_fired(&self, alert_id: Uuid, alert_rule_id: Uuid) {
        let has_chain = self.chains.read().contains_key(&alert_rule_id);
        if !has_chain { return; }

        let mut states = self.states.write();
        states.entry(alert_id).or_insert_with(|| {
            info!(alert_id = %alert_id, rule_id = %alert_rule_id, "Escalation tracking started");
            let now = Utc::now();
            EscalationState {
                alert_rule_id,
                alert_id,
                fired_at: now,
                current_level: 0,
                last_notified_at: now,
                acknowledged: false,
            }
        });
    }

    /// Called when an alert is acknowledged — stops escalation.
    pub fn on_alert_acknowledged(&self, alert_id: Uuid) {
        if let Some(state) = self.states.write().get_mut(&alert_id) {
            state.acknowledged = true;
            info!(alert_id = %alert_id, "Escalation stopped: alert acknowledged");
        }
    }

    /// Called when an alert resolves — cleans up tracking.
    pub fn on_alert_resolved(&self, alert_id: Uuid) {
        self.states.write().remove(&alert_id);
    }

    /// Tick — evaluate all tracked alerts and fire escalation tiers as needed.
    /// Call this on a regular interval (e.g., every 60 seconds).
    pub fn tick(&self) {
        let now = Utc::now();
        let chains = self.chains.read();
        let mut states = self.states.write();

        let alert_ids: Vec<Uuid> = states.keys().cloned().collect();

        for alert_id in alert_ids {
            let state = match states.get_mut(&alert_id) {
                Some(s) => s,
                None => continue,
            };

            if state.acknowledged { continue; }

            let chain = match chains.get(&state.alert_rule_id) {
                Some(c) => c,
                None => continue,
            };

            let current_idx = state.current_level as usize;
            if current_idx >= chain.levels.len() { continue; }

            let current_level = &chain.levels[current_idx];
            let elapsed = now - state.last_notified_at;

            if elapsed >= Duration::minutes(current_level.delay_minutes) {
                // Fire this escalation level
                let channels = current_level.notification_channel_ids.clone();
                info!(
                    alert_id = %alert_id,
                    level = current_level.level_order,
                    channels = channels.len(),
                    "Escalation tier firing"
                );

                if let Some(ref notify) = self.notify_fn {
                    notify(alert_id, channels);
                }

                state.current_level += 1;
                state.last_notified_at = now;
            }
        }
    }

    /// Background loop — runs tick() every 60 seconds.
    pub async fn run_loop(engine: Arc<EscalationEngine>) {
        let mut interval = time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            engine.tick();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalation_flow() {
        let engine = EscalationEngine::new(None);
        let rule_id = Uuid::new_v4();
        let alert_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();

        engine.upsert_chain(EscalationChain {
            alert_rule_id: rule_id,
            levels: vec![
                EscalationLevel { level_order: 0, delay_minutes: 5, notification_channel_ids: vec![channel_id], use_oncall: false },
                EscalationLevel { level_order: 1, delay_minutes: 15, notification_channel_ids: vec![channel_id], use_oncall: true },
            ],
        });

        engine.on_alert_fired(alert_id, rule_id);

        // State should now exist
        let states = engine.states.read();
        assert!(states.contains_key(&alert_id));
        assert_eq!(states[&alert_id].current_level, 0);
        drop(states);

        engine.on_alert_acknowledged(alert_id);
        let states = engine.states.read();
        assert!(states[&alert_id].acknowledged);
        drop(states);

        engine.on_alert_resolved(alert_id);
        assert!(!engine.states.read().contains_key(&alert_id));
    }
}
