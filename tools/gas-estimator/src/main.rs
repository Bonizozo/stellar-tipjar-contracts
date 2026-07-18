//! gas-estimator — CLI tool for estimating TipJar contract gas costs.
//!
//! Reads a gas report produced by the companion integration test
//! (`cargo test -p gas-estimator --test estimate`) and presents it in a
//! human-readable table, JSON, or Markdown format.
//!
//! # Usage
//!
//! ```text
//! # Run measurements and produce a fresh report:
//! cargo test -p gas-estimator --test estimate -- --nocapture
//!
//! # Display the report as a table (default):
//! cargo run -p gas-estimator
//!
//! # Compare against a saved baseline:
//! cargo run -p gas-estimator -- --baseline baseline.json
//!
//! # Save the current report as the new baseline:
//! cargo run -p gas-estimator -- --save-baseline baseline.json
//!
//! # Output as Markdown (for PRs / docs):
//! cargo run -p gas-estimator -- --format markdown
//!
//! # Show cost trend from history file:
//! cargo run -p gas-estimator -- --history gas-history.ndjson
//!
//! # List all measured functions without running estimates:
//! cargo run -p gas-estimator -- --list
//! ```

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use gas_estimator::{append_to_history, load_history, EstimationReport, Severity};

#[derive(Parser)]
#[command(
    name = "gas-estimator",
    about = "Estimate and analyse TipJar contract gas costs",
    long_about = "Reads a gas-estimates.json report produced by the integration test harness \
                  and displays per-function CPU/memory/XLM costs, batch estimates, \
                  cross-function comparisons, and optimisation suggestions.",
    version
)]
struct Cli {
    /// Path to the gas estimation report JSON
    #[arg(long, default_value = "gas-estimates.json")]
    report: String,

    /// Compare against a baseline report; flag regressions >10% CPU increase
    #[arg(long)]
    baseline: Option<String>,

    /// Save the current report as a new baseline file
    #[arg(long)]
    save_baseline: Option<String>,

    /// Append the current report to a newline-delimited JSON history file
    #[arg(long)]
    history: Option<String>,

    /// Show cost trend from a history file (last N entries)
    #[arg(long)]
    trend: Option<String>,

    /// Number of history entries to show in trend view (default: 5)
    #[arg(long, default_value = "5")]
    trend_limit: usize,

    /// Output format
    #[arg(long, value_enum, default_value = "table")]
    format: OutputFormat,

    /// Exit 1 if any function exceeds the CPU warning threshold or a regression is found
    #[arg(long)]
    strict: bool,

    /// List all functions in the report without full output
    #[arg(long)]
    list: bool,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Markdown,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Trend view doesn't need the main report
    if let Some(trend_path) = &cli.trend {
        return print_trend(trend_path, cli.trend_limit);
    }

    let json = std::fs::read_to_string(&cli.report)
        .with_context(|| format!("Cannot read report: {}. Run `cargo test -p gas-estimator --test estimate -- --nocapture` first.", cli.report))?;
    let report: EstimationReport = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse report: {}", cli.report))?;

    // --list: just print function names and exit
    if cli.list {
        println!("Functions in {}:", cli.report);
        for e in &report.estimates {
            println!("  {:<35} ({})", e.function_name, e.storage_variant);
        }
        return Ok(());
    }

    // Main output
    match cli.format {
        OutputFormat::Table => print_table(&report),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Markdown => print_markdown(&report),
    }

    // --save-baseline: copy the report file to the baseline path
    if let Some(baseline_out) = &cli.save_baseline {
        std::fs::copy(&cli.report, baseline_out)
            .with_context(|| format!("Failed to save baseline to {baseline_out}"))?;
        eprintln!("✅ Baseline saved to {baseline_out}");
    }

    // --history: append to history file
    if let Some(history_path) = &cli.history {
        append_to_history(history_path, &report)
            .with_context(|| format!("Failed to append to history: {history_path}"))?;
        eprintln!("📝 Appended to history: {history_path}");
    }

    // --baseline: regression check
    let mut regression = false;
    if let Some(baseline_path) = &cli.baseline {
        let baseline_json = std::fs::read_to_string(baseline_path)
            .with_context(|| format!("Cannot read baseline: {baseline_path}"))?;
        let baseline: EstimationReport = serde_json::from_str(&baseline_json)?;
        regression = compare_baseline(&baseline, &report);
    }

    // --strict: fail on regressions or critical suggestions
    if cli.strict {
        let has_critical = report
            .suggestions
            .iter()
            .any(|s| s.severity == Severity::Critical);
        if has_critical {
            eprintln!("\n❌ Critical gas issues detected (--strict mode).");
            std::process::exit(1);
        }
        if regression {
            eprintln!("\n❌ Gas regression detected (--strict mode).");
            std::process::exit(1);
        }
    }

    Ok(())
}

// ── Table output ──────────────────────────────────────────────────────────────

