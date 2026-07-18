/// Security monitoring service — orchestrates rate limiting, anomaly detection,
/// blacklist/whitelist checks, circuit breaker, and alerting.
use crate::alerting::{Alert, AlertingService};
use crate::anomaly_detector::{AnomalyDetector, ANOMALY_THRESHOLD};
use crate::circuit_breaker::CircuitBreaker;
use crate::rate_limiter::RateLimiter;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SecurityCheck {
    Approved,
    RateLimited,
    Suspicious,
    Blocked,
    CircuitOpen,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub hash: String,
    pub sender: String,
    pub creator: String,
    pub amount: i64,
}

pub struct SecurityMonitor {
    rate_limiter: RateLimiter,
    anomaly_detector: AnomalyDetector,
    circuit_breaker: CircuitBreaker,
    alerting: AlertingService,
}

impl SecurityMonitor {
    pub fn new(
        rate_limiter: RateLimiter,
        anomaly_detector: AnomalyDetector,
        circuit_breaker: CircuitBreaker,
        alerting: AlertingService,
    ) -> Self {
        Self {
            rate_limiter,
            anomaly_detector,
            circuit_breaker,
            alerting,
        }
    }

    /// Evaluate a transaction against all security controls.
    pub async fn check_transaction(&self, tx: &Transaction) -> SecurityCheck {
        // 1. Circuit breaker — fast path
        if self.circuit_breaker.is_open() {
            return SecurityCheck::CircuitOpen;
        }

        // 2. Blacklist check
        if self.rate_limiter.is_blacklisted(&tx.sender) {
            self.alerting
                .send_alert(Alert::Blacklisted {
                    address: tx.sender.clone(),
                })
                .await;
            return SecurityCheck::Blocked;
        }

        // 3. Rate limiting
        if !self.rate_limiter.check(&tx.sender) {
            self.alerting
                .send_alert(Alert::RateLimited {
                    address: tx.sender.clone(),
                })
                .await;
            return SecurityCheck::RateLimited;
        }

        // 4. Anomaly detection
        let score = self.anomaly_detector.score(&tx.sender, tx.amount);
        if score > ANOMALY_THRESHOLD {
            self.alerting
                .send_alert(Alert::AnomalyDetected {
                    tx_hash: tx.hash.clone(),
                    sender: tx.sender.clone(),
                    score,
                })
                .await;

            // Feed anomaly into circuit breaker
            if self.circuit_breaker.record_anomaly() {
                self.alerting
                    .send_alert(Alert::CircuitBreakerTripped {
                        reason: format!(
                            "Anomaly threshold exceeded; last tx={} score={score:.2}",
                            tx.hash
                        ),
                    })
                    .await;
            }

            return SecurityCheck::Suspicious;
        }

        info!(tx_hash = %tx.hash, sender = %tx.sender, amount = tx.amount, "transaction approved");
        SecurityCheck::Approved
    }

    // Delegate list management to the rate limiter
    pub fn blacklist(&self, address: &str) {
        self.rate_limiter.add_to_blacklist(address);
    }

    pub fn unblacklist(&self, address: &str) {
        self.rate_limiter.remove_from_blacklist(address);
    }

    pub fn whitelist(&self, address: &str) {
        self.rate_limiter.add_to_whitelist(address);
    }

    pub fn reset_circuit_breaker(&self) {
        self.circuit_breaker.reset();
    }
}
