//! Gas estimation library for TipJar contract operations.
//!
//! Provides types and helpers shared between the CLI binary and the
//! integration-test harness that actually runs the Soroban budget measurements.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Core types ────────────────────────────────────────────────────────────────

/// Raw budget numbers captured from `env.budget()` after a single invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEstimate {
    /// Contract function name.
    pub function_name: String,
    /// Storage access pattern: "cold", "warm", "batch-N", or a descriptive label.
    pub storage_variant: String,
    /// CPU instructions consumed by the invocation.
    pub cpu_instructions: u64,
    /// Memory bytes consumed by the invocation.
    pub memory_bytes: u64,
    /// Estimated cost in stroops (1 XLM = 10,000,000 stroops).
    ///
    /// Derived from the Stellar fee model:
    ///   fee_stroops = ceil(cpu_instructions / CPU_PER_STROOP)
    ///               + ceil(memory_bytes    / MEM_PER_STROOP)
    pub estimated_cost_stroops: i128,
    /// Human-readable XLM equivalent.
    pub estimated_cost_xlm: f64,
}

/// A complete estimation report covering all measured functions.
#[derive(Debug, Serialize, Deserialize)]
pub struct EstimationReport {
    /// ISO-8601 timestamp of when the report was generated.
    pub timestamp: DateTime<Utc>,
    /// Stellar network the estimates target (informational).
    pub network: String,
    /// All individual function estimates.
    pub estimates: Vec<GasEstimate>,
    /// Batch operation estimates (batch size → aggregate estimate).
    pub batch_estimates: Vec<BatchEstimate>,
    /// Comparison table between related operations.
    pub comparisons: Vec<Comparison>,
    /// Optimisation suggestions derived from the measurements.
    pub suggestions: Vec<Suggestion>,
}

/// Aggregate cost for a batch of N operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchEstimate {
    /// Human-readable label for the batch scenario.
    pub operation: String,
    /// Number of items in the batch.
    pub batch_size: u32,
    /// Whether this is a real measurement or an extrapolation.
    pub is_extrapolated: bool,
    pub total_cpu_instructions: u64,
    pub total_memory_bytes: u64,
    pub total_cost_stroops: i128,
    pub total_cost_xlm: f64,
    pub cost_per_item_stroops: i128,
    pub cost_per_item_xlm: f64,
}

/// Side-by-side comparison of two operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comparison {
    pub label: String,
    pub baseline: String,
    pub candidate: String,
    pub baseline_cpu: u64,
    pub candidate_cpu: u64,
    /// Positive = candidate is more expensive; negative = cheaper.
    pub delta_cpu: i64,
    pub delta_pct: f64,
}

/// A single optimisation recommendation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Suggestion {
    pub function: String,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// A historical record entry stored in the history file.
#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub report: EstimationReport,
}

// ── Fee model constants ───────────────────────────────────────────────────────

/// Stellar fee model: CPU instructions per stroop.
/// Based on Stellar Core's resource fee schedule (approximate).
pub const CPU_PER_STROOP: u64 = 10_000;

/// Stellar fee model: memory bytes per stroop.
pub const MEM_PER_STROOP: u64 = 1_024;

/// Stroops per XLM.
pub const STROOPS_PER_XLM: i128 = 10_000_000;

/// CPU threshold above which a warning is emitted.
pub const WARN_CPU: u64 = 1_000_000;

/// CPU threshold above which a critical alert is emitted.
pub const CRITICAL_CPU: u64 = 5_000_000;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert raw budget numbers to an estimated stroop cost.
pub fn compute_cost_stroops(cpu: u64, mem: u64) -> i128 {
    let cpu_fee = (cpu as i128 + CPU_PER_STROOP as i128 - 1) / CPU_PER_STROOP as i128;
    let mem_fee = (mem as i128 + MEM_PER_STROOP as i128 - 1) / MEM_PER_STROOP as i128;
    cpu_fee + mem_fee
}

/// Convert stroops to XLM.
pub fn stroops_to_xlm(stroops: i128) -> f64 {
    stroops as f64 / STROOPS_PER_XLM as f64
}

/// Build a `GasEstimate` from raw budget numbers.
pub fn make_estimate(
    function_name: &str,
    storage_variant: &str,
    cpu: u64,
    mem: u64,
) -> GasEstimate {
    let cost = compute_cost_stroops(cpu, mem);
    GasEstimate {
        function_name: function_name.to_string(),
        storage_variant: storage_variant.to_string(),
        cpu_instructions: cpu,
        memory_bytes: mem,
        estimated_cost_stroops: cost,
        estimated_cost_xlm: stroops_to_xlm(cost),
    }
}

