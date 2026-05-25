use chrono::{DateTime, Utc};
use std::time::Duration;

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker for protecting against cascading failures in plugin connections.
///
/// Pattern extracted from smarshnexus connector base.
#[derive(Debug)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,
    recovery_timeout: Duration,
    last_failure_at: Option<DateTime<Utc>>,
    last_success_at: Option<DateTime<Utc>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            recovery_timeout,
            last_failure_at: None,
            last_success_at: None,
        }
    }

    pub fn state(&self) -> CircuitState {
        self.state
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    pub fn can_execute(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_fail) = self.last_failure_at {
                    let elapsed = Utc::now().signed_duration_since(last_fail);
                    if elapsed.to_std().unwrap_or(Duration::ZERO) >= self.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.last_success_at = Some(Utc::now());
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_at = Some(Utc::now());

        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
        }
    }

    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.last_failure_at = None;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(300))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_lifecycle() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(10));
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.can_execute());

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.can_execute());

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
