use crate::models::threat::{ThreatEvent, ThreatRule};

/// Trait for threat detection engines
#[allow(async_fn_in_trait)]
pub trait ThreatDetector: Send + Sync {
    /// Name of this detector
    fn name(&self) -> &'static str;

    /// Process a detection cycle and return any newly detected threats
    async fn detect(&mut self) -> crate::Result<Vec<ThreatEvent>>;

    /// Load or reload detection rules
    fn load_rules(&mut self, rules: Vec<ThreatRule>);

    /// Reset internal state (e.g., sliding window counters)
    fn reset(&mut self);
}
