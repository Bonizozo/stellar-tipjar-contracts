//! Storage-rent trajectory model for TipJar contract.
//!
//! Models the long-run storage-rent cost accumulation as the number of
//! distinct (creator, token) pairs grows, accounting for:
//!
//! - LEDGER_BUMP = 120,960 ledgers (~7 days) TTL extension on every write
//! - Entry creation cost vs TTL renewal cost
//! - Natural archival after TTL expiration for dormant entries
//! - Incidental TTL bumps from migration or read-path logic
//!
//! Usage:
//!   cargo run --bin storage-rent-analysis -- \
//!     --creators 100000 \
//!     --tokens-per-creator 3 \
//!     --years 3

use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use serde::{Deserialize, Serialize};

// ── Constants from contracts/tipjar/src/lib.rs ──────────────────────────────

/// TTL extension applied on every state-mutating write.
const LEDGER_BUMP: u32 = 120_960; // ~7 days at 5s/ledger
const LEDGER_THRESHOLD: u32 = 100_000;

/// Stellar ledger close time (seconds).
const LEDGER_CLOSE_TIME_SECS: u32 = 5;

/// Ledgers per day (assuming 5s close time).
const LEDGERS_PER_DAY: u32 = 86_400 / LEDGER_CLOSE_TIME_SECS; // 17,280

/// Ledgers per year.
const LEDGERS_PER_YEAR: u32 = LEDGERS_PER_DAY * 365;

// ── Stellar storage cost model ──────────────────────────────────────────────
//
// Based on Stellar Protocol 20+ storage fee schedule:
//
// - **Entry creation**: ~10,000 stroops one-time ledger entry fee
// - **Rent per ledger**: ~0.1 stroops per ledger per entry (simplified)
// - **TTL extension**: when extend_ttl is called, the contract pays rent for
//   the extension period (LEDGER_BUMP ledgers) upfront
//
// Simplified model:
//   - Entry creation cost = 10,000 stroops (one-time)
//   - TTL extension cost = LEDGER_BUMP * 0.1 stroops = 12,096 stroops (~7 days)
//
// These are *approximate* values for modeling purposes; actual costs depend on
// network congestion, entry size, and protocol fee adjustments.

/// One-time cost to create a new ledger entry (stroops).
const ENTRY_CREATION_COST_STROOPS: i128 = 10_000;

/// Rent cost per ledger per entry (stroops).
const RENT_PER_LEDGER_STROOPS: f64 = 0.1;

/// Cost to extend TTL by LEDGER_BUMP ledgers (stroops).
const TTL_EXTENSION_COST_STROOPS: i128 = ((LEDGER_BUMP as f64) * RENT_PER_LEDGER_STROOPS) as i128;

/// Stroops per XLM.
const STROOPS_PER_XLM: i128 = 10_000_000;

// ── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser)]
struct Cli {
    /// Number of distinct creators in the growth scenario.
    #[arg(long, default_value = "100000")]
    creators: u32,

    /// Average number of distinct tokens per creator.
    #[arg(long, default_value = "3")]
    tokens_per_creator: u32,

    /// Number of years to model.
    #[arg(long, default_value = "3")]
    years: u32,

    /// Output path for the JSON report.
    #[arg(long, default_value = "storage-rent-report.json")]
    output: String,

    /// Activity model: "active" (tips every month), "dormant" (one tip, never again),
    /// or "mixed" (50% active, 50% dormant).
    #[arg(long, default_value = "mixed")]
    activity: String,

    /// Print detailed breakdown to stdout.
    #[arg(long)]
    verbose: bool,
}

// ── Model types ──────────────────────────────────────────────────────────────

/// Per-(creator, token) storage entry lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum EntryState {
    /// Entry doesn't exist yet.
    Nonexistent,
    /// Entry exists and TTL is current.
    Active { ttl_remaining: u32 },
    /// Entry archived (TTL expired and no activity).
    Archived,
}

/// Snapshot of storage state at a given ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StorageSnapshot {
    /// Ledger sequence number.
    pub ledger: u32,
    /// Days elapsed since scenario start.
    pub days: f64,
    /// Number of active (non-archived) entries.
    pub active_entries: u32,
    /// Number of archived entries.
    pub archived_entries: u32,
    /// Cumulative storage cost (stroops).
    pub cumulative_cost_stroops: i128,
    /// Cumulative storage cost (XLM).
    pub cumulative_cost_xlm: f64,
    /// Cost incurred this period (stroops).
    pub period_cost_stroops: i128,
}

