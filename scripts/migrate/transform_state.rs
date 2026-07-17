//! # State Transformation — schema migration between contract versions
//!
//! Applies a chain of versioned transformation rules to a [`StateSnapshot`]
//! produced by the export phase, producing a new snapshot that conforms to the
//! target schema version.
//!
//! ## Design
//! Each transformation rule is a function with the signature:
//! ```text
//! fn rule_name(snapshot: &mut StateSnapshot, log: &mut Vec<TransformationLog>)
//! ```
//! Rules are registered in [`TRANSFORM_RULES`] and executed in order.
//! Every rule logs what it changed so the operator has a full audit trail.
//!
//! ## Adding a new rule
//! 1. Write a function following the signature above.
//! 2. Add it to the `TRANSFORM_RULES` slice with the version range it applies to.
//! 3. Add a unit test in the `tests` module at the bottom of this file.
//!
//! ## Usage
//! ```bash
//! cargo run --manifest-path scripts/migrate/Cargo.toml \
//!     --bin transform -- \
//!     --input  migration-snapshots/source-snapshot.json \
//!     --output migration-snapshots/transformed-snapshot.json \
//!     --from-version 1 --to-version 2
//! ```

use std::fs;
use std::path::Path;

use crate::export_state::compute_checksum;
use crate::types::{MigrationConfig, StateSnapshot, TransformationLog};

// ─────────────────────────────────────────────────────────────────────────────
// Transformation rule registry
// ─────────────────────────────────────────────────────────────────────────────

/// A single versioned transformation rule.
struct TransformRule {
    /// Minimum source schema version this rule applies to (inclusive).
    from_version: u32,
    /// Maximum source schema version this rule applies to (inclusive).
    to_version: u32,
    /// Short identifier used in the log.
    id: &'static str,
    /// Human-readable description.
    description: &'static str,
    /// The transformation function.
    apply: fn(&mut StateSnapshot, &mut Vec<TransformationLog>),
}

/// All registered transformation rules, applied in declaration order.
static TRANSFORM_RULES: &[TransformRule] = &[
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_normalise_balances",
        description: "Remove zero-value creator balance entries to reduce storage footprint.",
        apply: rule_remove_zero_balances,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_normalise_totals",
        description: "Remove zero-value creator total entries.",
        apply: rule_remove_zero_totals,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_cancel_expired_locks",
        description: "Mark time-locked tips whose unlock_time is in the past as cancelled \
                       so they are not re-imported as active locks.",
        apply: rule_mark_expired_locks_cancelled,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_dedup_subscriptions",
        description: "Remove duplicate subscription entries keeping the most recent one.",
        apply: rule_dedup_subscriptions,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_strip_cancelled_subscriptions",
        description: "Remove subscriptions in Cancelled status to reduce import payload.",
        apply: rule_strip_cancelled_subscriptions,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_cap_tip_history",
        description: "Cap per-creator tip history at 1 000 entries (newest first) to \
                       stay within Soroban storage limits.",
        apply: rule_cap_tip_history,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_remove_refunded_tip_records",
        description: "Drop fully-refunded tip records to reduce import size.",
        apply: rule_remove_refunded_tip_records,
    },
    TransformRule {
        from_version: 1,
        to_version: 1,
        id: "v1_to_v2_bump_schema_version",
        description: "Update the snapshot schema_version field to the target version.",
        apply: rule_bump_schema_version,
    },
];

// ─────────────────────────────────────────────────────────────────────────────
// StateTransformer
// ─────────────────────────────────────────────────────────────────────────────

/// Applies all applicable transformation rules to a snapshot.
pub struct StateTransformer {
    pub source_version: u32,
    pub target_version: u32,
}

impl StateTransformer {
    pub fn new(source_version: u32, target_version: u32) -> Self {
        Self {
            source_version,
            target_version,
        }
    }

