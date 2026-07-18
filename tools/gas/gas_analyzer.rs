//! gas-analyzer — reads a gas-report.json produced by gas-profiler and prints
//! optimization recommendations based on CPU instruction counts.
//!
//! Usage:
//!   cargo run --bin gas-analyzer -- --report gas-report.json

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
struct Cli {
    /// Path to the gas report JSON (produced by gas-profiler)
    #[arg(long, default_value = "gas-report.json")]
    report: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchResult {
    function: String,
    cpu_instructions: u64,
    memory_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct GasReport {
    timestamp: String,
    results: Vec<BenchResult>,
}

/// Thresholds (CPU instructions) above which we emit recommendations.
const WARN_CPU: u64 = 500_000;
const CRITICAL_CPU: u64 = 2_000_000;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let json = std::fs::read_to_string(&cli.report)
        .with_context(|| format!("cannot read {}", cli.report))?;
    let report: GasReport = serde_json::from_str(&json)?;

    println!("=== TipJar Gas Analysis Report ===");
    println!("Generated: {}\n", report.timestamp);
    println!(
        "{:<45} {:>18} {:>14}",
        "Function", "CPU Instructions", "Memory Bytes"
    );
    println!("{}", "-".repeat(80));

    let mut any_recommendation = false;
    for r in &report.results {
        let marker = if r.cpu_instructions >= CRITICAL_CPU {
            "🔴"
        } else if r.cpu_instructions >= WARN_CPU {
            "🟡"
        } else {
            "🟢"
        };
        println!(
            "{} {:<43} {:>18} {:>14}",
            marker, r.function, r.cpu_instructions, r.memory_bytes
        );
    }

    println!("\n=== Optimization Recommendations ===\n");
    for r in &report.results {
        let recs = recommendations(&r.function, r.cpu_instructions, r.memory_bytes);
        if !recs.is_empty() {
            any_recommendation = true;
            println!("▶ {}:", r.function);
            for rec in recs {
                println!("  • {rec}");
            }
        }
    }

    if !any_recommendation {
        println!("✅ All functions are within acceptable gas limits. No recommendations.");
    }

    Ok(())
}

fn recommendations(function: &str, cpu: u64, mem: u64) -> Vec<String> {
    let mut recs = Vec::new();

    if cpu >= CRITICAL_CPU {
        recs.push(format!(
            "CPU usage ({cpu}) is critically high. Consider splitting this operation \
             or caching intermediate results in persistent storage."
        ));
    } else if cpu >= WARN_CPU {
        recs.push(format!(
            "CPU usage ({cpu}) exceeds the warning threshold ({WARN_CPU}). \
             Profile hot loops and reduce redundant storage reads."
        ));
    }

    if mem >= 50_000 {
        recs.push(format!(
            "Memory usage ({mem} bytes) is high. Avoid allocating large Vecs/Maps \
             inside the contract; prefer streaming or pagination."
        ));
    }

    // Function-specific hints
    if function.contains("batch") && cpu >= WARN_CPU {
        recs.push(
            "Batch operations: ensure per-item storage writes are minimised. \
             Consider accumulating deltas and writing once per creator."
                .to_string(),
        );
    }
    if function.contains("leaderboard") || function.contains("top_tippers") {
        recs.push(
            "Leaderboard queries iterate over all participants. \
             Maintain a pre-sorted index in storage to avoid O(n) scans."
                .to_string(),
        );
    }
    if function.contains("locked") && cpu >= WARN_CPU {
        recs.push(
            "Locked tip operations: use a counter-keyed map instead of scanning \
             all locked tips to find eligible withdrawals."
                .to_string(),
        );
    }

    recs
}
