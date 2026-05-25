use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use zentra_core::models::event::Event;
use parking_lot::RwLock;
use serde::Serialize;

/// Circular buffer storing recent events for API queries and dashboard display.
pub struct EventHistory {
    buffer: RwLock<VecDeque<Event>>,
    capacity: usize,
    total_published: AtomicU64,
    stats: RwLock<EventStats>,
}

/// Counters per event type
#[derive(Debug, Clone, Default, Serialize)]
pub struct EventStats {
    pub metrics_collected: u64,
    pub logs_ingested: u64,
    pub alerts_triggered: u64,
    pub alerts_resolved: u64,
    pub target_status_changed: u64,
    pub plugin_started: u64,
    pub plugin_stopped: u64,
    pub health_checks: u64,
    pub threats_detected: u64,
    pub capture_stats: u64,
    pub anomalies_detected: u64,
    pub trend_alerts_generated: u64,
    pub last_event_at: Option<DateTime<Utc>>,
}

impl EventHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
            total_published: AtomicU64::new(0),
            stats: RwLock::new(EventStats::default()),
        }
    }

    /// Record an event into the history buffer and update stats
    pub fn record(&self, event: &Event) {
        self.total_published.fetch_add(1, Ordering::Relaxed);

        // Update per-type stats
        {
            let mut stats = self.stats.write();
            stats.last_event_at = Some(event.timestamp());
            match event {
                Event::MetricsCollected { .. } => stats.metrics_collected += 1,
                Event::LogsIngested { .. } => stats.logs_ingested += 1,
                Event::AlertTriggered { .. } => stats.alerts_triggered += 1,
                Event::AlertResolved { .. } => stats.alerts_resolved += 1,
                Event::TargetStatusChanged { .. } => stats.target_status_changed += 1,
                Event::PluginStarted { .. } => stats.plugin_started += 1,
                Event::PluginStopped { .. } => stats.plugin_stopped += 1,
                Event::HealthCheck { .. } => stats.health_checks += 1,
                Event::ThreatDetected { .. } => stats.threats_detected += 1,
                Event::CaptureStats { .. } => stats.capture_stats += 1,
                Event::AnomalyDetected { .. } => stats.anomalies_detected += 1,
                Event::TrendAlertGenerated { .. } => stats.trend_alerts_generated += 1,
            }
        }

        // Push to circular buffer
        let mut buf = self.buffer.write();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(event.clone());
    }

    /// Get the last N events
    pub fn recent(&self, n: usize) -> Vec<Event> {
        let buf = self.buffer.read();
        buf.iter().rev().take(n).cloned().collect()
    }

    /// Get events filtered by type
    pub fn filter_by_type(&self, event_type: &str) -> Vec<Event> {
        let buf = self.buffer.read();
        buf.iter()
            .filter(|e| e.event_type() == event_type)
            .cloned()
            .collect()
    }

    /// Get aggregate statistics
    pub fn stats(&self) -> EventStats {
        self.stats.read().clone()
    }

    /// Total events ever published
    pub fn total_published(&self) -> u64 {
        self.total_published.load(Ordering::Relaxed)
    }

    /// Current buffer length
    pub fn buffer_len(&self) -> usize {
        self.buffer.read().len()
    }
}

impl Clone for EventHistory {
    fn clone(&self) -> Self {
        Self {
            buffer: RwLock::new(self.buffer.read().clone()),
            capacity: self.capacity,
            total_published: AtomicU64::new(self.total_published.load(Ordering::Relaxed)),
            stats: RwLock::new(self.stats.read().clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_event_history_basic() {
        let history = EventHistory::new(100);

        let event = Event::HealthCheck {
            component: "test".into(),
            healthy: true,
            timestamp: Utc::now(),
        };
        history.record(&event);
        history.record(&event);

        assert_eq!(history.total_published(), 2);
        assert_eq!(history.buffer_len(), 2);
        assert_eq!(history.stats().health_checks, 2);

        let recent = history.recent(10);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_event_history_overflow() {
        let history = EventHistory::new(3);

        for i in 0..5 {
            history.record(&Event::PluginStarted {
                plugin_id: format!("plugin-{}", i),
                timestamp: Utc::now(),
            });
        }

        assert_eq!(history.total_published(), 5);
        assert_eq!(history.buffer_len(), 3); // Capped at capacity
    }

    #[test]
    fn test_event_history_filter() {
        let history = EventHistory::new(100);

        history.record(&Event::HealthCheck {
            component: "a".into(),
            healthy: true,
            timestamp: Utc::now(),
        });
        history.record(&Event::PluginStarted {
            plugin_id: "b".into(),
            timestamp: Utc::now(),
        });
        history.record(&Event::HealthCheck {
            component: "c".into(),
            healthy: false,
            timestamp: Utc::now(),
        });

        let health_events = history.filter_by_type("health_check");
        assert_eq!(health_events.len(), 2);
    }
}