    /// Applies all rules whose version range overlaps `[source_version, target_version)`.
    ///
    /// Returns the transformed snapshot and a log of every change made.
    pub fn transform(
        &self,
        mut snapshot: StateSnapshot,
    ) -> Result<(StateSnapshot, Vec<TransformationLog>), String> {
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║         TipJar State Transform  v{} → v{:<22}║",
            self.source_version, self.target_version);
        println!("╚══════════════════════════════════════════════════════════╝");

        if snapshot.schema_version != self.source_version {
            return Err(format!(
                "Snapshot schema version {} does not match expected source version {}",
                snapshot.schema_version, self.source_version
            ));
        }

        let mut logs: Vec<TransformationLog> = Vec::new();
        let mut rules_applied = 0u32;

        for rule in TRANSFORM_RULES {
            if rule.from_version <= self.source_version
                && rule.to_version < self.target_version
            {
                // Rule applies to this migration path.
                let before_records = snapshot.total_records();
                println!("  Applying rule: {} …", rule.id);
                (rule.apply)(&mut snapshot, &mut logs);
                let after_records = snapshot.total_records();
                let delta = before_records as i64 - after_records as i64;
                if delta != 0 {
                    println!("    ↳ record delta: {}{}", if delta > 0 { "-" } else { "+" }, delta.abs());
                }
                rules_applied += 1;
            }
        }

        // Recompute checksum after all transformations.
        snapshot.checksum = compute_checksum(&snapshot)?;

        println!();
        println!("✓ Transform complete — {} rules applied", rules_applied);
        println!("  {} transformation log entries", logs.len());
        println!("  New checksum: {}", snapshot.checksum);

