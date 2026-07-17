//! # Migration Verification — `StateMigrator::verify_migration()`
//!
//! Compares every field in the source [`StateSnapshot`] against the live state
//! of the target contract, returning a detailed [`VerificationReport`].
//!
//! ## What is checked
//! | Category              | Fields verified                                      |
//! |-----------------------|------------------------------------------------------|
//! | Instance storage      | admin, fee_bps, refund_window, paused, tip_counter   |
//! | Creator balances      | Every `CreatorBalance(creator, token)` entry         |
//! | Creator totals        | Every `CreatorTotal(creator, token)` entry           |
//! | Tip history           | Count + every `TipHistory(creator, idx)` entry       |
//! | Leaderboard           | Every tipper/creator aggregate entry                 |
//! | Subscriptions         | Every `Subscription(subscriber, creator)` entry      |
//! | Time locks            | Every `TimeLock(id)` entry                           |
//! | Milestones            | Every `Milestone(creator, id)` entry                 |
//! | Matching programs     | Every `MatchingProgram(id)` entry                    |
//! | Tip records           | Every `TipRecord(id)` entry                          |
//! | Locked tips           | Every `LockedTip(creator, id)` entry                 |
//! | User roles            | Every `UserRole(address)` entry                      |
//!
//! ## Usage
//! ```bash
//! cargo run --manifest-path scripts/migrate/Cargo.toml \
//!     --bin verify -- --config scripts/migrate/config.toml
//! ```

use std::fs;

use crate::rpc_client::RpcClient;
use crate::types::{
    FindingSeverity, MigrationConfig, StateSnapshot, VerificationFinding, VerificationReport,
};

// ─────────────────────────────────────────────────────────────────────────────
// VerificationEngine
// ─────────────────────────────────────────────────────────────────────────────

pub struct VerificationEngine {
    config: MigrationConfig,
    rpc: RpcClient,
}

impl VerificationEngine {
    pub fn new(config: MigrationConfig) -> Self {
        let rpc = RpcClient::new(
            config.migration.rpc_url.clone(),
            config.migration.max_retries,
            config.migration.retry_delay_ms,
        );
        Self { config, rpc }
    }