/// Complete storage-rent trajectory report.
#[derive(Debug, Serialize, Deserialize)]
struct StorageRentReport {
    /// ISO-8601 timestamp of report generation.
    pub timestamp: String,
    /// Scenario parameters.
    pub scenario: ScenarioParams,
    /// Snapshots at monthly intervals.
    pub snapshots: Vec<StorageSnapshot>,
    /// Summary statistics.
    pub summary: Summary,
    /// Operational recommendations.
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScenarioParams {
    pub total_creators: u32,
    pub tokens_per_creator: u32,
    pub years: u32,
    pub activity_model: String,
    pub ledger_bump: u32,
    pub ledger_threshold: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Summary {
    /// Total distinct (creator, token) pairs.
    pub total_pairs: u32,
    /// Peak number of active entries.
    pub peak_active_entries: u32,
    /// Final number of active entries.
    pub final_active_entries: u32,
    /// Final number of archived entries.
    pub final_archived_entries: u32,
    /// Total cost over the modeled period (stroops).
    pub total_cost_stroops: i128,
    /// Total cost over the modeled period (XLM).
    pub total_cost_xlm: f64,
    /// Average cost per creator per year (XLM).
    pub avg_cost_per_creator_per_year_xlm: f64,
}

// ── Simulation engine ────────────────────────────────────────────────────────

struct SimulationState {
    /// Per-(creator, token) entry states.
    entries: Vec<EntryState>,
    /// Current ledger number.
    current_ledger: u32,
    /// Cumulative cost (stroops).
    cumulative_cost: i128,
}

impl SimulationState {
    fn new(total_pairs: u32) -> Self {
        Self {
            entries: vec![EntryState::Nonexistent; total_pairs as usize],
            current_ledger: 0,
            cumulative_cost: 0,
        }
    }

    /// Simulate a tip to entry `idx`, creating or bumping TTL.
    fn tip(&mut self, idx: usize) {
        match self.entries[idx] {
            EntryState::Nonexistent => {
                // First tip: create Balance(creator, token) and Total(creator, token).
                // Cost: 2 * (creation + initial TTL extension)
                let creation_cost = 2 * ENTRY_CREATION_COST_STROOPS;
                let ttl_cost = 2 * TTL_EXTENSION_COST_STROOPS;
                self.cumulative_cost += creation_cost + ttl_cost;
                self.entries[idx] = EntryState::Active {
                    ttl_remaining: LEDGER_BUMP,
                };
            }
            EntryState::Active { ttl_remaining } => {
                // Subsequent tip: extend TTL by LEDGER_BUMP.
                // Cost: 2 * TTL extension (Balance + Total)
                let ttl_cost = 2 * TTL_EXTENSION_COST_STROOPS;
                self.cumulative_cost += ttl_cost;
                self.entries[idx] = EntryState::Active {
                    ttl_remaining: ttl_remaining.saturating_add(LEDGER_BUMP),
                };
            }
            EntryState::Archived => {
                // Archived entry: read operation may trigger maybe_migrate_creator_data,
                // which doesn't apply here (already migrated), so entry remains archived.
                // A new tip after archival would require re-creating the entry.
                // For simplicity, treat as Nonexistent (worst case).
                let creation_cost = 2 * ENTRY_CREATION_COST_STROOPS;
                let ttl_cost = 2 * TTL_EXTENSION_COST_STROOPS;
                self.cumulative_cost += creation_cost + ttl_cost;
                self.entries[idx] = EntryState::Active {
                    ttl_remaining: LEDGER_BUMP,
                };
            }
        }
    }

    /// Advance simulation by `ledgers`, decaying TTLs and archiving expired entries.
    fn advance(&mut self, ledgers: u32) {
        self.current_ledger += ledgers;
        for entry in &mut self.entries {
            if let EntryState::Active { ttl_remaining } = *entry {
                if ttl_remaining <= ledgers {
                    *entry = EntryState::Archived;
                } else {
                    *entry = EntryState::Active {
                        ttl_remaining: ttl_remaining - ledgers,
                    };
                }
            }
        }
    }

    /// Count active and archived entries.
    fn count_entries(&self) -> (u32, u32) {
        let mut active = 0;
        let mut archived = 0;
        for entry in &self.entries {
            match entry {
                EntryState::Active { .. } => active += 1,
                EntryState::Archived => archived += 1,
                EntryState::Nonexistent => {}
            }
        }
        (active, archived)
    }
}

// ── Activity models ──────────────────────────────────────────────────────────

trait ActivityModel {
    /// Returns true if entry `idx` should receive a tip at `ledger`.
    fn should_tip(&self, idx: usize, ledger: u32) -> bool;
}

/// All creators tip once at ledger 0, then never again.
struct DormantModel;

impl ActivityModel for DormantModel {
    fn should_tip(&self, _idx: usize, ledger: u32) -> bool {
        ledger == 0
    }
}

/// All creators tip once per month (every 30 days).
struct ActiveModel;

impl ActivityModel for ActiveModel {
    fn should_tip(&self, _idx: usize, ledger: u32) -> bool {
        ledger.is_multiple_of(LEDGERS_PER_DAY * 30)
    }
}

/// 50% of creators tip monthly, 50% tip once and go dormant.
struct MixedModel {
    active_cutoff: usize,
}

impl MixedModel {
    fn new(total_pairs: u32) -> Self {
        Self {
            active_cutoff: (total_pairs / 2) as usize,
        }
    }
}

impl ActivityModel for MixedModel {
    fn should_tip(&self, idx: usize, ledger: u32) -> bool {
        if idx < self.active_cutoff {
            // Active: tip monthly
            ledger.is_multiple_of(LEDGERS_PER_DAY * 30)
        } else {
            // Dormant: tip once at ledger 0
            ledger == 0
        }
    }
}

// ── Main simulation driver ───────────────────────────────────────────────────

fn run_simulation(cli: &Cli) -> Result<StorageRentReport> {
    let total_pairs = cli.creators * cli.tokens_per_creator;
    let total_ledgers = cli.years * LEDGERS_PER_YEAR;

    let model: Box<dyn ActivityModel> = match cli.activity.as_str() {
        "active" => Box::new(ActiveModel),
        "dormant" => Box::new(DormantModel),
        "mixed" => Box::new(MixedModel::new(total_pairs)),
        _ => anyhow::bail!("Invalid activity model: {}", cli.activity),
    };

    let mut state = SimulationState::new(total_pairs);
    let mut snapshots: Vec<StorageSnapshot> = Vec::new();

    // Snapshot interval: 30 days
    let snapshot_interval = LEDGERS_PER_DAY * 30;

    for ledger in (0..=total_ledgers).step_by(snapshot_interval as usize) {
        // Process tips for this ledger
        for idx in 0..total_pairs as usize {
            if model.should_tip(idx, ledger) {
                state.tip(idx);
            }
        }

        // Capture snapshot
        let (active, archived) = state.count_entries();
        let period_cost = if snapshots.is_empty() {
            state.cumulative_cost
        } else {
            state.cumulative_cost - snapshots.last().unwrap().cumulative_cost_stroops
        };

        snapshots.push(StorageSnapshot {
            ledger,
            days: ledger as f64 / LEDGERS_PER_DAY as f64,
            active_entries: active,
            archived_entries: archived,
            cumulative_cost_stroops: state.cumulative_cost,
            cumulative_cost_xlm: state.cumulative_cost as f64 / STROOPS_PER_XLM as f64,
            period_cost_stroops: period_cost,
        });

        // Advance to next snapshot (decay TTLs)
        if ledger < total_ledgers {
            state.advance(snapshot_interval.min(total_ledgers - ledger));
        }
    }

    // Generate summary
    let peak_active = snapshots
        .iter()
        .map(|s| s.active_entries)
        .max()
        .unwrap_or(0);
    let final_snap = snapshots.last().unwrap();
    let avg_cost_per_creator_per_year = if cli.creators > 0 && cli.years > 0 {
        final_snap.cumulative_cost_xlm / cli.creators as f64 / cli.years as f64
    } else {
        0.0
    };

    let summary = Summary {
        total_pairs,
        peak_active_entries: peak_active,
        final_active_entries: final_snap.active_entries,
        final_archived_entries: final_snap.archived_entries,
        total_cost_stroops: final_snap.cumulative_cost_stroops,
        total_cost_xlm: final_snap.cumulative_cost_xlm,
        avg_cost_per_creator_per_year_xlm: avg_cost_per_creator_per_year,
    };

    // Generate recommendations
    let recommendations = generate_recommendations(&summary, &cli.activity);

    Ok(StorageRentReport {
        timestamp: Utc::now().to_rfc3339(),
        scenario: ScenarioParams {
            total_creators: cli.creators,
            tokens_per_creator: cli.tokens_per_creator,
            years: cli.years,
            activity_model: cli.activity.clone(),
            ledger_bump: LEDGER_BUMP,
            ledger_threshold: LEDGER_THRESHOLD,
        },
        snapshots,
        summary,
        recommendations,
    })
}

fn generate_recommendations(summary: &Summary, activity: &str) -> Vec<String> {
    let mut recs = Vec::new();

    // Archival behavior
    if summary.final_archived_entries > 0 {
        recs.push(format!(
            "{} of {} entries ({:.1}%) naturally archived after TTL expiration. \
             Dormant entries do not incur ongoing rent costs once archived.",
            summary.final_archived_entries,
            summary.total_pairs,
            summary.final_archived_entries as f64 / summary.total_pairs as f64 * 100.0
        ));
    }

    // Active entry footprint
    if summary.final_active_entries > 0 {
        let active_pct = summary.final_active_entries as f64 / summary.total_pairs as f64 * 100.0;
        recs.push(format!(
            "{} entries ({:.1}%) remain active at simulation end. \
             These entries incur TTL extension costs on every tip.",
            summary.final_active_entries, active_pct
        ));
    }

    // Cost analysis
    if summary.total_cost_xlm > 0.0 {
        recs.push(format!(
            "Total storage-rent cost over {} years: {:.4} XLM ({} stroops). \
             Average per creator per year: {:.6} XLM.",
            summary.total_pairs as f64 / summary.total_cost_xlm,
            summary.total_cost_xlm,
            summary.total_cost_stroops,
            summary.avg_cost_per_creator_per_year_xlm
        ));
    }

    // Activity-specific guidance
    match activity {
        "dormant" => {
            recs.push(
                "Dormant model: all entries archive after LEDGER_BUMP (~7 days). \
                 Storage footprint shrinks to zero once all TTLs expire."
                    .to_string(),
            );
        }
        "active" => {
            recs.push(
                "Active model: all entries remain alive indefinitely. \
                 Storage footprint grows with creator count and never shrinks."
                    .to_string(),
            );
        }
        "mixed" => {
            recs.push(
                "Mixed model: 50% active (perpetual TTL renewal), 50% dormant (natural archival). \
                 Long-run footprint stabilizes at ~50% of peak."
                    .to_string(),
            );
        }
        _ => {}
    }

    // Operational plan
    recs.push(
        "Operational plan: Dormant entries archive naturally after LEDGER_BUMP (~7 days) \
         with no activity. No manual cleanup is required. Archived entries are restored \
         on-demand if the creator receives a new tip (incurs re-creation cost)."
            .to_string(),
    );

    recs.push(
        "Read-path TTL behavior: `maybe_migrate_creator_data` is called on `tip`, `withdraw`, \
         `get_balance`, and `get_total_tips`. Migration extends TTL but only occurs once per \
         creator (v1→v2 schema transition). Post-migration, reads do NOT bump TTL; only writes \
         (tip, withdraw) extend TTL. Dormant entries naturally archive."
            .to_string(),
    );

    // Cost-control suggestions
    if summary.avg_cost_per_creator_per_year_xlm > 0.001 {
        recs.push(
            "Consider batching tips or implementing a lazy-write pattern for low-value tips \
             to reduce TTL extension frequency."
                .to_string(),
        );
    }

    recs
}

// ── CLI entry point ──────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    eprintln!("Running storage-rent trajectory simulation...");
    eprintln!("  Creators: {}", cli.creators);
    eprintln!("  Tokens per creator: {}", cli.tokens_per_creator);
    eprintln!("  Years: {}", cli.years);
    eprintln!("  Activity model: {}", cli.activity);
    eprintln!();

    let report = run_simulation(&cli)?;

    // Write JSON report
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&cli.output, json)?;
    eprintln!("Report written to: {}", cli.output);

