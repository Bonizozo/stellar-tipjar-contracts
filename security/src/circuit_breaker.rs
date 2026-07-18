use chrono::{DateTime, Utc};
/// Automated circuit breaker — trips after too many anomalies in a time window.
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed, // normal operation
    Open,   // tripped — all transactions blocked
}

pub struct CircuitBreaker {
    /// how many anomalies within the window trigger the breaker
    threshold: usize,
    /// window duration in seconds
    window_secs: i64,
    state: Mutex<CircuitState>,
    /// timestamps of recent anomaly events
    events: Mutex<Vec<DateTime<Utc>>>,
}

impl CircuitBreaker {
    pub fn new(threshold: usize, window_secs: i64) -> Self {
        Self {
            threshold,
            window_secs,
            state: Mutex::new(CircuitState::Closed),
            events: Mutex::new(Vec::new()),
        }
    }

    /// Record an anomaly event; returns true if the breaker just tripped.
    pub fn record_anomaly(&self) -> bool {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(self.window_secs);

        let mut events = self.events.lock().unwrap();
        events.retain(|t| *t > cutoff);
        events.push(now);

        if events.len() >= self.threshold {
            let mut state = self.state.lock().unwrap();
            if *state == CircuitState::Closed {
                *state = CircuitState::Open;
                return true; // just tripped
            }
        }
        false
    }

    pub fn is_open(&self) -> bool {
        *self.state.lock().unwrap() == CircuitState::Open
    }

    /// Manually reset the circuit breaker (operator action).
    pub fn reset(&self) {
        *self.state.lock().unwrap() = CircuitState::Closed;
        self.events.lock().unwrap().clear();
    }
}
