/// Anomaly detector using statistical thresholds on transaction amounts and velocity.
use std::collections::HashMap;
use std::sync::Mutex;

/// A score in [0.0, 1.0]; values above ANOMALY_THRESHOLD are flagged.
pub const ANOMALY_THRESHOLD: f64 = 0.7;

/// Baseline stats per address (running mean + variance via Welford's algorithm).
struct Stats {
    count: u64,
    mean: f64,
    m2: f64,
}

impl Stats {
    fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
    }

    fn std_dev(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        (self.m2 / (self.count - 1) as f64).sqrt()
    }
}

pub struct AnomalyDetector {
    /// per-address amount stats
    stats: Mutex<HashMap<String, Stats>>,
    /// absolute amount ceiling before flagging
    max_amount: i64,
}

impl AnomalyDetector {
    pub fn new(max_amount: i64) -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
            max_amount,
        }
    }

    /// Score a transaction. Returns a value in [0.0, 1.0].
    /// Updates internal baseline after scoring.
    pub fn score(&self, sender: &str, amount: i64) -> f64 {
        let mut stats = self.stats.lock().unwrap();
        let entry = stats.entry(sender.to_string()).or_insert_with(Stats::new);

        // Absolute ceiling check
        if amount > self.max_amount {
            entry.update(amount as f64);
            return 1.0;
        }

        let score = if entry.count < 2 {
            // Not enough history — treat as normal
            0.0
        } else {
            let std = entry.std_dev();
            if std < 1.0 {
                0.0
            } else {
                let z = ((amount as f64) - entry.mean).abs() / std;
                // Map z-score to [0, 1]: z >= 3 → score 1.0
                (z / 3.0).min(1.0)
            }
        };

        entry.update(amount as f64);
        score
    }
}