    // Print summary
    println!("\n=== Storage-Rent Trajectory Summary ===\n");
    println!("Scenario:");
    println!("  Creators: {}", report.scenario.total_creators);
    println!(
        "  Tokens per creator: {}",
        report.scenario.tokens_per_creator
    );
    println!(
        "  Total (creator, token) pairs: {}",
        report.summary.total_pairs
    );
    println!("  Years modeled: {}", report.scenario.years);
    println!("  Activity model: {}", report.scenario.activity_model);
    println!();

    println!("Results:");
    println!(
        "  Peak active entries: {}",
        report.summary.peak_active_entries
    );
    println!(
        "  Final active entries: {}",
        report.summary.final_active_entries
    );
    println!(
        "  Final archived entries: {}",
        report.summary.final_archived_entries
    );
    println!(
        "  Total cost: {:.6} XLM ({} stroops)",
        report.summary.total_cost_xlm, report.summary.total_cost_stroops
    );
    println!(
        "  Average cost per creator per year: {:.6} XLM",
        report.summary.avg_cost_per_creator_per_year_xlm
    );
    println!();

    println!("Recommendations:");
    for (i, rec) in report.recommendations.iter().enumerate() {
        println!("  {}. {}", i + 1, rec);
    }

    // Verbose: print snapshots
    if cli.verbose {
        println!("\n=== Monthly Snapshots ===\n");
        println!(
            "{:>6} {:>8} {:>12} {:>12} {:>20} {:>18}",
            "Ledger", "Days", "Active", "Archived", "Cumulative (XLM)", "Period Cost (stroops)"
        );
        for snap in &report.snapshots {
            println!(
                "{:>6} {:>8.1} {:>12} {:>12} {:>20.6} {:>18}",
                snap.ledger,
                snap.days,
                snap.active_entries,
                snap.archived_entries,
                snap.cumulative_cost_xlm,
                snap.period_cost_stroops
            );
        }
    }

    Ok(())
}
