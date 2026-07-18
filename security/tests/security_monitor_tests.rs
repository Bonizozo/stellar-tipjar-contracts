use tipjar_security::{
    alerting::AlertingService,
    anomaly_detector::AnomalyDetector,
    circuit_breaker::CircuitBreaker,
    monitor::{SecurityCheck, SecurityMonitor, Transaction},
    rate_limiter::RateLimiter,
};

fn make_monitor(max_tx: usize, max_amount: i64, cb_threshold: usize) -> SecurityMonitor {
    SecurityMonitor::new(
        RateLimiter::new(max_tx, 60),
        AnomalyDetector::new(max_amount),
        CircuitBreaker::new(cb_threshold, 60),
        AlertingService::new(None),
    )
}

fn tx(sender: &str, amount: i64) -> Transaction {
    Transaction {
        hash: format!("hash-{sender}-{amount}"),
        sender: sender.to_string(),
        creator: "creator1".to_string(),
        amount,
    }
}

// ── Rate limiting ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_rate_limit_enforced() {
    let monitor = make_monitor(3, 1_000_000, 100);
    for _ in 0..3 {
        assert_eq!(
            monitor.check_transaction(&tx("alice", 100)).await,
            SecurityCheck::Approved
        );
    }
    assert_eq!(
        monitor.check_transaction(&tx("alice", 100)).await,
        SecurityCheck::RateLimited
    );
}

#[tokio::test]
async fn test_whitelist_bypasses_rate_limit() {
    let monitor = make_monitor(1, 1_000_000, 100);
    monitor.whitelist("vip");
    for _ in 0..5 {
        assert_eq!(
            monitor.check_transaction(&tx("vip", 100)).await,
            SecurityCheck::Approved
        );
    }
}

// ── Blacklist ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_blacklist_blocks_address() {
    let monitor = make_monitor(10, 1_000_000, 100);
    monitor.blacklist("bad-actor");
    assert_eq!(
        monitor.check_transaction(&tx("bad-actor", 100)).await,
        SecurityCheck::Blocked
    );
}

#[tokio::test]
async fn test_unblacklist_restores_access() {
    let monitor = make_monitor(10, 1_000_000, 100);
    monitor.blacklist("addr");
    monitor.unblacklist("addr");
    assert_eq!(
        monitor.check_transaction(&tx("addr", 100)).await,
        SecurityCheck::Approved
    );
}

// ── Anomaly detection ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_anomaly_detected_on_huge_amount() {
    // max_amount = 500; sending 1_000_000 should be flagged
    let monitor = make_monitor(100, 500, 100);
    assert_eq!(
        monitor.check_transaction(&tx("bob", 1_000_000)).await,
        SecurityCheck::Suspicious
    );
}

#[tokio::test]
async fn test_normal_amounts_approved() {
    let monitor = make_monitor(100, 1_000_000, 100);
    for _ in 0..5 {
        assert_eq!(
            monitor.check_transaction(&tx("carol", 100)).await,
            SecurityCheck::Approved
        );
    }
}

// ── Circuit breaker ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_circuit_breaker_trips_after_threshold() {
    // threshold = 2 anomalies; max_amount = 1 so every tx is anomalous
    let monitor = make_monitor(100, 1, 2);
    monitor.check_transaction(&tx("x", 1_000_000)).await;
    monitor.check_transaction(&tx("y", 1_000_000)).await;
    // Circuit should now be open
    assert_eq!(
        monitor.check_transaction(&tx("z", 1)).await,
        SecurityCheck::CircuitOpen
    );
}

#[tokio::test]
async fn test_circuit_breaker_reset() {
    let monitor = make_monitor(100, 1, 1);
    monitor.check_transaction(&tx("x", 1_000_000)).await;
    assert_eq!(
        monitor.check_transaction(&tx("y", 1)).await,
        SecurityCheck::CircuitOpen
    );
    monitor.reset_circuit_breaker();
    assert_eq!(
        monitor.check_transaction(&tx("y", 1)).await,
        SecurityCheck::Approved
    );
}