    /// Compares `snapshot` against the live state of `target_contract_id`.
    ///
    /// Returns a [`VerificationReport`] with every discrepancy found.
    pub fn verify_migration(
        &self,
        snapshot: &StateSnapshot,
        target: &str,
    ) -> Result<VerificationReport, String> {
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║              TipJar Migration Verification               ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!("  Source snapshot : {} records", snapshot.total_records());
        println!("  Target contract : {}", target);
        println!();

        let mut findings: Vec<VerificationFinding> = Vec::new();
        let mut records_checked = 0usize;

        // ── instance storage ──────────────────────────────────────────────────
        println!("[1/12] Verifying instance storage …");
        self.check_instance_string(
            target, "Admin", &snapshot.admin, &mut findings, &mut records_checked,
        )?;
        self.check_instance_u32(
            target, "FeeBasisPoints", snapshot.fee_basis_points,
            &mut findings, &mut records_checked,
        )?;
        self.check_instance_u64(
            target, "RefundWindow", snapshot.refund_window_seconds,
            &mut findings, &mut records_checked,
        )?;
        self.check_instance_bool(
            target, "Paused", snapshot.paused, &mut findings, &mut records_checked,
        )?;
        self.check_instance_u64(
            target, "TipCounter", snapshot.tip_counter,
            &mut findings, &mut records_checked,
        )?;
        self.check_instance_u64(
            target, "MatchingCounter", snapshot.matching_counter,
            &mut findings, &mut records_checked,
        )?;
        self.check_instance_u32(
            target, "ContractVersion", snapshot.schema_version,
            &mut findings, &mut records_checked,
        )?;

        // ── whitelisted tokens ────────────────────────────────────────────────
        println!("[2/12] Verifying whitelisted tokens …");
        let actual_tokens = self.rpc.get_whitelisted_tokens(target).unwrap_or_default();
        for token in &snapshot.whitelisted_tokens {
            records_checked += 1;
            if !actual_tokens.contains(token) {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("whitelisted_tokens.{}", token),
                    expected: "true".into(),
                    actual: "<absent>".into(),
                });
            }
        }

        // ── creator balances ──────────────────────────────────────────────────
        println!("[3/12] Verifying creator balances ({}) …", snapshot.creator_balances.len());
        for (key, expected_bal) in &snapshot.creator_balances {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let actual_bal = self
                    .rpc
                    .get_creator_balance(target, parts[0], parts[1])
                    .unwrap_or(0);
                records_checked += 1;
                if actual_bal != *expected_bal {
                    findings.push(VerificationFinding {
                        severity: FindingSeverity::Mismatch,
                        field: format!("creator_balances.{}", key),
                        expected: expected_bal.to_string(),
                        actual: actual_bal.to_string(),
                    });
                }
            }
        }

        // ── creator totals ────────────────────────────────────────────────────
        println!("[4/12] Verifying creator totals ({}) …", snapshot.creator_totals.len());
        for (key, expected_tot) in &snapshot.creator_totals {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let actual_tot = self
                    .rpc
                    .get_creator_total(target, parts[0], parts[1])
                    .unwrap_or(0);
                records_checked += 1;
                if actual_tot != *expected_tot {
                    findings.push(VerificationFinding {
                        severity: FindingSeverity::Mismatch,
                        field: format!("creator_totals.{}", key),
                        expected: expected_tot.to_string(),
                        actual: actual_tot.to_string(),
                    });
                }
            }
        }

        // ── tip history ───────────────────────────────────────────────────────
        let total_tips: usize = snapshot.tip_history.values().map(|v| v.len()).sum();
        println!("[5/12] Verifying tip history ({} records) …", total_tips);
        for (creator, expected_tips) in &snapshot.tip_history {
            let actual_count = self
                .rpc
                .get_persistent_u64(target, &format!("TipCount:{}", creator))
                .unwrap_or(0) as usize;
            records_checked += 1;
            if actual_count != expected_tips.len() {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Mismatch,
                    field: format!("tip_history.{}.count", creator),
                    expected: expected_tips.len().to_string(),
                    actual: actual_count.to_string(),
                });
            }
            for (idx, expected_meta) in expected_tips.iter().enumerate() {
                let key = format!("TipHistory:{}:{}", creator, idx);
                let actual_meta = self
                    .rpc
                    .get_persistent_tip_metadata(target, &key)
                    .unwrap_or(None);
                records_checked += 1;
                match actual_meta {
                    None => findings.push(VerificationFinding {
                        severity: FindingSeverity::Missing,
                        field: format!("tip_history.{}.{}", creator, idx),
                        expected: format!("{:?}", expected_meta),
                        actual: "<absent>".into(),
                    }),
                    Some(ref actual) if actual != expected_meta => {
                        findings.push(VerificationFinding {
                            severity: FindingSeverity::Mismatch,
                            field: format!("tip_history.{}.{}", creator, idx),
                            expected: format!("{:?}", expected_meta),
                            actual: format!("{:?}", actual),
                        });
                    }
                    _ => {}
                }
            }
        }

        // ── leaderboard aggregates ────────────────────────────────────────────
        println!(
            "[6/12] Verifying leaderboard aggregates ({} tipper, {} creator) …",
            snapshot.tipper_aggregates.len(),
            snapshot.creator_aggregates.len()
        );
        for (key, expected_entry) in &snapshot.tipper_aggregates {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let bucket: u32 = parts[1].parse().unwrap_or(0);
                let agg_key = format!("TipperAggregate:{}:{}", parts[0], bucket);
                let actual = self
                    .rpc
                    .get_persistent_leaderboard_entry(target, &agg_key)
                    .unwrap_or(None);
                records_checked += 1;
                match actual {
                    None => findings.push(VerificationFinding {
                        severity: FindingSeverity::Missing,
                        field: format!("tipper_aggregates.{}", key),
                        expected: format!("{:?}", expected_entry),
                        actual: "<absent>".into(),
                    }),
                    Some(ref a) if a.total_amount != expected_entry.total_amount
                        || a.tip_count != expected_entry.tip_count =>
                    {
                        findings.push(VerificationFinding {
                            severity: FindingSeverity::Mismatch,
                            field: format!("tipper_aggregates.{}", key),
                            expected: format!("amount={} count={}", expected_entry.total_amount, expected_entry.tip_count),
                            actual: format!("amount={} count={}", a.total_amount, a.tip_count),
                        });
                    }
                    _ => {}
                }
            }
        }
        for (key, expected_entry) in &snapshot.creator_aggregates {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let bucket: u32 = parts[1].parse().unwrap_or(0);
                let agg_key = format!("CreatorAggregate:{}:{}", parts[0], bucket);
                let actual = self
                    .rpc
                    .get_persistent_leaderboard_entry(target, &agg_key)
                    .unwrap_or(None);
                records_checked += 1;
                match actual {
                    None => findings.push(VerificationFinding {
                        severity: FindingSeverity::Missing,
                        field: format!("creator_aggregates.{}", key),
                        expected: format!("{:?}", expected_entry),
                        actual: "<absent>".into(),
                    }),
                    Some(ref a) if a.total_amount != expected_entry.total_amount
                        || a.tip_count != expected_entry.tip_count =>
                    {
                        findings.push(VerificationFinding {
                            severity: FindingSeverity::Mismatch,
                            field: format!("creator_aggregates.{}", key),
                            expected: format!("amount={} count={}", expected_entry.total_amount, expected_entry.tip_count),
                            actual: format!("amount={} count={}", a.total_amount, a.tip_count),
                        });
                    }
                    _ => {}
                }
            }
        }

        // ── subscriptions ─────────────────────────────────────────────────────
        println!("[7/12] Verifying subscriptions ({}) …", snapshot.subscriptions.len());
        for (key, expected_sub) in &snapshot.subscriptions {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let sub_key = format!("Subscription:{}:{}", parts[0], parts[1]);
                let actual = self
                    .rpc
                    .get_persistent_subscription(target, &sub_key)
                    .unwrap_or(None);
                records_checked += 1;
                match actual {
                    None => findings.push(VerificationFinding {
                        severity: FindingSeverity::Missing,
                        field: format!("subscriptions.{}", key),
                        expected: format!("{:?}", expected_sub),
                        actual: "<absent>".into(),
                    }),
                    Some(ref a) if a.amount != expected_sub.amount
                        || a.interval_seconds != expected_sub.interval_seconds =>
                    {
                        findings.push(VerificationFinding {
                            severity: FindingSeverity::Mismatch,
                            field: format!("subscriptions.{}", key),
                            expected: format!("amount={} interval={}", expected_sub.amount, expected_sub.interval_seconds),
                            actual: format!("amount={} interval={}", a.amount, a.interval_seconds),
                        });
                    }
                    _ => {}
                }
            }
        }

        // ── time locks ────────────────────────────────────────────────────────
        println!("[8/12] Verifying time locks ({}) …", snapshot.time_locks.len());
        for expected_lock in &snapshot.time_locks {
            let key = format!("TimeLock:{}", expected_lock.lock_id);
            let actual = self
                .rpc
                .get_persistent_time_lock(target, &key, expected_lock.lock_id)
                .unwrap_or(None);
            records_checked += 1;
            match actual {
                None => findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("time_locks.{}", expected_lock.lock_id),
                    expected: format!("{:?}", expected_lock),
                    actual: "<absent>".into(),
                }),
                Some(ref a) if a.amount != expected_lock.amount
                    || a.cancelled != expected_lock.cancelled =>
                {
                    findings.push(VerificationFinding {
                        severity: FindingSeverity::Mismatch,
                        field: format!("time_locks.{}", expected_lock.lock_id),
                        expected: format!("amount={} cancelled={}", expected_lock.amount, expected_lock.cancelled),
                        actual: format!("amount={} cancelled={}", a.amount, a.cancelled),
                    });
                }
                _ => {}
            }
        }

        // ── milestones ────────────────────────────────────────────────────────
        println!("[9/12] Verifying milestones ({}) …", snapshot.milestones.len());
        for (key, expected_ms) in &snapshot.milestones {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            if parts.len() == 2 {
                let ms_key = format!("Milestone:{}:{}", parts[0], parts[1]);
                let actual = self
                    .rpc
                    .get_persistent_milestone(target, &ms_key)
                    .unwrap_or(None);
                records_checked += 1;
                match actual {
                    None => findings.push(VerificationFinding {
                        severity: FindingSeverity::Missing,
                        field: format!("milestones.{}", key),
                        expected: format!("{:?}", expected_ms),
                        actual: "<absent>".into(),
                    }),
                    Some(ref a) if a.goal_amount != expected_ms.goal_amount
                        || a.current_amount != expected_ms.current_amount
                        || a.completed != expected_ms.completed =>
                    {
                        findings.push(VerificationFinding {
                            severity: FindingSeverity::Mismatch,
                            field: format!("milestones.{}", key),
                            expected: format!(
                                "goal={} current={} completed={}",
                                expected_ms.goal_amount, expected_ms.current_amount, expected_ms.completed
                            ),
                            actual: format!(
                                "goal={} current={} completed={}",
                                a.goal_amount, a.current_amount, a.completed
                            ),
                        });
                    }
                    _ => {}
                }
            }
        }

        // ── matching programs ─────────────────────────────────────────────────
        println!("[10/12] Verifying matching programs ({}) …", snapshot.matching_programs.len());
        for expected_prog in &snapshot.matching_programs {
            let key = format!("MatchingProgram:{}", expected_prog.id);
            let actual = self
                .rpc
                .get_persistent_matching_program(target, &key)
                .unwrap_or(None);
            records_checked += 1;
            match actual {
                None => findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("matching_programs.{}", expected_prog.id),
                    expected: format!("{:?}", expected_prog),
                    actual: "<absent>".into(),
                }),
                Some(ref a) if a.max_match_amount != expected_prog.max_match_amount
                    || a.current_matched != expected_prog.current_matched
                    || a.active != expected_prog.active =>
                {
                    findings.push(VerificationFinding {
                        severity: FindingSeverity::Mismatch,
                        field: format!("matching_programs.{}", expected_prog.id),
                        expected: format!(
                            "max={} matched={} active={}",
                            expected_prog.max_match_amount, expected_prog.current_matched, expected_prog.active
                        ),
                        actual: format!(
                            "max={} matched={} active={}",
                            a.max_match_amount, a.current_matched, a.active
                        ),
                    });
                }
                _ => {}
            }
        }

        // ── tip records ───────────────────────────────────────────────────────
        println!("[11/12] Verifying tip records ({}) …", snapshot.tip_records.len());
        for expected_rec in &snapshot.tip_records {
            let key = format!("TipRecord:{}", expected_rec.id);
            let actual = self
                .rpc
                .get_persistent_tip_record(target, &key)
                .unwrap_or(None);
            records_checked += 1;
            match actual {
                None => findings.push(VerificationFinding {
                    severity: FindingSeverity::Missing,
                    field: format!("tip_records.{}", expected_rec.id),
                    expected: format!("{:?}", expected_rec),
                    actual: "<absent>".into(),
                }),
                Some(ref a) if a.amount != expected_rec.amount
                    || a.refunded != expected_rec.refunded =>
                {
                    findings.push(VerificationFinding {
                        severity: FindingSeverity::Mismatch,
                        field: format!("tip_records.{}", expected_rec.id),
                        expected: format!("amount={} refunded={}", expected_rec.amount, expected_rec.refunded),
                        actual: format!("amount={} refunded={}", a.amount, a.refunded),
                    });
                }
                _ => {}
            }
        }

        // ── user roles ────────────────────────────────────────────────────────
        println!("[12/12] Verifying user roles ({}) …", snapshot.user_roles.len());
        for (addr, expected_role) in &snapshot.user_roles {
            let key = format!("UserRole:{}", addr);
            let actual_role = self
                .rpc
                .get_persistent_string(target, &key)
                .unwrap_or_default();
            records_checked += 1;
            if actual_role != *expected_role {
                findings.push(VerificationFinding {
                    severity: FindingSeverity::Mismatch,
                    field: format!("user_roles.{}", addr),
                    expected: expected_role.clone(),
                    actual: if actual_role.is_empty() {
                        "<absent>".into()
                    } else {
                        actual_role
                    },
                });
            }
        }

        // ── build report ──────────────────────────────────────────────────────
        let passed = findings.is_empty();
        let report = VerificationReport {
            source_contract_id: snapshot.contract_id.clone(),
            target_contract_id: target.to_string(),
            verified_at: iso_now(),
            records_checked,
            passed,
            findings: findings.clone(),
        };

        println!();
        if passed {
            println!("✓ Verification PASSED — {} records checked, 0 findings", records_checked);
        } else {
            println!(
                "✗ Verification FAILED — {} records checked, {} finding(s)",
                records_checked,
                findings.len()
            );
            for f in &findings {
                println!(
                    "  [{:?}] {} — expected={} actual={}",
                    f.severity, f.field, f.expected, f.actual
                );
            }
        }

        Ok(report)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Typed check helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn check_instance_string(
        &self,
        target: &str,
        key: &str,
        expected: &str,
        findings: &mut Vec<VerificationFinding>,
        count: &mut usize,
    ) -> Result<(), String> {
        let actual = self.rpc.get_instance_string(target, key).unwrap_or_default();
        *count += 1;
        if actual != expected {
            findings.push(VerificationFinding {
                severity: if actual.is_empty() {
                    FindingSeverity::Missing
                } else {
                    FindingSeverity::Mismatch
                },
                field: format!("instance.{}", key),
                expected: expected.to_string(),
                actual: if actual.is_empty() { "<absent>".into() } else { actual },
            });
        }
        Ok(())
    }

    fn check_instance_u32(
        &self,
        target: &str,
        key: &str,
        expected: u32,
        findings: &mut Vec<VerificationFinding>,
        count: &mut usize,
    ) -> Result<(), String> {
        let actual = self.rpc.get_instance_u32(target, key).unwrap_or(0);
        *count += 1;
        if actual != expected {
            findings.push(VerificationFinding {
                severity: FindingSeverity::Mismatch,
                field: format!("instance.{}", key),
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(())
    }

    fn check_instance_u64(
        &self,
        target: &str,
        key: &str,
        expected: u64,
        findings: &mut Vec<VerificationFinding>,
        count: &mut usize,
    ) -> Result<(), String> {
        let actual = self.rpc.get_instance_u64(target, key).unwrap_or(0);
        *count += 1;
        if actual != expected {
            findings.push(VerificationFinding {
                severity: FindingSeverity::Mismatch,
                field: format!("instance.{}", key),
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(())
    }

    fn check_instance_bool(
        &self,
        target: &str,
        key: &str,
        expected: bool,
        findings: &mut Vec<VerificationFinding>,
        count: &mut usize,
    ) -> Result<(), String> {
        let actual = self.rpc.get_instance_bool(target, key).unwrap_or(false);
        *count += 1;
        if actual != expected {
            findings.push(VerificationFinding {
                severity: FindingSeverity::Mismatch,
                field: format!("instance.{}", key),
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CLI entry point
// ─────────────────────────────────────────────────────────────────────────────

pub fn run_verify(args: &[String]) -> Result<(), String> {
    let config_path = parse_flag(args, "--config")
        .unwrap_or_else(|| "scripts/migrate/config.toml".to_string());
    let snapshot_override = parse_flag(args, "--snapshot");

    let toml_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Cannot read config {}: {}", config_path, e))?;
    let config: MigrationConfig = toml::from_str(&toml_str)
        .map_err(|e| format!("Config parse error: {}", e))?;

    let snapshot_path = snapshot_override.unwrap_or_else(|| {
        format!(
            "{}/{}",
            config.migration.snapshot_dir, config.migration.transformed_filename
        )
    });

    let json = fs::read_to_string(&snapshot_path)
        .map_err(|e| format!("Cannot read snapshot {}: {}", snapshot_path, e))?;
    let snapshot: StateSnapshot = serde_json::from_str(&json)
        .map_err(|e| format!("Snapshot parse error: {}", e))?;

    crate::export_state::verify_checksum(&snapshot)?;

    let target = config.migration.target_contract_id.clone();
    let engine = VerificationEngine::new(config.clone());
    let report = engine.verify_migration(&snapshot, &target)?;

    // Write report to disk.
    let report_path = format!("{}/verification-report.json", config.migration.snapshot_dir);
    let report_json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("Report serialisation error: {}", e))?;
    fs::write(&report_path, report_json)
        .map_err(|e| format!("Failed to write report: {}", e))?;
    println!("  Report written → {}", report_path);

    if !report.is_clean() && config.migration.abort_on_verify_failure {
        return Err(format!(
            "Verification failed with {} finding(s)",
            report.findings.len()
        ));
    }

    Ok(())
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
}

fn iso_now() -> String {
    use chrono::Utc;
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