/// Build a `BatchEstimate` from a pre-computed aggregate `GasEstimate`.
pub fn make_batch_estimate(
    operation: &str,
    size: u32,
    is_extrapolated: bool,
    estimate: &GasEstimate,
) -> BatchEstimate {
    let total_cost = compute_cost_stroops(estimate.cpu_instructions, estimate.memory_bytes);
    let per_item = if size > 0 {
        total_cost / size as i128
    } else {
        0
    };
    BatchEstimate {
        operation: operation.to_string(),
        batch_size: size,
        is_extrapolated,
        total_cpu_instructions: estimate.cpu_instructions,
        total_memory_bytes: estimate.memory_bytes,
        total_cost_stroops: total_cost,
        total_cost_xlm: stroops_to_xlm(total_cost),
        cost_per_item_stroops: per_item,
        cost_per_item_xlm: stroops_to_xlm(per_item),
    }
}

/// Derive optimisation suggestions from a list of estimates.
pub fn generate_suggestions(estimates: &[GasEstimate]) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    for e in estimates {
        // CPU threshold alerts
        let cpu_severity = if e.cpu_instructions >= CRITICAL_CPU {
            Some(Severity::Critical)
        } else if e.cpu_instructions >= WARN_CPU {
            Some(Severity::Warning)
        } else {
            None
        };

        if let Some(sev) = cpu_severity {
            suggestions.push(Suggestion {
                function: format!("{} ({})", e.function_name, e.storage_variant),
                severity: sev,
                message: format!(
                    "CPU usage ({} instructions) is high. Consider caching storage reads \
                     or splitting the operation into smaller steps.",
                    e.cpu_instructions
                ),
            });
        }

        // Memory threshold alert
        if e.memory_bytes >= 50_000 {
            suggestions.push(Suggestion {
                function: format!("{} ({})", e.function_name, e.storage_variant),
                severity: Severity::Warning,
                message: format!(
                    "Memory usage ({} bytes) is elevated. Avoid allocating large \
                     Vecs/Maps inside the contract; prefer pagination.",
                    e.memory_bytes
                ),
            });
        }

        // Function-specific hints
        if e.function_name.contains("leaderboard") {
            suggestions.push(Suggestion {
                function: e.function_name.clone(),
                severity: Severity::Info,
                message: "Leaderboard queries iterate over all participants. \
                          Maintain a pre-sorted index in storage to avoid O(n) scans."
                    .to_string(),
            });
        }

        if e.function_name.contains("split") {
            suggestions.push(Suggestion {
                function: e.function_name.clone(),
                severity: Severity::Info,
                message: "tip_split writes one storage entry per recipient (2–10). \
                          Cost scales linearly with recipient count."
                    .to_string(),
            });
        }

        if e.function_name.contains("subscription") && e.cpu_instructions >= WARN_CPU {
            suggestions.push(Suggestion {
                function: e.function_name.clone(),
                severity: Severity::Info,
                message: "Subscription operations read and write the full Subscription struct. \
                          Keep the struct small to minimise serialisation cost."
                    .to_string(),
            });
        }

        if e.function_name == "tip_with_fee" && e.storage_variant.contains("high") {
            suggestions.push(Suggestion {
                function: e.function_name.clone(),
                severity: Severity::Info,
                message: "High-congestion tip_with_fee runs additional fee-adjustment logic. \
                          Users should be warned that fees are elevated during congestion."
                    .to_string(),
            });
        }
    }

    // Cold vs warm overhead hint for `tip`
    let cold = estimates
        .iter()
        .find(|e| e.function_name == "tip" && e.storage_variant == "cold");
    let warm = estimates
        .iter()
        .find(|e| e.function_name == "tip" && e.storage_variant == "warm");
    if let (Some(c), Some(w)) = (cold, warm) {
        if w.cpu_instructions > 0 {
            let overhead_pct = (c.cpu_instructions as f64 - w.cpu_instructions as f64)
                / w.cpu_instructions as f64
                * 100.0;
            if overhead_pct > 30.0 {
                suggestions.push(Suggestion {
                    function: "tip".to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "Cold-storage tip is {overhead_pct:.0}% more expensive than warm. \
                         First-time creator tips allocate new ledger entries; this is expected \
                         but worth surfacing to users who tip new creators."
                    ),
                });
            }
        }
    }

    // tip_with_fee congestion comparison
    let fee_low = estimates
        .iter()
        .find(|e| e.function_name == "tip_with_fee" && e.storage_variant == "low-congestion");
    let fee_high = estimates
        .iter()
        .find(|e| e.function_name == "tip_with_fee" && e.storage_variant == "high-congestion");
    if let (Some(low), Some(high)) = (fee_low, fee_high) {
        if low.cpu_instructions > 0 {
            let overhead_pct = (high.cpu_instructions as f64 - low.cpu_instructions as f64)
                / low.cpu_instructions as f64
                * 100.0;
            if overhead_pct.abs() > 5.0 {
                suggestions.push(Suggestion {
                    function: "tip_with_fee".to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "High-congestion tip_with_fee costs {overhead_pct:+.1}% vs low-congestion. \
                         The dynamic fee path adds overhead proportional to congestion level."
                    ),
                });
            }
        }
    }

    suggestions
}