fn print_table(report: &EstimationReport) {
    println!(
        "╔══════════════════════════════════════════════════════════════════════════════════╗"
    );
    println!("║                  TipJar Gas Cost Estimation Report                              ║");
    println!(
        "╚══════════════════════════════════════════════════════════════════════════════════╝"
    );
    println!(
        "  Generated : {}",
        report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("  Network   : {}", report.network);
    println!();

    // Per-function estimates
    println!("┌──────────────────────────────────────────┬──────────────────┬──────────────────┬──────────────┬──────────────────┐");
    println!("│ Function (variant)                       │ Status           │ CPU Instructions │ Memory Bytes │ Est. Cost (XLM)  │");
    println!("├──────────────────────────────────────────┼──────────────────┼──────────────────┼──────────────┼──────────────────┤");
    for e in &report.estimates {
        let marker = severity_marker(e.cpu_instructions);
        let label = format!("{} ({})", e.function_name, e.storage_variant);
        println!(
            "│ {:<40} │ {:<16} │ {:>16} │ {:>12} │ {:>16.8} │",
            truncate(&label, 40),
            marker,
            e.cpu_instructions,
            e.memory_bytes,
            e.estimated_cost_xlm,
        );
    }
    println!("└──────────────────────────────────────────┴──────────────────┴──────────────────┴──────────────┴──────────────────┘");
    println!("  🟢 < 1M CPU   🟡 1M–5M CPU   🔴 > 5M CPU");
    println!();

    // Batch estimates
    if !report.batch_estimates.is_empty() {
        println!("Batch Operation Estimates");
        println!("{}", "─".repeat(90));
        println!(
            "  {:<35} {:>5}  {:>6}  {:>18}  {:>16}  {:>16}",
            "Operation", "N", "Extrap", "Total CPU", "Total XLM", "Per-item XLM"
        );
        println!("  {}", "─".repeat(86));
        for b in &report.batch_estimates {
            let extrap = if b.is_extrapolated { "yes" } else { "no" };
            println!(
                "  {:<35} {:>5}  {:>6}  {:>18}  {:>16.8}  {:>16.8}",
                b.operation,
                b.batch_size,
                extrap,
                b.total_cpu_instructions,
                b.total_cost_xlm,
                b.cost_per_item_xlm,
            );
        }
        println!();
    }

    // Comparisons
    if !report.comparisons.is_empty() {
        println!("Cost Comparisons");
        println!("{}", "─".repeat(90));
        for c in &report.comparisons {
            let (arrow, sign) = if c.delta_cpu >= 0 {
                ("▲", "+")
            } else {
                ("▼", "")
            };
            println!(
                "  {:<55}  {}{}{:.1}%  ({}{} CPU)",
                c.label, arrow, sign, c.delta_pct, sign, c.delta_cpu
            );
        }
        println!();
    }

    // Suggestions
    if !report.suggestions.is_empty() {
        println!("Optimisation Suggestions");
        println!("{}", "─".repeat(90));
        for s in &report.suggestions {
            let icon = match s.severity {
                Severity::Info => "ℹ️ ",
                Severity::Warning => "⚠️ ",
                Severity::Critical => "🔴",
            };
            println!("  {} [{}]  {}", icon, s.function, s.message);
        }
        println!();
    } else {
        println!("✅ All functions within acceptable gas limits.");
        println!();
    }
}

// ── Markdown output ───────────────────────────────────────────────────────────

fn print_markdown(report: &EstimationReport) {
    println!("# TipJar Gas Cost Estimation Report");
    println!();
    println!(
        "**Generated:** {}  ",
        report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("**Network:** {}  ", report.network);
    println!();

    println!("## Per-Function Estimates");
    println!();
    println!("| Function | Variant | CPU Instructions | Memory Bytes | Est. Cost (XLM) |");
    println!("|---|---|---:|---:|---:|");
    for e in &report.estimates {
        let badge = md_badge(e.cpu_instructions);
        println!(
            "| `{}` | {} | {} {} | {} | `{:.8}` |",
            e.function_name,
            e.storage_variant,
            e.cpu_instructions,
            badge,
            e.memory_bytes,
            e.estimated_cost_xlm,
        );
    }
    println!();

    if !report.batch_estimates.is_empty() {
        println!("## Batch Operation Estimates");
        println!();
        println!("| Operation | N | Extrapolated | Total CPU | Total XLM | Per-item XLM |");
        println!("|---|---:|:---:|---:|---:|---:|");
        for b in &report.batch_estimates {
            let extrap = if b.is_extrapolated { "✓" } else { "" };
            println!(
                "| `{}` | {} | {} | {} | `{:.8}` | `{:.8}` |",
                b.operation,
                b.batch_size,
                extrap,
                b.total_cpu_instructions,
                b.total_cost_xlm,
                b.cost_per_item_xlm,
            );
        }
        println!();
    }

    if !report.comparisons.is_empty() {
        println!("## Cost Comparisons");
        println!();
        println!("| Comparison | Baseline CPU | Candidate CPU | Delta CPU | Delta % |");
        println!("|---|---:|---:|---:|---:|");
        for c in &report.comparisons {
            let sign = if c.delta_cpu >= 0 { "+" } else { "" };
            println!(
                "| {} | {} | {} | {}{} | {}{:.1}% |",
                c.label, c.baseline_cpu, c.candidate_cpu, sign, c.delta_cpu, sign, c.delta_pct,
            );
        }
        println!();
    }

    if !report.suggestions.is_empty() {
        println!("## Optimisation Suggestions");
        println!();
        for s in &report.suggestions {
            let level = match s.severity {
                Severity::Info => "INFO",
                Severity::Warning => "⚠️ WARNING",
                Severity::Critical => "🔴 CRITICAL",
            };
            println!("- **[{}] `{}`** — {}", level, s.function, s.message);
        }
        println!();
    }
}

// ── Baseline comparison ───────────────────────────────────────────────────────

/// Returns `true` if any regression (>10% CPU increase) was detected.
fn compare_baseline(baseline: &EstimationReport, current: &EstimationReport) -> bool {
    println!("\n=== Gas Regression Report ===");
    println!(
        "  Baseline : {}",
        baseline.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!(
        "  Current  : {}",
        current.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!();
    println!(
        "  {:<45} {:>14} {:>14} {:>10}",
        "Function (variant)", "Baseline CPU", "Current CPU", "Delta %"
    );
    println!("  {}", "─".repeat(88));

    let mut regression = false;
    for cur in &current.estimates {
        let key = format!("{} ({})", cur.function_name, cur.storage_variant);
        if let Some(base) = baseline.estimates.iter().find(|b| {
            b.function_name == cur.function_name && b.storage_variant == cur.storage_variant
        }) {
            let delta_pct = if base.cpu_instructions > 0 {
                (cur.cpu_instructions as f64 - base.cpu_instructions as f64)
                    / base.cpu_instructions as f64
                    * 100.0
            } else {
                0.0
            };
            let flag = if delta_pct > 10.0 {
                regression = true;
                " ⚠ REGRESSION"
            } else if delta_pct < -5.0 {
                " ✅ IMPROVEMENT"
            } else {
                ""
            };
            println!(
                "  {:<45} {:>14} {:>14} {:>9.1}%{}",
                key, base.cpu_instructions, cur.cpu_instructions, delta_pct, flag
            );
        } else {
            println!(
                "  {:<45} {:>14} {:>14}  (new)",
                key, "—", cur.cpu_instructions
            );
        }
    }

    println!();
    if regression {
        eprintln!("❌ Gas regression detected (>10% CPU increase on one or more functions).");
    } else {
        println!("✅ No gas regressions detected.");
    }

    regression
}

// ── Trend view ────────────────────────────────────────────────────────────────

fn print_trend(history_path: &str, limit: usize) -> Result<()> {
    let entries = load_history(history_path)
        .with_context(|| format!("Cannot read history: {history_path}"))?;

    if entries.is_empty() {
        println!("No history entries found in {history_path}.");
        return Ok(());
    }

    let recent: Vec<_> = entries.iter().rev().take(limit).collect();

    println!("=== Gas Cost Trend (last {} runs) ===", recent.len());
    println!("  History file: {history_path}");
    println!();

    // Collect all unique function+variant keys from the most recent entry
    let keys: Vec<(String, String)> = recent[0]
        .report
        .estimates
        .iter()
        .map(|e| (e.function_name.clone(), e.storage_variant.clone()))
        .collect();

    for (fn_name, variant) in &keys {
        let label = format!("{fn_name} ({variant})");
        print!("  {:<45}", label);
        for entry in recent.iter().rev() {
            if let Some(e) = entry
                .report
                .estimates
                .iter()
                .find(|e| &e.function_name == fn_name && &e.storage_variant == variant)
            {
                print!("  {:>12}", e.cpu_instructions);
            } else {
                print!("  {:>12}", "—");
            }
        }
        println!();
    }

    println!();
    print!("  {:<45}", "Timestamp");
    for entry in recent.iter().rev() {
        print!("  {:>12}", entry.timestamp.format("%m-%d %H:%M"));
    }
    println!();

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn severity_marker(cpu: u64) -> &'static str {
    if cpu >= gas_estimator::CRITICAL_CPU {
        "🔴 CRITICAL"
    } else if cpu >= gas_estimator::WARN_CPU {
        "🟡 WARNING "
    } else {
        "🟢 OK      "
    }
}

fn md_badge(cpu: u64) -> &'static str {
    if cpu >= gas_estimator::CRITICAL_CPU {
        "🔴"
    } else if cpu >= gas_estimator::WARN_CPU {
        "🟡"
    } else {
        "🟢"
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}