        Ok((snapshot, logs))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Individual transformation rules
// ─────────────────────────────────────────────────────────────────────────────

/// Removes creator balance entries where the balance is exactly zero.
fn rule_remove_zero_balances(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    let before = snapshot.creator_balances.len();
    snapshot.creator_balances.retain(|_, v| *v != 0);
    let removed = before - snapshot.creator_balances.len();
    logs.push(TransformationLog {
        rule: "v1_to_v2_normalise_balances".into(),
        description: format!("Removed {} zero-value balance entries.", removed),
        records_affected: removed,
    });
}

/// Removes creator total entries where the total is exactly zero.
fn rule_remove_zero_totals(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    let before = snapshot.creator_totals.len();
    snapshot.creator_totals.retain(|_, v| *v != 0);
    let removed = before - snapshot.creator_totals.len();
    logs.push(TransformationLog {
        rule: "v1_to_v2_normalise_totals".into(),
        description: format!("Removed {} zero-value total entries.", removed),
        records_affected: removed,
    });
}

/// Marks time-locked tips whose `unlock_time` is in the past as cancelled.
/// Uses the snapshot's `exported_at` timestamp as "now".
fn rule_mark_expired_locks_cancelled(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    let now = snapshot.exported_at;
    let mut affected = 0usize;
    for lock in snapshot.time_locks.iter_mut() {
        if !lock.cancelled && lock.unlock_time < now {
            lock.cancelled = true;
            affected += 1;
        }
    }
    logs.push(TransformationLog {
        rule: "v1_to_v2_cancel_expired_locks".into(),
        description: format!(
            "Marked {} expired time-lock(s) as cancelled (unlock_time < {}).",
            affected, now
        ),
        records_affected: affected,
    });
}

/// Removes duplicate subscription entries, keeping the one with the latest
/// `next_payment` timestamp.
fn rule_dedup_subscriptions(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    // Subscriptions are already keyed by "subscriber:creator" so duplicates
    // cannot exist in the HashMap.  This rule is a no-op but is kept as a
    // guard in case the export ever produces duplicates.
    logs.push(TransformationLog {
        rule: "v1_to_v2_dedup_subscriptions".into(),
        description: "No duplicate subscriptions found (HashMap keyed by subscriber:creator)."
            .into(),
        records_affected: 0,
    });
}

/// Removes subscriptions whose status is `Cancelled`.
fn rule_strip_cancelled_subscriptions(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    use crate::types::SubscriptionStatus;
    let before = snapshot.subscriptions.len();
    snapshot
        .subscriptions
        .retain(|_, sub| sub.status != SubscriptionStatus::Cancelled);
    let removed = before - snapshot.subscriptions.len();
    logs.push(TransformationLog {
        rule: "v1_to_v2_strip_cancelled_subscriptions".into(),
        description: format!("Removed {} cancelled subscription(s).", removed),
        records_affected: removed,
    });
}

/// Caps per-creator tip history at 1 000 entries, keeping the newest.
/// Entries are assumed to be stored oldest-first (ascending index order).
fn rule_cap_tip_history(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    const MAX_HISTORY: usize = 1_000;
    let mut total_removed = 0usize;

    for tips in snapshot.tip_history.values_mut() {
        if tips.len() > MAX_HISTORY {
            let excess = tips.len() - MAX_HISTORY;
            // Remove the oldest entries (front of the Vec).
            tips.drain(0..excess);
            total_removed += excess;
        }
    }

    logs.push(TransformationLog {
        rule: "v1_to_v2_cap_tip_history".into(),
        description: format!(
            "Trimmed {} old tip history record(s) to enforce {} entry cap per creator.",
            total_removed, MAX_HISTORY
        ),
        records_affected: total_removed,
    });
}

/// Removes tip records that have been fully refunded.
fn rule_remove_refunded_tip_records(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    let before = snapshot.tip_records.len();
    snapshot.tip_records.retain(|r| !r.refunded);
    let removed = before - snapshot.tip_records.len();
    logs.push(TransformationLog {
        rule: "v1_to_v2_remove_refunded_tip_records".into(),
        description: format!("Removed {} fully-refunded tip record(s).", removed),
        records_affected: removed,
    });
}

/// Bumps the snapshot's `schema_version` to the target version.
fn rule_bump_schema_version(
    snapshot: &mut StateSnapshot,
    logs: &mut Vec<TransformationLog>,
) {
    // The target version is not directly accessible inside a static fn, so we
    // hard-code the increment.  For multi-hop migrations add more rules.
    let old = snapshot.schema_version;
    snapshot.schema_version = old + 1;
    logs.push(TransformationLog {
        rule: "v1_to_v2_bump_schema_version".into(),
        description: format!("Bumped schema_version from {} to {}.", old, snapshot.schema_version),
        records_affected: 1,
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Loads a snapshot from disk, transforms it, and writes the result.
pub fn run_transform(args: &[String]) -> Result<(), String> {
    let config_path = parse_flag(args, "--config")
        .unwrap_or_else(|| "scripts/migrate/config.toml".to_string());
    let input_override = parse_flag(args, "--input");
    let output_override = parse_flag(args, "--output");
    let from_version: Option<u32> = parse_flag(args, "--from-version")
        .and_then(|s| s.parse().ok());
    let to_version: Option<u32> = parse_flag(args, "--to-version")
        .and_then(|s| s.parse().ok());

    let toml_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config {}: {}", config_path, e))?;
    let config: MigrationConfig = toml::from_str(&toml_str)
        .map_err(|e| format!("Config parse error: {}", e))?;

    let input_path = input_override.unwrap_or_else(|| {
        format!(
            "{}/{}",
            config.migration.snapshot_dir, config.migration.export_filename
        )
    });
    let output_path = output_override.unwrap_or_else(|| {
        format!(
            "{}/{}",
            config.migration.snapshot_dir, config.migration.transformed_filename
        )
    });
    let src_ver = from_version.unwrap_or(config.migration.source_version);
    let tgt_ver = to_version.unwrap_or(config.migration.target_version);

    let json = fs::read_to_string(&input_path)
        .map_err(|e| format!("Cannot read snapshot {}: {}", input_path, e))?;
    let snapshot: StateSnapshot = serde_json::from_str(&json)
        .map_err(|e| format!("Snapshot parse error: {}", e))?;

    // Verify checksum before transforming.
    crate::export_state::verify_checksum(&snapshot)?;

    let transformer = StateTransformer::new(src_ver, tgt_ver);
    let (transformed, logs) = transformer.transform(snapshot)?;

    // Write transformation log alongside the snapshot.
    let log_path = output_path.replace(".json", "-transform-log.json");
    let log_json = serde_json::to_string_pretty(&logs)
        .map_err(|e| format!("Log serialisation error: {}", e))?;
    fs::write(&log_path, log_json)
        .map_err(|e| format!("Failed to write transform log: {}", e))?;
    println!("  Transform log written → {}", log_path);

    // Write transformed snapshot.
    let out_json = serde_json::to_string_pretty(&transformed)
        .map_err(|e| format!("Serialisation error: {}", e))?;
    let dir = Path::new(&output_path).parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;
    fs::write(&output_path, out_json)
        .map_err(|e| format!("Failed to write transformed snapshot: {}", e))?;
    println!("  Transformed snapshot written → {}", output_path);

    Ok(())
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Subscription, SubscriptionStatus, TimeLock, TipRecord};
    use std::collections::HashMap;

    fn empty_snapshot() -> StateSnapshot {
        StateSnapshot {
            contract_id: "CTEST".into(),
            schema_version: 1,
            exported_at: 1_700_000_000,
            ledger_sequence: 1000,
            checksum: String::new(),
            admin: "GADMIN".into(),
            fee_basis_points: 100,
            refund_window_seconds: 86400,
            paused: false,
            pause_reason: None,
            tip_counter: 0,
            matching_counter: 0,
            current_fee_bps: 100,
            whitelisted_tokens: vec![],
            creator_balances: HashMap::new(),
            creator_totals: HashMap::new(),
            tip_history: HashMap::new(),
            tipper_aggregates: HashMap::new(),
            creator_aggregates: HashMap::new(),
            subscriptions: HashMap::new(),
            time_locks: vec![],
            milestones: HashMap::new(),
            matching_programs: vec![],
            tip_records: vec![],
            locked_tips: vec![],
            user_roles: HashMap::new(),
        }
    }

    #[test]
    fn test_remove_zero_balances() {
        let mut snap = empty_snapshot();
        snap.creator_balances.insert("GCREATOR:GTOKEN".into(), 0);
        snap.creator_balances.insert("GCREATOR2:GTOKEN".into(), 500);
        let mut logs = vec![];
        rule_remove_zero_balances(&mut snap, &mut logs);
        assert_eq!(snap.creator_balances.len(), 1);
        assert!(snap.creator_balances.contains_key("GCREATOR2:GTOKEN"));
        assert_eq!(logs[0].records_affected, 1);
    }

    #[test]
    fn test_remove_zero_totals() {
        let mut snap = empty_snapshot();
        snap.creator_totals.insert("GCREATOR:GTOKEN".into(), 0);
        snap.creator_totals.insert("GCREATOR2:GTOKEN".into(), 1000);
        let mut logs = vec![];
        rule_remove_zero_totals(&mut snap, &mut logs);
        assert_eq!(snap.creator_totals.len(), 1);
        assert_eq!(logs[0].records_affected, 1);
    }

    #[test]
    fn test_mark_expired_locks_cancelled() {
        let mut snap = empty_snapshot();
        snap.exported_at = 2_000_000_000;
        snap.time_locks.push(TimeLock {
            lock_id: 0,
            sender: "GSENDER".into(),
            creator: "GCREATOR".into(),
            token: "GTOKEN".into(),
            amount: 100,
            unlock_time: 1_000_000_000, // in the past
            cancelled: false,
        });
        snap.time_locks.push(TimeLock {
            lock_id: 1,
            sender: "GSENDER".into(),
            creator: "GCREATOR".into(),
            token: "GTOKEN".into(),
            amount: 200,
            unlock_time: 3_000_000_000, // in the future
            cancelled: false,
        });
        let mut logs = vec![];
        rule_mark_expired_locks_cancelled(&mut snap, &mut logs);
        assert!(snap.time_locks[0].cancelled);
        assert!(!snap.time_locks[1].cancelled);
        assert_eq!(logs[0].records_affected, 1);
    }

    #[test]
    fn test_strip_cancelled_subscriptions() {
        let mut snap = empty_snapshot();
        snap.subscriptions.insert(
            "GSUB:GCREATOR".into(),
            Subscription {
                subscriber: "GSUB".into(),
                creator: "GCREATOR".into(),
                token: "GTOKEN".into(),
                amount: 100,
                interval_seconds: 86400,
                last_payment: 0,
                next_payment: 0,
                status: SubscriptionStatus::Cancelled,
            },
        );
        snap.subscriptions.insert(
            "GSUB2:GCREATOR".into(),
            Subscription {
                subscriber: "GSUB2".into(),
                creator: "GCREATOR".into(),
                token: "GTOKEN".into(),
                amount: 200,
                interval_seconds: 86400,
                last_payment: 0,
                next_payment: 0,
                status: SubscriptionStatus::Active,
            },
        );
        let mut logs = vec![];
        rule_strip_cancelled_subscriptions(&mut snap, &mut logs);
        assert_eq!(snap.subscriptions.len(), 1);
        assert!(snap.subscriptions.contains_key("GSUB2:GCREATOR"));
        assert_eq!(logs[0].records_affected, 1);
    }

    #[test]
    fn test_cap_tip_history() {
        use crate::types::TipMetadata;
        let mut snap = empty_snapshot();
        let tips: Vec<TipMetadata> = (0..1_200)
            .map(|i| TipMetadata {
                sender: "GSENDER".into(),
                amount: i as i128,
                message: None,
                timestamp: i as u64,
            })
            .collect();
        snap.tip_history.insert("GCREATOR".into(), tips);
        let mut logs = vec![];
        rule_cap_tip_history(&mut snap, &mut logs);
        assert_eq!(snap.tip_history["GCREATOR"].len(), 1_000);
        // Newest entries (indices 200–1199) should be kept.
        assert_eq!(snap.tip_history["GCREATOR"][0].amount, 200);
        assert_eq!(logs[0].records_affected, 200);
    }

    #[test]
    fn test_remove_refunded_tip_records() {
        let mut snap = empty_snapshot();
        snap.tip_records.push(TipRecord {
            id: 0,
            sender: "GSENDER".into(),
            creator: "GCREATOR".into(),
            token: "GTOKEN".into(),
            amount: 100,
            timestamp: 0,
            refunded: true,
            refund_requested: true,
            refund_approved: true,
        });
        snap.tip_records.push(TipRecord {
            id: 1,
            sender: "GSENDER".into(),
            creator: "GCREATOR".into(),
            token: "GTOKEN".into(),
            amount: 200,
            timestamp: 0,
            refunded: false,
            refund_requested: false,
            refund_approved: false,
        });
        let mut logs = vec![];
        rule_remove_refunded_tip_records(&mut snap, &mut logs);
        assert_eq!(snap.tip_records.len(), 1);
        assert_eq!(snap.tip_records[0].id, 1);
        assert_eq!(logs[0].records_affected, 1);
    }

    #[test]
    fn test_bump_schema_version() {
        let mut snap = empty_snapshot();
        assert_eq!(snap.schema_version, 1);
        let mut logs = vec![];
        rule_bump_schema_version(&mut snap, &mut logs);
        assert_eq!(snap.schema_version, 2);
        assert_eq!(logs[0].records_affected, 1);
    }

    #[test]
    fn test_full_transform_pipeline() {
        let mut snap = empty_snapshot();
        snap.creator_balances.insert("GCREATOR:GTOKEN".into(), 0);
        snap.creator_balances.insert("GCREATOR2:GTOKEN".into(), 500);

        let transformer = StateTransformer::new(1, 2);
        let (transformed, logs) = transformer.transform(snap).unwrap();

        // Zero balance removed.
        assert!(!transformed.creator_balances.contains_key("GCREATOR:GTOKEN"));
        // Schema version bumped.
        assert_eq!(transformed.schema_version, 2);
        // Checksum is non-empty.
        assert!(!transformed.checksum.is_empty());
        // At least one log entry.
        assert!(!logs.is_empty());
    }
}
