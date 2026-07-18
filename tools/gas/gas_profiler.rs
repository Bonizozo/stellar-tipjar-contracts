//! gas-profiler — runs `cargo test -- bench --nocapture` and captures the
//! `[BENCH]` lines emitted by `tests/gas/benchmarks.rs`, then writes a
//! structured JSON report to `gas-report.json` (or a path given via --output).
//!
//! Usage:
//!   cargo run --bin gas-profiler -- [--output path/to/report.json]

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Parser)]
struct Cli {
    /// Where to write the JSON report (default: gas-report.json)
    #[arg(long, default_value = "gas-report.json")]
    output: String,

    /// Optional baseline report to compare against
    #[arg(long)]
    baseline: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BenchResult {
    pub function: String,
    pub cpu_instructions: u64,
    pub memory_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GasReport {
    pub timestamp: String,
    pub results: Vec<BenchResult>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    eprintln!("Running gas benchmarks...");
    let output = Command::new("cargo")
        .args(["test", "-p", "tipjar", "--", "bench", "--nocapture"])
        .output()
        .context("failed to run cargo test")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    let results = parse_bench_output(&combined);
    if results.is_empty() {
        bail!("No [BENCH] lines found in test output. Make sure the benchmarks compile and run.");
    }

    let report = GasReport {
        timestamp: Utc::now().to_rfc3339(),
        results,
    };

    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&cli.output, &json).context("failed to write report")?;
    eprintln!("Report written to {}", cli.output);

    // Optional baseline comparison
    if let Some(baseline_path) = cli.baseline {
        let baseline_json =
            std::fs::read_to_string(&baseline_path).context("failed to read baseline report")?;
        let baseline: GasReport = serde_json::from_str(&baseline_json)?;
        compare(&baseline, &report);
    }

    Ok(())
}

/// Parse lines like: `[BENCH] tip (cold storage): cpu=12345 instructions, mem=6789 bytes`
fn parse_bench_output(output: &str) -> Vec<BenchResult> {
    let mut results = Vec::new();
    for line in output.lines() {
        let Some(rest) = line.strip_prefix("[BENCH] ") else {
            continue;
        };
        // Split on ": cpu="
        let Some((func, metrics)) = rest.split_once(": cpu=") else {
            continue;
        };
        // metrics = "12345 instructions, mem=6789 bytes"
        let Some((cpu_part, mem_part)) = metrics.split_once(" instructions, mem=") else {
            continue;
        };
        let Some((mem_val, _)) = mem_part.split_once(" bytes") else {
            continue;
        };
        let Ok(cpu) = cpu_part.trim().parse::<u64>() else {
            continue;
        };
        let Ok(mem) = mem_val.trim().parse::<u64>() else {
            continue;
        };
        results.push(BenchResult {
            function: func.trim().to_string(),
            cpu_instructions: cpu,
            memory_bytes: mem,
        });
    }
    results
}

fn compare(baseline: &GasReport, current: &GasReport) {
    println!("\n=== Gas Regression Report ===");
    println!(
        "{:<40} {:>15} {:>15} {:>10}",
        "Function", "Baseline CPU", "Current CPU", "Delta %"
    );
    println!("{}", "-".repeat(85));

    let mut regression = false;
    for cur in &current.results {
        if let Some(base) = baseline.results.iter().find(|b| b.function == cur.function) {
            let delta_pct = (cur.cpu_instructions as f64 - base.cpu_instructions as f64)
                / base.cpu_instructions as f64
                * 100.0;
            let flag = if delta_pct > 10.0 {
                " ⚠ REGRESSION"
            } else {
                ""
            };
            if delta_pct > 10.0 {
                regression = true;
            }
            println!(
                "{:<40} {:>15} {:>15} {:>9.1}%{}",
                cur.function, base.cpu_instructions, cur.cpu_instructions, delta_pct, flag
            );
        }
    }

    if regression {
        eprintln!("\n❌ Gas regression detected (>10% increase). Failing.");
        std::process::exit(1);
    } else {
        println!("\n✅ No gas regressions detected.");
    }
}