/// Build comparison entries from a slice of estimates.
pub fn generate_comparisons(estimates: &[GasEstimate]) -> Vec<Comparison> {
    let mut comparisons = Vec::new();

    // (label, baseline_fn, baseline_variant, candidate_fn, candidate_variant)
    let pairs: &[(&str, &str, &str, &str, &str)] = &[
        ("tip: cold vs warm storage", "tip", "cold", "tip", "warm"),
        (
            "tip vs tip_with_fee (low congestion)",
            "tip",
            "cold",
            "tip_with_fee",
            "low-congestion",
        ),
        (
            "tip_with_fee: low vs high congestion",
            "tip_with_fee",
            "low-congestion",
            "tip_with_fee",
            "high-congestion",
        ),
        (
            "tip vs tip_split (3 recipients)",
            "tip",
            "cold",
            "tip_split",
            "3-recipients",
        ),
        (
            "tip_split: 3 vs 10 recipients",
            "tip_split",
            "3-recipients",
            "tip_split",
            "10-recipients",
        ),
        (
            "withdraw vs get_withdrawable_balance",
            "withdraw",
            "warm",
            "get_withdrawable_balance",
            "warm",
        ),
        (
            "create_subscription vs execute_subscription_payment",
            "create_subscription",
            "cold",
            "execute_subscription_payment",
            "warm",
        ),
        (
            "execute_conditional_tip vs tip (cold)",
            "tip",
            "cold",
            "execute_conditional_tip",
            "cold",
        ),
        (
            "get_leaderboard: 1 vs 10 creators",
            "get_leaderboard",
            "1-creator",
            "get_leaderboard",
            "10-creators",
        ),
    ];

    for (label, base_fn, base_var, cand_fn, cand_var) in pairs {
        let base = estimates
            .iter()
            .find(|e| e.function_name == *base_fn && e.storage_variant == *base_var);
        let cand = estimates
            .iter()
            .find(|e| e.function_name == *cand_fn && e.storage_variant == *cand_var);
        if let (Some(b), Some(c)) = (base, cand) {
            let delta = c.cpu_instructions as i64 - b.cpu_instructions as i64;
            let delta_pct = if b.cpu_instructions > 0 {
                delta as f64 / b.cpu_instructions as f64 * 100.0
            } else {
                0.0
            };
            comparisons.push(Comparison {
                label: label.to_string(),
                baseline: format!("{} ({})", b.function_name, b.storage_variant),
                candidate: format!("{} ({})", c.function_name, c.storage_variant),
                baseline_cpu: b.cpu_instructions,
                candidate_cpu: c.cpu_instructions,
                delta_cpu: delta,
                delta_pct,
            });
        }
    }

    comparisons
}

/// Append a report to a history file (newline-delimited JSON).
///
/// Each line is a JSON object with `timestamp` and the full report.
/// This lets you track cost trends over time without overwriting old data.
pub fn append_to_history(history_path: &str, report: &EstimationReport) -> std::io::Result<()> {
    use std::io::Write;
    let entry = HistoryEntry {
        timestamp: report.timestamp,
        report: EstimationReport {
            timestamp: report.timestamp,
            network: report.network.clone(),
            estimates: report.estimates.clone(),
            batch_estimates: report.batch_estimates.clone(),
            comparisons: report.comparisons.clone(),
            suggestions: report.suggestions.clone(),
        },
    };
    let line = serde_json::to_string(&entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)?;
    writeln!(file, "{}", line)
}

/// Load all history entries from a newline-delimited JSON history file.
pub fn load_history(history_path: &str) -> std::io::Result<Vec<HistoryEntry>> {
    let content = std::fs::read_to_string(history_path)?;
    let entries = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<HistoryEntry>(l).ok())
        .collect();
    Ok(entries)
}
